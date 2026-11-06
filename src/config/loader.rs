use crate::config::config::Config;
use crate::config::defaults::default_config;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
}

impl std::fmt::Display for ConfigDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.severity {
            DiagnosticSeverity::Error => "Error",
            DiagnosticSeverity::Warning => "Warning",
        };
        if let (Some(l), Some(c)) = (self.line, self.col) {
            write!(f, "{prefix} [line {l}, col {c}]: {}", self.message)
        } else if let Some(l) = self.line {
            write!(f, "{prefix} [line {l}]: {}", self.message)
        } else {
            write!(f, "{prefix}: {}", self.message)
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "I/O error: {}", err),
            ConfigError::Toml(err) => write!(f, "TOML error: {}", err),
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn config_path() -> Option<PathBuf> {
    if let Ok(val) = std::env::var("VELOX_CONFIG") {
        let p = PathBuf::from(val);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(h) = std::env::var("HOME") {
        let global_path = PathBuf::from(&h)
            .join(".config")
            .join("velox")
            .join("config.toml");
        if global_path.exists() {
            return Some(global_path);
        }
    }
    let local_path = PathBuf::from("config.toml");
    if local_path.exists() {
        return Some(local_path);
    }
    let local_velox = PathBuf::from("velox.toml");
    if local_velox.exists() {
        return Some(local_velox);
    }
    std::env::var("HOME").ok().map(|h| {
        PathBuf::from(h)
            .join(".config")
            .join("velox")
            .join("config.toml")
    })
}

fn byte_to_line_col(s: &str, byte_idx: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, b) in s.bytes().enumerate() {
        if i >= byte_idx {
            break;
        }
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Parses a configuration string with error diagnostics and resilient section recovery.
pub fn load_from_str(contents: &str) -> (Config, Vec<ConfigDiagnostic>) {
    let mut diagnostics = Vec::new();

    // 1. Try full typed deserialization
    match toml::from_str::<Config>(contents) {
        Ok(cfg) => {
            diagnostics.extend(cfg.validate());
            return (cfg, diagnostics);
        }
        Err(e) => {
            let (line, col) = e
                .span()
                .map(|span| byte_to_line_col(contents, span.start))
                .unzip();
            diagnostics.push(ConfigDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: e.message().to_string(),
                line,
                col,
            });
        }
    }

    // 2. Resilient partial recovery: try parsing as a generic TOML table to preserve valid sections
    let mut cfg = default_config();
    if let Ok(table) = toml::from_str::<toml::Table>(contents) {
        // [font]
        if let Some(font_val) = table.get("font") {
            match font_val.clone().try_into::<super::config::FontConfig>() {
                Ok(fc) => cfg.font = fc,
                Err(e) => diagnostics.push(ConfigDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: format!("Failed to parse [font] section: {e}"),
                    line: None,
                    col: None,
                }),
            }
        }
        // [window]
        if let Some(win_val) = table.get("window") {
            match win_val.clone().try_into::<super::config::WindowConfig>() {
                Ok(wc) => cfg.window = wc,
                Err(e) => diagnostics.push(ConfigDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: format!("Failed to parse [window] section: {e}"),
                    line: None,
                    col: None,
                }),
            }
        }
        // [tabs]
        if let Some(tabs_val) = table.get("tabs") {
            match tabs_val.clone().try_into::<super::config::TabsConfig>() {
                Ok(tc) => cfg.tabs = tc,
                Err(e) => diagnostics.push(ConfigDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: format!("Failed to parse [tabs] section: {e}"),
                    line: None,
                    col: None,
                }),
            }
        }
        // [panes]
        if let Some(panes_val) = table.get("panes") {
            match panes_val.clone().try_into::<super::config::PanesConfig>() {
                Ok(pc) => cfg.panes = pc,
                Err(e) => diagnostics.push(ConfigDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: format!("Failed to parse [panes] section: {e}"),
                    line: None,
                    col: None,
                }),
            }
        }
        // [colors]
        if let Some(col_val) = table.get("colors") {
            match col_val.clone().try_into::<super::config::ConfigColors>() {
                Ok(cc) => cfg.colors = Some(cc),
                Err(e) => diagnostics.push(ConfigDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: format!("Failed to parse [colors] section: {e}"),
                    line: None,
                    col: None,
                }),
            }
        }

        // Top-level fields
        if let Some(sh) = table.get("shell").and_then(|v| v.as_str()) {
            cfg.shell = Some(sh.to_string());
        }
        if let Some(at) = table.get("app_title").and_then(|v| v.as_str()) {
            cfg.app_title = Some(at.to_string());
        }
        if let Some(si) = table.get("single_instance").and_then(|v| v.as_bool()) {
            cfg.single_instance = Some(si);
        }
        if let Some(wd) = table.get("working_directory").and_then(|v| v.as_str()) {
            cfg.working_directory = Some(wd.to_string());
        }

        // Legacy flat fields
        if let Some(ff) = table.get("font_family").and_then(|v| v.as_str()) {
            cfg.font.font_family = ff.to_string();
        }
        if let Some(fs) = table.get("font_size").and_then(|v| v.as_float()) {
            cfg.font.font_size = fs as f32;
        }
        if let Some(fsm) = table
            .get("font_scale_multiplier")
            .and_then(|v| v.as_float())
        {
            cfg.font.font_scale_multiplier = Some(fsm as f32);
        }
        if let Some(lh) = table.get("line_height").and_then(|v| v.as_float()) {
            cfg.font.line_height = Some(lh as f32);
        }
        if let Some(op) = table.get("opacity").and_then(|v| v.as_float()) {
            cfg.window.opacity = Some(op as f32);
        }
    }

    diagnostics.extend(cfg.validate());
    (cfg, diagnostics)
}

/// Loads user configuration along with any diagnostic messages and the resolved file path.
pub fn load_with_diagnostics() -> (Config, Vec<ConfigDiagnostic>, Option<PathBuf>) {
    let path = match config_path() {
        Some(p) => p,
        None => return (default_config(), Vec::new(), None),
    };

    if !path.exists() {
        let default_cfg = default_config();
        let _ = save(&default_cfg);
        return (default_cfg, Vec::new(), Some(path));
    }

    match fs::read_to_string(&path) {
        Ok(contents) => {
            let (cfg, diags) = load_from_str(&contents);
            (cfg, diags, Some(path))
        }
        Err(e) => {
            let diags = vec![ConfigDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: format!("Failed to read config file {}: {}", path.display(), e),
                line: None,
                col: None,
            }];
            (default_config(), diags, Some(path))
        }
    }
}

pub fn load() -> Result<Config, ConfigError> {
    let (cfg, diags, _) = load_with_diagnostics();
    if let Some(err) = diags
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        Err(ConfigError::Toml(err.message.clone()))
    } else {
        Ok(cfg)
    }
}

pub fn save(config: &Config) -> Result<(), ConfigError> {
    let path = match config_path() {
        Some(p) => p,
        None => {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Home directory not found",
            )));
        }
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ConfigError::Io)?;
    }

    let contents = toml::to_string_pretty(config).map_err(|e| ConfigError::Toml(e.to_string()))?;
    fs::write(&path, contents).map_err(ConfigError::Io)?;
    Ok(())
}
