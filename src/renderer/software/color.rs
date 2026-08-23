use crate::screen::cell::{Cell, CellFlags, Color};
use crate::theme::theme::Theme;

/// Fast packed ARGB/XRGB color: `0xAARRGGBB`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedColor(pub u32);

impl PackedColor {
    #[inline(always)]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self((0xFF << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub const fn from_argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    #[inline(always)]
    pub const fn from_color(c: Color) -> Self {
        Self::from_rgb(c.r, c.g, c.b)
    }

    #[inline(always)]
    pub const fn from_premultiplied(c: Color, alpha: u8) -> Self {
        let a = alpha as u32;
        let r = ((c.r as u32) * a) / 255;
        let g = ((c.g as u32) * a) / 255;
        let b = ((c.b as u32) * a) / 255;
        Self((a << 24) | (r << 16) | (g << 8) | b)
    }

    #[inline(always)]
    pub const fn to_u32(self) -> u32 {
        self.0
    }

    /// Fast integer dim attenuation (~60% intensity: `val * 153 >> 8`)
    #[inline(always)]
    pub const fn dim(self) -> Self {
        let a = self.0 & 0xFF000000;
        let r = (((self.0 >> 16) & 0xFF) * 153) >> 8;
        let g = (((self.0 >> 8) & 0xFF) * 153) >> 8;
        let b = ((self.0 & 0xFF) * 153) >> 8;
        Self(a | (r << 16) | (g << 8) | b)
    }
}

/// Precomputed lookup tables for terminal theme colors to avoid runtime conversions.
#[derive(Debug, Clone)]
pub struct PrecomputedPalette {
    pub ansi_colors: [u32; 16],
    pub ansi_colors_raw: [Color; 16],
    pub default_fg: u32,
    pub default_bg: u32,
    pub raw_default_bg: Color,
    pub tab_accent: u32,
    pub tab_bar_bg: u32,
    pub tab_inactive_bg: u32,
    pub tab_hover_bg: u32,
    pub tab_inactive_fg: u32,
    pub tab_close_fg: u32,
}

impl PrecomputedPalette {
    pub fn new(theme: &Theme, opacity: f32) -> Self {
        let opacity = opacity.clamp(0.0, 1.0);
        let alpha = (opacity * 255.0).round() as u8;
        let mut ansi_colors = [0u32; 16];
        let mut ansi_colors_raw = [Color { r: 0, g: 0, b: 0 }; 16];
        for i in 0..16 {
            ansi_colors[i] = PackedColor::from_color(theme.ansi_colors[i]).to_u32();
            ansi_colors_raw[i] = theme.ansi_colors[i];
        }

        let tab_bar_bg_raw = Color {
            r: (theme.default_bg.r as f32 * 0.6) as u8,
            g: (theme.default_bg.g as f32 * 0.6) as u8,
            b: (theme.default_bg.b as f32 * 0.6) as u8,
        };
        let tab_inactive_bg_raw = Color {
            r: (theme.default_bg.r as f32 * 0.72) as u8,
            g: (theme.default_bg.g as f32 * 0.72) as u8,
            b: (theme.default_bg.b as f32 * 0.72) as u8,
        };
        let tab_hover_bg_raw = Color {
            r: (theme.default_bg.r as f32 * 0.85) as u8,
            g: (theme.default_bg.g as f32 * 0.85) as u8,
            b: (theme.default_bg.b as f32 * 0.85) as u8,
        };
        let tab_inactive_fg_raw = Color {
            r: (theme.default_fg.r as f32 * 0.7) as u8,
            g: (theme.default_fg.g as f32 * 0.7) as u8,
            b: (theme.default_fg.b as f32 * 0.7) as u8,
        };
        let tab_close_fg_raw = Color {
            r: (theme.default_fg.r as f32 * 0.6) as u8,
            g: (theme.default_fg.g as f32 * 0.6) as u8,
            b: (theme.default_fg.b as f32 * 0.6) as u8,
        };

        Self {
            ansi_colors,
            ansi_colors_raw,
            default_fg: PackedColor::from_color(theme.default_fg).to_u32(),
            default_bg: PackedColor::from_premultiplied(theme.default_bg, alpha).to_u32(),
            raw_default_bg: theme.default_bg,
            tab_accent: PackedColor::from_color(theme.resolve_tab_accent_color()).to_u32(),
            tab_bar_bg: PackedColor::from_premultiplied(tab_bar_bg_raw, alpha).to_u32(),
            tab_inactive_bg: PackedColor::from_premultiplied(tab_inactive_bg_raw, alpha).to_u32(),
            tab_hover_bg: PackedColor::from_premultiplied(tab_hover_bg_raw, alpha).to_u32(),
            tab_inactive_fg: PackedColor::from_color(tab_inactive_fg_raw).to_u32(),
            tab_close_fg: PackedColor::from_color(tab_close_fg_raw).to_u32(),
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

        let cell_bg = if cell.background == self.raw_default_bg {
            self.default_bg
        } else {
            PackedColor::from_color(cell.background).to_u32()
        };

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
        };
        let packed = PackedColor::from_color(color);
        assert_eq!(packed.to_u32(), 0xFF123456);

        let premult = PackedColor::from_premultiplied(
            Color {
                r: 200,
                g: 100,
                b: 50,
            },
            128,
        );
        assert_eq!((premult.to_u32() >> 24) & 0xFF, 128);
        assert_eq!((premult.to_u32() >> 16) & 0xFF, (200 * 128) / 255);
    }

    #[test]
    fn test_packed_color_dim() {
        let packed = PackedColor::from_rgb(200, 100, 50);
        let dimmed = packed.dim();
        assert_eq!(dimmed.to_u32(), 0xFF000000 | (119 << 16) | (59 << 8) | 29);
    }
}
