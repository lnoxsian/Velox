use crate::config::config::Config;
use crate::config::loader::ConfigError;

pub fn validate(_config: &Config) -> Result<(), ConfigError> {
    Ok(())
}
