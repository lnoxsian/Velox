pub mod csd;
pub mod event;
pub mod wayland;
pub mod x11;

pub use event::{
    CursorIcon, ElementState, Key, ModifiersState, MouseButton, NamedKey, PlatformEvent,
};
pub use wayland::WaylandWindow;
pub use x11::X11Window;

#[allow(clippy::large_enum_variant)]
pub enum PlatformWindow {
    X11(X11Window),
    Wayland(Box<WaylandWindow>),
    Dummy { width: u32, height: u32 },
}

impl PlatformWindow {
    pub fn dummy(width: u32, height: u32) -> Self {
        Self::Dummy { width, height }
    }
    pub fn new(
        title: &str,
        width: u32,
        height: u32,
        bg_pixel: Option<u32>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let preferred_backend = crate::platform::detect_backend_from_env();

        match preferred_backend {
            crate::platform::LinuxWindowBackend::X11 => {
                match X11Window::new(title, width, height, bg_pixel) {
                    Ok(w) => Ok(PlatformWindow::X11(w)),
                    Err(e) => {
                        log::warn!(
                            "X11 window creation failed ({}), attempting Wayland fallback...",
                            e
                        );
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
                        log::warn!(
                            "Wayland connection failed ({}), attempting X11 fallback...",
                            e
                        );
                        if let Ok(xd) = std::env::var("DISPLAY")
                            && !xd.trim().is_empty()
                        {
                            match X11Window::new(title, width, height, bg_pixel) {
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
                            log::warn!(
                                "Wayland connection failed ({}), attempting X11 fallback...",
                                e
                            );
                        }
                    }
                }

                if let Ok(xd) = std::env::var("DISPLAY")
                    && !xd.trim().is_empty()
                {
                    match X11Window::new(title, width, height, bg_pixel) {
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
    pub fn width(&self) -> u32 {
        match self {
            Self::X11(w) => w.width,
            Self::Wayland(w) => w.state.width,
            Self::Dummy { width, .. } => *width,
        }
    }

    #[inline]
    pub fn height(&self) -> u32 {
        match self {
            Self::X11(w) => w.height,
            Self::Wayland(w) => w.state.height,
            Self::Dummy { height, .. } => *height,
        }
    }

    #[inline]
    pub fn detect_refresh_rate(&self) -> Option<u32> {
        match self {
            Self::X11(w) => w.detect_display_refresh_rate(),
            Self::Wayland(_) | Self::Dummy { .. } => None,
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        match self {
            Self::X11(w) => w.set_title(title),
            Self::Wayland(w) => w.set_title(title),
            Self::Dummy { .. } => {}
        }
    }

    #[inline]
    pub fn set_cursor(&mut self, icon: CursorIcon) {
        match self {
            Self::X11(w) => w.set_cursor(icon),
            Self::Wayland(w) => w.set_cursor(icon),
            Self::Dummy { .. } => {}
        }
    }

    #[inline]
    pub fn set_opacity(&self, opacity: f32) {
        match self {
            Self::X11(w) => w.set_opacity(opacity),
            Self::Wayland(_) | Self::Dummy { .. } => {}
        }
    }

    #[inline]
    pub fn set_background_pixel(&self, bg_pixel: u32) {
        match self {
            Self::X11(w) => w.set_background_pixel(bg_pixel),
            Self::Wayland(_) | Self::Dummy { .. } => {}
        }
    }

    #[inline]
    pub fn present_frame(&mut self, src_pixels: &[u32], damage: Option<DamageRect>) {
        match self {
            Self::X11(w) => w.present_frame(src_pixels, damage),
            Self::Wayland(w) => w.present_frame(src_pixels, damage),
            Self::Dummy { .. } => {}
        }
    }

    #[inline]
    pub fn poll_event(&mut self) -> Option<PlatformEvent> {
        match self {
            Self::X11(w) => w.poll_event(),
            Self::Wayland(w) => w.poll_event(),
            Self::Dummy { .. } => None,
        }
    }

    #[inline]
    pub fn next_wake_time(&self) -> Option<std::time::Instant> {
        match self {
            Self::X11(_) | Self::Dummy { .. } => None,
            Self::Wayland(w) => w.next_wake_time(),
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
            Self::Dummy { .. } => -1,
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
    pub fn resize_dummy(&mut self, new_w: u32, new_h: u32) {
        if let Self::Dummy { width, height } = self {
            *width = new_w;
            *height = new_h;
        }
    }

    #[inline]
    pub fn set_visible(&mut self, visible: bool) {
        match self {
            Self::X11(w) => w.set_visible(visible),
            Self::Wayland(w) => w.set_visible(visible),
            Self::Dummy { .. } => {}
        }
    }

    #[inline]
    pub fn flush(&self) {
        match self {
            Self::X11(w) => w.flush(),
            Self::Wayland(w) => {
                let _ = w.conn.flush();
            }
            Self::Dummy { .. } => {}
        }
    }

    #[inline]
    pub fn request_redraw(&self) {}

    #[inline]
    pub fn uses_csd(&self) -> bool {
        match self {
            Self::X11(_) | Self::Dummy { .. } => false,
            Self::Wayland(w) => w.uses_csd(),
        }
    }

    #[inline]
    pub fn csd_title_bar_height(&self) -> u32 {
        match self {
            Self::X11(_) | Self::Dummy { .. } => 0,
            Self::Wayland(w) => w.csd_title_bar_height(),
        }
    }

    #[inline]
    pub fn start_interactive_move(&self) {
        match self {
            Self::X11(_) | Self::Dummy { .. } => {}
            Self::Wayland(w) => w.start_interactive_move(),
        }
    }

    #[inline]
    pub fn minimize(&self) {
        match self {
            Self::X11(_) | Self::Dummy { .. } => {}
            Self::Wayland(w) => w.minimize(),
        }
    }

    #[inline]
    pub fn toggle_maximize(&mut self) {
        match self {
            Self::X11(_) | Self::Dummy { .. } => {}
            Self::Wayland(w) => w.toggle_maximize(),
        }
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        match self {
            Self::X11(_) | Self::Dummy { .. } => false,
            Self::Wayland(w) => w.is_maximized(),
        }
    }

    #[inline]
    pub fn show_window_menu(&self, x: i32, y: i32) {
        match self {
            Self::X11(_) | Self::Dummy { .. } => {}
            Self::Wayland(w) => w.show_window_menu(x, y),
        }
    }

    #[inline]
    pub fn start_interactive_resize(&self, edge: WindowResizeEdge) {
        match self {
            Self::X11(_) | Self::Dummy { .. } => {}
            Self::Wayland(w) => w.start_interactive_resize(edge),
        }
    }

    #[inline]
    pub fn post_close_requested(&mut self) {
        match self {
            Self::X11(w) => {
                // Post CloseRequested event to X11 event queue
                // (X11 event processing handles this)
                let _ = w;
            }
            Self::Wayland(w) => {
                w.post_close_requested();
            }
            Self::Dummy { .. } => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowResizeEdge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
