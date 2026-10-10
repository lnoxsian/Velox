use crate::window::event::{Key, ModifiersState, NamedKey};
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

/// State machine wrapping dynamic `xkbcommon` with an evdev fallback.
pub struct XkbHandler {
    lib: Option<xkbcommon_dl::XkbCommon>,
    ctx: *mut xkbcommon_dl::xkb_context,
    keymap: *mut xkbcommon_dl::xkb_keymap,
    state: *mut xkbcommon_dl::xkb_state,
}

impl Default for XkbHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl XkbHandler {
    pub fn new() -> Self {
        let (lib_opt, ctx) = match unsafe {
            xkbcommon_dl::XkbCommon::open("libxkbcommon.so.0")
                .or_else(|_| xkbcommon_dl::XkbCommon::open("libxkbcommon.so"))
        } {
            Ok(lib) => {
                let ctx = unsafe {
                    (lib.xkb_context_new)(xkbcommon_dl::xkb_context_flags::XKB_CONTEXT_NO_FLAGS)
                };
                if ctx.is_null() {
                    log::warn!("Failed to create XKB context; using evdev fallback");
                    (None, std::ptr::null_mut())
                } else {
                    (Some(lib), ctx)
                }
            }
            Err(e) => {
                log::warn!("libxkbcommon.so not found (error: {:?}); using evdev fallback", e);
                (None, std::ptr::null_mut())
            }
        };

        Self {
            lib: lib_opt,
            ctx,
            keymap: std::ptr::null_mut(),
            state: std::ptr::null_mut(),
        }
    }

