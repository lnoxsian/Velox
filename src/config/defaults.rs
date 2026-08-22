use crate::config::config::{Config, ConfigColors, FontConfig, WindowConfig};

pub fn default_config() -> Config {
    Config {
        font: FontConfig {
            font_family: "monospace".to_string(),
            font_size: 12.0,
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
            cursor_color: Some("default".to_string()),
            cursor_text_color: Some("default".to_string()),
            scroll_on_output: Some(true),
            scroll_on_keystroke: Some(true),
            opacity: Some(1.0),
        },
        colors: Some(ConfigColors {
            default_fg: Some("#d4d4d4".to_string()),
            default_bg: Some("#181818".to_string()),
            black: Some("#000000".to_string()),
            red: Some("#cd3131".to_string()),
            green: Some("#0dbc79".to_string()),
            yellow: Some("#e5e510".to_string()),
            blue: Some("#2472c8".to_string()),
            magenta: Some("#bc3fbc".to_string()),
            cyan: Some("#11a8cd".to_string()),
            white: Some("#e5e5e5".to_string()),
            bright_black: Some("#666666".to_string()),
            bright_red: Some("#f14c4c".to_string()),
            bright_green: Some("#23d18b".to_string()),
            bright_yellow: Some("#f5f543".to_string()),
            bright_blue: Some("#3b8eea".to_string()),
            bright_magenta: Some("#d670d6".to_string()),
            bright_cyan: Some("#29b8db".to_string()),
            bright_white: Some("#ffffff".to_string()),
        }),
        shell: None,
        app_title: None,
        single_instance: Some(true),
        ..Default::default()
    }
}
