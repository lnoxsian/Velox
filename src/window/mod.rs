pub mod event;
pub mod wayland;
pub mod x11;

pub use event::{
    CursorIcon, ElementState, Key, ModifiersState, MouseButton, NamedKey, PlatformEvent,
};
pub use wayland::WaylandWindow;
pub use x11::X11Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowBackendKind {
    X11,
    Wayland,
}

pub enum PlatformWindow {
    X11(X11Window),
    Wayland(Box<WaylandWindow>),
}

impl PlatformWindow {
    pub fn new(
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let preferred_backend = crate::platform::detect_backend_from_env();

        match preferred_backend {
            crate::platform::LinuxWindowBackend::X11 => {
                match X11Window::new(title, width, height) {
                    Ok(w) => Ok(PlatformWindow::X11(w)),
                    Err(e) => {
                        log::warn!("X11 window creation failed ({}), attempting Wayland fallback...", e);
                        if let Ok(wd) = std::env::var("WAYLAND_DISPLAY")
                            && !wd.trim().is_empty()
                        {
                            match WaylandWindow::new(title, width, height) {
                                Ok(w) => return Ok(PlatformWindow::Wayland(Box::new(w))),
                                Err(w_err) => {
                                    log::warn!("Wayland fallback also failed: {}", w_err);
                                }
                            }
                        }
                        Err(format!("Failed to connect to X11 display: {}", e).into())
                    }
                }
            }
            crate::platform::LinuxWindowBackend::Wayland => {
                match WaylandWindow::new(title, width, height) {
                    Ok(w) => Ok(PlatformWindow::Wayland(Box::new(w))),
                    Err(e) => {
                        log::warn!("Wayland connection failed ({}), attempting X11 fallback...", e);
                        if let Ok(xd) = std::env::var("DISPLAY")
                            && !xd.trim().is_empty()
                        {
                            match X11Window::new(title, width, height) {
                                Ok(w) => return Ok(PlatformWindow::X11(w)),
                                Err(x_err) => {
                                    log::warn!("X11 fallback also failed: {}", x_err);
                                }
                            }
                        }
                        Err(format!("Failed to connect to Wayland display: {}", e).into())
                    }
                }
            }
            crate::platform::LinuxWindowBackend::Unknown => {
                if let Ok(wd) = std::env::var("WAYLAND_DISPLAY")
                    && !wd.trim().is_empty()
                {
                    match WaylandWindow::new(title, width, height) {
                        Ok(w) => return Ok(PlatformWindow::Wayland(Box::new(w))),
                        Err(e) => {
                            log::warn!("Wayland connection failed ({}), attempting X11 fallback...", e);
                        }
                    }
                }

                if let Ok(xd) = std::env::var("DISPLAY")
                    && !xd.trim().is_empty()
                {
                    match X11Window::new(title, width, height) {
                        Ok(w) => return Ok(PlatformWindow::X11(w)),
                        Err(e) => {
                            return Err(format!("Failed to connect to X11 display: {}", e).into());
                        }
                    }
                }

                Err("No graphical display found. Ensure WAYLAND_DISPLAY or DISPLAY is set in your environment.".into())
            }
        }
    }

    #[inline]
    pub fn backend_kind(&self) -> WindowBackendKind {
        match self {
            Self::X11(_) => WindowBackendKind::X11,
            Self::Wayland(_) => WindowBackendKind::Wayland,
        }
    }

    #[inline]
    pub fn width(&self) -> u32 {
        match self {
            Self::X11(w) => w.width,
            Self::Wayland(w) => w.state.width,
        }
    }

    #[inline]
    pub fn height(&self) -> u32 {
        match self {
            Self::X11(w) => w.height,
            Self::Wayland(w) => w.state.height,
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        match self {
            Self::X11(w) => w.set_title(title),
            Self::Wayland(w) => w.set_title(title),
        }
    }

    #[inline]
    pub fn set_cursor(&self, icon: CursorIcon) {
        match self {
            Self::X11(w) => w.set_cursor(icon),
            Self::Wayland(w) => w.set_cursor(icon),
        }
    }

    #[inline]
    pub fn present_frame(&mut self, src_pixels: &[u32]) {
        match self {
            Self::X11(w) => w.present_frame(src_pixels),
            Self::Wayland(w) => w.present_frame(src_pixels),
        }
    }

    #[inline]
    pub fn poll_event(&mut self) -> Option<PlatformEvent> {
        match self {
            Self::X11(w) => w.poll_event(),
            Self::Wayland(w) => w.poll_event(),
        }
    }

    #[inline]
    pub fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        match self {
            Self::X11(w) => w.as_raw_fd(),
            Self::Wayland(w) => {
                use std::os::fd::{AsFd, AsRawFd};
                w.conn.as_fd().as_raw_fd()
            }
        }
    }

    #[inline]
    pub fn inner_size(&self) -> WindowSize {
        WindowSize {
            width: self.width(),
            height: self.height(),
        }
    }

    #[inline]
    pub fn set_visible(&self, _visible: bool) {}

    #[inline]
    pub fn request_redraw(&self) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}
