use crate::window::event::{Key, ModifiersState, NamedKey};
use smallvec::{SmallVec, smallvec};

#[inline(always)]
fn xterm_mod_seq(prefix: &[u8], mod_num: u8, suffix: &[u8]) -> SmallVec<[u8; 16]> {
    let mut buf = SmallVec::<[u8; 16]>::new();
    buf.extend_from_slice(prefix);
    if mod_num >= 10 {
        buf.push(b'0' + (mod_num / 10));
    }
    buf.push(b'0' + (mod_num % 10));
    buf.extend_from_slice(suffix);
    buf
}

#[inline(always)]
fn kitty_char_seq(cp: u32, mod_num: u8) -> SmallVec<[u8; 16]> {
    let mut buf = SmallVec::<[u8; 16]>::new();
    buf.extend_from_slice(b"\x1b[");
    let mut digits = [0u8; 10];
    let mut n = cp;
    let mut i = 0;
    if n == 0 {
        digits[0] = b'0';
        i = 1;
    } else {
        while n > 0 {
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
    }
    for idx in (0..i).rev() {
        buf.push(digits[idx]);
    }
    buf.push(b';');
    if mod_num >= 10 {
        buf.push(b'0' + (mod_num / 10));
    }
    buf.push(b'0' + (mod_num % 10));
    buf.push(b'u');
    buf
}

pub fn translate_key(
    key: &Key,
    text: Option<&str>,
    modifiers: ModifiersState,
    cursor_keys_mode: bool,
    kitty_flags: u16,
) -> Option<SmallVec<[u8; 16]>> {
    let has_shift = modifiers.shift_key();
    let has_alt = modifiers.alt_key();
    let has_ctrl = modifiers.control_key();
    let has_super = modifiers.super_key();

    let xterm_mod = 1
        + (if has_shift { 1 } else { 0 })
        + (if has_alt { 2 } else { 0 })
        + (if has_ctrl { 4 } else { 0 })
        + (if has_super { 8 } else { 0 });

    // ── 0. Reject modifier-only key events from generating PTY input ───────
    if matches!(
        key,
        Key::Named(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)
    ) {
        return None;
    }

    // ── 1. Kitty Keyboard Protocol (if enabled) ──────────────────────────────
    if kitty_flags > 0 {
        let disambiguate = (kitty_flags & 1) != 0;
        let report_all = (kitty_flags & 8) != 0;

        if disambiguate || report_all {
            let kitty_mod = xterm_mod as u8;
            match key {
                Key::Named(NamedKey::Enter) if kitty_mod > 1 || report_all => {
                    return Some(xterm_mod_seq(b"\x1b[13;", kitty_mod, b"u"));
                }
                Key::Named(NamedKey::Tab) if kitty_mod > 1 || report_all => {
                    return Some(xterm_mod_seq(b"\x1b[9;", kitty_mod, b"u"));
                }
                Key::Named(NamedKey::Backspace) if kitty_mod > 1 || report_all => {
                    return Some(xterm_mod_seq(b"\x1b[127;", kitty_mod, b"u"));
                }
                Key::Named(NamedKey::Escape) if kitty_mod > 1 || report_all => {
                    return Some(xterm_mod_seq(b"\x1b[27;", kitty_mod, b"u"));
                }
                Key::Character(s) if s.len() == 1 => {
                    // In disambiguate mode, only encode modified keys (Ctrl, Alt, Super).
                    // Shift alone on printable characters must produce standard text (not escape codes).
                    let has_command_mod = has_ctrl || has_alt || has_super;
                    if report_all || has_command_mod {
                        let c = s.chars().next().unwrap();
                        let cp = c.to_ascii_lowercase() as u32;
                        return Some(kitty_char_seq(cp, kitty_mod));
                    }
                }
                _ => {}
            }
        }
    }

    // ── 2. Shift+Tab (Cursor Backward Tab / CBT) ─────────────────────────────
    if has_shift && matches!(key, Key::Named(NamedKey::Tab)) {
        return Some(SmallVec::from_slice(b"\x1b[Z"));
    }

    // ── 3. Control Key Combinations for Printable Characters ─────────────────
    if has_ctrl {
        let ctrl_code = match key {
            Key::Character(s) if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    Some(c.to_ascii_uppercase() as u8 - b'A' + 1)
                } else {
                    match c {
                        '@' | '2' | '`' => Some(0),
                        '[' | '3' => Some(27),
                        '\\' | '4' => Some(28),
                        ']' | '5' => Some(29),
                        '^' | '6' => Some(30),
                        '-' | '_' | '7' | '/' => Some(31),
                        '8' | '?' => Some(127),
                        ' ' => Some(0),
                        _ => None,
                    }
                }
            }
            Key::Named(NamedKey::Space) => Some(0),
            _ => None,
        };

        if let Some(code) = ctrl_code {
            if has_alt {
                return Some(smallvec![27, code]);
            } else {
                return Some(smallvec![code]);
            }
        }
    }

    // ── 4. Standard XTerm Modified & Unmodified Keys ─────────────────────────
    // If platform resolved printable text (including dead keys, AltGr, IME, shifted symbols) and no Ctrl/Super modifier
    if !has_ctrl
        && !has_super
        && let Some(t) = text
        && !t.is_empty()
    {
        if has_alt {
            let mut escaped = SmallVec::<[u8; 16]>::with_capacity(1 + t.len());
            escaped.push(27);
            escaped.extend_from_slice(t.as_bytes());
            return Some(escaped);
        } else {
            return Some(SmallVec::from_slice(t.as_bytes()));
        }
    }

    match key {
        Key::Character(s) => {
            if has_alt {
                let mut escaped = SmallVec::<[u8; 16]>::with_capacity(1 + s.len());
                escaped.push(27);
                escaped.extend_from_slice(s.as_bytes());
                Some(escaped)
            } else {
                Some(SmallVec::from_slice(s.as_bytes()))
            }
        }
        Key::Dead(Some(c)) => {
            let mut char_buf = [0u8; 4];
            let s = c.encode_utf8(&mut char_buf);
            if has_alt {
                let mut escaped = SmallVec::<[u8; 16]>::with_capacity(1 + s.len());
                escaped.push(27);
                escaped.extend_from_slice(s.as_bytes());
                Some(escaped)
            } else {
                Some(SmallVec::from_slice(s.as_bytes()))
            }
        }
        Key::Named(NamedKey::Enter) => {
            if has_alt {
                Some(smallvec![27, 13])
            } else {
                Some(smallvec![13])
            }
        }
        Key::Named(NamedKey::Backspace) => {
            if has_alt && has_ctrl {
                Some(smallvec![27, 8])
            } else if has_alt {
                Some(smallvec![27, 127])
            } else if has_ctrl {
                Some(smallvec![8]) // ASCII BS for Ctrl+Backspace (word deletion)
            } else {
                Some(smallvec![127])
            }
        }
        Key::Named(NamedKey::Tab) => {
            if has_alt {
                Some(smallvec![27, 9])
            } else {
                Some(smallvec![9])
            }
        }
        Key::Named(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super) => None,
        Key::Named(NamedKey::Escape) => {
            if has_alt {
                Some(smallvec![27, 27])
            } else {
                Some(smallvec![27])
            }
        }
        Key::Named(NamedKey::Space) => {
            if has_alt {
                Some(smallvec![27, 32])
            } else {
                Some(smallvec![32])
            }
        }

        // Arrow Keys (with full modifier encoding)
        Key::Named(NamedKey::ArrowUp) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"A"))
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOA"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[A"))
            }
        }
        Key::Named(NamedKey::ArrowDown) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"B"))
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOB"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[B"))
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"C"))
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOC"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[C"))
            }
        }
        Key::Named(NamedKey::ArrowLeft) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"D"))
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOD"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[D"))
            }
        }

        // Navigation Keys (with modifier encoding)
        Key::Named(NamedKey::Home) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"H"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[H"))
            }
        }
        Key::Named(NamedKey::End) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"F"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[F"))
            }
        }
        Key::Named(NamedKey::PageUp) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[5;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[5~"))
            }
        }
        Key::Named(NamedKey::PageDown) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[6;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[6~"))
            }
        }
        Key::Named(NamedKey::Delete) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[3;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[3~"))
            }
        }
        Key::Named(NamedKey::Insert) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[2;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[2~"))
            }
        }

        // Function Keys (with modifier encoding)
        Key::Named(NamedKey::F1) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"P"))
            } else {
                Some(SmallVec::from_slice(b"\x1bOP"))
            }
        }
        Key::Named(NamedKey::F2) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"Q"))
            } else {
                Some(SmallVec::from_slice(b"\x1bOQ"))
            }
        }
        Key::Named(NamedKey::F3) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"R"))
            } else {
                Some(SmallVec::from_slice(b"\x1bOR"))
            }
        }
        Key::Named(NamedKey::F4) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[1;", xterm_mod as u8, b"S"))
            } else {
                Some(SmallVec::from_slice(b"\x1bOS"))
            }
        }
        Key::Named(NamedKey::F5) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[15;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[15~"))
            }
        }
        Key::Named(NamedKey::F6) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[17;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[17~"))
            }
        }
        Key::Named(NamedKey::F7) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[18;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[18~"))
            }
        }
        Key::Named(NamedKey::F8) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[19;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[19~"))
            }
        }
        Key::Named(NamedKey::F9) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[20;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[20~"))
            }
        }
        Key::Named(NamedKey::F10) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[21;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[21~"))
            }
        }
        Key::Named(NamedKey::F11) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[23;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[23~"))
            }
        }
        Key::Named(NamedKey::F12) => {
            if xterm_mod > 1 {
                Some(xterm_mod_seq(b"\x1b[24;", xterm_mod as u8, b"~"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[24~"))
            }
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_tab_traditional_and_kitty() {
        // Traditional xterm CBT sequence (\x1b[Z) when kitty_flags == 0
        let cbt = translate_key(
            &Key::Named(NamedKey::Tab),
            None,
            ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(cbt.as_deref(), Some(&b"\x1b[Z"[..]));

        // Negotiated Kitty CSI-u sequence (\x1b[9;2u) when kitty_flags > 0
        let kitty_cbt = translate_key(
            &Key::Named(NamedKey::Tab),
            None,
            ModifiersState::SHIFT,
            false,
            1,
        );
        assert_eq!(kitty_cbt.as_deref(), Some(&b"\x1b[9;2u"[..]));

        // Plain tab -> 0x09
        let plain_tab = translate_key(
            &Key::Named(NamedKey::Tab),
            Some("\t"),
            ModifiersState::empty(),
            false,
            0,
        );
        assert_eq!(plain_tab.as_deref(), Some(&[9][..]));
    }

    #[test]
    fn test_control_and_alt_printable_keys() {
        // Ctrl+C -> 0x03
        let ctrl_c = translate_key(
            &Key::Character("c".to_string()),
            Some("c"),
            ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(ctrl_c.as_deref(), Some(&[3][..]));

        // Alt+x -> ESC x (\x1b x)
        let alt_x = translate_key(
            &Key::Character("x".to_string()),
            Some("x"),
            ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(alt_x.as_deref(), Some(&b"\x1bx"[..]));

        // Ctrl+Alt+a -> ESC 0x01
        let ctrl_alt_a = translate_key(
            &Key::Character("a".to_string()),
            Some("a"),
            ModifiersState::CONTROL | ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(ctrl_alt_a.as_deref(), Some(&[27, 1][..]));

        // Ctrl+Alt+c -> ESC 0x03
        let ctrl_alt_c = translate_key(
            &Key::Character("c".to_string()),
            Some("c"),
            ModifiersState::CONTROL | ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(ctrl_alt_c.as_deref(), Some(&[27, 3][..]));

        // Unbound Ctrl+Shift+R -> Ctrl+R (0x12) when kitty is off
        let ctrl_shift_r = translate_key(
            &Key::Character("r".to_string()),
            Some("r"),
            ModifiersState::CONTROL | ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(ctrl_shift_r.as_deref(), Some(&[18][..]));

        // Ctrl+- -> 0x1f (Unit Separator / Undo)
        let ctrl_minus = translate_key(
            &Key::Character("-".to_string()),
            Some("-"),
            ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(ctrl_minus.as_deref(), Some(&[31][..]));

        // Ctrl+` -> 0x00 (NUL)
        let ctrl_backtick = translate_key(
            &Key::Character("`".to_string()),
            Some("`"),
            ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(ctrl_backtick.as_deref(), Some(&[0][..]));
    }

    #[test]
    fn test_modified_arrows() {
        // Ctrl+Up -> ESC [ 1;5 A
        let ctrl_up = translate_key(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(ctrl_up.as_deref(), Some(&b"\x1b[1;5A"[..]));

        // Alt+Up -> ESC [ 1;3 A
        let alt_up = translate_key(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(alt_up.as_deref(), Some(&b"\x1b[1;3A"[..]));

        // Ctrl+Shift+Up -> ESC [ 1;6 A
        let ctrl_shift_up = translate_key(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(ctrl_shift_up.as_deref(), Some(&b"\x1b[1;6A"[..]));

        // Ctrl+Alt+Up -> ESC [ 1;7 A
        let ctrl_alt_up = translate_key(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::CONTROL | ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(ctrl_alt_up.as_deref(), Some(&b"\x1b[1;7A"[..]));

        // Ctrl+Alt+Shift+Up -> ESC [ 1;8 A
        let ctrl_alt_shift_up = translate_key(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::CONTROL | ModifiersState::ALT | ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(ctrl_alt_shift_up.as_deref(), Some(&b"\x1b[1;8A"[..]));
    }

    #[test]
    fn test_modified_navigation_keys() {
        // Shift+Home (mod 2) -> ESC [ 1;2 H
        let shift_home = translate_key(
            &Key::Named(NamedKey::Home),
            None,
            ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(shift_home.as_deref(), Some(&b"\x1b[1;2H"[..]));

        // Ctrl+End (mod 5) -> ESC [ 1;5 F
        let ctrl_end = translate_key(
            &Key::Named(NamedKey::End),
            None,
            ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(ctrl_end.as_deref(), Some(&b"\x1b[1;5F"[..]));

        // Alt+Delete (mod 3) -> ESC [ 3;3 ~
        let alt_del = translate_key(
            &Key::Named(NamedKey::Delete),
            None,
            ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(alt_del.as_deref(), Some(&b"\x1b[3;3~"[..]));

        // Ctrl+Shift+Insert (mod 6) -> ESC [ 2;6 ~
        let ctrl_shift_ins = translate_key(
            &Key::Named(NamedKey::Insert),
            None,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(ctrl_shift_ins.as_deref(), Some(&b"\x1b[2;6~"[..]));

        // Ctrl+PageUp (mod 5) -> ESC [ 5;5 ~
        let ctrl_pgup = translate_key(
            &Key::Named(NamedKey::PageUp),
            None,
            ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(ctrl_pgup.as_deref(), Some(&b"\x1b[5;5~"[..]));

        // Alt+PageDown (mod 3) -> ESC [ 6;3 ~
        let alt_pgdn = translate_key(
            &Key::Named(NamedKey::PageDown),
            None,
            ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(alt_pgdn.as_deref(), Some(&b"\x1b[6;3~"[..]));
    }

    #[test]
    fn test_modified_function_keys() {
        // Shift+F1 (mod 2) -> ESC [ 1;2 P
        let shift_f1 = translate_key(
            &Key::Named(NamedKey::F1),
            None,
            ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(shift_f1.as_deref(), Some(&b"\x1b[1;2P"[..]));

        // Alt+F4 (mod 3) -> ESC [ 1;3 S
        let alt_f4 = translate_key(
            &Key::Named(NamedKey::F4),
            None,
            ModifiersState::ALT,
            false,
            0,
        );
        assert_eq!(alt_f4.as_deref(), Some(&b"\x1b[1;3S"[..]));

        // Ctrl+F5 (mod 5) -> ESC [ 15;5 ~
        let ctrl_f5 = translate_key(
            &Key::Named(NamedKey::F5),
            None,
            ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(ctrl_f5.as_deref(), Some(&b"\x1b[15;5~"[..]));

        // Ctrl+Alt+Shift+F12 (mod 8) -> ESC [ 24;8 ~
        let full_f12 = translate_key(
            &Key::Named(NamedKey::F12),
            None,
            ModifiersState::CONTROL | ModifiersState::ALT | ModifiersState::SHIFT,
            false,
            0,
        );
        assert_eq!(full_f12.as_deref(), Some(&b"\x1b[24;8~"[..]));
    }

    #[test]
    fn test_dead_keys_and_resolved_text() {
        // Dead tilde
        let dead = translate_key(
            &Key::Dead(Some('~')),
            None,
            ModifiersState::empty(),
            false,
            0,
        );
        assert_eq!(dead.as_deref(), Some(&b"~"[..]));

        // Alt+dead circumflex
        let alt_dead = translate_key(&Key::Dead(Some('^')), None, ModifiersState::ALT, false, 0);
        assert_eq!(alt_dead.as_deref(), Some(&b"\x1b^"[..]));

        // Resolved text from AltGr or composed key (e.g. '@' or '€')
        let euro = translate_key(
            &Key::Character("€".to_string()),
            Some("€"),
            ModifiersState::empty(),
            false,
            0,
        );
        assert_eq!(euro.as_deref(), Some("€".as_bytes()));
    }

    #[test]
    fn test_super_meta_combinations() {
        // Super+Up (mod 9) -> ESC [ 1;9 A
        let super_up = translate_key(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::SUPER,
            false,
            0,
        );
        assert_eq!(super_up.as_deref(), Some(&b"\x1b[1;9A"[..]));

        // Super+Ctrl+Up (mod 13) -> ESC [ 1;13 A
        let super_ctrl_up = translate_key(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::SUPER | ModifiersState::CONTROL,
            false,
            0,
        );
        assert_eq!(super_ctrl_up.as_deref(), Some(&b"\x1b[1;13A"[..]));

        // Kitty protocol Super+a (mod 9) -> ESC [ 97;9 u
        let kitty_super_a = translate_key(
            &Key::Character("a".to_string()),
            Some("a"),
            ModifiersState::SUPER,
            false,
            1,
        );
        assert_eq!(kitty_super_a.as_deref(), Some(&b"\x1b[97;9u"[..]));
    }

    #[test]
    fn test_modifier_only_keys_return_none() {
        assert!(
            translate_key(
                &Key::Named(NamedKey::Shift),
                None,
                ModifiersState::SHIFT,
                false,
                0
            )
            .is_none()
        );
        assert!(
            translate_key(
                &Key::Named(NamedKey::Control),
                None,
                ModifiersState::CONTROL,
                false,
                0
            )
            .is_none()
        );
        assert!(
            translate_key(
                &Key::Named(NamedKey::Alt),
                None,
                ModifiersState::ALT,
                false,
                0
            )
            .is_none()
        );
        assert!(
            translate_key(
                &Key::Named(NamedKey::Super),
                None,
                ModifiersState::SUPER,
                false,
                0
            )
            .is_none()
        );
    }
}
