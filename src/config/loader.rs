use std::fs;
use std::path::PathBuf;
use crate::config::config::Config;
use crate::config::defaults::default_config;

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(String),
}

fn config_path() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(|h| {
        PathBuf::from(h).join(".config").join("velox").join("config.toml")
    })
}

pub fn load() -> Result<Config, ConfigError> {
    let path = match config_path() {
        Some(p) => p,
        None => return Ok(default_config()),
    };

    if !path.exists() {
        let default_cfg = default_config();
        let _ = save(&default_cfg);
        return Ok(default_cfg);
    }

    let contents = fs::read_to_string(&path).map_err(ConfigError::Io)?;
    let config: Config = toml::from_str(&contents)
        .map_err(|e| ConfigError::Toml(e.to_string()))?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<(), ConfigError> {
    let path = match config_path() {
        Some(p) => p,
        None => return Err(ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Home directory not found",
        ))),
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ConfigError::Io)?;
    }

    let contents = toml::to_string_pretty(config)
        .map_err(|e| ConfigError::Toml(e.to_string()))?;
    fs::write(&path, contents).map_err(ConfigError::Io)?;
    Ok(())
}

pub fn reload() -> Result<Config, ConfigError> {
    load()
}

pub fn watch_config() -> Result<(), ConfigError> {
    Ok(())
}
