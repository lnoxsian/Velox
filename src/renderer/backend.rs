use crate::config::config::Config;
use crate::renderer::CpuRenderer;
use crate::theme::theme::Theme;
use crate::window::PlatformWindow;

/// Structured error describing failure at any stage of software renderer initialization.
#[derive(Debug)]
pub enum RendererInitError {
    WindowCreation(String),
    SoftwareInit(String),
    Font(crate::font::resolved::FontError),
}

impl std::fmt::Display for RendererInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WindowCreation(e) => write!(f, "Failed to create OS window: {}", e),
            Self::SoftwareInit(e) => {
                write!(
                    f,
                    "Failed to initialize software renderer: {}",
                    e
                )
            }
            Self::Font(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for RendererInitError {}

pub type WindowAndRenderer = (PlatformWindow, CpuRenderer);

/// Create a zero-C-dependency platform window (X11 / Wayland) and its pure CPU software renderer.
pub fn create_window_and_renderer(
    title: &str,
    width: u32,
    height: u32,
    config: &Config,
) -> Result<WindowAndRenderer, RendererInitError> {
    let window = PlatformWindow::new(title, width, height)
        .map_err(|e| RendererInitError::WindowCreation(e.to_string()))?;

    let win_width = window.width();
    let win_height = window.height();

    let theme = Theme::from_config(config);
    let font_size = config.font_size();
    let font_scale_multiplier = config.font_scale_multiplier().unwrap_or(1.5);
    let bold_is_bright = config.bold_is_bright().unwrap_or(true);
    let opacity = config.opacity();

    let renderer = CpuRenderer::try_new(
        config.font_family(),
        font_size,
        font_scale_multiplier,
        &theme,
        win_width,
        win_height,
        bold_is_bright,
        opacity,
    )
    .map_err(RendererInitError::Font)?;

    log::info!("Zero-C pure-Rust CPU software renderer initialized successfully ({}x{}).", win_width, win_height);

    Ok((window, renderer))
}
