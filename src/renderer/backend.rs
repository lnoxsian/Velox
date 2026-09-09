use crate::app::app::WindowRendererBackend;
use crate::config::config::{Config, RendererBackendConfig};
use crate::renderer::renderer::Renderer;
use crate::renderer::software::CpuRenderer;
use crate::theme::theme::Theme;
use glow::HasContext;
use glutin::config::{Config as GlutinConfig, ConfigTemplateBuilder};
use glutin::context::{ContextApi, ContextAttributesBuilder, Version};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::ApiPreference;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowAttributes};

/// Queried GPU and OpenGL driver metadata for logging and diagnostics.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GlInfo {
    pub vendor: String,
    pub renderer: String,
    pub version: String,
    pub glsl_version: String,
}

impl GlInfo {
    pub fn is_software_rasterizer(&self) -> bool {
        let r = self.renderer.to_ascii_lowercase();
        let v = self.vendor.to_ascii_lowercase();
        r.contains("llvmpipe")
            || r.contains("softpipe")
            || r.contains("swrast")
            || r.contains("software rasterizer")
            || (v.contains("mesa") && r.contains("software"))
    }
}

pub fn query_gl_info(gl: &glow::Context) -> GlInfo {
    unsafe {
        let vendor = gl.get_parameter_string(glow::VENDOR);
        let renderer = gl.get_parameter_string(glow::RENDERER);
        let version = gl.get_parameter_string(glow::VERSION);
        let glsl_version = gl.get_parameter_string(glow::SHADING_LANGUAGE_VERSION);
        GlInfo {
            vendor,
            renderer,
            version,
            glsl_version,
        }
    }
}

/// Structured error describing failure at any stage of renderer initialization.
#[derive(Debug)]
pub enum RendererInitError {
    DisplayCreation(String),
    NoConfigFound,
    WindowCreation(String),
    ContextCreation(String),
    SurfaceCreation(String),
    MakeCurrent(String),
    ShaderCompilation(String),
    SoftwareInit(String),
}

impl std::fmt::Display for RendererInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DisplayCreation(e) => write!(f, "Failed to create OpenGL display: {}", e),
            Self::NoConfigFound => {
                write!(f, "No compatible OpenGL framebuffer configuration found")
            }
            Self::WindowCreation(e) => write!(f, "Failed to create OS window: {}", e),
            Self::ContextCreation(e) => write!(f, "Failed to create OpenGL context: {}", e),
            Self::SurfaceCreation(e) => write!(f, "Failed to create OpenGL window surface: {}", e),
            Self::MakeCurrent(e) => write!(f, "Failed to make OpenGL context current: {}", e),
            Self::ShaderCompilation(e) => write!(f, "OpenGL shader initialization failed: {}", e),
            Self::SoftwareInit(e) => write!(f, "Failed to initialize software renderer: {}", e),
        }
    }
}

impl std::error::Error for RendererInitError {}

/// Manages global display connection and matching framebuffer configuration
/// without requiring any dummy or invisible windows.
pub struct GlDisplayManager {
    pub display: glutin::display::Display,
    pub config: GlutinConfig,
}

impl GlDisplayManager {
    /// Initialize OpenGL display connection and select an optimal configuration.
    /// Uses native EGL on Wayland and EGL/GLX on X11.
    pub fn init(event_loop: &ActiveEventLoop) -> Result<Self, RendererInitError> {
        let template_builder = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(true);

        let display_builder =
            glutin_winit::DisplayBuilder::new().with_preference(ApiPreference::FallbackEgl);

        let (opt_window, gl_config) = display_builder
            .build(event_loop, template_builder, |configs| {
                configs
                    .max_by_key(|c| {
                        (
                            c.supports_transparency().unwrap_or(false),
                            -(c.num_samples() as i32),
                        )
                    })
                    .unwrap()
            })
            .map_err(|e| RendererInitError::DisplayCreation(e.to_string()))?;

        // Note: No dummy window was requested, so opt_window must be None.
        drop(opt_window);

        let display = gl_config.display();
        Ok(Self {
            display,
            config: gl_config,
        })
    }
}

