# Velox Architecture Documentation

## Overview

Velox is a modular terminal emulator designed with strict architectural principles to ensure maintainability, testability, and performance. This document outlines the high-level architecture and relationships between major components.

## Architectural Principles

```
1. Every module owns exactly one responsibility
2. No circular dependencies between modules
3. No global mutable state
4. Every module testable independently
5. Every subsystem replaceable
6. No unnecessary abstraction
7. No runtime polymorphism unless required
8. Prefer enums over trait objects
9. Prefer stack allocation
10. Avoid Arc/Mutex unless profiling justifies them
```

## Component Overview

### Application Layer

#### `app::App`
The main application struct implementing `ApplicationHandler` from Winit. Orchestrates the event loop and coordinates subsystems.

**Responsibilities:**
- Manage event loop lifecycle (startup, shutdown, events)
- Coordinate between window, terminal, and renderer
- Handle user input events (keyboard, mouse)
- Maintain application state

**Key Fields:**
- `window` - Winit window handle
- `gl` - OpenGL context
- `renderer` - Text renderer
- `terminal` - Terminal state machine
- `pty_master` - PTY I/O manager

### Window Management (`window::`)

Handles window creation, sizing, and DPI-aware rendering.

**Key Components:**
- `window.rs` - Window creation and configuration
- `event_loop.rs` - Event loop integration
- `resize.rs` - Window resize handling
- `dpi.rs` - DPI scaling calculations

**Responsibilities:**
- Create and manage Winit window
- Handle resize events and viewport updates
- Calculate DPI scaling for rendering
- Manage physical vs logical pixels

### Terminal Emulation (`terminal::`)

Implements the terminal state machine that interprets commands and updates terminal state.

**Key Components:**
- `terminal.rs` - Main terminal struct
- `state.rs` - Terminal mode flags and state
- `commands.rs` - Command processing
- `keyboard.rs` - Keyboard event handling
- `mouse.rs` - Mouse event handling

**Responsibilities:**
- Process ANSI escape sequences
- Maintain terminal state (modes, cursor position, attributes)
- Execute terminal commands (cursor movement, scrolling, etc.)
- Handle input events and convert to PTY data

**State Management:**
- Terminal modes (insert/replace, origin, wraparound, etc.)
- Cursor position and attributes
- Selection state
- Tab stops

### Screen Buffer (`screen::`)

Manages the visible grid of characters and scrollback buffer.

**Key Components:**
- `grid.rs` - Core character grid data structure
- `cell.rs` - Individual cell attributes (color, bold, etc.)
- `cursor.rs` - Cursor state and positioning
- `damage.rs` - Tracks changed regions for efficient rendering
- `scrollback.rs` - Scrollback history management
- `selection.rs` - Text selection state

**Responsibilities:**
- Store character data and attributes
- Track which regions have changed (dirty tracking)
- Maintain scrollback history
- Manage text selection for copy/paste
- Provide efficient access patterns for rendering

**Optimization:**
- Uses `SmallVec` for cell storage
- Damage tracking minimizes rendering work
- Efficient memory layout for cache locality

### Rendering Engine (`renderer::`)

GPU-accelerated text rendering using OpenGL.

**Key Components:**
- `renderer.rs` - Main renderer struct
- `atlas.rs` - Glyph atlas management
- `glyph.rs` - Individual glyph rendering
- `batch.rs` - Batching for efficient rendering
- `shader.rs` - GLSL shader management
- `frame.rs` - Frame preparation and composition
- `gl.rs` - OpenGL abstraction layer
- `damage.rs` - Damage-aware rendering

**Responsibilities:**
- Load and cache glyphs in texture atlas
- Batch similar glyphs for efficient drawing
- Manage OpenGL state and buffers
- Apply color and effects (bold, underline, etc.)
- Only render dirty regions (damage-driven)

**Performance Features:**
- Texture atlas with LRU eviction
- Geometry batching (minimize draw calls)
- Viewport clipping
- Dirty region tracking

### PTY Management (`pty::`)

Handles pseudo-terminal creation, I/O, and shell process management.

**Key Components:**
- `master.rs` - PTY master side
- `slave.rs` - PTY slave side
- `process.rs` - Shell process spawning
- `epoll.rs` - Efficient I/O polling
- `shell.rs` - Shell detection and configuration

**Responsibilities:**
- Create pseudo-terminal pair
- Spawn shell process (bash, zsh, etc.)
- Handle PTY I/O with minimal allocations
- Resize PTY on window resize
- Manage process lifecycle

**I/O Pattern:**
- Uses `epoll` for efficient event-driven I/O
- Async reading from PTY
- Non-blocking writes
- Integrates with Winit event loop via custom events

### Input Handling (`input::`)

Processes keyboard and mouse input events.

**Key Components:**
- `keyboard.rs` - Keyboard event processing
- `mouse.rs` - Mouse event processing
- `bindings.rs` - Key binding configuration

**Responsibilities:**
- Convert Winit keyboard events to terminal sequences
- Handle modifier keys (Ctrl, Alt, Shift)
- Process mouse events (click, drag, scroll)
- Apply user-configured key bindings
- Generate appropriate ANSI sequences

**Sequences Generated:**
- CSI sequences for special keys
- Modifier-key combinations
- Mouse events (button, motion, scroll)

### ANSI Parser (`ansi::`)

Parses ANSI/VT escape sequences from PTY data.

**Key Components:**
- `parser.rs` - Main parser state machine
- `csi.rs` - CSI (Control Sequence Introducer) handling
- `osc.rs` - OSC (Operating System Command) handling
- `dcs.rs` - DCS (Device Control String) handling
- `esc.rs` - ESC sequence handling
- `state.rs` - Parser state machine

