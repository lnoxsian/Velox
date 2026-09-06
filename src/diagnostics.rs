use crate::config::config::Config;
use crate::platform::{self, CANONICAL_APP_ID};
use std::path::PathBuf;

/// Diagnostic information about the host environment, display, and rendering setup.
pub fn run_diagnostics(config: &Config) {
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

    println!("\n[Renderer Configuration]");
    let requested_backend = config.renderer_backend();
    let legacy_gpu = config.gpu_acceleration();
    println!(
        "  Requested Backend:  {:?} (gpu_acceleration = {:?})",
        requested_backend, legacy_gpu
    );
    println!("  Target GL Profile:  OpenGL 3.3 Core (EGL preferred)");

    // Test OpenGL availability via headless event loop / probe
    print!("  OpenGL Probe:       ");
    match crate::renderer::backend::probe_opengl() {
        Ok(gl_info) => {
            if gl_info.is_software_rasterizer() {
                println!("Available (Software Rasterizer - CPU fallback)");
            } else {
                println!("Available (Hardware Accelerated)");
            }
            println!("  GL Vendor:          {}", gl_info.vendor);
            println!("  GL Renderer:        {}", gl_info.renderer);
            println!("  GL Version:         {}", gl_info.version);
            println!("  GLSL Version:       {}", gl_info.glsl_version);
            if requested_backend == crate::config::config::RendererBackendConfig::Auto
                && gl_info.is_software_rasterizer()
            {
                println!(
                    "  Effective Backend:  Software (auto-selected: native CPU softbuffer renderer avoids software rasterizer overhead)"
                );
            }
        }
        Err(err) => {
            println!("Failed ({})", err);
            println!(
                "  Softbuffer Fallback:Ready (CPU Software renderer will be used in auto mode)"
            );
        }
    }

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
