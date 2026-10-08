use std::fmt;

/// Canonical application identity used across Wayland app-id, X11 WM_CLASS,
/// .desktop entry, and desktop icon hierarchies.
pub const CANONICAL_APP_ID: &str = "io.github.lnoxsian.Velox";

/// Canonical instance name for X11 WM_CLASS (WM_CLASS = instance, general).
pub const CANONICAL_WM_CLASS_INSTANCE: &str = "velox";

/// Canonical general class name for X11 WM_CLASS.
pub const CANONICAL_WM_CLASS_GENERAL: &str = "Velox";

/// Underlying windowing backend detected at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LinuxWindowBackend {
    Wayland,
    X11,
    Unknown,
}

impl LinuxWindowBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for LinuxWindowBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Diagnostic helper that inspects environment variables.
pub fn detect_backend_from_env() -> LinuxWindowBackend {
    if let Ok(b) = std::env::var("VELOX_BACKEND") {
        if b.eq_ignore_ascii_case("x11") {
            return LinuxWindowBackend::X11;
        } else if b.eq_ignore_ascii_case("wayland") {
            return LinuxWindowBackend::Wayland;
        }
    }


    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    if let Some(ref st) = session_type {
        if st.eq_ignore_ascii_case("x11") {
            return LinuxWindowBackend::X11;
        } else if st.eq_ignore_ascii_case("wayland") {
            return LinuxWindowBackend::Wayland;
        }
    }

    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let x11_display = std::env::var("DISPLAY").ok();

    if wayland_display.as_ref().is_some_and(|wd| !wd.trim().is_empty()) {
        return LinuxWindowBackend::Wayland;
    }

    if x11_display.as_ref().is_some_and(|d| !d.trim().is_empty()) {
        return LinuxWindowBackend::X11;
    }

    LinuxWindowBackend::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_id_format() {
        assert_eq!(CANONICAL_APP_ID, "io.github.lnoxsian.Velox");
        assert_eq!(CANONICAL_WM_CLASS_INSTANCE, "velox");
    }

    #[test]
    fn test_linux_backend_display() {
        assert_eq!(LinuxWindowBackend::Wayland.to_string(), "Wayland");
        assert_eq!(LinuxWindowBackend::X11.to_string(), "X11");
        assert_eq!(LinuxWindowBackend::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_backend_env_detection() {
        let backend = detect_backend_from_env();
        assert!(matches!(
            backend,
            LinuxWindowBackend::Wayland | LinuxWindowBackend::X11 | LinuxWindowBackend::Unknown
        ));
    }
}
