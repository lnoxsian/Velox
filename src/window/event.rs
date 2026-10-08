use bitflags::bitflags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Other(u16),
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct ModifiersState: u8 {
        const SHIFT = 1 << 0;
        const CONTROL = 1 << 1;
        const ALT = 1 << 2;
        const SUPER = 1 << 3;
    }
}

impl ModifiersState {
    #[inline(always)]
    pub fn shift_key(&self) -> bool {
        self.contains(Self::SHIFT)
    }

    #[inline(always)]
    pub fn control_key(&self) -> bool {
        self.contains(Self::CONTROL)
    }

    #[inline(always)]
    pub fn alt_key(&self) -> bool {
        self.contains(Self::ALT)
    }

    #[inline(always)]
    pub fn super_key(&self) -> bool {
        self.contains(Self::SUPER)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorIcon {
    Default,
    Pointer,
    Text,
    ColResize,
    RowResize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Character(String),
    Named(NamedKey),
    Dead(Option<char>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    Enter,
    Tab,
    Space,
    Backspace,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
    Delete,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

#[derive(Debug, Clone)]
pub enum PlatformEvent {
    Resized {
        width: u32,
        height: u32,
    },
    RedrawRequested,
    KeyPressed {
        key: Key,
        text: Option<String>,
        modifiers: ModifiersState,
    },
    KeyReleased {
        key: Key,
        modifiers: ModifiersState,
    },
    CursorMoved {
        x: f64,
        y: f64,
    },
    MouseInput {
        state: ElementState,
        button: MouseButton,
    },
    MouseWheel {
        delta_y: f32,
    },
    Focused(bool),
    ModifiersChanged(ModifiersState),
    CloseRequested,
}
