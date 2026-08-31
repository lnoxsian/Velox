use smallvec::{SmallVec, smallvec};
use winit::keyboard::{Key, ModifiersState, NamedKey};

pub fn translate_key(
    key: &Key,
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

    // ── 1. Kitty Keyboard Protocol (if enabled) ──────────────────────────────
    if kitty_flags > 0 {
        let disambiguate = (kitty_flags & 1) != 0;
        let report_all = (kitty_flags & 8) != 0;

        if disambiguate || report_all {
            let kitty_mod = xterm_mod;
            match key {
                Key::Named(NamedKey::Enter) if kitty_mod > 1 || report_all => {
                    let mut buf = SmallVec::<[u8; 16]>::new();
                    let s = format!("\x1b[13;{}u", kitty_mod);
                    buf.extend_from_slice(s.as_bytes());
                    return Some(buf);
                }
                Key::Named(NamedKey::Tab) if kitty_mod > 1 || report_all => {
                    let mut buf = SmallVec::<[u8; 16]>::new();
                    let s = format!("\x1b[9;{}u", kitty_mod);
                    buf.extend_from_slice(s.as_bytes());
                    return Some(buf);
                }
                Key::Named(NamedKey::Backspace) if kitty_mod > 1 || report_all => {
                    let mut buf = SmallVec::<[u8; 16]>::new();
                    let s = format!("\x1b[127;{}u", kitty_mod);
                    buf.extend_from_slice(s.as_bytes());
                    return Some(buf);
                }
                Key::Named(NamedKey::Escape) if kitty_mod > 1 || report_all => {
                    let mut buf = SmallVec::<[u8; 16]>::new();
                    let s = format!("\x1b[27;{}u", kitty_mod);
                    buf.extend_from_slice(s.as_bytes());
                    return Some(buf);
                }
                Key::Character(s) if s.len() == 1 && (kitty_mod > 1 || report_all) => {
                    let c = s.chars().next().unwrap();
                    let cp = c as u32;
                    let mut buf = SmallVec::<[u8; 16]>::new();
                    let s = format!("\x1b[{};{}u", cp, kitty_mod);
                    buf.extend_from_slice(s.as_bytes());
                    return Some(buf);
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
    if has_ctrl && !has_alt {
        match key {
            Key::Character(s) if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    let code = c.to_ascii_uppercase() as u8 - b'A' + 1;
                    return Some(smallvec![code]);
                }
                match c {
                    '@' | '2' | ' ' => return Some(smallvec![0]),
                    '[' | '3' => return Some(smallvec![27]),
                    '\\' | '4' => return Some(smallvec![28]),
                    ']' | '5' => return Some(smallvec![29]),
                    '^' | '6' => return Some(smallvec![30]),
                    '_' | '7' => return Some(smallvec![31]),
                    '8' | '?' => return Some(smallvec![127]),
                    _ => {}
                }
            }
            Key::Named(NamedKey::Space) => return Some(smallvec![0]),
            _ => {}
        }
    }

    // ── 4. Standard XTerm Modified & Unmodified Keys ─────────────────────────
    match key {
        Key::Character(s) => {
            let buf = SmallVec::<[u8; 16]>::from_slice(s.as_bytes());
            if has_alt {
                let mut escaped: SmallVec<[u8; 16]> = smallvec![27u8];
                escaped.extend_from_slice(&buf);
                Some(escaped)
            } else {
                Some(buf)
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
            if has_alt {
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
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}A", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOA"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[A"))
            }
        }
        Key::Named(NamedKey::ArrowDown) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}B", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOB"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[B"))
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}C", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOC"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[C"))
            }
        }
        Key::Named(NamedKey::ArrowLeft) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}D", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else if cursor_keys_mode {
                Some(SmallVec::from_slice(b"\x1bOD"))
            } else {
                Some(SmallVec::from_slice(b"\x1b[D"))
            }
        }

        // Navigation Keys (with modifier encoding)
        Key::Named(NamedKey::Home) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}H", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[H"))
            }
        }
        Key::Named(NamedKey::End) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}F", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[F"))
            }
        }
        Key::Named(NamedKey::PageUp) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[5;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[5~"))
            }
        }
        Key::Named(NamedKey::PageDown) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[6;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[6~"))
            }
        }
        Key::Named(NamedKey::Delete) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[3;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[3~"))
            }
        }
        Key::Named(NamedKey::Insert) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[2;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[2~"))
            }
        }

        // Function Keys (with modifier encoding)
        Key::Named(NamedKey::F1) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}P", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1bOP"))
            }
        }
        Key::Named(NamedKey::F2) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}Q", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1bOQ"))
            }
        }
        Key::Named(NamedKey::F3) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}R", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1bOR"))
            }
        }
        Key::Named(NamedKey::F4) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[1;{}S", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1bOS"))
            }
        }
        Key::Named(NamedKey::F5) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[15;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[15~"))
            }
        }
        Key::Named(NamedKey::F6) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[17;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[17~"))
            }
        }
        Key::Named(NamedKey::F7) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[18;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[18~"))
            }
        }
        Key::Named(NamedKey::F8) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[19;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[19~"))
            }
        }
        Key::Named(NamedKey::F9) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[20;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[20~"))
            }
        }
        Key::Named(NamedKey::F10) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[21;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[21~"))
            }
        }
        Key::Named(NamedKey::F11) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[23;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[23~"))
            }
        }
        Key::Named(NamedKey::F12) => {
            if xterm_mod > 1 {
                let mut buf = SmallVec::<[u8; 16]>::new();
                let s = format!("\x1b[24;{}~", xterm_mod);
                buf.extend_from_slice(s.as_bytes());
                Some(buf)
            } else {
                Some(SmallVec::from_slice(b"\x1b[24~"))
            }
        }

        _ => None,
    }
}
