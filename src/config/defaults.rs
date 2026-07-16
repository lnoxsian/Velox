use crate::config::config::Config;

pub fn default_config() -> Config {
    Config {
        font_family: "monospace".to_string(),
        font_size: 14.0,
        shell: "/bin/sh".to_string(),
    }
}
