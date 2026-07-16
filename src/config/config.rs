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
    pub colors: Option<ConfigColors>,
    #[serde(default)]
    pub enable_nerdfont: Option<bool>,
    #[serde(default)]
    pub scrollback_limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigColors {
    pub black: Option<String>,
    pub red: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub cyan: Option<String>,
    pub white: Option<String>,
    pub bright_black: Option<String>,
    pub bright_red: Option<String>,
    pub bright_green: Option<String>,
    pub bright_yellow: Option<String>,
    pub bright_blue: Option<String>,
    pub bright_magenta: Option<String>,
    pub bright_cyan: Option<String>,
    pub bright_white: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_scrollback_limit() {
        let toml_str = r#"
            font_family = "monospace"
            font_size = 14.0
            shell = "/bin/sh"
            scrollback_limit = 30000
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.scrollback_limit, Some(30000));

        let toml_str_no_limit = r#"
            font_family = "monospace"
            font_size = 14.0
            shell = "/bin/sh"
        "#;
        let config_no_limit: Config = toml::from_str(toml_str_no_limit).unwrap();
        assert_eq!(config_no_limit.scrollback_limit, None);
    }
}
