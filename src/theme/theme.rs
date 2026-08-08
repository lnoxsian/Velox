use crate::screen::cell::Color;

pub struct Theme {
    pub default_fg: Color,
    pub default_bg: Color,
    pub ansi_colors: [Color; 16],
}

impl Theme {
    pub fn new() -> Self {
        Self {
            default_fg: Color { r: 248, g: 248, b: 242, a: 255 },
            default_bg: Color { r: 39, g: 40, b: 34, a: 255 },
            ansi_colors: [
                Color { r: 39, g: 40, b: 34, a: 255 },     // Black
                Color { r: 249, g: 38, b: 114, a: 255 },   // Red
                Color { r: 166, g: 226, b: 46, a: 255 },   // Green
                Color { r: 244, g: 191, b: 117, a: 255 },  // Yellow
                Color { r: 102, g: 217, b: 239, a: 255 },  // Blue
                Color { r: 174, g: 129, b: 255, a: 255 },  // Magenta
                Color { r: 161, g: 239, b: 228, a: 255 },  // Cyan
                Color { r: 248, g: 248, b: 242, a: 255 },  // White
                // Brights
                Color { r: 117, g: 113, b: 94, a: 255 },   // Bright Black
                Color { r: 249, g: 38, b: 114, a: 255 },   // Bright Red
                Color { r: 166, g: 226, b: 46, a: 255 },   // Bright Green
                Color { r: 244, g: 191, b: 117, a: 255 },  // Bright Yellow
                Color { r: 102, g: 217, b: 239, a: 255 },  // Bright Blue
                Color { r: 174, g: 129, b: 255, a: 255 },  // Bright Magenta
                Color { r: 161, g: 239, b: 228, a: 255 },  // Bright Cyan
                Color { r: 248, g: 248, b: 240, a: 255 },  // Bright White
            ]
        }
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
            Color { r, g, b, a: 255 }
        } else {
            let gray = 8 + (idx - 232) * 10;
            Color { r: gray, g: gray, b: gray, a: 255 }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}
