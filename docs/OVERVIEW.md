# Velox Terminal Overview

Velox is a Linux terminal emulator written in Rust. The current codebase focuses on GPU-accelerated rendering, ANSI/VT parsing, PTY management, font fallback, clipboard integration, hyperlink detection, and a configurable terminal state machine.

## Implemented Features

- GPU-backed OpenGL renderer with glyph atlas loading and cell batching
- ANSI/VT parser with CSI, OSC, ESC, and parser state handling
- Shell spawning through a pseudo-terminal master/slave pair
- Keyboard translation for control, alt, navigation, and function keys
- Mouse, selection, scrollback, and damage tracking in the screen grid
- Alternate screen support, bracketed paste, synchronized output, and focus tracking
- Semantic prompt markers and last-command exit code tracking
- Configurable theme, cursor shape, scrollback, padding, FPS cap, and font scaling
- Font fallback discovery across system fonts and common symbol/Nerd Font families
- URL detection plus OSC-8 hyperlink parsing
- Clipboard helpers for Wayland and X11, plus OSC 52 encoding helpers
- Config loading and saving from `~/.config/velox/config.toml`

## Module Tree

```text
src/
├── main.rs
├── ansi/
│   ├── csi.rs
│   ├── esc.rs
│   ├── osc.rs
│   ├── parser.rs
│   └── state.rs
├── app/
│   ├── app.rs
│   ├── keyboard.rs
│   └── mouse.rs
├── clipboard/
│   └── clipboard.rs
├── config/
│   ├── config.rs
│   ├── defaults.rs
│   └── loader.rs
├── font/
│   ├── fallback.rs
│   └── loader.rs
├── hyperlink/
│   ├── detector.rs
│   └── osc8.rs
├── input/
│   └── keyboard.rs
├── pty/
│   ├── master.rs
│   └── process.rs
├── renderer/
│   └── renderer.rs
├── screen/
│   ├── cell.rs
│   ├── cursor.rs
│   ├── damage.rs
│   ├── grid.rs
│   ├── reflow.rs
│   ├── scroll.rs
│   ├── scrollback.rs
│   └── selection.rs
├── terminal/
│   └── terminal.rs
└── theme/
    └── theme.rs
```

## Performance Targets

| Metric | Target |
| :--- | :--- |
| Startup | `< 15ms` |
| Idle memory | `< 30MB` |
| Frame rate | `120-240 FPS` |
| Rendering | Dirty-region driven |
| Allocations | Minimal after initialization |

## Configuration

Velox reads configuration from `~/.config/velox/config.toml` or `$XDG_CONFIG_HOME/velox/config.toml`.

Key settings currently supported by the code include:

- `font_family`
- `font_size`
- `shell`
- `default_fg` and `default_bg`
- `colors`
- `scrollback_limit` and `infinite_scrollback`
- `gpu_acceleration`
- `scroll_multiplier`
- `fps_limit`
- `bold_is_bright`
- `app_title`
- `padding_x` and `padding_y`
- `font_scale_multiplier`
- `cursor_shape`
- `cursor_blink`

## Building

```bash
cargo build --release
cargo build --profile optimized-release
```

## Running

```bash
./target/release/velox
./target/optimized-release/velox
```

## Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [App Module Documentation](APP_MODULE.md)
- [Development Plan](PLAN.md)
