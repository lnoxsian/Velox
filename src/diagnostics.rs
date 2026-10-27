use crate::config::config::Config;
use crate::platform::{self, CANONICAL_APP_ID};
use std::path::PathBuf;

/// Diagnostic information about the host environment, display, and rendering setup.
pub fn run_diagnostics(_config: &Config) {
    println!("================================================================================");
    println!("                           Velox System Diagnostics                             ");
    println!("================================================================================");
    println!("Velox version:        {}", env!("CARGO_PKG_VERSION"));
    println!("Operating System:     {}", std::env::consts::OS);
    println!("Target Architecture:  {}", std::env::consts::ARCH);

    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "not set".to_string());
    println!("Session Type:         {} (XDG_SESSION_TYPE)", session_type);

    println!("\n[Display Environment]");
    let wayland_disp = std::env::var("WAYLAND_DISPLAY").ok();
    match wayland_disp {
        Some(ref val) if !val.is_empty() => println!("  WAYLAND_DISPLAY:    {} (available)", val),
        _ => println!("  WAYLAND_DISPLAY:    not set"),
    }

    let x11_disp = std::env::var("DISPLAY").ok();
    match x11_disp {
        Some(ref val) if !val.is_empty() => println!("  DISPLAY:            {} (available)", val),
        _ => println!("  DISPLAY:            not set"),
    }

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok();
    match runtime_dir {
        Some(ref val) => println!("  XDG_RUNTIME_DIR:    {} (available)", val),
        None => println!("  XDG_RUNTIME_DIR:    not set"),
    }

    let env_backend = platform::detect_backend_from_env();
    println!("  Environment Hint:   {}", env_backend);

    println!("\n[Renderer Architecture]");
    println!("  Rendering Pipeline: Pure CPU Software Renderer");
    println!("  Framebuffer:        CPU-owned 32-bit linear memory buffer (0x00RRGGBB)");
    println!(
        "  Presentation:       Pure-Rust Native Shared Memory Display (X11 MIT-SHM / Wayland wl_shm)"
    );

    println!("\n[PTY & Thread Architecture]");
    println!("  PTY I/O Architecture: Shared Epoll Reactor (1 persistent thread for all panes)");
    println!(
        "  Persistent Threads:   2 (main event/render thread + shared PTY reactor; 3 with IPC enabled)"
    );

    println!("\n[Desktop & Icon Integration]");
    println!("  Canonical App ID:   {}", CANONICAL_APP_ID);

    // Check runtime icon asset
    let runtime_icon_exists =
        std::path::Path::new("assets/generated_icons/icon_128x128.png").exists();
    if runtime_icon_exists {
        println!(
            "  Runtime Icon Asset: Embedded & Found (assets/generated_icons/icon_128x128.png)"
        );
    } else {
        println!("  Runtime Icon Asset: Embedded compiled byte slice");
    }

    // Check system desktop file locations
    let system_desktop =
        PathBuf::from("/usr/share/applications").join(format!("{}.desktop", CANONICAL_APP_ID));
    let local_desktop = dirs_or_home_desktop();
    let desktop_status = if system_desktop.exists() {
        format!("Found ({})", system_desktop.display())
    } else if let Some(p) = &local_desktop
        && p.exists()
    {
        format!("Found ({})", p.display())
    } else {
        "Not installed in standard XDG application directories".to_string()
    };
    println!("  Desktop Entry:      {}", desktop_status);

    // Check hicolor icon locations
    let hicolor_status = check_hicolor_status();
    println!("  Hicolor Icon Set:   {}", hicolor_status);

    let ipc_socket = crate::ipc::socket_path();
    println!("  IPC Socket Path:    {}", ipc_socket.display());

    println!("\n[Font System]");
    println!("  Configured Family:  {}", _config.font_family());
    println!("  Configured Size:    {} pt", _config.font_size());
    let db = crate::font::fallback::get_system_font_db();
    let total_faces = db.faces().count();
    println!("  System Font Faces:  {}", total_faces);
    match crate::font::resolved::resolve_regular_font(db, _config.font_family()) {
        Ok((_, _, resolved_family)) => {
            println!("  Resolved Face:      {}", resolved_family);
        }
        Err(e) => {
            println!("  Resolved Face:      Failed ({})", e);
        }
    }

    println!("================================================================================");
}

