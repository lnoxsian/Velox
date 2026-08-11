# App Module Documentation

## Overview

The `app` module is the runtime orchestrator for Velox. It owns the Winit application handler, initializes graphics and terminal state, starts the shell inside a PTY, and routes input and custom events through the rest of the system.

## Module Structure

```text
src/app/
├── mod.rs
├── app.rs
├── keyboard.rs
└── mouse.rs
```

## Main Concepts

### App Struct

`App` stores the live application state used by the event loop and renderer.

**Important groups of fields:**
- Windowing and GL state: window, GL context, display, and surface handles
- Terminal state: `Terminal`, renderer, PTY master, and reusable render buffers
- Runtime settings: scroll multiplier, FPS limit, font size, padding, and cursor blink settings
- Interaction state: mouse position, click tracking, focus state, and modifier keys
- Scheduling state: redraw flags, title refresh timing, and cursor blink timing

### CustomEvent

```rust
pub enum CustomEvent {
    PtyData(Vec<u8>),
    PtyExit,
}
```

Custom events let the PTY reader thread deliver terminal data back to the main event loop.

## Lifecycle

### 1. Creation

`App::new()` creates an uninitialized application state with defaults for runtime settings and redraw tracking.

### 2. Resumed

When Winit calls `resumed`, the application:

- Loads user configuration
- Builds the window and OpenGL context
- Creates the renderer
- Initializes the terminal with an initial grid size
- Spawns the configured shell inside a PTY
- Starts the PTY reader thread
- Applies runtime settings like FPS cap, padding, cursor blinking, and scroll multiplier

### 3. Event Processing

The event loop routes three main categories of events:

**Window events**
- Keyboard input is translated into terminal bytes
- Mouse input updates selection, click, and drag state
- Resize events update the viewport and PTY dimensions
- Close requests and focus changes update application state

**Custom events**
- `PtyData` bytes are parsed and fed into the terminal
- `PtyExit` triggers shutdown behavior

**Idle processing**
- Redraws happen when content is dirty or a scheduled refresh is due
- Cursor blinking and title checks are handled on timing intervals

### 4. Shutdown

When the PTY exits or the window closes, the app tears down the event flow and releases the active graphics and terminal resources through normal Rust drop semantics.

## Input Routing

### Keyboard

Keyboard events are translated through `input::keyboard::translate_key` and then written to the PTY master.

### Mouse

Mouse input is tracked in `app::mouse` and used for selection, drag, and scroll behavior.

## Performance Notes

- The app reuses a single render-cell buffer to reduce allocations
- PTY reads happen on a dedicated thread
- Redraws are gated by dirty-state and timing checks
- Cursor blink and title refresh are handled with lightweight timers

## Testing

The app module can be tested by verifying that event routing updates the terminal and PTY state correctly, especially for keyboard input, resize handling, and custom PTY events.

## Future Improvements

- More elaborate mouse protocol support
- Multi-window support
- Tab/session management
- Configuration hot reload
- Richer title and focus handling
