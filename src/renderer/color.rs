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
    pub raw_default_fg: Color,
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
            raw_default_fg: theme.default_fg,
            raw_default_bg: theme.default_bg,
            tab_accent: PackedColor::from_color(theme.resolve_tab_accent_color()).to_u32(),
            tab_bar_bg: PackedColor::from_premultiplied(tab_bar_bg_raw, alpha).to_u32(),
            tab_inactive_bg: PackedColor::from_premultiplied(tab_inactive_bg_raw, alpha).to_u32(),
            tab_hover_bg: PackedColor::from_premultiplied(tab_hover_bg_raw, alpha).to_u32(),
            tab_inactive_fg: PackedColor::from_color(tab_inactive_fg_raw).to_u32(),
            tab_close_fg: PackedColor::from_color(tab_close_fg_raw).to_u32(),
        }
    }

    /// Resolve the effective foreground and background `u32` pixel colors for a cell using the pane's theme and default background.
    #[inline(always)]
    pub fn resolve_cell_colors_pane(
        &self,
        cell: &Cell,
        is_inverted: bool,
        bold_is_bright: bool,
        dim: f32,
        pane_theme: &Theme,
        default_pane_bg: u32,
    ) -> (u32, u32) {
        let cell_fg_color = cell.foreground;
        let mut cell_fg_raw = cell_fg_color;

        // Bold-bright: remap base 8 ANSI colors to bright variants (8..15)
        if bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
            for i in 0..8 {
                if cell_fg_color == pane_theme.ansi_colors[i] {
                    cell_fg_raw = pane_theme.ansi_colors[i + 8];
                    break;
                }
            }
        }

        let cell_bg_raw = cell.background;

        let (mut fg, bg) = if is_inverted {
            let mut inv_fg_raw = cell_bg_raw;
            let mut inv_bg_raw = cell_fg_raw;

            let lum_fg = 0.299 * inv_fg_raw.r as f32
                + 0.587 * inv_fg_raw.g as f32
                + 0.114 * inv_fg_raw.b as f32;
            let lum_bg = 0.299 * inv_bg_raw.r as f32
                + 0.587 * inv_bg_raw.g as f32
                + 0.114 * inv_bg_raw.b as f32;

            let lum_theme_bg = 0.299 * pane_theme.default_bg.r as f32
                + 0.587 * pane_theme.default_bg.g as f32
                + 0.114 * pane_theme.default_bg.b as f32;

            // When both colors are dark, both are light, or contrast is too low
            if (lum_fg < 128.0 && lum_bg < 128.0)
                || (lum_fg >= 128.0 && lum_bg >= 128.0)
                || (lum_fg - lum_bg).abs() < 30.0
            {
                if lum_theme_bg < 128.0 {
                    // Dark theme: invert to light background
                    inv_bg_raw = pane_theme.default_fg;
                    let lum_orig_fg = 0.299 * cell_fg_raw.r as f32
                        + 0.587 * cell_fg_raw.g as f32
                        + 0.114 * cell_fg_raw.b as f32;
                    inv_fg_raw = if lum_orig_fg < 120.0 {
                        cell_fg_raw
                    } else {
                        pane_theme.default_bg
                    };
                } else {
                    // Light theme: invert to dark background
                    inv_bg_raw = pane_theme.default_fg;
                    let lum_orig_fg = 0.299 * cell_fg_raw.r as f32
                        + 0.587 * cell_fg_raw.g as f32
                        + 0.114 * cell_fg_raw.b as f32;
                    inv_fg_raw = if lum_orig_fg >= 135.0 {
                        cell_fg_raw
                    } else {
                        pane_theme.default_bg
                    };
                }
            }

            // Both foreground text and selection background must be solid, full-alpha colors
            let fg_packed = PackedColor::from_color(inv_fg_raw.dim(dim)).to_u32();
            let bg_packed = PackedColor::from_color(inv_bg_raw).to_u32();
            (fg_packed, bg_packed)
        } else {
            let fg_packed = PackedColor::from_color(cell_fg_raw.dim(dim)).to_u32();
            let bg_packed = if cell_bg_raw == pane_theme.default_bg {
                default_pane_bg
            } else {
                PackedColor::from_color(cell_bg_raw).to_u32()
            };
            (fg_packed, bg_packed)
        };

        if cell.flags.contains(CellFlags::DIM) {
            fg = PackedColor(fg).dim().to_u32();
        }

        (fg, bg)
    }

    /// Resolve the effective foreground and background `u32` pixel colors for a cell.
    #[inline(always)]
    pub fn resolve_cell_colors(
        &self,
        cell: &Cell,
        is_inverted: bool,
        bold_is_bright: bool,
        dim: f32,
    ) -> (u32, u32) {
        let theme = Theme {
            default_fg: self.raw_default_fg,
            default_bg: self.raw_default_bg,
            ansi_colors: self.ansi_colors_raw,
            ..Theme::new()
        };
        self.resolve_cell_colors_pane(
            cell,
            is_inverted,
            bold_is_bright,
            dim,
            &theme,
            self.default_bg,
        )
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

    #[test]
    fn test_dark_theme_and_light_theme_inversion() {
        let theme_dark = Theme {
            default_fg: Color {
                r: 205,
                g: 214,
                b: 244,
            }, // Light
            default_bg: Color {
                r: 30,
                g: 30,
                b: 46,
            }, // Dark
            ..Theme::new()
        };
        let palette_dark = PrecomputedPalette::new(&theme_dark, 1.0);

        // Dark theme: fish pager selection with #060606 foreground and default background
        let cell = Cell::new(
            'a',
            Color { r: 6, g: 6, b: 6 },
            theme_dark.default_bg,
            CellFlags::REVERSE,
        );
        let (fg, bg) = palette_dark.resolve_cell_colors(&cell, true, false, 0.0);
        // bg must be inverted to light default_fg
        assert_eq!(bg, PackedColor::from_color(theme_dark.default_fg).to_u32());
        // fg must be dark and have high contrast
        assert_eq!(
            fg,
            PackedColor::from_color(Color { r: 6, g: 6, b: 6 }).to_u32()
        );

        // Light theme: dark text on white background
        let theme_light = Theme {
            default_fg: Color { r: 0, g: 0, b: 0 },
            default_bg: Color {
                r: 255,
                g: 255,
                b: 255,
            },
            ..Theme::new()
        };
        let palette_light = PrecomputedPalette::new(&theme_light, 1.0);
        let cell_light = Cell::new(
            'a',
            Color { r: 6, g: 6, b: 6 },
            theme_light.default_bg,
            CellFlags::REVERSE,
        );
        let (fg_l, bg_l) = palette_light.resolve_cell_colors(&cell_light, true, false, 0.0);
        assert_eq!(
            bg_l,
            PackedColor::from_color(Color { r: 6, g: 6, b: 6 }).to_u32()
        );
        assert_eq!(
            fg_l,
            PackedColor::from_color(theme_light.default_bg).to_u32()
        );
    }

    #[test]
    fn test_resolve_cell_colors_bold_bright_with_dim() {
        let base_theme = Theme::new();
        // Emulate an unfocused window with a dimmed palette
        let dimmed_theme = base_theme.dimmed(0.15);
        let palette = PrecomputedPalette::new(&dimmed_theme, 1.0);

        let cell = Cell {
            character: 'B',
            foreground: base_theme.ansi_colors[1], // Red
            background: base_theme.default_bg,
            underline_color: None,
            flags: CellFlags::BOLD,
        };

        // Bold-is-bright enabled with 15% dimming
        let (fg, bg) = palette.resolve_cell_colors_pane(
            &cell,
            false,
            true,
            0.15,
            &base_theme,
            palette.default_bg,
        );

        let expected_bright_red = base_theme.ansi_colors[9].dim(0.15);
        assert_eq!(fg, PackedColor::from_color(expected_bright_red).to_u32());
        assert_eq!(bg, palette.default_bg);
    }
}
