use std::fmt;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowAttributes;

/// Canonical application identity used across Wayland app-id, X11 WM_CLASS,
/// .desktop entry, and desktop icon hierarchies.
pub const CANONICAL_APP_ID: &str = "io.github.lnoxsian.Velox";

/// Canonical instance name for X11 WM_CLASS (WM_CLASS = instance, general).
pub const CANONICAL_WM_CLASS_INSTANCE: &str = "velox";

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

/// Detect windowing backend using winit's ActiveEventLoop platform extensions.
/// This is the primary and most authoritative detection mechanism.
pub fn detect_backend_from_event_loop(event_loop: &ActiveEventLoop) -> LinuxWindowBackend {
    use winit::platform::wayland::ActiveEventLoopExtWayland;
    if event_loop.is_wayland() {
        LinuxWindowBackend::Wayland
    } else {
        LinuxWindowBackend::X11
    }
}

/// Detect windowing backend from an initialized EventLoop prior to running.
pub fn detect_backend_from_loop<T: 'static>(event_loop: &EventLoop<T>) -> LinuxWindowBackend {
    use winit::platform::wayland::EventLoopExtWayland;
    if event_loop.is_wayland() {
        LinuxWindowBackend::Wayland
    } else {
        LinuxWindowBackend::X11
    }
}

/// Diagnostic helper that inspects environment variables.
/// Note: This is an advisory hint when no active event loop is available.
pub fn detect_backend_from_env() -> LinuxWindowBackend {
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let x11_display = std::env::var("DISPLAY").ok();
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();

    // If WAYLAND_DISPLAY is non-empty, Wayland compositor is available
    if wayland_display
        .as_ref()
        .is_some_and(|wd| !wd.trim().is_empty())
    {
        return LinuxWindowBackend::Wayland;
    }

    // Check XDG_SESSION_TYPE hint if WAYLAND_DISPLAY wasn't set
    if session_type
        .as_ref()
        .is_some_and(|st| st.eq_ignore_ascii_case("wayland"))
    {
        return LinuxWindowBackend::Wayland;
    }

    // Fall back to X11 DISPLAY
    if x11_display.as_ref().is_some_and(|d| !d.trim().is_empty()) {
        return LinuxWindowBackend::X11;
    }

    LinuxWindowBackend::Unknown
}

/// Configure Wayland app-id or X11 WM_CLASS window attributes to match the
/// canonical desktop application identity.
pub fn apply_platform_window_attributes(
    event_loop: &ActiveEventLoop,
    mut attrs: WindowAttributes,
) -> WindowAttributes {
    use winit::platform::wayland::ActiveEventLoopExtWayland;
    use winit::platform::wayland::WindowAttributesExtWayland;
    use winit::platform::x11::WindowAttributesExtX11;

    if event_loop.is_wayland() {
        attrs = WindowAttributesExtWayland::with_name(
            attrs,
            CANONICAL_APP_ID,
            CANONICAL_WM_CLASS_INSTANCE,
        );
    } else {
        attrs =
            WindowAttributesExtX11::with_name(attrs, CANONICAL_APP_ID, CANONICAL_WM_CLASS_INSTANCE);
    }

    attrs
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
        // Test that detect_backend_from_env does not panic and returns a valid variant
        let backend = detect_backend_from_env();
        assert!(matches!(
            backend,
            LinuxWindowBackend::Wayland | LinuxWindowBackend::X11 | LinuxWindowBackend::Unknown
        ));
    }
}