**Responsibilities:**
- Parse incoming byte stream from PTY
- Extract complete escape sequences
- Generate parsed events for terminal to consume
- Handle incomplete sequences across buffers

**Sequences Supported:**
- CSI - Cursor movement, text attributes, etc.
- OSC - Terminal operations (title, hyperlinks, etc.)
- DCS - Device control (sixel, etc.)
- Simple escape sequences

### Font Management (`font::`)

Loads and caches fonts with intelligent fallback.

**Key Components:**
- `loader.rs` - Font loading from system
- `cache.rs` - Glyph cache with LRU eviction
- `atlas.rs` - Texture atlas management
- `fallback.rs` - Font fallback logic

**Responsibilities:**
- Load TrueType/OpenType fonts from system
- Cache rendered glyphs efficiently
- Fallback to alternative fonts for missing glyphs
- Manage texture atlas for GPU storage

**Fallback Strategy:**
1. Primary font
2. Configured fallback fonts
3. System emoji/symbol fonts
4. Box-drawing characters

### Theme Management (`theme::`)

Color scheme and styling support.

**Key Components:**
- `theme.rs` - Theme struct
- `builtin.rs` - Built-in color schemes

**Responsibilities:**
- Store color palette (foreground, background, 256-color palette)
- Provide theme switching
- Validate color configurations
- Apply ANSI color codes

**Features:**
- Support for 16-color, 256-color, and truecolor (24-bit)
- Built-in themes (common terminal color schemes)
- User configuration support

### Configuration (`config::`)

TOML-based configuration loading and validation.

**Key Components:**
- `config.rs` - Main config struct
- `loader.rs` - File loading
- `validator.rs` - Validation logic
- `defaults.rs` - Default values

**Responsibilities:**
- Parse TOML configuration files
- Validate configuration values
- Provide reasonable defaults
- Handle configuration errors gracefully

**Configuration Areas:**
- Window size and position
- Font selection
- Color theme
- Key bindings
- Performance tuning
- Platform-specific options

### Search Functionality (`search::`)

Find and highlight text in the terminal buffer.

**Key Components:**
- `finder.rs` - Search algorithm
- `regex.rs` - Regex pattern support
- `highlight.rs` - Highlighting matched text

**Responsibilities:**
- Search terminal buffer for text/regex
- Highlight matches
- Navigate between matches
- Maintain search state

### Hyperlink Support (`hyperlink::`)

OSC-8 protocol support and URL detection.

**Key Components:**
- `detector.rs` - Regex-based URL detection
- `osc8.rs` - OSC-8 protocol handling
- `mod.rs` - Hyperlink state management

**Responsibilities:**
- Parse OSC-8 hyperlink sequences
- Auto-detect URLs in text
- Store hyperlink metadata
- Provide click handling

### Platform-Specific Code (`platform::`)

Linux-specific implementation details.

**Key Components:**
- `linux.rs` - Linux common code
- `wayland.rs` - Wayland support
- `x11.rs` - X11 support

**Responsibilities:**
- Platform initialization
- Windowing system integration
- Clipboard access
- System integration

### Utilities (`utils::`)

Common utilities and performance monitoring.

**Key Components:**
- `logger.rs` - Logging setup
- `fps.rs` - FPS counter
- `timer.rs` - Performance timing
- `ringbuffer.rs` - Ring buffer data structure
- `allocator.rs` - Memory allocation tracking
- `utf8.rs` - UTF-8 utilities

## Data Flow

### Rendering Pipeline

```
1. PTY Data → ANSI Parser
2. ANSI Parser → Terminal (state machine)
3. Terminal → Screen (buffer update)
4. Screen + Damage → Renderer
5. Renderer (batching, glyph caching) → OpenGL
6. OpenGL → GPU → Display
```

### Input Pipeline

```
1. Winit Event → App
2. App → Input Handler
3. Input Handler → Terminal Keyboard/Mouse
4. Terminal → PTY Master
5. PTY Master → Shell Process
```

### Event Loop

```
1. Winit event_loop.run_app(&mut app)
2. Resumed: Initialize graphics, PTY, renderer
3. WindowEvent: Route to input/window handlers
4. UserEvent (PtyData): Process PTY data
5. AboutToWait: Render frame if dirty
6. LoopExiting: Cleanup
```

## Performance Considerations

### Memory

- Prefer `SmallVec` for commonly-small collections
- Stack allocate when possible
- Reuse buffers (e.g., `render_cells_buf`)
- Lazy allocation (allocate on first use)

### CPU

- Damage tracking to minimize rendering work
- Glyph caching to avoid re-rendering
- Batch rendering to reduce draw calls
- SIMD UTF-8 parsing where beneficial
- epoll for efficient I/O

### GPU

- Texture atlas for efficient glyph storage
- Batching for draw call reduction
- Viewport clipping
- Only render dirty regions

## Extension Points

### Adding a New Module

1. Create module directory under `src/`
2. Create `mod.rs` with public interface
3. Add module declaration in `src/main.rs`
4. Ensure no circular dependencies
5. Follow architectural principles

### Replacing Components

- Renderer: Implement same interface, swap in `App::new()`
- PTY: Implement `PtyMaster` interface
- Font Loader: Implement font loading trait
- Config Source: Extend config loader

## Testing Strategy

Each module should be independently testable:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature() {
        // Test module without dependencies
    }
}
```

## Future Improvements

- Profile-guided optimization
- SIMD operations
- GPU-accelerated font rasterization
- Async/await refactoring
- Plugin system
