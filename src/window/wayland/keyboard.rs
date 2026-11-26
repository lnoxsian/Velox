use crate::window::event::{Key, KeypadKey, ModifiersState, NamedKey};
use std::os::fd::AsRawFd;

/// Key repeat timing configuration sent by compositor.
#[derive(Debug, Clone, Copy)]
pub struct KeyRepeatConfig {
    pub rate: i32,  // Characters per second (e.g. 25)
    pub delay: i32, // Delay in milliseconds before repeat starts (e.g. 400)
}

/// Active key repeat state for a key currently held down.
#[derive(Debug, Clone)]
pub struct ActiveKeyRepeat {
    pub scancode: u32,
    pub key: Key,
    pub text: Option<String>,
    pub modifiers: ModifiersState,
    pub next_deadline: std::time::Instant,
    pub interval: std::time::Duration,
}

/// State machine wrapping pure-Rust `kbvm` with an evdev fallback.
pub struct XkbHandler {
    context: kbvm::xkb::Context,
    lookup_table: Option<kbvm::lookup::LookupTable>,
    components: kbvm::Components,
}

impl Default for XkbHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl XkbHandler {
    pub fn new() -> Self {
        Self {
            context: kbvm::xkb::Context::default(),
            lookup_table: None,
            components: kbvm::Components::default(),
        }
    }

    /// Compile a new keymap from the compositor's shared memory fd using pure-Rust `kbvm`.
    pub fn update_keymap(&mut self, fd: &std::os::fd::OwnedFd, size: u32) {
        if size == 0 {
            return;
        }

        let mmap = unsafe {
            match memmap2::MmapOptions::new()
                .len(size as usize)
                .map(fd.as_raw_fd())
            {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Failed to mmap Wayland keymap fd: {}", e);
                    return;
                }
            }
        };

        let len = if size > 0 && mmap.last() == Some(&0) {
            size as usize - 1
        } else {
            size as usize
        };

