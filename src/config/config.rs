use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub font_family: String,
    pub font_size: f32,
    pub shell: String,
}
