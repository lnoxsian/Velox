use smallvec::{SmallVec, smallvec};
use winit::keyboard::{Key, ModifiersState, NamedKey};

pub fn translate_key(
    key: &Key,
    modifiers: ModifiersState,
    cursor_keys_mode: bool,
) -> Option<SmallVec<[u8; 16]>> {
    // 1. Control key combinations
    if modifiers.control_key() {
        match key {
            Key::Character(s) if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    let code = c.to_ascii_uppercase() as u8 - b'A' + 1;
                    return Some(smallvec![code]);
                }
                match c {
                    '@' => return Some(smallvec![0]),
                    '[' => return Some(smallvec![27]),
                    '\\' => return Some(smallvec![28]),
                    ']' => return Some(smallvec![29]),
                    '^' => return Some(smallvec![30]),
                    '_' => return Some(smallvec![31]),
                    '?' => return Some(smallvec![127]),
                    _ => {}
                }
            }
            Key::Named(NamedKey::Space) => return Some(smallvec![0]),
            _ => {}
        }
    }

    // 2. Base mapping (and Alt combinations)
    let base_seq = match key {
        Key::Character(s) => Some(SmallVec::from_slice(s.as_bytes())),
        Key::Named(NamedKey::Enter) => Some(smallvec![13]),
        Key::Named(NamedKey::Backspace) => Some(smallvec![127]),
        Key::Named(NamedKey::Tab) => Some(smallvec![9]),
        Key::Named(NamedKey::Escape) => Some(smallvec![27]),
        Key::Named(NamedKey::Space) => Some(smallvec![32]),

        // Arrow Keys
        Key::Named(NamedKey::ArrowUp) => Some(if cursor_keys_mode {
            SmallVec::from_slice(b"\x1bOA")
        } else {
            SmallVec::from_slice(b"\x1b[A")
        }),
        Key::Named(NamedKey::ArrowDown) => Some(if cursor_keys_mode {
            SmallVec::from_slice(b"\x1bOB")
        } else {
            SmallVec::from_slice(b"\x1b[B")
        }),
        Key::Named(NamedKey::ArrowRight) => Some(if cursor_keys_mode {
            SmallVec::from_slice(b"\x1bOC")
        } else {
            SmallVec::from_slice(b"\x1b[C")
        }),
        Key::Named(NamedKey::ArrowLeft) => Some(if cursor_keys_mode {
            SmallVec::from_slice(b"\x1bOD")
        } else {
            SmallVec::from_slice(b"\x1b[D")
        }),

        // Navigation Keys
        Key::Named(NamedKey::Home) => Some(SmallVec::from_slice(b"\x1b[H")),
        Key::Named(NamedKey::End) => Some(SmallVec::from_slice(b"\x1b[F")),
        Key::Named(NamedKey::PageUp) => Some(SmallVec::from_slice(b"\x1b[5~")),
        Key::Named(NamedKey::PageDown) => Some(SmallVec::from_slice(b"\x1b[6~")),
        Key::Named(NamedKey::Delete) => Some(SmallVec::from_slice(b"\x1b[3~")),
        Key::Named(NamedKey::Insert) => Some(SmallVec::from_slice(b"\x1b[2~")),

        // Function Keys
        Key::Named(NamedKey::F1) => Some(SmallVec::from_slice(b"\x1bOP")),
        Key::Named(NamedKey::F2) => Some(SmallVec::from_slice(b"\x1bOQ")),
        Key::Named(NamedKey::F3) => Some(SmallVec::from_slice(b"\x1bOR")),
        Key::Named(NamedKey::F4) => Some(SmallVec::from_slice(b"\x1bOS")),
        Key::Named(NamedKey::F5) => Some(SmallVec::from_slice(b"\x1b[15~")),
        Key::Named(NamedKey::F6) => Some(SmallVec::from_slice(b"\x1b[17~")),
        Key::Named(NamedKey::F7) => Some(SmallVec::from_slice(b"\x1b[18~")),
        Key::Named(NamedKey::F8) => Some(SmallVec::from_slice(b"\x1b[19~")),
        Key::Named(NamedKey::F9) => Some(SmallVec::from_slice(b"\x1b[20~")),
        Key::Named(NamedKey::F10) => Some(SmallVec::from_slice(b"\x1b[21~")),
        Key::Named(NamedKey::F11) => Some(SmallVec::from_slice(b"\x1b[23~")),
        Key::Named(NamedKey::F12) => Some(SmallVec::from_slice(b"\x1b[24~")),

        _ => None,
    };

    if let Some(seq) = base_seq {
        if modifiers.alt_key() {
            let mut escaped: SmallVec<[u8; 16]> = smallvec![27u8];
            escaped.extend_from_slice(&seq);
            Some(escaped)
        } else {
            Some(seq)
        }
    } else {
        None
    }
}