fn dirs_or_home_desktop() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(|h| {
        PathBuf::from(h)
            .join(".local/share/applications")
            .join(format!("{}.desktop", CANONICAL_APP_ID))
    })
}

fn check_hicolor_status() -> String {
    let sizes = [
        "16x16", "32x32", "48x48", "64x64", "128x128", "256x256", "512x512",
    ];
    let mut installed_count = 0;
    for s in &sizes {
        let p = PathBuf::from(format!(
            "/usr/share/icons/hicolor/{}/apps/{}.png",
            s, CANONICAL_APP_ID
        ));
        if p.exists() {
            installed_count += 1;
        } else if let Ok(home) = std::env::var("HOME") {
            let user_p = PathBuf::from(home).join(format!(
                ".local/share/icons/hicolor/{}/apps/{}.png",
                s, CANONICAL_APP_ID
            ));
            if user_p.exists() {
                installed_count += 1;
            }
        }
    }

    if installed_count == sizes.len() {
        "Fully installed (all standard resolutions present)".to_string()
    } else if installed_count > 0 {
        format!(
            "Partially installed ({}/{} resolutions present)",
            installed_count,
            sizes.len()
        )
    } else {
        "Not installed (run 'make install-icons' or 'make install' to install)".to_string()
    }
}

use std::sync::atomic::{AtomicUsize, Ordering};

/// Lightweight instrumentation counters for tracking interactive resize performance.
pub struct ResizeStats;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResizeStatsSnapshot {
    pub pixel_resize_events: usize,
    pub applied_viewport_resizes: usize,
    pub terminal_resize_calls: usize,
    pub normal_grid_reflows: usize,
    pub alternate_grid_reflows: usize,
    pub pty_resize_calls: usize,
    pub framebuffer_resize_calls: usize,
    pub framebuffer_growth_events: usize,
    pub framebuffer_shrink_events: usize,
    pub pane_state_clears: usize,
    pub pane_state_invalidations: usize,
    pub full_redraws: usize,
    pub frames_rendered: usize,
}

static PIXEL_RESIZE_EVENTS: AtomicUsize = AtomicUsize::new(0);
static APPLIED_VIEWPORT_RESIZES: AtomicUsize = AtomicUsize::new(0);
static TERMINAL_RESIZE_CALLS: AtomicUsize = AtomicUsize::new(0);
static NORMAL_GRID_REFLOWS: AtomicUsize = AtomicUsize::new(0);
static ALTERNATE_GRID_REFLOWS: AtomicUsize = AtomicUsize::new(0);
static PTY_RESIZE_CALLS: AtomicUsize = AtomicUsize::new(0);
static FRAMEBUFFER_RESIZE_CALLS: AtomicUsize = AtomicUsize::new(0);
static FRAMEBUFFER_GROWTH_EVENTS: AtomicUsize = AtomicUsize::new(0);
static FRAMEBUFFER_SHRINK_EVENTS: AtomicUsize = AtomicUsize::new(0);
static PANE_STATE_CLEARS: AtomicUsize = AtomicUsize::new(0);
static PANE_STATE_INVALIDATIONS: AtomicUsize = AtomicUsize::new(0);
static FULL_REDRAWS: AtomicUsize = AtomicUsize::new(0);
static FRAMES_RENDERED: AtomicUsize = AtomicUsize::new(0);

