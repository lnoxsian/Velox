# Velox Terminal Overview

A modern, high-performance terminal emulator built in Rust with GPU acceleration, minimal allocations, and a clean modular architecture.

## Features

- **GPU Accelerated Rendering** - OpenGL-based glyph atlas rendering for smooth, fast text display
- **High Performance** - Startup in <15ms, idle memory <30MB, 120-240 FPS rendering
- **Async PTY** - Non-blocking pseudo-terminal handling with epoll-driven I/O
- **Modular Architecture** - Clean separation of concerns with zero circular dependencies
- **Linux Support** - Wayland and X11 support via Winit
- **ANSI/VT Support** - Full ANSI escape sequence parsing and terminal command support
- **Font Fallback** - Intelligent font fallback system with glyph caching
- **Hyperlink Detection** - OSC-8 hyperlink protocol support with regex-based detection
- **Search Functionality** - Built-in find/search with regex support and highlighting
- **Configuration** - TOML-based configuration with validation and defaults
- **Clipboard Integration** - Full clipboard read/write support

## Architecture

Velox is designed following strict architectural principles:

```text
Every module owns exactly one responsibility.
No circular dependencies.
No global mutable state.
Every module testable independently.
Every subsystem replaceable.
Prefer enums over trait objects.
Prefer stack allocation over heap allocation.
```

### Core Modules

- **app** - Main application loop and state management
- **window** - Window management, DPI handling, and event processing
- **terminal** - Terminal state machine and command processing
- **screen** - Grid-based character buffer, damage tracking, and scrollback
- **renderer** - OpenGL-based text rendering with batching and caching
- **pty** - Pseudo-terminal spawning, I/O, and process management
- **input** - Keyboard and mouse input processing with key bindings
- **ansi** - ANSI escape sequence parsing (CSI, OSC, DCS, ESC)
- **font** - Font loading, glyph caching, and font fallback
- **theme** - Color scheme management and built-in themes
- **config** - Configuration loading, validation, and defaults
- **search** - Search/find functionality with regex support
- **hyperlink** - Hyperlink detection and OSC-8 protocol
- **platform** - Platform-specific code (Linux, Wayland, X11)
- **utils** - Utility functions (logger, FPS counter, ringbuffer, etc.)

## Building

### Requirements

- Rust (stable)
- Linux with X11 or Wayland
- OpenGL 4.1+

### Build

```bash
cargo build --release
```

### Production Optimized Build

For maximum performance, minimal binary size, and full production optimizations (Fat LTO, codegen-units=1, panic=abort, strip):

```bash
cargo build --profile optimized-release
```

### Run

Standard release:
```bash
./target/release/velox
```

Optimized release:
```bash
./target/optimized-release/velox
```

## Configuration

Velox reads configuration from `~/.config/velox/config.toml` (or `$XDG_CONFIG_HOME/velox/config.toml`).

Example configuration:

```toml
font_family = "monospace"
font_size = 14.0
# shell = "/bin/bash" # Optional: defaults to your system's $SHELL if omitted
scrollback_limit = 1000

# Enable/Disable GPU acceleration (setting to false enables software rendering)
gpu_acceleration = true

# Scroll sensitivity multiplier for mouse scrolling (e.g. 2.0 for faster scroll, 0.5 for slower)
scroll_multiplier = 1.0

# Frame rate limit (e.g. 60, 120, 144, 240, or 0 for uncapped)
fps_limit = 120

# Window padding margins (in pixels)
padding_x = 8.0
padding_y = 4.0

# Font size scaling multiplier
font_scale_multiplier = 1.5

# Enable or disable cursor blinking (default is true)
cursor_blink = true
```

## Performance Targets

- **Startup Time**: <15ms
- **Idle Memory**: <30MB
- **Rendering**: 120-240 FPS
- **Parsing**: Millions of characters/sec
- **Allocations**: Near-zero after startup
- **Rendering**: Dirty-region only

## Project Structure

```text
src/
├── main.rs           # Entry point
├── app/              # Application lifecycle
│   ├── app.rs        # Main App struct and event handler
│   ├── startup.rs    # Initialization logic
│   └── shutdown.rs   # Cleanup logic
├── window/           # Window management
├── terminal/         # Terminal state machine
├── screen/           # Display buffer and grid
├── renderer/         # OpenGL rendering
├── pty/              # Pseudo-terminal I/O
├── input/            # Input handling
├── ansi/             # ANSI parser
├── font/             # Font management
├── theme/            # Color schemes
├── config/           # Configuration
├── search/           # Search functionality
├── hyperlink/        # Link detection
├── platform/         # Platform-specific code
└── utils/            # Utilities
```

## Development

### Logging

Enable logging with:

```bash
RUST_LOG=debug cargo run
```

### Testing

```bash
cargo test
```

## Design Philosophy

Velox prioritizes:

1. **Performance** - Every decision considers runtime and memory impact
2. **Simplicity** - No unnecessary abstractions or over-engineering
3. **Modularity** - Each component is independently testable and replaceable
4. **Correctness** - Proper ANSI/VT sequence support and terminal semantics
5. **Maintainability** - Clear code organization and minimal dependencies

## Dependencies

Core dependencies are kept minimal (~15) to maintain code clarity and reduce build times:

- **winit** - Cross-platform window/event handling
- **glow** - OpenGL bindings
- **glutin** - OpenGL context management
- **nix** - Unix system calls
- **fontdb** - Font database
- **toml** - Configuration parsing
- **serde** - Serialization

## License

See [LICENSE](../LICENSE) file.

## Documentation

For detailed architecture and module documentation, see:

- [Architecture Overview](ARCHITECTURE.md)
- [App Module Documentation](APP_MODULE.md)
- [Design Plan](PLAN.md)
