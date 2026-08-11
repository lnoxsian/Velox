# Velox Architecture Documentation

## Overview

Velox is a modular terminal emulator built around a small set of concrete Rust modules. The current architecture centers on an event-driven application loop, a pseudo-terminal backend, a terminal state machine, and a GPU-backed renderer.

## Architectural Principles

```text
1. Each module owns one responsibility
2. Keep dependencies narrow and explicit
3. Avoid unnecessary abstraction
4. Prefer data-oriented state over hidden global state
5. Make modules independently testable
6. Favor stack allocation and reusable buffers
7. Use enums and bitflags where they keep control flow clear
```

## Component Overview

### Application Layer

#### `app::App`
`App` is the Winit `ApplicationHandler` implementation. It owns the window, GL context, renderer, terminal, PTY master, and the runtime state used for redraw scheduling, cursor blinking, mouse handling, and title updates.

**Responsibilities:**
- Create the window and OpenGL context on resume
- Load configuration and initialize runtime settings
- Spawn the shell inside a PTY
- Route window, keyboard, mouse, and custom PTY events
- Render dirty regions and manage redraw cadence

### ANSI Parsing (`ansi::`)

The ANSI layer translates PTY byte streams into terminal actions.

**Key Components:**
- `parser.rs` - byte-stream parser and dispatch
- `csi.rs` - CSI command handling
- `osc.rs` - OSC command handling
- `esc.rs` - ESC sequence handling
- `state.rs` - parser state tracking

**Responsibilities:**
- Consume raw PTY bytes
- Detect complete escape sequences
- Update the terminal state machine
- Preserve partial sequences across reads

### Terminal State (`terminal::`)

The terminal module owns the visible and alternate screen grids, current colors, cursor modes, bracketed paste state, synchronized output state, and semantic prompt metadata.

**Responsibilities:**
- Track cursor and color state
- Apply ANSI commands to the grid
- Switch between normal and alternate screen buffers
- Manage prompt marks and command exit metadata
- Format pasted text for bracketed paste mode

### Screen Buffer (`screen::`)

The screen layer stores cells, cursor state, scrollback, selection state, and damage tracking.

**Key Components:**
- `cell.rs` - cell data, color representation, and style flags
- `cursor.rs` - cursor shape and visibility
- `damage.rs` - dirty-row tracking
- `grid.rs` - grid storage and character placement
- `reflow.rs` - grid reflow helpers
- `scroll.rs` - scroll helpers
- `scrollback.rs` - scrollback history
- `selection.rs` - text selection state

**Responsibilities:**
- Store characters and styling information
- Handle wide characters and combining sequences
- Track damaged rows for efficient redraws
- Maintain scrollback and selection state

### Rendering (`renderer::`)

The renderer turns terminal cells into GPU draw calls.

**Responsibilities:**
- Manage the GL program, buffers, and vertex data
- Load fonts and glyphs through the font loader
- Draw color and monochrome glyphs
- Handle block and box-drawing characters efficiently
- Respect bold-bright and dim color handling

### PTY Management (`pty::`)

PTY code handles shell spawning and terminal I/O.

**Key Components:**
- `master.rs` - PTY master read/write/resize helpers
- `process.rs` - shell spawning and PTY setup

**Responsibilities:**
- Create the PTY pair
- Spawn the configured shell or fallback shell
- Read and write terminal data
- Resize the terminal when the window changes size

### Input Handling (`input::`)

The input layer converts Winit keyboard events into terminal byte sequences.

**Responsibilities:**
- Translate printable keys and control combinations
- Map navigation and function keys to ANSI sequences
- Apply Alt-modified escape prefixes
- Respect cursor-key application mode

### Font Management (`font::`)

The font layer discovers system fonts and falls back to families that can render missing glyphs.

**Responsibilities:**
- Load fonts from the system database
- Cache discovered fallback faces
- Prefer symbol and Nerd Font families for special glyphs
- Fall back to other system fonts for uncovered characters

### Hyperlink Support (`hyperlink::`)

The hyperlink module provides simple URL detection and OSC-8 parsing.

**Responsibilities:**
- Detect `http`, `https`, `mailto`, and `file` URLs in text
- Parse OSC-8 parameters and hyperlink targets
- Open URLs through the system handler on Linux

### Clipboard Support (`clipboard::`)

Clipboard helpers integrate with Wayland and X11 tools.

**Responsibilities:**
- Copy text through `wl-copy`, `xclip`, or `xsel`
- Read paste and primary selection content
- Encode and decode OSC 52 payloads

### Theme Management (`theme::`)

The theme module stores ANSI colors and default foreground/background colors.

**Responsibilities:**
- Provide default terminal colors
- Preserve initial colors for reset behavior
- Resolve 16-color and 256-color palette values

### Configuration (`config::`)

Configuration loading and saving is TOML-based and file-backed.

**Responsibilities:**
- Load config from the user config directory
- Fall back to defaults when no config file exists
- Persist the default config on first run
- Parse colors, font settings, cursor options, and performance settings

## Data Flow

### Rendering Pipeline

```text
PTY bytes -> ANSI parser -> Terminal state -> Screen grid -> Renderer -> OpenGL -> Display
```

### Input Pipeline

```text
Winit event -> App -> input::keyboard -> PTY master -> shell process
```

### Event Loop

```text
1. Winit creates the application handler
2. App resumes and initializes graphics, terminal state, and PTY
3. Keyboard, mouse, resize, and custom PTY events are routed through App
4. PTY data is parsed into terminal state updates
5. Dirty rows are rendered on the next redraw opportunity
6. Shutdown happens when the PTY exits or the window closes
```

## Performance Considerations

- Reuse the render cell buffer instead of allocating per frame
- Use dirty-row damage tracking to minimize redraw work
- Keep the PTY reader on a background thread and communicate through events
- Cache fallback font lookups so missing glyphs are not rediscovered repeatedly
- Keep grid and selection state local to the terminal and screen modules

## Extension Points

- Add more escape-sequence handling in `ansi::`
- Extend keyboard and mouse mapping in `input::` and `app::`
- Add richer terminal modes to `terminal::`
- Expand font discovery and shaping logic in `font::`
- Add more clipboard or URL handling in `clipboard::` and `hyperlink::`

## Testing Strategy

Each module should remain independently testable through focused unit tests. Existing tests already cover config parsing, font fallback lookup, URL detection, OSC-8 parsing, screen grid behavior, selection handling, and terminal state behavior.

## Future Improvements

- Better shaping and font selection for complex scripts
- More input and mouse protocol coverage
- Additional hyperlink metadata handling
- Further renderer optimization and batching improvements
- Broader terminal protocol coverage
