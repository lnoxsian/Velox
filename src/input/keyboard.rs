use winit::keyboard::{Key, ModifiersState, NamedKey};

pub fn translate_key(
    key: &Key,
    modifiers: ModifiersState,
    cursor_keys_mode: bool,
) -> Option<Vec<u8>> {
    // 1. Control key combinations
    if modifiers.control_key() {
        match key {
            Key::Character(s) if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    let code = c.to_ascii_uppercase() as u8 - b'A' + 1;
                    return Some(vec![code]);
                }
                match c {
                    '@' => return Some(vec![0]),
                    '[' => return Some(vec![27]),
                    '\\' => return Some(vec![28]),
                    ']' => return Some(vec![29]),
                    '^' => return Some(vec![30]),
                    '_' => return Some(vec![31]),
                    '?' => return Some(vec![127]),
                    _ => {}
                }
            }
            Key::Named(NamedKey::Space) => return Some(vec![0]),
            _ => {}
        }
    }

    // 2. Base mapping (and Alt combinations)
    let base_seq = match key {
        Key::Character(s) => Some(s.as_bytes().to_vec()),
        Key::Named(NamedKey::Enter) => Some(vec![13]),
        Key::Named(NamedKey::Backspace) => Some(vec![127]),
        Key::Named(NamedKey::Tab) => Some(vec![9]),
        Key::Named(NamedKey::Escape) => Some(vec![27]),
        Key::Named(NamedKey::Space) => Some(vec![32]),

        // Arrow Keys
        Key::Named(NamedKey::ArrowUp) => Some(if cursor_keys_mode {
            b"\x1bOA".to_vec()
        } else {
            b"\x1b[A".to_vec()
        }),
        Key::Named(NamedKey::ArrowDown) => Some(if cursor_keys_mode {
            b"\x1bOB".to_vec()
        } else {
            b"\x1b[B".to_vec()
        }),
        Key::Named(NamedKey::ArrowRight) => Some(if cursor_keys_mode {
            b"\x1bOC".to_vec()
        } else {
            b"\x1b[C".to_vec()
        }),
        Key::Named(NamedKey::ArrowLeft) => Some(if cursor_keys_mode {
            b"\x1bOD".to_vec()
        } else {
            b"\x1b[D".to_vec()
        }),

        // Navigation Keys
        Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
        Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
        Key::Named(NamedKey::PageUp) => Some(b"\x1b[5~".to_vec()),
        Key::Named(NamedKey::PageDown) => Some(b"\x1b[6~".to_vec()),
        Key::Named(NamedKey::Delete) => Some(b"\x1b[3~".to_vec()),
        Key::Named(NamedKey::Insert) => Some(b"\x1b[2~".to_vec()),

        // Function Keys
        Key::Named(NamedKey::F1) => Some(b"\x1bOP".to_vec()),
        Key::Named(NamedKey::F2) => Some(b"\x1bOQ".to_vec()),
        Key::Named(NamedKey::F3) => Some(b"\x1bOR".to_vec()),
        Key::Named(NamedKey::F4) => Some(b"\x1bOS".to_vec()),
        Key::Named(NamedKey::F5) => Some(b"\x1b[15~".to_vec()),
        Key::Named(NamedKey::F6) => Some(b"\x1b[17~".to_vec()),
        Key::Named(NamedKey::F7) => Some(b"\x1b[18~".to_vec()),
        Key::Named(NamedKey::F8) => Some(b"\x1b[19~".to_vec()),
        Key::Named(NamedKey::F9) => Some(b"\x1b[20~".to_vec()),
        Key::Named(NamedKey::F10) => Some(b"\x1b[21~".to_vec()),
        Key::Named(NamedKey::F11) => Some(b"\x1b[23~".to_vec()),
        Key::Named(NamedKey::F12) => Some(b"\x1b[24~".to_vec()),

        _ => None,
    };

    if let Some(mut seq) = base_seq {
        if modifiers.alt_key() {
            let mut escaped = vec![27];
            escaped.append(&mut seq);
            Some(escaped)
        } else {
            Some(seq)
        }
    } else {
        None
    }
}
