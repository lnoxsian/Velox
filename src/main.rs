#![allow(clippy::module_inception)]

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

fn main() {
    println!("Hello, Velox!");
}