    /// Compile a new keymap from the compositor's shared memory fd.
    pub fn update_keymap(&mut self, fd: &std::os::fd::OwnedFd, size: u32) {
        let Some(lib) = &self.lib else {
            return;
        };
        if self.ctx.is_null() || size == 0 {
            return;
        }

        let mmap = unsafe {
            match memmap2::MmapOptions::new().len(size as usize).map(fd.as_raw_fd()) {
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

        let keymap = unsafe {
            (lib.xkb_keymap_new_from_buffer)(
                self.ctx,
                mmap.as_ptr() as *const _,
                len,
                xkbcommon_dl::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };

        if keymap.is_null() {
            log::error!("xkb_keymap_new_from_buffer failed to compile keymap");
            return;
        }

        let state = unsafe { (lib.xkb_state_new)(keymap) };
        if state.is_null() {
            log::error!("xkb_state_new failed to create state from keymap");
            unsafe { (lib.xkb_keymap_unref)(keymap) };
            return;
        }

        if !self.state.is_null() {
            unsafe { (lib.xkb_state_unref)(self.state) };
        }
        if !self.keymap.is_null() {
            unsafe { (lib.xkb_keymap_unref)(self.keymap) };
        }

        self.keymap = keymap;
        self.state = state;
    }

    /// Update active modifier state masks from compositor's `Modifiers` event.
    pub fn update_modifiers(
        &mut self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) -> ModifiersState {
        let Some(lib) = &self.lib else {
            let mask = mods_depressed | mods_latched | mods_locked;
            let mut m = ModifiersState::empty();
            if mask & 1 != 0 {
                m |= ModifiersState::SHIFT;
            }
            if mask & 4 != 0 {
                m |= ModifiersState::CONTROL;
            }
            if mask & 8 != 0 {
                m |= ModifiersState::ALT;
            }
            if mask & 64 != 0 {
                m |= ModifiersState::SUPER;
            }
            return m;
        };

        if self.state.is_null() {
            return ModifiersState::empty();
        }

        unsafe {
            (lib.xkb_state_update_mask)(
                self.state,
                mods_depressed,
                mods_latched,
                mods_locked,
                0,
                0,
                group,
            );
        }

        let mut m = ModifiersState::empty();
        unsafe {
            if (lib.xkb_state_mod_name_is_active)(
                self.state,
                xkbcommon_dl::XKB_MOD_NAME_SHIFT.as_ptr() as *const _,
                xkbcommon_dl::xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) == 1
            {
                m |= ModifiersState::SHIFT;
            }
            if (lib.xkb_state_mod_name_is_active)(
                self.state,
                xkbcommon_dl::XKB_MOD_NAME_CTRL.as_ptr() as *const _,
                xkbcommon_dl::xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) == 1
            {
                m |= ModifiersState::CONTROL;
            }
            if (lib.xkb_state_mod_name_is_active)(
                self.state,
                xkbcommon_dl::XKB_MOD_NAME_ALT.as_ptr() as *const _,
                xkbcommon_dl::xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) == 1
            {
                m |= ModifiersState::ALT;
            }
            if (lib.xkb_state_mod_name_is_active)(
                self.state,
                xkbcommon_dl::XKB_MOD_NAME_LOGO.as_ptr() as *const _,
                xkbcommon_dl::xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) == 1
            {
                m |= ModifiersState::SUPER;
            }
        }
        m
    }

    /// Translate a Linux evdev scancode into a `(Key, Option<String>, repeats)` tuple.
    pub fn translate_key(
        &self,
        scancode: u32,
        modifiers: ModifiersState,
    ) -> (Option<Key>, Option<String>, bool) {
        if let Some(lib) = &self.lib
            && !self.state.is_null()
            && !self.keymap.is_null()
        {
            let xkb_keycode = scancode + 8;
            let sym = unsafe { (lib.xkb_state_key_get_one_sym)(self.state, xkb_keycode) };
            let repeats = unsafe { (lib.xkb_keymap_key_repeats)(self.keymap, xkb_keycode) == 1 };

            let mut utf8_buf = [0u8; 64];
            let len = unsafe {
                (lib.xkb_state_key_get_utf8)(
                    self.state,
                    xkb_keycode,
                    utf8_buf.as_mut_ptr() as *mut _,
                    utf8_buf.len(),
                )
            };
            let xkb_text = if len > 0 {
                std::str::from_utf8(&utf8_buf[..len as usize])
                    .ok()
                    .map(|s| s.to_string())
            } else {
                None
            };

            let (mut key, mut text) = xkb_keysym_to_key(sym);
            if text.is_none() && xkb_text.is_some() {
                text = xkb_text;
            }
            if key.is_none()
                && let Some(t) = &text
            {
                key = Some(Key::Character(t.clone()));
            }
            return (key, text, repeats);
        }

        // Fallback to internal evdev scancode table
        let (key, text) = evdev_scancode_to_key(scancode, modifiers);
        let repeats = !matches!(scancode, 29 | 42 | 54 | 56 | 58 | 97 | 100 | 125 | 126);
        (key, text, repeats)
    }
}

impl Drop for XkbHandler {
    fn drop(&mut self) {
        if let Some(lib) = &self.lib {
            if !self.state.is_null() {
                unsafe { (lib.xkb_state_unref)(self.state) };
            }
            if !self.keymap.is_null() {
                unsafe { (lib.xkb_keymap_unref)(self.keymap) };
            }
            if !self.ctx.is_null() {
                unsafe { (lib.xkb_context_unref)(self.ctx) };
            }
        }
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
        2 => if shift { (Some(Key::Character("!".to_string())), Some("!".to_string())) } else { (Some(Key::Character("1".to_string())), Some("1".to_string())) },
        3 => if shift { (Some(Key::Character("@".to_string())), Some("@".to_string())) } else { (Some(Key::Character("2".to_string())), Some("2".to_string())) },
        4 => if shift { (Some(Key::Character("#".to_string())), Some("#".to_string())) } else { (Some(Key::Character("3".to_string())), Some("3".to_string())) },
        5 => if shift { (Some(Key::Character("$".to_string())), Some("$".to_string())) } else { (Some(Key::Character("4".to_string())), Some("4".to_string())) },
        6 => if shift { (Some(Key::Character("%".to_string())), Some("%".to_string())) } else { (Some(Key::Character("5".to_string())), Some("5".to_string())) },
        7 => if shift { (Some(Key::Character("^".to_string())), Some("^".to_string())) } else { (Some(Key::Character("6".to_string())), Some("6".to_string())) },
        8 => if shift { (Some(Key::Character("&".to_string())), Some("&".to_string())) } else { (Some(Key::Character("7".to_string())), Some("7".to_string())) },
        9 => if shift { (Some(Key::Character("*".to_string())), Some("*".to_string())) } else { (Some(Key::Character("8".to_string())), Some("8".to_string())) },
        10 => if shift { (Some(Key::Character("(".to_string())), Some("(".to_string())) } else { (Some(Key::Character("9".to_string())), Some("9".to_string())) },
        11 => if shift { (Some(Key::Character(")".to_string())), Some(")".to_string())) } else { (Some(Key::Character("0".to_string())), Some("0".to_string())) },
        12 => if shift { (Some(Key::Character("_".to_string())), Some("_".to_string())) } else { (Some(Key::Character("-".to_string())), Some("-".to_string())) },
        13 => if shift { (Some(Key::Character("+".to_string())), Some("+".to_string())) } else { (Some(Key::Character("=".to_string())), Some("=".to_string())) },
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
        26 => if shift { (Some(Key::Character("{".to_string())), Some("{".to_string())) } else { (Some(Key::Character("[".to_string())), Some("[".to_string())) },
        27 => if shift { (Some(Key::Character("}".to_string())), Some("}".to_string())) } else { (Some(Key::Character("]".to_string())), Some("]".to_string())) },
        28 => (Some(Key::Named(NamedKey::Enter)), Some("\r".to_string())),
        29 => (None, None), // LCtrl

        30 => letter('a', shift),
        31 => letter('s', shift),
        32 => letter('d', shift),
        33 => letter('f', shift),
        34 => letter('g', shift),
        35 => letter('h', shift),
        36 => letter('j', shift),
        37 => letter('k', shift),
        38 => letter('l', shift),
        39 => if shift { (Some(Key::Character(":".to_string())), Some(":".to_string())) } else { (Some(Key::Character(";".to_string())), Some(";".to_string())) },
        40 => if shift { (Some(Key::Character("\"".to_string())), Some("\"".to_string())) } else { (Some(Key::Character("'".to_string())), Some("'".to_string())) },
        41 => if shift { (Some(Key::Character("~".to_string())), Some("~".to_string())) } else { (Some(Key::Character("`".to_string())), Some("`".to_string())) },
        42 => (None, None), // LShift
        43 => if shift { (Some(Key::Character("|".to_string())), Some("|".to_string())) } else { (Some(Key::Character("\\".to_string())), Some("\\".to_string())) },

        44 => letter('z', shift),
        45 => letter('x', shift),
        46 => letter('c', shift),
        47 => letter('v', shift),
        48 => letter('b', shift),
        49 => letter('n', shift),
        50 => letter('m', shift),
        51 => if shift { (Some(Key::Character("<".to_string())), Some("<".to_string())) } else { (Some(Key::Character(",".to_string())), Some(",".to_string())) },
        52 => if shift { (Some(Key::Character(">".to_string())), Some(">".to_string())) } else { (Some(Key::Character(".".to_string())), Some(".".to_string())) },
        53 => if shift { (Some(Key::Character("?".to_string())), Some("?".to_string())) } else { (Some(Key::Character("/".to_string())), Some("/".to_string())) },
        54 => (None, None), // RShift
        56 => (None, None), // LAlt
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

        97 => (None, None), // RCtrl
        100 => (None, None), // RAlt
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
        125 => (None, None), // LSuper
        126 => (None, None), // RSuper

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
        0xff08 => (Some(Key::Named(NamedKey::Backspace)), None),
        0xff09 => (Some(Key::Named(NamedKey::Tab)), Some("\t".to_string())),
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

        0xffe1..=0xffec => (None, None), // Modifiers (Shift, Ctrl, Alt, Super, Caps)

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

        0x01000100..=0x0110ffff => {
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
        let (key, text, repeats) = handler.translate_key(30, ModifiersState::empty());
        assert!(key.is_some());
        assert_eq!(text, Some("a".to_string()));
        assert!(repeats);
    }
}
