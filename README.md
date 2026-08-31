<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/primary_images/velox_primary_white_nobg.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/primary_images/velox_primary_dark_nobg.png">
    <img alt="Velox Terminal Logo" src="assets/primary_images/velox_primary_dark_nobg.png" width="450">
  </picture>
</p>

<p align="center">
  <strong>Ultra-fast, GPU-accelerated, lightweight terminal emulator built in Rust.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-v0.2.0-blue.svg?style=for-the-badge" alt="Version 0.2.0">
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-stable-brightgreen.svg?style=for-the-badge&logo=rust" alt="Rust"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=for-the-badge" alt="License"></a>
  <a href="https://platform.linux.org"><img src="https://img.shields.io/badge/platform-Linux%20%7C%20Wayland%20%7C%20X11-informational.svg?style=for-the-badge&logo=linux" alt="Platform"></a>
  <img src="https://img.shields.io/badge/startup-%3C15ms-orange.svg?style=for-the-badge&logo=speedtest" alt="Startup <15ms">
  <img src="https://img.shields.io/badge/fps-120--240-purple.svg?style=for-the-badge" alt="120-240 FPS">
</p>

<p align="center">
  <a href="#key-features">Key Features</a> •
  <a href="#performance-targets">Performance</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#tab-management">Tab Management</a> •
  <a href="#documentation">Documentation</a>
</p>

---

> [!NOTE]
> **Velox v0.2.0** is engineered with zero-compromise performance principles: instant zero-flicker startup under 15ms, OpenGL text rendering, native CPU software fallback via `softbuffer`, multi-tab workflow with per-tab font isolation, single-process IPC architecture, low memory footprint, and clean modular isolation.

---

## Key Features

| Feature | Description | Source Module |
| :--- | :--- | :--- |
| **Dual Rendering Engines** | Hardware OpenGL 3.3+ texture atlas glyph rendering (120–240 FPS) and native pure-Rust CPU software rendering via `softbuffer` with `DamageMap` dirty-row tracking. | [`src/renderer/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/renderer/renderer.rs) & [`src/renderer/software/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/renderer/software/mod.rs) |
| **Multi-Tab Workflows** | Built-in tab bar with `Auto`, `Always`, or `Never` visibility, interactive tab closing/creation, middle-click close, per-tab font size isolation, and customizable tab accent colors. | [`src/app/tab.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/app/tab.rs) |
| **Single-Process IPC** | Run all terminal windows and tabs inside a single background process using display-isolated Unix domain sockets (`velox msg create-window`, `velox msg create-tab`). | [`src/ipc.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/ipc.rs) & [`src/cli.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/cli.rs) |
| **Zero-Flicker Cold Startup** | Cold starts under **15ms** with initially hidden window creation, synchronous first frame presentation, and transparent window protection. | [`src/app/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/app/app.rs) |
| **Infinite Scrollback & Memory Control** | Fast chunked scrollback history with disk paging, bounded RAM usage, and automatic idle memory trimming (PTY inactivity allocator cleanup). | [`src/screen/scrollback.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/screen/scrollback.rs) & [`src/memory.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/memory.rs) |
| **Synthetic Italics & Typography** | Dynamic synthetic italic outline shearing fallback when native italic faces are missing, system font fallbacks via `fontdb`, Nerd Fonts, Powerline prompt glyphs, and PNG color emojis. | [`src/font/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/font/loader.rs) |
| **Rich Text Styling & Underlines** | Single, double, curly, dotted, and dashed underlines with SGR underline color customization, strikethrough, dimming, and bold-as-bright remapping. | [`src/renderer/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/renderer/renderer.rs) & [`src/renderer/software/decorations.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/renderer/software/decorations.rs) |
| **CSI / OSC VT Protocols** | 256-color & 24-bit TrueColor, OSC-7 working dir, OSC-8 explicit hyperlinks, OSC-52 clipboard, OSC-133 shell integration, DECSCUSR cursor shapes, and mouse tracking (X10, SGR 1006, button/drag). | [`src/ansi/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/ansi/) & [`src/terminal/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/terminal/terminal.rs) |
| **Visual Customizations** | Window background opacity / transparency, unfocused window dimming (`window_dim`), customizable cursor colors / text colors, and theme presets. | [`src/config/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/config/config.rs) & [`assets/velox_terminal_themes/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/assets/velox_terminal_themes/) |
| **Wayland & X11 Native** | Linux windowing via Winit with HiDPI support and bundled multi-resolution application icons. | [`src/app/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/app/app.rs) |

---

## Performance Targets

Velox guarantees strict performance metrics across runtime workloads:

| Metric | Target / Benchmark |
| :--- | :--- |
| **Version** | `v0.2.0` |
| **Startup Time** | `< 15ms` |
| **Idle Memory Footprint** | `< 30MB` (Standalone) / `~3–5MB` (IPC sub-window/tab) |
| **Frame Rate** | `120 – 240 FPS` (GPU) / `60 FPS` (Software) |
| **Heap Allocations** | Reused frame vertex buffers & cell buffers; bounded font fallback cache |
| **IPC Creation Latency** | `< 3ms` |

---

## Quick Start

### Prerequisites

- **Rust**: 1.75+ (Stable toolchain)
- **Platform**: Linux (Wayland or X11)
- **Graphics**: OpenGL 3.3+ drivers or CPU software renderer

### 1. Build

Standard Release:
```bash
cargo build --release
```

Production Optimized Build (Fat LTO, stripped binary, panic=abort):
```bash
cargo build --profile optimized-release
```

### 2. Run