        match self.context.keymap_from_bytes(
            kbvm::xkb::diagnostic::WriteToLog,
            None,
            &mmap[..len],
        ) {
            Ok(keymap) => {
                let table = keymap.to_builder().build_lookup_table();
                self.lookup_table = Some(table);
            }
            Err(e) => {
                log::error!("kbvm failed to compile Wayland keymap: {}", e);
            }
        }
    }

    /// Update active modifier state masks from compositor's `Modifiers` event.
    pub fn update_modifiers(
        &mut self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) -> ModifiersState {
        self.components.mods_pressed = kbvm::ModifierMask(mods_depressed);
        self.components.mods_latched = kbvm::ModifierMask(mods_latched);
        self.components.mods_locked = kbvm::ModifierMask(mods_locked);
        self.components.group_locked = kbvm::GroupIndex(group);
        self.components.update_effective();

        let effective = self.components.mods;
        let mut m = ModifiersState::empty();
        if effective.contains(kbvm::ModifierMask::SHIFT) {
            m |= ModifiersState::SHIFT;
        }
        if effective.contains(kbvm::ModifierMask::CONTROL) {
            m |= ModifiersState::CONTROL;
        }
        if effective.contains(kbvm::ModifierMask::ALT) {
            m |= ModifiersState::ALT;
        }
        if effective.contains(kbvm::ModifierMask::SUPER) {
            m |= ModifiersState::SUPER;
        }
        m
    }

    /// Translate a Linux evdev scancode into a `(Key, Option<String>, repeats, ModifiersState)` tuple.
    pub fn translate_key(
        &self,
        scancode: u32,
        mut modifiers: ModifiersState,
    ) -> (Option<Key>, Option<String>, bool, ModifiersState) {
        if let Some(table) = &self.lookup_table {
            let keycode = kbvm::Keycode::from_evdev(scancode);
            let lookup = table.lookup(self.components.group, self.components.mods, keycode);
            let repeats = lookup.repeats();

            let mut resolved_sym = None;
            let mut resolved_char = None;
            for prop in lookup {
                resolved_sym = Some(prop.keysym());
                resolved_char = prop.char();
                break;
            }

            if let Some(sym) = resolved_sym {
                let (mut key, mut text) = xkb_keysym_to_key(sym.0);
                if sym.0 == 0xfe20 {
                    modifiers.insert(ModifiersState::SHIFT);
                }
                if text.is_none()
                    && let Some(ch) = resolved_char
                    && !matches!(key, Some(Key::Named(NamedKey::Backspace)))
                {
                    text = Some(ch.to_string());
                }
                if key.is_none()
                    && let Some(t) = &text
                {
                    key = Some(Key::Character(t.clone()));
                }
                if let Some(ref k) = key {
                    match k {
                        Key::Named(NamedKey::Shift) => modifiers.insert(ModifiersState::SHIFT),
                        Key::Named(NamedKey::Control) => modifiers.insert(ModifiersState::CONTROL),
                        Key::Named(NamedKey::Alt) => modifiers.insert(ModifiersState::ALT),
                        Key::Named(NamedKey::Super) => modifiers.insert(ModifiersState::SUPER),
                        _ => {}
                    }
                }
                return (key, text, repeats, modifiers);
            }
        }

        // Fallback to internal evdev scancode table
        let (key, text) = evdev_scancode_to_key(scancode, modifiers);
        let repeats = !matches!(scancode, 29 | 42 | 54 | 56 | 58 | 97 | 100 | 125 | 126);
        if let Some(ref k) = key {
            match k {
                Key::Named(NamedKey::Shift) => modifiers.insert(ModifiersState::SHIFT),
                Key::Named(NamedKey::Control) => modifiers.insert(ModifiersState::CONTROL),
                Key::Named(NamedKey::Alt) => modifiers.insert(ModifiersState::ALT),
                Key::Named(NamedKey::Super) => modifiers.insert(ModifiersState::SUPER),
                _ => {}
            }
        }
        (key, text, repeats, modifiers)
    }
}

