use crate::window::event::{Key, KeypadKey, NamedKey};

/// Converts a standard X11 keysym to a portable `Key` and optional text string.
pub fn keysym_to_key(keysym: u32) -> (Option<Key>, Option<String>) {
    match keysym {
        0xff08 => (Some(Key::Named(NamedKey::Backspace)), None),
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
        0xffaa => (Some(Key::Keypad(KeypadKey::Multiply)), Some("*".to_string())),
        0xffaf => (Some(Key::Keypad(KeypadKey::Divide)), Some("/".to_string())),
        0xffae => (Some(Key::Keypad(KeypadKey::Decimal)), Some(".".to_string())),
        0xffac => (Some(Key::Keypad(KeypadKey::Separator)), Some(",".to_string())),
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

        // Modifiers (Shift, Ctrl, Alt, Super/Hyper)
        0xffe1 | 0xffe2 => (Some(Key::Named(NamedKey::Shift)), None),
        0xffe3 | 0xffe4 => (Some(Key::Named(NamedKey::Control)), None),
        0xffe7 | 0xffe8 | 0xffe9 | 0xffea => (Some(Key::Named(NamedKey::Alt)), None),
        0xffeb | 0xffec | 0xffed | 0xffee => (Some(Key::Named(NamedKey::Super)), None),

        // ASCII & Latin-1 direct printable mapping
        0x20..=0x7e | 0xa0..=0xff => {
            if let Some(ch) = char::from_u32(keysym) {
                let s = ch.to_string();
                (Some(Key::Character(s.clone())), Some(s))
            } else {
                (None, None)
            }
        }

        // Direct Unicode keysyms (0x01000000 ..= 0x0110ffff)
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
    fn test_keypad_keysyms() {
        assert_eq!(
            keysym_to_key(0xffab),
            (Some(Key::Keypad(KeypadKey::Plus)), Some("+".to_string()))
        );
        assert_eq!(
            keysym_to_key(0xffad),
            (Some(Key::Keypad(KeypadKey::Minus)), Some("-".to_string()))
        );
        assert_eq!(
            keysym_to_key(0xffaa),
            (
                Some(Key::Keypad(KeypadKey::Multiply)),
                Some("*".to_string())
            )
        );
        assert_eq!(
            keysym_to_key(0xffaf),
            (Some(Key::Keypad(KeypadKey::Divide)), Some("/".to_string()))
        );
        assert_eq!(
            keysym_to_key(0xff8d),
            (Some(Key::Keypad(KeypadKey::Enter)), Some("\r".to_string()))
        );
        assert_eq!(
            keysym_to_key(0xffb5),
            (Some(Key::Keypad(KeypadKey::Num5)), Some("5".to_string()))
        );
        assert_eq!(
            keysym_to_key(0xffae),
            (Some(Key::Keypad(KeypadKey::Decimal)), Some(".".to_string()))
        );
        assert_eq!(
            keysym_to_key(0xff95),
            (Some(Key::Named(NamedKey::Home)), None)
        );
        assert_eq!(
            keysym_to_key(0xff9f),
            (Some(Key::Named(NamedKey::Delete)), None)
        );
    }
}
