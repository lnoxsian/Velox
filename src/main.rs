pub mod ansi;
pub mod app;
pub mod clipboard;
pub mod config;
pub mod font;
pub mod hyperlink;
pub mod input;
pub mod pty;
pub mod renderer;
pub mod screen;
pub mod terminal;
pub mod theme;

use winit::event_loop::EventLoop;
use crate::app::app::{App, CustomEvent};

fn main() {
    env_logger::init();

    // Load config to check settings at startup
    let config = crate::config::loader::load().unwrap_or_else(|_| {
        crate::config::defaults::default_config()
    });

    if !config.gpu_acceleration.unwrap_or(true) {
        log::info!("GPU acceleration disabled. Setting LIBGL_ALWAYS_SOFTWARE=1 and GALLIUM_DRIVER=softpipe.");
        unsafe {
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
            std::env::set_var("GALLIUM_DRIVER", "softpipe");
        }
    }

    let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).unwrap();
}