/// Fallback translation from Linux evdev scancode to Key and text string.
pub fn evdev_scancode_to_key(
    scancode: u32,
    modifiers: ModifiersState,
) -> (Option<Key>, Option<String>) {
    let shift = modifiers.contains(ModifiersState::SHIFT);
    match scancode {
        1 => (Some(Key::Named(NamedKey::Escape)), Some("\x1b".to_string())),
        2 => {
            if shift {
                (Some(Key::Character("!".to_string())), Some("!".to_string()))
            } else {
                (Some(Key::Character("1".to_string())), Some("1".to_string()))
            }
        }
        3 => {
            if shift {
                (Some(Key::Character("@".to_string())), Some("@".to_string()))
            } else {
                (Some(Key::Character("2".to_string())), Some("2".to_string()))
            }
        }
        4 => {
            if shift {
                (Some(Key::Character("#".to_string())), Some("#".to_string()))
            } else {
                (Some(Key::Character("3".to_string())), Some("3".to_string()))
            }
        }
        5 => {
            if shift {
                (Some(Key::Character("$".to_string())), Some("$".to_string()))
            } else {
                (Some(Key::Character("4".to_string())), Some("4".to_string()))
            }
        }
        6 => {
            if shift {
                (Some(Key::Character("%".to_string())), Some("%".to_string()))
            } else {
                (Some(Key::Character("5".to_string())), Some("5".to_string()))
            }
        }
        7 => {
            if shift {
                (Some(Key::Character("^".to_string())), Some("^".to_string()))
            } else {
                (Some(Key::Character("6".to_string())), Some("6".to_string()))
            }
        }
        8 => {
            if shift {
                (Some(Key::Character("&".to_string())), Some("&".to_string()))
            } else {
                (Some(Key::Character("7".to_string())), Some("7".to_string()))
            }
        }
        9 => {
            if shift {
                (Some(Key::Character("*".to_string())), Some("*".to_string()))
            } else {
                (Some(Key::Character("8".to_string())), Some("8".to_string()))
            }
        }
        10 => {
            if shift {
                (Some(Key::Character("(".to_string())), Some("(".to_string()))
            } else {
                (Some(Key::Character("9".to_string())), Some("9".to_string()))
            }
        }
        11 => {
            if shift {
                (Some(Key::Character(")".to_string())), Some(")".to_string()))
            } else {
                (Some(Key::Character("0".to_string())), Some("0".to_string()))
            }
        }
        12 => {
            if shift {
                (Some(Key::Character("_".to_string())), Some("_".to_string()))
            } else {
                (Some(Key::Character("-".to_string())), Some("-".to_string()))
            }
        }
        13 => {
            if shift {
                (Some(Key::Character("+".to_string())), Some("+".to_string()))
            } else {
                (Some(Key::Character("=".to_string())), Some("=".to_string()))
            }
        }
        14 => (Some(Key::Named(NamedKey::Backspace)), None),
        15 => (Some(Key::Named(NamedKey::Tab)), Some("\t".to_string())),

        16 => letter('q', shift),
        17 => letter('w', shift),
        18 => letter('e', shift),
        19 => letter('r', shift),
        20 => letter('t', shift),
        21 => letter('y', shift),
        22 => letter('u', shift),
        23 => letter('i', shift),
        24 => letter('o', shift),
        25 => letter('p', shift),
        26 => {
            if shift {
                (Some(Key::Character("{".to_string())), Some("{".to_string()))
            } else {
                (Some(Key::Character("[".to_string())), Some("[".to_string()))
            }
        }
        27 => {
            if shift {
                (Some(Key::Character("}".to_string())), Some("}".to_string()))
            } else {
                (Some(Key::Character("]".to_string())), Some("]".to_string()))
            }
        }
        28 => (Some(Key::Named(NamedKey::Enter)), Some("\r".to_string())),
        29 => (Some(Key::Named(NamedKey::Control)), None), // LCtrl

        30 => letter('a', shift),
        31 => letter('s', shift),
        32 => letter('d', shift),
        33 => letter('f', shift),
        34 => letter('g', shift),
        35 => letter('h', shift),
        36 => letter('j', shift),
        37 => letter('k', shift),
        38 => letter('l', shift),
        39 => {
            if shift {
                (Some(Key::Character(":".to_string())), Some(":".to_string()))
            } else {
                (Some(Key::Character(";".to_string())), Some(";".to_string()))
            }
        }
        40 => {
            if shift {
                (
                    Some(Key::Character("\"".to_string())),
                    Some("\"".to_string()),
                )
            } else {
                (Some(Key::Character("'".to_string())), Some("'".to_string()))
            }
        }
        41 => {
            if shift {
                (Some(Key::Character("~".to_string())), Some("~".to_string()))
            } else {
                (Some(Key::Character("`".to_string())), Some("`".to_string()))
            }
        }
        42 => (Some(Key::Named(NamedKey::Shift)), None), // LShift
        43 => {
            if shift {
                (Some(Key::Character("|".to_string())), Some("|".to_string()))
            } else {
                (
                    Some(Key::Character("\\".to_string())),
                    Some("\\".to_string()),
                )
            }
        }

        44 => letter('z', shift),
        45 => letter('x', shift),
        46 => letter('c', shift),
        47 => letter('v', shift),
        48 => letter('b', shift),
        49 => letter('n', shift),
        50 => letter('m', shift),
        51 => {
            if shift {
                (Some(Key::Character("<".to_string())), Some("<".to_string()))
            } else {
                (Some(Key::Character(",".to_string())), Some(",".to_string()))
            }
        }
        52 => {
            if shift {
                (Some(Key::Character(">".to_string())), Some(">".to_string()))
            } else {
                (Some(Key::Character(".".to_string())), Some(".".to_string()))
            }
        }
        53 => {
            if shift {
                (Some(Key::Character("?".to_string())), Some("?".to_string()))
            } else {
                (Some(Key::Character("/".to_string())), Some("/".to_string()))
            }
        }
        54 => (Some(Key::Named(NamedKey::Shift)), None), // RShift
        56 => (Some(Key::Named(NamedKey::Alt)), None),   // LAlt
        57 => (Some(Key::Named(NamedKey::Space)), Some(" ".to_string())),
        58 => (None, None), // CapsLock

        59 => (Some(Key::Named(NamedKey::F1)), None),
        60 => (Some(Key::Named(NamedKey::F2)), None),
        61 => (Some(Key::Named(NamedKey::F3)), None),
        62 => (Some(Key::Named(NamedKey::F4)), None),
        63 => (Some(Key::Named(NamedKey::F5)), None),
        64 => (Some(Key::Named(NamedKey::F6)), None),
        65 => (Some(Key::Named(NamedKey::F7)), None),
        66 => (Some(Key::Named(NamedKey::F8)), None),
        67 => (Some(Key::Named(NamedKey::F9)), None),
        68 => (Some(Key::Named(NamedKey::F10)), None),
        87 => (Some(Key::Named(NamedKey::F11)), None),
        88 => (Some(Key::Named(NamedKey::F12)), None),

        97 => (Some(Key::Named(NamedKey::Control)), None), // RCtrl
        100 => (Some(Key::Named(NamedKey::Alt)), None),    // RAlt
        102 => (Some(Key::Named(NamedKey::Home)), None),
        103 => (Some(Key::Named(NamedKey::ArrowUp)), None),
        104 => (Some(Key::Named(NamedKey::PageUp)), None),
        105 => (Some(Key::Named(NamedKey::ArrowLeft)), None),
        106 => (Some(Key::Named(NamedKey::ArrowRight)), None),
        107 => (Some(Key::Named(NamedKey::End)), None),
        108 => (Some(Key::Named(NamedKey::ArrowDown)), None),
        109 => (Some(Key::Named(NamedKey::PageDown)), None),
        110 => (Some(Key::Named(NamedKey::Insert)), None),
        111 => (Some(Key::Named(NamedKey::Delete)), None),
        125 => (Some(Key::Named(NamedKey::Super)), None), // LSuper
        126 => (Some(Key::Named(NamedKey::Super)), None), // RSuper

        _ => (None, None),
    }
}