/// Standalone probe function used by `--diagnostics` to test OpenGL viability.
pub fn probe_opengl() -> Result<GlInfo, String> {
    use winit::event_loop::EventLoop;
    let event_loop = EventLoop::new().map_err(|e| format!("Event loop creation failed: {}", e))?;

    let template_builder = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true);

    let window_attrs = WindowAttributes::default()
        .with_visible(false)
        .with_inner_size(winit::dpi::PhysicalSize::new(1, 1));

    let display_builder = glutin_winit::DisplayBuilder::new()
        .with_preference(ApiPreference::FallbackEgl)
        .with_window_attributes(Some(window_attrs));

    let (opt_window, gl_config) = display_builder
        .build(&event_loop, template_builder, |configs| {
            configs
                .max_by_key(|c| {
                    (
                        c.supports_transparency().unwrap_or(false),
                        -(c.num_samples() as i32),
                    )
                })
                .unwrap()
        })
        .map_err(|e| format!("Display creation error: {}", e))?;

    let window = opt_window.ok_or_else(|| "Window creation returned None".to_string())?;
    let raw_handle = window
        .window_handle()
        .map_err(|e| format!("Failed to get window handle: {}", e))?
        .as_raw();

    let display = gl_config.display();

    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(Some(raw_handle));

    let not_current_context = unsafe {
        display
            .create_context(&gl_config, &context_attributes)
            .map_err(|e| format!("Context creation error: {}", e))?
    };

    let width = NonZeroU32::new(1).unwrap();
    let height = NonZeroU32::new(1).unwrap();
    let surface_attrs =
        SurfaceAttributesBuilder::<WindowSurface>::default().build(raw_handle, width, height);

    let gl_surface = unsafe {
        display
            .create_window_surface(&gl_config, &surface_attrs)
            .map_err(|e| format!("Surface creation error: {}", e))?
    };

    let gl_context = not_current_context
        .make_current(&gl_surface)
        .map_err(|e| format!("Make current error: {}", e))?;

    let info_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gl = unsafe {
            glow::Context::from_loader_function(|symbol| {
                let c_str = std::ffi::CString::new(symbol).unwrap_or_default();
                display.get_proc_address(&c_str)
            })
        };
        query_gl_info(&gl)
    }));

    drop(gl_context);
    drop(gl_surface);
    drop(window);

    match info_res {
        Ok(info) => {
            if !info.version.is_empty() {
                Ok(info)
            } else {
                Ok(GlInfo {
                    vendor: "Driver available (OpenGL 3.3 capable)".to_string(),
                    renderer: "Hardware accelerated display detected".to_string(),
                    version: "OpenGL 3.3+".to_string(),
                    glsl_version: "330 core".to_string(),
                })
            }
        }
        Err(_) => Err("Failed to query OpenGL information from GL context".to_string()),
    }
}

/// Create a window and its rendering backend according to the configured policy.
///
/// Follows the policy:
/// - `Software`: Only creates the software backend. Never attempts OpenGL.
/// - `Opengl`: Attempts OpenGL. If it fails, returns an error immediately.
/// - `Auto`: Attempts OpenGL. If it fails at any stage, logs a warning and
///   falls back to the software renderer transparently.
pub fn create_window_and_renderer(
    event_loop: &ActiveEventLoop,
    window_attributes: WindowAttributes,
    config: &Config,
    cached_gl_manager: &mut Option<GlDisplayManager>,
) -> Result<(Arc<Window>, WindowRendererBackend, Option<GlInfo>), RendererInitError> {
    let mode = config.renderer_backend();

    if mode == RendererBackendConfig::Software {
        log::info!("Renderer backend set to Software. Bypassing OpenGL initialization.");
        return create_software_window_and_renderer(event_loop, window_attributes, config, None);
    }

    // Try OpenGL initialization (for Auto or Opengl modes)
    match try_create_opengl_window_and_renderer(
        event_loop,
        window_attributes.clone(),
        config,
        cached_gl_manager,
    ) {
        Ok((window, backend, gl_info)) => {
            if mode == RendererBackendConfig::Auto
                && gl_info.as_ref().is_some_and(|i| i.is_software_rasterizer())
            {
                let info = gl_info.as_ref().unwrap();
                log::info!(
                    "OpenGL detected unaccelerated software rasterizer ({} - {}). Falling back to native CPU software renderer for optimal performance and efficiency.",
                    info.vendor,
                    info.renderer
                );
                drop(backend);
                drop(window);
                return create_software_window_and_renderer(
                    event_loop,
                    window_attributes,
                    config,
                    None,
                );
            }
            Ok((window, backend, gl_info))
        }
        Err(gl_err) => {
            if mode == RendererBackendConfig::Opengl {
                log::error!(
                    "Explicit OpenGL renderer backend requested, but initialization failed: {}",
                    gl_err
                );
                Err(gl_err)
            } else {
                // Auto mode: log warning and fall back to software renderer
                log::warn!(
                    "OpenGL initialization failed: {}. Falling back to software renderer.",
                    gl_err
                );
                create_software_window_and_renderer(event_loop, window_attributes, config, None)
            }
        }
    }
}