Standard Run (Default Single-Process Mode):
```bash
./target/release/velox
```

Single-Instance Window Creation:
```bash
./target/release/velox msg create-window -w ~/projects -t "Project Terminal" -e htop
```

Single-Instance Tab Creation:
```bash
./target/release/velox msg create-tab -w ~/projects -t "Logs"
```

Start Background Daemon Mode:
```bash
./target/release/velox --daemon &
```

Force Native CPU Software Rendering:
Set `gpu_acceleration = false` in `config.toml` or launch with software configuration:
```bash
./target/release/velox
```

---

## Configuration

Velox reads user configuration from `~/.config/velox/config.toml` (or `$XDG_CONFIG_HOME/velox/config.toml`).

```toml
# Top-level settings
shell = "/bin/bash"
app_title = "{program} - Velox"
single_instance = true

[font]
font_family = "ComicShannsMono Nerd Font"
font_scale_multiplier = 1.5
font_size = 11.0
bold_is_bright = true

[window]
scrollback_limit = 2000
infinite_scrollback = true
gpu_acceleration = true
scroll_multiplier = 5.0
fps_limit = 120
padding_x = 8.0
padding_y = 4.0
cursor_shape = "beam"         # "block", "beam", "underline", "hollow_block"
cursor_blink = true
cursor_color = "default"      # "default", "inverted", or hex like "#ffffff"
cursor_text_color = "default" # "default", "inverted", or hex like "#000000"
scroll_on_output = true
scroll_on_keystroke = true
opacity = 1.0                 # 0.0 (transparent) to 1.0 (opaque)
window_dim = 0.0              # 0.0 (normal) to 1.0 (fully dimmed when unfocused)

[tabs]
show_tab_bar = "auto"         # "auto", "always", or "never"
tab_bar_height = 28           # Custom tab bar height in pixels (optional)
show_close_button = true
show_new_tab_button = false
font_size = 10.0              # Custom font size for tab headers (optional)
tab_accent_color = "blue"     # "blue", "magenta", "green", "cyan", "red", "yellow", or hex "#3b8eea"

[colors]
default_fg = "#e0def4"
default_bg = "#191724"
black = "#26233a"
red = "#eb6f92"
green = "#31748f"
yellow = "#f6c177"
blue = "#9ccfd8"
magenta = "#c4a7e7"
cyan = "#ebbcba"
white = "#e0def4"
bright_black = "#6e6a86"
bright_red = "#eb6f92"
bright_green = "#31748f"
bright_yellow = "#f6c177"
bright_blue = "#9ccfd8"
bright_magenta = "#c4a7e7"
bright_cyan = "#ebbcba"
bright_white = "#e0def4"
```

---

## Tab Management

| Action | Shortcut / Interaction |
| :--- | :--- |
| **New Tab** | `Ctrl + Shift + T` or click `+` button |
| **Close Tab** | `Ctrl + Shift + W`, click `×`, or middle-click tab |
| **Next Tab** | `Ctrl + Tab` or `Ctrl + Shift + ]` |
| **Previous Tab** | `Ctrl + Shift + Tab` or `Ctrl + Shift + [` |
| **Switch to Tab N** | `Alt + 1` .. `Alt + 9` |
| **Zoom Tab In/Out** | `Ctrl + Plus` / `Ctrl + Minus` / `Ctrl + 0` (Isolated per tab) |

---

## Architecture Overview

```text
src/
├── main.rs           # Application entry point & CLI routing
├── lib.rs            # Core library root & crate exports
├── cli.rs            # Command line argument parser & action protocol
├── ipc.rs            # Unix domain socket single-instance IPC server & client
├── memory.rs         # Allocator memory trimming & heap compaction helpers
├── app/              # Multi-window orchestrator, tab manager & event loop
│   ├── app.rs        # WindowState, App, and winit ApplicationHandler
│   ├── tab.rs        # Tab, TabBar, hit-testing & tab bar render model
│   ├── keyboard.rs   # Keyboard event translation & shortcuts
│   └── mouse.rs      # Mouse clicks, drags, selections & tab interactions
├── terminal/         # VT state machine, mode flags & CSI/OSC protocol engine
├── screen/           # Character grid, cursor, selection & chunked scrollback
├── renderer/         # Dual rendering backends
│   ├── renderer.rs   # Hardware OpenGL 3.3+ shader atlas renderer
│   └── software/     # Pure-Rust CPU software renderer via softbuffer
├── pty/              # Asynchronous PTY process streams & fork execution
├── input/            # Keymaps, ANSI translations & keyboard helpers
├── ansi/             # High-speed CSI / OSC / DCS / ESC byte parsers
├── font/             # FontDB, resolved font sets, synthetic italics & fallback LRU
├── hyperlink/        # OSC-8 & HTTP(S) URL regex detector & opener
├── clipboard/        # System clipboard integration (Wayland / X11 / OSC-52)
└── theme/            # Color scheme primitives, palette resolver & hex parser
```

---

## Documentation

The detailed project manuals are located inside the `docs/` directory:

- **[Detailed Overview & Features](docs/OVERVIEW.md)** - Full feature list, dependencies, and build configurations.
- **[System Architecture](docs/ARCHITECTURE.md)** - Module hierarchy, rendering pipeline, and state flow.
- **[App Module Specifications](docs/APP_MODULE.md)** - Deep dive into application state machine, tabs, and event handling.
- **[Development Plan & Roadmap](docs/PLAN.md)** - Technical roadmap and implementation targets.

---

## License

This project is licensed under the terms outlined in the [LICENSE](LICENSE) file.
