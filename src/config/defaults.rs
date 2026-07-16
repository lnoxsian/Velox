use crate::config::config::Config;

pub fn default_config() -> Config {
    Config {
        font_family: "monospace".to_string(),
        font_size: 14.0,
        shell: "/bin/sh".to_string(),
        default_fg: Some("#F8F8F2".to_string()),
        default_bg: Some("#272822".to_string()),
        ansi_colors: Some(vec![
            "#272822".to_string(), // Black
            "#F92672".to_string(), // Red
            "#A6E22E".to_string(), // Green
            "#F4B575".to_string(), // Yellow
            "#66D9EF".to_string(), // Blue
            "#AE81FF".to_string(), // Magenta
            "#A1EFE4".to_string(), // Cyan
            "#F8F8F2".to_string(), // White
            "#75715E".to_string(), // Bright Black
            "#F92672".to_string(), // Bright Red
            "#A6E22E".to_string(), // Bright Green
            "#F4B575".to_string(), // Bright Yellow
            "#66D9EF".to_string(), // Bright Blue
            "#AE81FF".to_string(), // Bright Magenta
            "#A1EFE4".to_string(), // Bright Cyan
            "#F8F8F0".to_string(), // Bright White
        ]),
    }
}