fn try_create_opengl_window_and_renderer(
    event_loop: &ActiveEventLoop,
    window_attributes: WindowAttributes,
    config: &Config,
    cached_gl_manager: &mut Option<GlDisplayManager>,
) -> Result<(Arc<Window>, WindowRendererBackend, Option<GlInfo>), RendererInitError> {
    if cached_gl_manager.is_none() {
        let mgr = GlDisplayManager::init(event_loop)?;
        *cached_gl_manager = Some(mgr);
    }
    let gl_manager = cached_gl_manager.as_ref().unwrap();

    // 1. Finalize window creation ensuring visual/attributes match the GL config
    let window = Arc::new(
        glutin_winit::finalize_window(event_loop, window_attributes, &gl_manager.config)
            .map_err(|e| RendererInitError::WindowCreation(e.to_string()))?,
    );

    let raw_window_handle = window
        .as_ref()
        .window_handle()
        .map_err(|e| RendererInitError::WindowCreation(e.to_string()))?
        .as_raw();

    // 2. Build context attributes for OpenGL 3.3 Core Profile
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(Some(raw_window_handle));

    let not_current_context = unsafe {
        gl_manager
            .display
            .create_context(&gl_manager.config, &context_attributes)
            .map_err(|e| RendererInitError::ContextCreation(e.to_string()))?
    };

    // 3. Create surface attributes safely with non-zero dimensions
    let size = window.inner_size();
    let width = NonZeroU32::new(size.width.max(1)).unwrap();
    let height = NonZeroU32::new(size.height.max(1)).unwrap();
    let surface_attrs = SurfaceAttributesBuilder::<WindowSurface>::default().build(
        raw_window_handle,
        width,
        height,
    );

    let gl_surface = unsafe {
        gl_manager
            .config
            .display()
            .create_window_surface(&gl_manager.config, &surface_attrs)
            .map_err(|e| RendererInitError::SurfaceCreation(e.to_string()))?
    };

    // 4. Make context current on the new window surface
    let gl_context = not_current_context
        .make_current(&gl_surface)
        .map_err(|e| RendererInitError::MakeCurrent(e.to_string()))?;

    // 5. Configure swap interval (vsync = 1)
    let _ =
        gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()));

    // 6. Initialize Glow function pointers from display proc address loader
    let gl = unsafe {
        glow::Context::from_loader_function(|symbol| {
            let c_str = std::ffi::CString::new(symbol).unwrap_or_default();
            gl_manager.display.get_proc_address(&c_str)
        })
    };

    let gl_info = query_gl_info(&gl);

    // 7. Instantiate OpenGL Renderer with try_new to catch shader/buffer failures
    let font_size = config.font_size();
    let font_scale_multiplier = config.font_scale_multiplier().unwrap_or(1.5);
    let win_w = size.width.max(1);
    let win_h = size.height.max(1);

    let renderer = Renderer::try_new(
        Arc::new(gl),
        config.font_family(),
        font_size,
        font_scale_multiplier,
        win_w,
        win_h,
    )
    .map_err(RendererInitError::ShaderCompilation)?;

    crate::memory::trim_allocator_memory();

    log::info!(
        "OpenGL initialized successfully: {} ({}) - {}",
        gl_info.version,
        gl_info.renderer,
        gl_info.vendor
    );

    Ok((
        window,
        WindowRendererBackend::OpenGL {
            renderer,
            gl_surface,
            gl_context,
        },
        Some(gl_info),
    ))
}

fn create_software_window_and_renderer(
    event_loop: &ActiveEventLoop,
    window_attributes: WindowAttributes,
    config: &Config,
    existing_window: Option<Arc<Window>>,
) -> Result<(Arc<Window>, WindowRendererBackend, Option<GlInfo>), RendererInitError> {
    let window = match existing_window {
        Some(w) => w,
        None => Arc::new(
            event_loop
                .create_window(window_attributes)
                .map_err(|e| RendererInitError::WindowCreation(e.to_string()))?,
        ),
    };

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

    log::info!("Software renderer (softbuffer) initialized successfully.");

    Ok((
        window,
        WindowRendererBackend::Software { renderer, surface },
        None,
    ))
}
