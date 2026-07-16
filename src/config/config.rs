use serde::{Deserialize, Serialize};
use crate::screen::cell::Color;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub font_family: String,
    pub font_size: f32,
    pub shell: String,
    #[serde(default)]
    pub default_fg: Option<String>,
    #[serde(default)]
    pub default_bg: Option<String>,
    #[serde(default)]
    pub ansi_colors: Option<Vec<String>>,
}

pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color { r, g, b, a: 255 })
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some(Color { r, g, b, a })
    } else {
        None
    }
}