impl ResizeStats {
    #[inline(always)]
    pub fn record_pixel_resize() {
        PIXEL_RESIZE_EVENTS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_applied_viewport_resize() {
        APPLIED_VIEWPORT_RESIZES.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_terminal_resize() {
        TERMINAL_RESIZE_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_normal_grid_reflow() {
        NORMAL_GRID_REFLOWS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_alternate_grid_reflow() {
        ALTERNATE_GRID_REFLOWS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_pty_resize() {
        PTY_RESIZE_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_framebuffer_resize() {
        FRAMEBUFFER_RESIZE_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_framebuffer_growth() {
        FRAMEBUFFER_GROWTH_EVENTS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_framebuffer_shrink() {
        FRAMEBUFFER_SHRINK_EVENTS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_pane_state_clear() {
        PANE_STATE_CLEARS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_pane_state_invalidation() {
        PANE_STATE_INVALIDATIONS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_full_redraw() {
        FULL_REDRAWS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_frame_rendered() {
        FRAMES_RENDERED.fetch_add(1, Ordering::Relaxed);
    }

    pub fn reset() {
        PIXEL_RESIZE_EVENTS.store(0, Ordering::Relaxed);
        APPLIED_VIEWPORT_RESIZES.store(0, Ordering::Relaxed);
        TERMINAL_RESIZE_CALLS.store(0, Ordering::Relaxed);
        NORMAL_GRID_REFLOWS.store(0, Ordering::Relaxed);
        ALTERNATE_GRID_REFLOWS.store(0, Ordering::Relaxed);
        PTY_RESIZE_CALLS.store(0, Ordering::Relaxed);
        FRAMEBUFFER_RESIZE_CALLS.store(0, Ordering::Relaxed);
        FRAMEBUFFER_GROWTH_EVENTS.store(0, Ordering::Relaxed);
        FRAMEBUFFER_SHRINK_EVENTS.store(0, Ordering::Relaxed);
        PANE_STATE_CLEARS.store(0, Ordering::Relaxed);
        PANE_STATE_INVALIDATIONS.store(0, Ordering::Relaxed);
        FULL_REDRAWS.store(0, Ordering::Relaxed);
        FRAMES_RENDERED.store(0, Ordering::Relaxed);
    }

    pub fn get() -> ResizeStatsSnapshot {
        ResizeStatsSnapshot {
            pixel_resize_events: PIXEL_RESIZE_EVENTS.load(Ordering::Relaxed),
            applied_viewport_resizes: APPLIED_VIEWPORT_RESIZES.load(Ordering::Relaxed),
            terminal_resize_calls: TERMINAL_RESIZE_CALLS.load(Ordering::Relaxed),
            normal_grid_reflows: NORMAL_GRID_REFLOWS.load(Ordering::Relaxed),
            alternate_grid_reflows: ALTERNATE_GRID_REFLOWS.load(Ordering::Relaxed),
            pty_resize_calls: PTY_RESIZE_CALLS.load(Ordering::Relaxed),
            framebuffer_resize_calls: FRAMEBUFFER_RESIZE_CALLS.load(Ordering::Relaxed),
            framebuffer_growth_events: FRAMEBUFFER_GROWTH_EVENTS.load(Ordering::Relaxed),
            framebuffer_shrink_events: FRAMEBUFFER_SHRINK_EVENTS.load(Ordering::Relaxed),
            pane_state_clears: PANE_STATE_CLEARS.load(Ordering::Relaxed),
            pane_state_invalidations: PANE_STATE_INVALIDATIONS.load(Ordering::Relaxed),
            full_redraws: FULL_REDRAWS.load(Ordering::Relaxed),
            frames_rendered: FRAMES_RENDERED.load(Ordering::Relaxed),
        }
    }

    pub fn summary() -> String {
        let s = Self::get();
        format!(
            "resize stats:\n  pixel events: {}\n  viewport resizes: {}\n  terminal resizes: {}\n  normal reflows: {}\n  alternate reflows: {}\n  pty resizes: {}\n  framebuffer resize calls: {}\n  framebuffer growth events: {}\n  framebuffer shrink events: {}\n  pane-state clears: {}\n  pane-state invalidations: {}\n  full redraws: {}\n  frames rendered: {}",
            s.pixel_resize_events,
            s.applied_viewport_resizes,
            s.terminal_resize_calls,
            s.normal_grid_reflows,
            s.alternate_grid_reflows,
            s.pty_resize_calls,
            s.framebuffer_resize_calls,
            s.framebuffer_growth_events,
            s.framebuffer_shrink_events,
            s.pane_state_clears,
            s.pane_state_invalidations,
            s.full_redraws,
            s.frames_rendered,
        )
    }
}
