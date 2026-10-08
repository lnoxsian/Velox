use crate::window::event::{Key, NamedKey};

/// Converts a standard X11 keysym to a portable `Key` and optional text string.
pub fn keysym_to_key(keysym: u32) -> (Option<Key>, Option<String>) {
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

        // Function keys F1-F12
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

        // ASCII & Latin-1 direct printable mapping
        0x20..=0x7e | 0xa0..=0xff => {
            if let Some(ch) = char::from_u32(keysym) {
                let s = ch.to_string();
                (Some(Key::Character(s.clone())), Some(s))
            } else {
                (None, None)
            }
        }

        // Direct Unicode keysyms (0x01000100 ..= 0x0110ffff)
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
