use crate::screen::cell::Color;
use serde::{Deserialize, Serialize};

fn default_font_family() -> String {
    "monospace".to_string()
}

fn default_font_size() -> f32 {
    14.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FontConfig {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default)]
    pub font_scale_multiplier: Option<f32>,
    #[serde(default)]
    pub bold_is_bright: Option<bool>,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            font_family: default_font_family(),
            font_size: default_font_size(),
            font_scale_multiplier: Some(1.5),
            bold_is_bright: Some(true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WindowConfig {
    #[serde(default)]
    pub scrollback_limit: Option<usize>,
    #[serde(default)]
    pub infinite_scrollback: Option<bool>,
    #[serde(default)]
    pub gpu_acceleration: Option<bool>,
    #[serde(default)]
    pub scroll_multiplier: Option<f64>,
    #[serde(default)]
    pub fps_limit: Option<u32>,
    #[serde(default)]
    pub padding_x: Option<f32>,
    #[serde(default)]
    pub padding_y: Option<f32>,
    #[serde(default)]
    pub cursor_shape: Option<String>,
    #[serde(default)]
    pub cursor_blink: Option<bool>,
    #[serde(default, alias = "cursor")]
    pub cursor_color: Option<String>,
    #[serde(
        default,
        alias = "cursor_text",
        alias = "cursor_fg",
        alias = "cursor_text_fg"
    )]
    pub cursor_text_color: Option<String>,
    #[serde(default, alias = "scroll_to_bottom_on_output")]
    pub scroll_on_output: Option<bool>,
    #[serde(
        default,
        alias = "scroll_on_keystrock",
        alias = "scroll_to_bottom_on_keystroke",
        alias = "scroll_on_key",
        alias = "scroll_to_bottom_on_input"
    )]
    pub scroll_on_keystroke: Option<bool>,
    #[serde(
        default,
        alias = "background_opacity",
        alias = "alpha",
        alias = "window_opacity"
    )]
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ConfigColors {
    #[serde(default)]
    pub default_fg: Option<String>,
    #[serde(default)]
    pub default_bg: Option<String>,
    #[serde(default)]
    pub black: Option<String>,
    #[serde(default)]
    pub red: Option<String>,
    #[serde(default)]
    pub green: Option<String>,
    #[serde(default)]
    pub yellow: Option<String>,
    #[serde(default)]
    pub blue: Option<String>,
    #[serde(default)]
    pub magenta: Option<String>,
    #[serde(default)]
    pub cyan: Option<String>,
    #[serde(default)]
    pub white: Option<String>,
    #[serde(default)]
    pub bright_black: Option<String>,
    #[serde(default)]
    pub bright_red: Option<String>,
    #[serde(default)]
    pub bright_green: Option<String>,
    #[serde(default)]
    pub bright_yellow: Option<String>,
    #[serde(default)]
    pub bright_blue: Option<String>,
    #[serde(default)]
    pub bright_magenta: Option<String>,
    #[serde(default)]
    pub bright_cyan: Option<String>,
    #[serde(default)]
    pub bright_white: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub font: FontConfig,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub colors: Option<ConfigColors>,

    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub app_title: Option<String>,
    #[serde(default)]
    pub single_instance: Option<bool>,

    // Top-level fallback fields for legacy flat TOML files
    #[serde(
        default,
        rename = "font_family",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) font_family_legacy: Option<String>,
    #[serde(default, rename = "font_size", skip_serializing_if = "Option::is_none")]
    pub(crate) font_size_legacy: Option<f32>,
    #[serde(
        default,
        rename = "font_scale_multiplier",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) font_scale_multiplier_legacy: Option<f32>,
    #[serde(
        default,
        rename = "bold_is_bright",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) bold_is_bright_legacy: Option<bool>,
    #[serde(
        default,
        rename = "scrollback_limit",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) scrollback_limit_legacy: Option<usize>,
    #[serde(
        default,
        rename = "infinite_scrollback",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) infinite_scrollback_legacy: Option<bool>,
    #[serde(
        default,
        rename = "gpu_acceleration",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) gpu_acceleration_legacy: Option<bool>,
    #[serde(
        default,
        rename = "scroll_multiplier",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) scroll_multiplier_legacy: Option<f64>,
    #[serde(default, rename = "fps_limit", skip_serializing_if = "Option::is_none")]
    pub(crate) fps_limit_legacy: Option<u32>,
    #[serde(default, rename = "padding_x", skip_serializing_if = "Option::is_none")]
    pub(crate) padding_x_legacy: Option<f32>,
    #[serde(default, rename = "padding_y", skip_serializing_if = "Option::is_none")]
    pub(crate) padding_y_legacy: Option<f32>,
    #[serde(
        default,
        rename = "cursor_shape",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) cursor_shape_legacy: Option<String>,
    #[serde(
        default,
        rename = "cursor_blink",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) cursor_blink_legacy: Option<bool>,
    #[serde(
        default,
        rename = "cursor_color",
        alias = "cursor",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) cursor_color_legacy: Option<String>,
    #[serde(
        default,
        rename = "cursor_text_color",
        alias = "cursor_text",
        alias = "cursor_fg",
        alias = "cursor_text_fg",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) cursor_text_color_legacy: Option<String>,
    #[serde(
        default,
        rename = "scroll_on_output",
        alias = "scroll_to_bottom_on_output",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) scroll_on_output_legacy: Option<bool>,
    #[serde(
        default,
        rename = "scroll_on_keystroke",
        alias = "scroll_on_keystrock",
        alias = "scroll_to_bottom_on_keystroke",
        alias = "scroll_on_key",
        alias = "scroll_to_bottom_on_input",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) scroll_on_keystroke_legacy: Option<bool>,
    #[serde(
        default,
        rename = "default_fg",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) default_fg_legacy: Option<String>,
    #[serde(
        default,
        rename = "default_bg",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) default_bg_legacy: Option<String>,
    #[serde(
        default,
        rename = "opacity",
        alias = "background_opacity",
        alias = "alpha",
        alias = "window_opacity",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) opacity_legacy: Option<f32>,
}

