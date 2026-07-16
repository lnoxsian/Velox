use crate::config::config::Config;

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(String),
}

pub fn load() -> Result<Config, ConfigError> {
    Ok(crate::config::defaults::default_config())
}

pub fn save(_config: &Config) -> Result<(), ConfigError> {
    Ok(())
}

pub fn reload() -> Result<Config, ConfigError> {
    Ok(crate::config::defaults::default_config())
}

pub fn watch_config() -> Result<(), ConfigError> {
    Ok(())
}
