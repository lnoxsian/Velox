pub mod ansi;
pub mod app;
pub mod benchmark;
pub mod clipboard;
pub mod config;
pub mod cursor;
pub mod font;
pub mod hyperlink;
pub mod input;
pub mod parser;
pub mod platform;
pub mod profiler;
pub mod pty;
pub mod renderer;
pub mod screen;
pub mod search;
pub mod selection;
pub mod terminal;
pub mod theme;
pub mod utils;
pub mod window;

use winit::event_loop::EventLoop;
use crate::app::app::{App, CustomEvent};

fn main() {
    env_logger::init();
    let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).unwrap();
}
