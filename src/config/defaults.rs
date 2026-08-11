use crate::config::config::{Config, ConfigColors, FontConfig, WindowConfig};

pub fn default_config() -> Config {
    Config {
        font: FontConfig {
            font_family: "ComicShannsMono Nerd Font".to_string(),
            font_size: 11.0,
            font_scale_multiplier: Some(1.5),
            bold_is_bright: Some(true),
        },
        window: WindowConfig {
            scrollback_limit: Some(2000),
            infinite_scrollback: Some(true),
            gpu_acceleration: Some(true),
            scroll_multiplier: Some(5.0),
            fps_limit: Some(120),
            padding_x: Some(8.0),
            padding_y: Some(4.0),
            cursor_shape: Some("beam".to_string()),
            cursor_blink: Some(true),
        },
        colors: Some(ConfigColors {
            default_fg: Some("#e0def4".to_string()),
            default_bg: Some("#191724".to_string()),
            black: Some("#26233a".to_string()),
            red: Some("#eb6f92".to_string()),
            green: Some("#31748f".to_string()),
            yellow: Some("#f6c177".to_string()),
            blue: Some("#9ccfd8".to_string()),
            magenta: Some("#c4a7e7".to_string()),
            cyan: Some("#ebbcba".to_string()),
            white: Some("#e0def4".to_string()),
            bright_black: Some("#6e6a86".to_string()),
            bright_red: Some("#eb6f92".to_string()),
            bright_green: Some("#31748f".to_string()),
            bright_yellow: Some("#f6c177".to_string()),
            bright_blue: Some("#9ccfd8".to_string()),
            bright_magenta: Some("#c4a7e7".to_string()),
            bright_cyan: Some("#ebbcba".to_string()),
            bright_white: Some("#e0def4".to_string()),
        }),
        shell: None,
        app_title: None,
        single_instance: Some(true),
        ..Default::default()
    }
}