impl Config {
    pub fn opacity(&self) -> f32 {
        self.window
            .opacity
            .or(self.opacity_legacy)
            .unwrap_or(1.0)
            .clamp(0.0, 1.0)
    }
    pub fn font_family(&self) -> &str {
        if let Some(ref ff) = self.font_family_legacy {
            ff.as_str()
        } else {
            &self.font.font_family
        }
    }

    pub fn font_size(&self) -> f32 {
        self.font_size_legacy.unwrap_or(self.font.font_size)
    }

    pub fn font_scale_multiplier(&self) -> Option<f32> {
        self.font
            .font_scale_multiplier
            .or(self.font_scale_multiplier_legacy)
    }

    pub fn bold_is_bright(&self) -> Option<bool> {
        self.font.bold_is_bright.or(self.bold_is_bright_legacy)
    }

    pub fn scrollback_limit(&self) -> Option<usize> {
        self.window
            .scrollback_limit
            .or(self.scrollback_limit_legacy)
    }

    pub fn infinite_scrollback(&self) -> Option<bool> {
        self.window
            .infinite_scrollback
            .or(self.infinite_scrollback_legacy)
    }

    pub fn gpu_acceleration(&self) -> Option<bool> {
        self.window
            .gpu_acceleration
            .or(self.gpu_acceleration_legacy)
    }

    pub fn scroll_multiplier(&self) -> Option<f64> {
        self.window
            .scroll_multiplier
            .or(self.scroll_multiplier_legacy)
    }

    pub fn fps_limit(&self) -> Option<u32> {
        self.window.fps_limit.or(self.fps_limit_legacy)
    }

    pub fn padding_x(&self) -> Option<f32> {
        self.window.padding_x.or(self.padding_x_legacy)
    }

    pub fn padding_y(&self) -> Option<f32> {
        self.window.padding_y.or(self.padding_y_legacy)
    }

    pub fn cursor_shape(&self) -> Option<&str> {
        self.window
            .cursor_shape
            .as_deref()
            .or(self.cursor_shape_legacy.as_deref())
    }

    pub fn cursor_blink(&self) -> Option<bool> {
        self.window.cursor_blink.or(self.cursor_blink_legacy)
    }

    pub fn cursor_color(&self) -> Option<&str> {
        self.window
            .cursor_color
            .as_deref()
            .or(self.cursor_color_legacy.as_deref())
    }

