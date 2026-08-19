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
  <img src="https://img.shields.io/badge/version-v0.1.6-blue.svg?style=for-the-badge" alt="Version 0.1.6">
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
  <a href="#documentation">Documentation</a>
</p>

---

> [!NOTE]
> **Velox v0.1.6** is engineered with zero-compromise performance principles: instant startup under 15ms, OpenGL text rendering, single-process IPC architecture, low memory footprint, and clean modular isolation.

---

## Key Features

| Feature | Description | Source Module |
| :--- | :--- | :--- |
| **GPU Accelerated Rendering** | OpenGL 3.3+ texture atlas glyph rendering delivering 120–240 FPS text display. | [`src/renderer/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/renderer/renderer.rs) |
| **Software CPU Fallback** | Automated Mesa LLVMpipe software rasterization and 60 FPS capping when `gpu_acceleration = false`. | [`src/main.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/main.rs) |
| **Single-Process IPC** | Run all terminal windows inside a single process using display-isolated Unix domain sockets (`velox msg create-window`). | [`src/ipc.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/ipc.rs) & [`src/cli.rs`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/cli.rs) |
| **Blazing Fast Startup** | Cold starts under **15ms** with idle memory footprint under **30MB** (down to **~3–5MB** per window in IPC mode). | [`src/app/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/app/app.rs) |
| **Async Non-Blocking PTY** | Dedicated PTY reader threads with support for custom command execution (`-e`), working directory (`-w`), and `--hold`. | [`src/pty/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/pty/process.rs) |
| **Font Fallback & Emoji Support** | System font fallbacks via `fontdb`, Nerd Fonts, Powerline prompt separators, PUA icons, and PNG color emojis. | [`src/font/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/font/loader.rs) |
| **CSI / OSC VT Protocols** | 256-color & 24-bit TrueColor, OSC-7 working dir, OSC-8 hyperlinks, OSC-52 clipboard, OSC-133 shell integration, and DECSCUSR cursor shapes. | [`src/ansi/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/ansi/) & [`src/terminal/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/terminal/terminal.rs) |
| **Hyperlinks & Text Selection** | Interactive OSC-8 / HTTP(S) regex link detection, word/line text selection, SGR 1006 mouse mode, and clipboard integration (`Ctrl+Shift+C`/`V`). | [`src/hyperlink/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/hyperlink/detector.rs) |
| **TOML Config Engine** | Configurable font stack, font size, scrollback limit, FPS cap, padding, cursor blink, color schemes, and single-instance toggle. | [`src/config/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/config/config.rs) |
| **Wayland & X11 Native** | Linux windowing via Winit with HiDPI support and bundled RGBA application icon. | [`src/app/`](file:///home/lnoxsian/lnox-files/project-rust/Velox/src/app/app.rs) |

---

## Performance Targets

Velox guarantees strict performance metrics across runtime workloads:

| Metric | Target / Benchmark |
| :--- | :--- |
| **Version** | `v0.1.6` |
| **Startup Time** | `< 15ms` |
| **Idle Memory Footprint** | `< 30MB` (Standalone) / `~3–5MB` (IPC sub-window) |
| **Frame Rate** | `120 – 240 FPS` (GPU) / `60 FPS` (Software) |
| **Heap Allocations** | Reused frame vertex buffers & cell buffers |
| **IPC Creation Latency** | `< 3ms` |

---

## Quick Start

### Prerequisites

- **Rust**: 1.70+ (Stable toolchain)
- **Platform**: Linux (Wayland or X11)
- **Graphics**: OpenGL 3.3+ drivers or Mesa Software Rasterizer

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

Single-Instance Window Creation (Alacritty style):
```bash
./target/release/velox msg create-window -w ~/projects -t "Project Terminal" -e htop
```

Start Background Daemon Mode:
```bash
./target/release/velox --daemon &
```

Force Software (CPU) Rendering:
```bash
LIBGL_ALWAYS_SOFTWARE=1 ./target/release/velox
```

---

## Configuration

Velox reads user configuration from `~/.config/velox/config.toml` (or `$XDG_CONFIG_HOME/velox/config.toml`).

```toml
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
cursor_shape = "beam"
cursor_blink = true
opacity = 1.0

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

## Architecture Overview

```text
src/
├── main.rs           # Application entry point & CLI routing
├── app/              # Multi-window orchestrator & event loops
├── cli.rs            # Command line argument parser & action protocol
├── ipc.rs            # Unix domain socket single-instance IPC server
├── terminal/         # VT state machine & CSI/OSC protocol engine
├── screen/           # Character grid, selection & scrollback history
├── renderer/         # OpenGL text quad renderer & shader pipeline
├── pty/              # Asynchronous PTY process streams & fork execution
├── input/            # Keymaps, key translations & mouse handlers
├── ansi/             # High-speed CSI / OSC / DCS parser
├── font/             # FontDB, glyph rasterizer, atlas & fallbacks
├── hyperlink/        # OSC-8 & HTTP(S) URL regex detector
├── clipboard/        # System clipboard integration
└── theme/            # Color scheme primitives & hex parser
```

---

## Documentation

The detailed project manuals are located inside the `docs/` directory:

- **[Detailed Overview & Features](docs/OVERVIEW.md)** - Full feature list, dependencies, and build configurations.
- **[System Architecture](docs/ARCHITECTURE.md)** - Module hierarchy, rendering pipeline, and state flow.
- **[App Module Specifications](docs/APP_MODULE.md)** - Deep dive into application state machine and event handling.
- **[Development Plan & Roadmap](docs/PLAN.md)** - Technical roadmap and implementation targets.

---

## License

This project is licensed under the terms outlined in the [LICENSE](LICENSE) file.
