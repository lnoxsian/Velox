#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::needless_range_loop,
    clippy::single_match,
    clippy::too_many_arguments,
    clippy::manual_range_contains,
    clippy::manual_div_ceil,
    clippy::manual_map,
    clippy::string_from_utf8_as_bytes,
    clippy::manual_range_patterns,
    clippy::chunks_exact_to_as_chunks
)]

pub mod ansi;
pub mod app;
pub mod cli;
pub mod clipboard;
pub mod config;
pub mod diagnostics;
pub mod font;
pub mod hyperlink;
pub mod input;
pub mod ipc;
pub mod memory;
pub mod platform;
pub mod pty;
pub mod renderer;
pub mod screen;
pub mod terminal;
pub mod theme;
pub mod window;
