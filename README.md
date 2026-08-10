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
> Velox is engineered with zero-compromise performance principles: instant startup under 15ms, dirty-region GPU rendering, minimal allocations after boot, and clean modular isolation.

---

## Key Features

| Feature | Description |
| :--- | :--- |
| **GPU Accelerated Rendering** | OpenGL-based glyph atlas rendering powering ultra-smooth 120–240 FPS text rendering. |
| **Blazing Fast Startup** | Cold starts under **15ms** with an idle memory footprint under **30MB**. |
| **Async Non-Blocking PTY** | Epoll-driven I/O loop designed for zero-latency terminal input and output streaming. |
| **Font Fallback & Glyph Cache** | Intelligent font fallback system supporting complex Unicode symbols and custom font stacks. |
| **OSC-8 Hyperlinks** | Built-in regex detection and clickable OSC-8 hyperlink protocol integration. |
| **Interactive Regex Search** | Instant full-buffer find and search capability with active regex match highlighting. |
| **Theme & Config Engine** | Live-reloadable TOML configuration with built-in color palette options. |
| **Wayland & X11 Native** | Seamless windowing support across modern Linux desktop environments via Winit. |

---

## Performance Targets

Velox guarantees strict performance metrics across runtime workloads:

| Metric | Target / Benchmark |
| :--- | :--- |
| **Startup Time** | `< 15ms` |
| **Idle Memory Footprint** | `< 30MB` |
| **Frame Rate** | `120 – 240 FPS` |
| **Throughput** | `Millions of chars / sec` |
| **Heap Allocations** | Near-zero after initialization |
| **Redraw Efficiency** | Dirty-region tracking only |

---

## Quick Start

### Prerequisites

- **Rust**: 1.70+ (Stable toolchain)
- **Platform**: Linux (Wayland or X11)
- **Graphics**: OpenGL 4.1+ drivers

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

```bash
# Standard Build
./target/release/velox

# Optimized Release Build
./target/optimized-release/velox
```

---

## Configuration

Velox reads user configuration from `~/.config/velox/config.toml` (or `$XDG_CONFIG_HOME/velox/config.toml`).

```toml
font_family = "monospace"
font_size = 14.0
scrollback_limit = 1000

# GPU Acceleration (set to false for software fallback)
gpu_acceleration = true

# Scroll Sensitivity & FPS Cap
scroll_multiplier = 1.0
fps_limit = 120

# Window Margin Padding (pixels)
padding_x = 8.0
padding_y = 4.0

# Cursor Animation
cursor_blink = true
```

---

## Architecture Overview

Velox adheres to strict architectural isolation:

```text
src/
├── main.rs           # Application Entry point
├── app/              # Lifecycle management & event loops
├── window/           # Windowing, DPI & surface scaling
├── terminal/         # State machine & VT command engine
├── screen/           # Character grid, damage tracking & scrollback
├── renderer/         # OpenGL text & quad renderer
├── pty/              # Asynchronous PTY process streams
├── input/            # Keymaps, bindings & mouse events
├── ansi/             # High-speed CSI / OSC / DCS parser
├── font/             # Font DB, glyph rasterizer & atlas
├── theme/            # Color scheme primitives
└── search/           # Full-text buffer search engine
```

> [!TIP]
> For in-depth architectural design, execution lifecycle diagrams, and module internals, refer to the documents linked below.

---

## Documentation

The detailed project manuals have been organized inside the `docs/` directory:

- **[Detailed Overview & Features](docs/OVERVIEW.md)** - Full feature list, dependencies, and build configurations.
- **[System Architecture](docs/ARCHITECTURE.md)** - Module hierarchy, rendering pipeline, and state flow.
- **[App Module Specifications](docs/APP_MODULE.md)** - Deep dive into application state machine and event handling.
- **[Development Plan & Roadmap](docs/PLAN.md)** - Technical roadmap and implementation targets.

---

## License

This project is licensed under the terms outlined in the [LICENSE](LICENSE) file.