    pub fn cursor_text_color(&self) -> Option<&str> {
        self.window
            .cursor_text_color
            .as_deref()
            .or(self.cursor_text_color_legacy.as_deref())
    }

    pub fn scroll_on_output(&self) -> Option<bool> {
        self.window
            .scroll_on_output
            .or(self.scroll_on_output_legacy)
    }

    pub fn scroll_on_keystroke(&self) -> Option<bool> {
        self.window
            .scroll_on_keystroke
            .or(self.scroll_on_keystroke_legacy)
    }

    pub fn default_fg(&self) -> Option<&str> {
        self.colors
            .as_ref()
            .and_then(|c| c.default_fg.as_deref())
            .or(self.default_fg_legacy.as_deref())
    }

    pub fn default_bg(&self) -> Option<&str> {
        self.colors
            .as_ref()
            .and_then(|c| c.default_bg.as_deref())
            .or(self.default_bg_legacy.as_deref())
    }
}

pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex).trim();
    if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
        let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
        let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
        Some(Color {
            r: r * 17,
            g: g * 17,
            b: b * 17,
        })
    } else if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color { r, g, b })
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let _a = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some(Color { r, g, b })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_section_parsing() {
        let toml_str = r##"
            [font]
            font_family = "ComicShannsMono Nerd Font"
            font_scale_multiplier = 1.5
            font_size = 11.0
            bold_is_bright = true

            [window]
            scrollback_limit = 2000
            infinite_scrollback = true
            gpu_acceleration = true
            scroll_multiplier = 5.0
            fps_limit = 120
            padding_x = 8.0
            padding_y = 4.0
            cursor_shape = "beam"
            cursor_blink = true
            scroll_on_output = false
            scroll_on_keystroke = true
            opacity = 0.85

            [colors]
            default_fg = "#e0def4"
            default_bg = "#191724"
            black = "#26233a"
            red = "#eb6f92"
            green = "#31748f"
            yellow = "#f6c177"
            blue = "#9ccfd8"
            magenta = "#c4a7e7"
            cyan = "#ebbcba"
            white = "#e0def4"
            bright_black = "#6e6a86"
            bright_red = "#eb6f92"
            bright_green = "#31748f"
            bright_yellow = "#f6c177"
            bright_blue = "#9ccfd8"
            bright_magenta = "#c4a7e7"
            bright_cyan = "#ebbcba"
            bright_white = "#e0def4"
        "##;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font_family(), "ComicShannsMono Nerd Font");
        assert_eq!(config.font_size(), 11.0);
        assert_eq!(config.font_scale_multiplier(), Some(1.5));
        assert_eq!(config.bold_is_bright(), Some(true));

        assert_eq!(config.scrollback_limit(), Some(2000));
        assert_eq!(config.infinite_scrollback(), Some(true));
        assert_eq!(config.gpu_acceleration(), Some(true));
        assert_eq!(config.scroll_multiplier(), Some(5.0));
        assert_eq!(config.fps_limit(), Some(120));
        assert_eq!(config.padding_x(), Some(8.0));
        assert_eq!(config.padding_y(), Some(4.0));
        assert_eq!(config.cursor_shape(), Some("beam"));
        assert_eq!(config.cursor_blink(), Some(true));
        assert_eq!(config.scroll_on_output(), Some(false));
        assert_eq!(config.scroll_on_keystroke(), Some(true));
        assert_eq!(config.opacity(), 0.85);

        assert_eq!(config.default_fg(), Some("#e0def4"));
        assert_eq!(config.default_bg(), Some("#191724"));
        let colors = config.colors.unwrap();
        assert_eq!(colors.black, Some("#26233a".to_string()));
        assert_eq!(colors.red, Some("#eb6f92".to_string()));
    }

    #[test]
    fn test_config_legacy_flat_parsing() {
        let toml_str = r#"
            font_family = "monospace"
            font_size = 14.0
            shell = "/bin/sh"
            scrollback_limit = 30000
            bold_is_bright = true
            app_title = "{program} - Velox"
            scroll_on_output = true
            scroll_on_keystroke = false
            opacity = 0.75
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.scrollback_limit(), Some(30000));
        assert_eq!(config.infinite_scrollback(), None);
        assert_eq!(config.bold_is_bright(), Some(true));
        assert_eq!(config.app_title, Some("{program} - Velox".to_string()));
        assert_eq!(config.scroll_on_output(), Some(true));
        assert_eq!(config.scroll_on_keystroke(), Some(false));
        assert_eq!(config.opacity(), 0.75);
    }

    #[test]
    fn test_config_opacity_parsing_and_clamping() {
        // Default opacity is 1.0
        let empty_cfg: Config = toml::from_str("").unwrap();
        assert_eq!(empty_cfg.opacity(), 1.0);

        // Aliases: background_opacity in [window]
        let toml_alias1 = r#"
            [window]
            background_opacity = 0.6
        "#;
        let cfg1: Config = toml::from_str(toml_alias1).unwrap();
        assert_eq!(cfg1.opacity(), 0.6);

        // Aliases: alpha in flat TOML
        let toml_alias2 = r#"
            alpha = 0.4
        "#;
        let cfg2: Config = toml::from_str(toml_alias2).unwrap();
        assert_eq!(cfg2.opacity(), 0.4);

        // Clamping: negative clamped to 0.0, > 1.0 clamped to 1.0
        let toml_underflow = r#"
            [window]
            opacity = -0.5
        "#;
        let cfg_underflow: Config = toml::from_str(toml_underflow).unwrap();
        assert_eq!(cfg_underflow.opacity(), 0.0);

        let toml_overflow = r#"
            [window]
            opacity = 1.5
        "#;
        let cfg_overflow: Config = toml::from_str(toml_overflow).unwrap();
        assert_eq!(cfg_overflow.opacity(), 1.0);
    }

    #[test]
    fn test_config_cursor_color_parsing() {
        // [window] section
        let toml1 = r##"
            [window]
            cursor_color = "#ff0000"
            cursor_text_color = "#00ff00"
        "##;
        let cfg1: Config = toml::from_str(toml1).unwrap();
        assert_eq!(cfg1.cursor_color(), Some("#ff0000"));
        assert_eq!(cfg1.cursor_text_color(), Some("#00ff00"));

        // [window] aliases: cursor and cursor_text / cursor_fg
        let toml2 = r##"
            [window]
            cursor = "#fff"
            cursor_text = "#000"
        "##;
        let cfg2: Config = toml::from_str(toml2).unwrap();
        assert_eq!(cfg2.cursor_color(), Some("#fff"));
        assert_eq!(cfg2.cursor_text_color(), Some("#000"));

        // [window] section with "inverted" / "default"
        let toml3 = r##"
            [window]
            cursor_color = "inverted"
            cursor_text_color = "default"
        "##;
        let cfg3: Config = toml::from_str(toml3).unwrap();
        assert_eq!(cfg3.cursor_color(), Some("inverted"));
        assert_eq!(cfg3.cursor_text_color(), Some("default"));

        // Legacy flat config
        let toml4 = r##"
            cursor_color = "#123456"
            cursor_text_color = "#654321"
        "##;
        let cfg4: Config = toml::from_str(toml4).unwrap();
        assert_eq!(cfg4.cursor_color(), Some("#123456"));
        assert_eq!(cfg4.cursor_text_color(), Some("#654321"));

        // Legacy flat aliases
        let toml5 = r##"
            cursor = "#abcdef"
            cursor_fg = "#fedcba"
        "##;
        let cfg5: Config = toml::from_str(toml5).unwrap();
        assert_eq!(cfg5.cursor_color(), Some("#abcdef"));
        assert_eq!(cfg5.cursor_text_color(), Some("#fedcba"));
    }

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(
            parse_hex_color("#fff"),
            Some(Color {
                r: 255,
                g: 255,
                b: 255
            })
        );
        assert_eq!(parse_hex_color("000"), Some(Color { r: 0, g: 0, b: 0 }));
        assert_eq!(
            parse_hex_color("#ff0000"),
            Some(Color { r: 255, g: 0, b: 0 })
        );
        assert_eq!(
            parse_hex_color("#00ff00ff"),
            Some(Color { r: 0, g: 255, b: 0 })
        );
        assert_eq!(parse_hex_color("invalid"), None);
    }
}
