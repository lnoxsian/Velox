use crate::screen::cell::Color;

pub struct Theme {
    pub default_fg: Color,
    pub default_bg: Color,
    pub ansi_colors: [Color; 16],
    pub initial_fg: Color,
    pub initial_bg: Color,
    pub initial_ansi_colors: [Color; 16],
    pub cursor_color: Option<Color>,
}

impl Theme {
    pub fn new() -> Self {
        let default_fg = Color {
            r: 248,
            g: 248,
            b: 242,
        };
        let default_bg = Color {
            r: 39,
            g: 40,
            b: 34,
        };
        let ansi_colors = [
            Color {
                r: 39,
                g: 40,
                b: 34,
            }, // Black
            Color {
                r: 249,
                g: 38,
                b: 114,
            }, // Red
            Color {
                r: 166,
                g: 226,
                b: 46,
            }, // Green
            Color {
                r: 244,
                g: 191,
                b: 117,
            }, // Yellow
            Color {
                r: 102,
                g: 217,
                b: 239,
            }, // Blue
            Color {
                r: 174,
                g: 129,
                b: 255,
            }, // Magenta
            Color {
                r: 161,
                g: 239,
                b: 228,
            }, // Cyan
            Color {
                r: 248,
                g: 248,
                b: 242,
            }, // White
            // Brights
            Color {
                r: 117,
                g: 113,
                b: 94,
            }, // Bright Black
            Color {
                r: 249,
                g: 38,
                b: 114,
            }, // Bright Red
            Color {
                r: 166,
                g: 226,
                b: 46,
            }, // Bright Green
            Color {
                r: 244,
                g: 191,
                b: 117,
            }, // Bright Yellow
            Color {
                r: 102,
                g: 217,
                b: 239,
            }, // Bright Blue
            Color {
                r: 174,
                g: 129,
                b: 255,
            }, // Bright Magenta
            Color {
                r: 161,
                g: 239,
                b: 228,
            }, // Bright Cyan
            Color {
                r: 248,
                g: 248,
                b: 240,
            }, // Bright White
        ];
        Self {
            default_fg,
            default_bg,
            ansi_colors,
            initial_fg: default_fg,
            initial_bg: default_bg,
            initial_ansi_colors: ansi_colors,
            cursor_color: None,
        }
    }

    pub fn save_initial_colors(&mut self) {
        self.initial_fg = self.default_fg;
        self.initial_bg = self.default_bg;
        self.initial_ansi_colors = self.ansi_colors;
    }

    pub fn get_ansi_color(&self, idx: u16, _is_bg: bool) -> Color {
        self.ansi_colors[(idx as usize) % 16]
    }

    pub fn get_256_color(&self, idx: u8) -> Color {
        if idx < 16 {
            self.ansi_colors[idx as usize]
        } else if idx < 232 {
            let val = idx - 16;
            let r = (val / 36) * 51;
            let g = ((val % 36) / 6) * 51;
            let b = (val % 6) * 51;
            Color { r, g, b }
        } else {
            let gray = 8 + (idx - 232) * 10;
            Color {
                r: gray,
                g: gray,
                b: gray,
            }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}
