use crate::screen::cell::{Cell, CellFlags, Color};
use crate::theme::theme::Theme;

/// Fast packed ARGB/XRGB color: `0x00RRGGBB`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedColor(pub u32);

impl PackedColor {
    #[inline(always)]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    #[inline(always)]
    pub const fn from_color(c: Color) -> Self {
        Self::from_rgb(c.r, c.g, c.b)
    }

    #[inline(always)]
    pub const fn to_u32(self) -> u32 {
        self.0
    }

    /// Fast integer dim attenuation (~60% intensity: `val * 153 >> 8`)
    #[inline(always)]
    pub const fn dim(self) -> Self {
        let r = (((self.0 >> 16) & 0xFF) * 153) >> 8;
        let g = (((self.0 >> 8) & 0xFF) * 153) >> 8;
        let b = ((self.0 & 0xFF) * 153) >> 8;
        Self((r << 16) | (g << 8) | b)
    }
}

/// Precomputed lookup tables for terminal theme colors to avoid runtime conversions.
#[derive(Debug, Clone)]
pub struct PrecomputedPalette {
    pub ansi_colors: [u32; 16],
    pub ansi_colors_raw: [Color; 16],
    pub default_fg: u32,
    pub default_bg: u32,
}

impl PrecomputedPalette {
    pub fn new(theme: &Theme) -> Self {
        let mut ansi_colors = [0u32; 16];
        let mut ansi_colors_raw = [Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }; 16];
        for i in 0..16 {
            ansi_colors[i] = PackedColor::from_color(theme.ansi_colors[i]).to_u32();
            ansi_colors_raw[i] = theme.ansi_colors[i];
        }
        Self {
            ansi_colors,
            ansi_colors_raw,
            default_fg: PackedColor::from_color(theme.default_fg).to_u32(),
            default_bg: PackedColor::from_color(theme.default_bg).to_u32(),
        }
    }

    /// Resolve the effective foreground and background `u32` pixel colors for a cell.
    #[inline(always)]
    pub fn resolve_cell_colors(
        &self,
        cell: &Cell,
        is_inverted: bool,
        bold_is_bright: bool,
    ) -> (u32, u32) {
        let cell_fg_color = cell.foreground;
        let mut cell_fg = PackedColor::from_color(cell_fg_color).to_u32();

        // Bold-bright: remap base 8 ANSI colors to bright variants (8..15)
        if bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
            for i in 0..8 {
                if cell_fg_color == self.ansi_colors_raw[i] {
                    cell_fg = self.ansi_colors[i + 8];
                    break;
                }
            }
        }

        let cell_bg = PackedColor::from_color(cell.background).to_u32();

        let (mut fg, bg) = if is_inverted {
            (cell_bg, cell_fg)
        } else {
            (cell_fg, cell_bg)
        };

        if cell.flags.contains(CellFlags::DIM) {
            fg = PackedColor(fg).dim().to_u32();
        }

        (fg, bg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packed_color_conversion() {
        let color = Color {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0xFF,
        };
        let packed = PackedColor::from_color(color);
        assert_eq!(packed.to_u32(), 0x00123456);
    }

    #[test]
    fn test_packed_color_dim() {
        let packed = PackedColor::from_rgb(200, 100, 50);
        let dimmed = packed.dim();
        assert_eq!(dimmed.to_u32(), (119 << 16) | (59 << 8) | 29);
    }
}
