use crate::config::config::Config;
use crate::renderer::CpuRenderer;
use crate::theme::theme::Theme;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

/// Structured error describing failure at any stage of software renderer initialization.
#[derive(Debug)]
pub enum RendererInitError {
    WindowCreation(String),
    SoftwareInit(String),
}

impl std::fmt::Display for RendererInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WindowCreation(e) => write!(f, "Failed to create OS window: {}", e),
            Self::SoftwareInit(e) => {
                write!(
                    f,
                    "Failed to initialize software renderer (softbuffer): {}",
                    e
                )
            }
        }
    }
}

impl std::error::Error for RendererInitError {}

pub type SoftwareSurface = softbuffer::Surface<Arc<Window>, Arc<Window>>;
pub type WindowAndRenderer = (Arc<Window>, CpuRenderer, SoftwareSurface);

/// Create an OS window and its pure CPU software rendering surface (via `softbuffer`).
pub fn create_window_and_renderer(
    event_loop: &ActiveEventLoop,
    window_attributes: WindowAttributes,
    config: &Config,
) -> Result<WindowAndRenderer, RendererInitError> {
    let window = Arc::new(
        event_loop
            .create_window(window_attributes)
            .map_err(|e| RendererInitError::WindowCreation(e.to_string()))?,
    );

    let context = softbuffer::Context::new(window.clone())
        .map_err(|e| RendererInitError::SoftwareInit(e.to_string()))?;
    let mut surface = softbuffer::Surface::new(&context, window.clone())
        .map_err(|e| RendererInitError::SoftwareInit(e.to_string()))?;

    let size = window.inner_size();
    let win_width = size.width.max(1);
    let win_height = size.height.max(1);

    if let (Some(w), Some(h)) = (NonZeroU32::new(win_width), NonZeroU32::new(win_height)) {
        let _ = surface.resize(w, h);
    }

    let theme = Theme::from_config(config);
    let font_size = config.font_size();
    let font_scale_multiplier = config.font_scale_multiplier().unwrap_or(1.5);
    let bold_is_bright = config.bold_is_bright().unwrap_or(true);
    let opacity = config.opacity();

    let renderer = CpuRenderer::new(
        config.font_family(),
        font_size,
        font_scale_multiplier,
        &theme,
        win_width,
        win_height,
        bold_is_bright,
        opacity,
    );

    log::info!("CPU software renderer (softbuffer) initialized successfully.");

    Ok((window, renderer, surface))
}