#[inline(always)]
fn letter(c: char, shift: bool) -> (Option<Key>, Option<String>) {
    let out = if shift {
        c.to_ascii_uppercase().to_string()
    } else {
        c.to_string()
    };
    (Some(Key::Character(out.clone())), Some(out))
}

/// Translates XKB keysym to `Key` and optional printable text string.
pub fn xkb_keysym_to_key(keysym: u32) -> (Option<Key>, Option<String>) {
    match keysym {
        0xff08 | 0x08 | 0x7f => (Some(Key::Named(NamedKey::Backspace)), None),
        0xff09 => (Some(Key::Named(NamedKey::Tab)), Some("\t".to_string())),
        0xfe20 => (Some(Key::Named(NamedKey::Tab)), None), // XK_ISO_Left_Tab (Shift+Tab)
        0xff0d => (Some(Key::Named(NamedKey::Enter)), Some("\r".to_string())),
        0xff1b => (Some(Key::Named(NamedKey::Escape)), Some("\x1b".to_string())),
        0x0020 => (Some(Key::Named(NamedKey::Space)), Some(" ".to_string())),
        0xff50 => (Some(Key::Named(NamedKey::Home)), None),
        0xff51 => (Some(Key::Named(NamedKey::ArrowLeft)), None),
        0xff52 => (Some(Key::Named(NamedKey::ArrowUp)), None),
        0xff53 => (Some(Key::Named(NamedKey::ArrowRight)), None),
        0xff54 => (Some(Key::Named(NamedKey::ArrowDown)), None),
        0xff55 => (Some(Key::Named(NamedKey::PageUp)), None),
        0xff56 => (Some(Key::Named(NamedKey::PageDown)), None),
        0xff57 => (Some(Key::Named(NamedKey::End)), None),
        0xff63 => (Some(Key::Named(NamedKey::Insert)), None),
        0xffff => (Some(Key::Named(NamedKey::Delete)), None),

        // Keypad keys
        0xff8d => (Some(Key::Keypad(KeypadKey::Enter)), Some("\r".to_string())),
        0xffab => (Some(Key::Keypad(KeypadKey::Plus)), Some("+".to_string())),
        0xffad => (Some(Key::Keypad(KeypadKey::Minus)), Some("-".to_string())),
        0xffaa => (
            Some(Key::Keypad(KeypadKey::Multiply)),
            Some("*".to_string()),
        ),
        0xffaf => (Some(Key::Keypad(KeypadKey::Divide)), Some("/".to_string())),
        0xffae => (Some(Key::Keypad(KeypadKey::Decimal)), Some(".".to_string())),
        0xffac => (
            Some(Key::Keypad(KeypadKey::Separator)),
            Some(",".to_string()),
        ),
        0xffbd => (Some(Key::Keypad(KeypadKey::Equal)), Some("=".to_string())),
        0xffb0..=0xffb9 => {
            const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
            let idx = (keysym - 0xffb0) as usize;
            let kp = match idx {
                0 => KeypadKey::Num0,
                1 => KeypadKey::Num1,
                2 => KeypadKey::Num2,
                3 => KeypadKey::Num3,
                4 => KeypadKey::Num4,
                5 => KeypadKey::Num5,
                6 => KeypadKey::Num6,
                7 => KeypadKey::Num7,
                8 => KeypadKey::Num8,
                _ => KeypadKey::Num9,
            };
            (Some(Key::Keypad(kp)), Some(DIGITS[idx].to_string()))
        }
        0xff95 => (Some(Key::Named(NamedKey::Home)), None),
        0xff96 => (Some(Key::Named(NamedKey::ArrowLeft)), None),
        0xff97 => (Some(Key::Named(NamedKey::ArrowUp)), None),
        0xff98 => (Some(Key::Named(NamedKey::ArrowRight)), None),
        0xff99 => (Some(Key::Named(NamedKey::ArrowDown)), None),
        0xff9a => (Some(Key::Named(NamedKey::PageUp)), None),
        0xff9b => (Some(Key::Named(NamedKey::PageDown)), None),
        0xff9c => (Some(Key::Named(NamedKey::End)), None),
        0xff9d => (Some(Key::Named(NamedKey::Space)), Some(" ".to_string())),
        0xff9e => (Some(Key::Named(NamedKey::Insert)), None),
        0xff9f => (Some(Key::Named(NamedKey::Delete)), None),

        // Modifiers (Shift, Ctrl, Alt, Super/Hyper)
        0xffe1 | 0xffe2 => (Some(Key::Named(NamedKey::Shift)), None),
        0xffe3 | 0xffe4 => (Some(Key::Named(NamedKey::Control)), None),
        0xffe7 | 0xffe8 | 0xffe9 | 0xffea => (Some(Key::Named(NamedKey::Alt)), None),
        0xffeb | 0xffec | 0xffed | 0xffee => (Some(Key::Named(NamedKey::Super)), None),

        0xffbe => (Some(Key::Named(NamedKey::F1)), None),
        0xffbf => (Some(Key::Named(NamedKey::F2)), None),
        0xffc0 => (Some(Key::Named(NamedKey::F3)), None),
        0xffc1 => (Some(Key::Named(NamedKey::F4)), None),
        0xffc2 => (Some(Key::Named(NamedKey::F5)), None),
        0xffc3 => (Some(Key::Named(NamedKey::F6)), None),
        0xffc4 => (Some(Key::Named(NamedKey::F7)), None),
        0xffc5 => (Some(Key::Named(NamedKey::F8)), None),
        0xffc6 => (Some(Key::Named(NamedKey::F9)), None),
        0xffc7 => (Some(Key::Named(NamedKey::F10)), None),
        0xffc8 => (Some(Key::Named(NamedKey::F11)), None),
        0xffc9 => (Some(Key::Named(NamedKey::F12)), None),

        0x20..=0x7e | 0xa0..=0xff => {
            if let Some(ch) = char::from_u32(keysym) {
                let s = ch.to_string();
                (Some(Key::Character(s.clone())), Some(s))
            } else {
                (None, None)
            }
        }

        0x01000000..=0x0110ffff => {
            let cp = keysym - 0x01000000;
            if let Some(ch) = char::from_u32(cp) {
                let s = ch.to_string();
                (Some(Key::Character(s.clone())), Some(s))
            } else {
                (None, None)
            }
        }

        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xkb_keysym_to_key_named() {
        assert_eq!(
            xkb_keysym_to_key(0xff1b),
            (Some(Key::Named(NamedKey::Escape)), Some("\x1b".to_string()))
        );
        assert_eq!(
            xkb_keysym_to_key(0xff0d),
            (Some(Key::Named(NamedKey::Enter)), Some("\r".to_string()))
        );
        assert_eq!(
            xkb_keysym_to_key(0xff08),
            (Some(Key::Named(NamedKey::Backspace)), None)
        );
        assert_eq!(
            xkb_keysym_to_key(0xff51),
            (Some(Key::Named(NamedKey::ArrowLeft)), None)
        );
        assert_eq!(
            xkb_keysym_to_key(0xffbe),
            (Some(Key::Named(NamedKey::F1)), None)
        );
        assert_eq!(
            xkb_keysym_to_key(0xfe20),
            (Some(Key::Named(NamedKey::Tab)), None)
        );
    }

    #[test]
    fn test_evdev_fallback_letters_and_digits() {
        let (k, t) = evdev_scancode_to_key(30, ModifiersState::empty());
        assert_eq!(k, Some(Key::Character("a".to_string())));
        assert_eq!(t, Some("a".to_string()));

        let (k, t) = evdev_scancode_to_key(30, ModifiersState::SHIFT);
        assert_eq!(k, Some(Key::Character("A".to_string())));
        assert_eq!(t, Some("A".to_string()));

        let (k, t) = evdev_scancode_to_key(2, ModifiersState::empty());
        assert_eq!(k, Some(Key::Character("1".to_string())));
        assert_eq!(t, Some("1".to_string()));

        let (k, t) = evdev_scancode_to_key(2, ModifiersState::SHIFT);
        assert_eq!(k, Some(Key::Character("!".to_string())));
        assert_eq!(t, Some("!".to_string()));

        let (k, t) = evdev_scancode_to_key(28, ModifiersState::empty());
        assert_eq!(k, Some(Key::Named(NamedKey::Enter)));
        assert_eq!(t, Some("\r".to_string()));
    }

    #[test]
    fn test_evdev_fallback_navigation() {
        let (k, _) = evdev_scancode_to_key(103, ModifiersState::empty());
        assert_eq!(k, Some(Key::Named(NamedKey::ArrowUp)));

        let (k, _) = evdev_scancode_to_key(108, ModifiersState::empty());
        assert_eq!(k, Some(Key::Named(NamedKey::ArrowDown)));

        let (k, _) = evdev_scancode_to_key(102, ModifiersState::empty());
        assert_eq!(k, Some(Key::Named(NamedKey::Home)));

        let (k, _) = evdev_scancode_to_key(107, ModifiersState::empty());
        assert_eq!(k, Some(Key::Named(NamedKey::End)));
    }

    #[test]
    fn test_xkb_handler_initialization() {
        let handler = XkbHandler::new();
        let (key, text, repeats, _) = handler.translate_key(30, ModifiersState::empty());
        assert!(key.is_some());
        assert_eq!(text, Some("a".to_string()));
        assert!(repeats);
    }

    #[test]
    fn test_kbvm_modifier_updates() {
        let mut handler = XkbHandler::new();
        assert_eq!(handler.components.mods, kbvm::ModifierMask::NONE);

        // Shift = 1
        let m = handler.update_modifiers(1, 0, 0, 0);
        assert_eq!(m, ModifiersState::SHIFT);

        // Control = 4
        let m = handler.update_modifiers(4, 0, 0, 0);
        assert_eq!(m, ModifiersState::CONTROL);

        // Alt = 8
        let m = handler.update_modifiers(8, 0, 0, 0);
        assert_eq!(m, ModifiersState::ALT);

        // Super = 64
        let m = handler.update_modifiers(64, 0, 0, 0);
        assert_eq!(m, ModifiersState::SUPER);

        // Combined Shift + Control + Alt + Super
        let m = handler.update_modifiers(1 | 4 | 8 | 64, 0, 0, 0);
        assert!(m.contains(ModifiersState::SHIFT));
        assert!(m.contains(ModifiersState::CONTROL));
        assert!(m.contains(ModifiersState::ALT));
        assert!(m.contains(ModifiersState::SUPER));
    }

    #[test]
    fn test_kbvm_keymap_compilation_and_translation() {
        let mut handler = XkbHandler::new();
        const TEST_KEYMAP: &str = r#"
xkb_keymap {
    xkb_keycodes {
        minimum = 8;
        maximum = 255;
        <AC01> = 38;
        <AD01> = 24;
    };
    xkb_types {
        type "ALPHABETIC" {
            modifiers = Shift;
            map[Shift] = Level2;
            level_name[Level1] = "Base";
            level_name[Level2] = "Caps";
        };
    };
    xkb_compat {
    };
    xkb_symbols {
        key <AC01> {
            type = "ALPHABETIC",
            symbols[Group1] = [ a, A ]
        };
        key <AD01> {
            type = "ALPHABETIC",
            symbols[Group1] = [ q, Q ]
        };
    };
};
"#;
        let res = handler.context.keymap_from_bytes(
            kbvm::xkb::diagnostic::WriteToLog,
            None,
            TEST_KEYMAP.as_bytes(),
        );
        assert!(res.is_ok(), "Failed to parse test keymap: {:?}", res.err());
        let keymap = res.unwrap();
        handler.lookup_table = Some(keymap.to_builder().build_lookup_table());

        // AC01 keycode is 38 => evdev scancode is 38 - 8 = 30 ('a')
        let (key, text, repeats, mods) = handler.translate_key(30, ModifiersState::empty());
        assert_eq!(key, Some(Key::Character("a".to_string())));
        assert_eq!(text, Some("a".to_string()));
        assert!(repeats);
        assert_eq!(mods, ModifiersState::empty());

        // Update modifiers with Shift (bit 0 = 1)
        let active_mods = handler.update_modifiers(1, 0, 0, 0);
        assert!(active_mods.contains(ModifiersState::SHIFT));

        // Now translate with Shift
        let (key_shifted, text_shifted, repeats_shifted, _) = handler.translate_key(30, active_mods);
        assert_eq!(key_shifted, Some(Key::Character("A".to_string())));
        assert_eq!(text_shifted, Some("A".to_string()));
        assert!(repeats_shifted);

        // Reset modifiers and test AD01 (scancode 16 -> 'q')
        let _ = handler.update_modifiers(0, 0, 0, 0);
        let (key_q, text_q, _, _) = handler.translate_key(16, ModifiersState::empty());
        assert_eq!(key_q, Some(Key::Character("q".to_string())));
        assert_eq!(text_q, Some("q".to_string()));
    }
}
