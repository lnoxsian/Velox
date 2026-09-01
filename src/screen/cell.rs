use bitflags::bitflags;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    #[inline(always)]
    pub fn dim(&self, amount: f32) -> Self {
        if amount <= 0.0 {
            *self
        } else if amount >= 1.0 {
            Self { r: 0, g: 0, b: 0 }
        } else {
            let mult = ((1.0 - amount) * 256.0).round() as u32;
            Self {
                r: ((self.r as u32 * mult) >> 8).min(255) as u8,
                g: ((self.g as u32 * mult) >> 8).min(255) as u8,
                b: ((self.b as u32 * mult) >> 8).min(255) as u8,
            }
        }
    }
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
    pub underline_color: Option<Color>,
    pub flags: CellFlags,
}

impl Cell {
    #[inline(always)]
    pub fn new(character: char, foreground: Color, background: Color, flags: CellFlags) -> Self {
        Self {
            character,
            foreground,
            background,
            underline_color: None,
            flags,
        }
    }
}

impl Default for Cell {
    #[inline(always)]
    fn default() -> Self {
        Self {
            character: ' ',
            foreground: Color {
                r: 255,
                g: 255,
                b: 255,
            },
            background: Color { r: 0, g: 0, b: 0 },
            underline_color: None,
            flags: CellFlags::empty(),
        }
    }
}

const _: () = assert!(std::mem::size_of::<Cell>() == 16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_dim() {
        let white = Color {
            r: 200,
            g: 100,
            b: 50,
        };

        // 0.0 dim -> unchanged
        assert_eq!(white.dim(0.0), white);
        assert_eq!(white.dim(-0.5), white);

        // 0.5 dim -> halved
        let half = white.dim(0.5);
        assert_eq!(half.r, 100);
        assert_eq!(half.g, 50);
        assert_eq!(half.b, 25);

        // 1.0 dim -> 0
        let full = white.dim(1.0);
        assert_eq!(full, Color { r: 0, g: 0, b: 0 });

        // > 1.0 dim -> clamped to 0
        let over = white.dim(1.5);
        assert_eq!(over, Color { r: 0, g: 0, b: 0 });
    }
}
