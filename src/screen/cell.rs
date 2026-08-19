use bitflags::bitflags;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct CellFlags: u16 {
        const BOLD = 1 << 0;
        const ITALIC = 1 << 1;
        const UNDERLINE = 1 << 2;
        const BLINK = 1 << 3;
        const REVERSE = 1 << 4;
        const HIDDEN = 1 << 5;
        const STRIKE = 1 << 6;
        const WIDE = 1 << 7;
        const WIDE_CONTINUATION = 1 << 8;
        const DIM = 1 << 9;
        const DOUBLE_UNDERLINE = 1 << 10;
        const CURLY_UNDERLINE = 1 << 11;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub character: char,
    pub foreground: Color,
    pub background: Color,
    pub flags: CellFlags,
}
