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
    pub scrollback_limit: Option<usize>,
    #[serde(default)]
    pub gpu_acceleration: Option<bool>,
    #[serde(default)]
    pub scroll_multiplier: Option<f64>,
    #[serde(default)]
    pub fps_limit: Option<u32>,
    #[serde(default)]
    pub bold_is_bright: Option<bool>,
    #[serde(default)]
    pub app_title: Option<String>,
    #[serde(default)]
    pub padding_x: Option<f32>,
    #[serde(default)]
    pub padding_y: Option<f32>,
    #[serde(default)]
    pub font_scale_multiplier: Option<f32>,
    #[serde(default)]
    pub cursor_shape: Option<String>,
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
            bold_is_bright = true
            app_title = "{program} - Velox"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.scrollback_limit, Some(30000));
        assert_eq!(config.bold_is_bright, Some(true));
        assert_eq!(config.app_title, Some("{program} - Velox".to_string()));

        let toml_str_no_limit = r#"
            font_family = "monospace"
            font_size = 14.0
            shell = "/bin/sh"
        "#;
        let config_no_limit: Config = toml::from_str(toml_str_no_limit).unwrap();
        assert_eq!(config_no_limit.scrollback_limit, None);
        assert_eq!(config_no_limit.gpu_acceleration, None);
        assert_eq!(config_no_limit.scroll_multiplier, None);
        assert_eq!(config_no_limit.fps_limit, None);
        assert_eq!(config_no_limit.bold_is_bright, None);
        assert_eq!(config_no_limit.app_title, None);

        let toml_str_gpu = r#"
            font_family = "monospace"
            font_size = 14.0
            shell = "/bin/sh"
            gpu_acceleration = false
            scroll_multiplier = 2.5
            fps_limit = 60
            bold_is_bright = false
            app_title = "Velox Terminal"
        "#;
        let config_gpu: Config = toml::from_str(toml_str_gpu).unwrap();
        assert_eq!(config_gpu.gpu_acceleration, Some(false));
        assert_eq!(config_gpu.scroll_multiplier, Some(2.5));
        assert_eq!(config_gpu.fps_limit, Some(60));
        assert_eq!(config_gpu.bold_is_bright, Some(false));
        assert_eq!(config_gpu.app_title, Some("Velox Terminal".to_string()));
    }
}
