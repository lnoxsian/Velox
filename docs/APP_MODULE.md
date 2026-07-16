# App Module Documentation

## Overview

The `app` module is the core orchestrator of Velox, managing the application lifecycle, event loop, and coordination between subsystems. It implements the `ApplicationHandler` trait from Winit to drive the event loop.

## Module Structure

```
src/app/
├── mod.rs        # Public API exports
├── app.rs        # Main App struct and ApplicationHandler implementation
├── startup.rs    # Application startup/initialization logic
└── shutdown.rs   # Application cleanup/shutdown logic
```

## Main Concepts

### App Struct

The `App` struct is the central hub that holds references to all major subsystems:

```rust
pub struct App {
    event_loop_proxy: winit::event_loop::EventLoopProxy<CustomEvent>,
    modifiers: winit::keyboard::ModifiersState,
    window: Option<Window>,
    gl: Option<Arc<glow::Context>>,
    gl_context: Option<glutin::context::PossiblyCurrentContext>,
    gl_display: Option<glutin::display::Display>,
    gl_surface: Option<glutin::surface::Surface<glutin::surface::WindowSurface>>,
    renderer: Option<Renderer>,
    terminal: Option<Terminal>,
    pty_master: Option<Arc<PtyMaster>>,
    mouse_x: f64,
    mouse_y: f64,
    render_cells_buf: Vec<Cell>,
}
```

**Key Fields:**
- `event_loop_proxy` - Allows posting custom events (e.g., PTY data) to the event loop
- `modifiers` - Current keyboard modifier state (Ctrl, Alt, Shift)
- `window` - Winit window handle
- `gl` - OpenGL context wrapper
- `gl_context/display/surface` - Glutin/OpenGL state
- `renderer` - Text rendering engine
- `terminal` - Terminal state machine
- `pty_master` - Pseudo-terminal manager
- `mouse_x/y` - Current mouse position
- `render_cells_buf` - Reusable buffer for efficient rendering

### CustomEvent

```rust
pub enum CustomEvent {
    PtyData(Vec<u8>),  // Data from pseudo-terminal
    PtyExit,           // PTY process exited
}
```

Custom events allow the PTY thread to communicate with the main event loop without busy-waiting.

### AppError

```rust
#[derive(Debug)]
pub enum AppError {
    Initialization(String),
}
```

Error type for application-level failures.

## Lifecycle

### 1. Creation

```rust
let event_loop = EventLoop::<CustomEvent>::with_user_event().build()?;
let proxy = event_loop.create_proxy();
let mut app = App::new(proxy);
```

The `App` is created with an event loop proxy to enable custom events.

**State:** All subsystems are uninitialized (`None`).

### 2. Resumed (Graphics Context Acquired)

Called when the application window is created and graphics context is available.

**Responsibilities:**
- Create Winit window
- Initialize Glutin display and context
- Create OpenGL context (via glow)
- Create OpenGL surface
- Initialize renderer
- Initialize terminal
- Spawn PTY and shell process
- Start PTY reading thread

**Key Code Paths:**
```
resumed()
├── Create window attributes
├── Create Glutin display/config
├── Create OpenGL surface
├── Create glow context
├── Initialize renderer (compile shaders, create atlas)
├── Initialize terminal
└── Spawn PTY (start reading thread)
```

### 3. Event Processing

The event loop continuously processes events:

**WindowEvent:**
- `KeyboardInput` → Input handler → Terminal keyboard processing → PTY write
- `MouseInput`/`MouseMotion`/`MouseWheel` → Input handler → Terminal mouse processing → PTY write
- `Resized` → Update viewport → Resize PTY
- `RedrawRequested` → Trigger render
- `CloseRequested` → Shutdown

**UserEvent (Custom):**
- `PtyData(bytes)` → ANSI parser → Terminal processing → Screen update
- `PtyExit` → Shutdown

**AboutToWait:**
- If screen has changes (damage) → Render frame

### 4. Shutdown

Called when application exits (window closed, exit command, etc.).

**Responsibilities:**
- Stop PTY reading thread
- Kill shell process
- Cleanup OpenGL resources
- Destroy window

## Key Methods

### `new(proxy)`

Creates a new uninitialized `App`.

**Usage:**
```rust
let app = App::new(proxy);
```

### `initialize()`

Placeholder for initialization logic (currently stubbed).

### `run()`

Placeholder for main execution logic (currently stubbed).

### `render()`

Renders the current frame.

**Logic:**
1. Get dirty regions from screen (damage tracking)
2. Prepare cells for rendering
3. Set OpenGL viewport
4. Clear screen
5. Batch render cells
6. Swap buffers

**Optimization:**
- Only renders if screen has damage
- Uses damage tracking to minimize rendering work

### `shutdown()`

Placeholder for cleanup logic (currently stubbed).

## ApplicationHandler Implementation

The `App` implements Winit's `ApplicationHandler<CustomEvent>` trait:

### `resumed(&mut self, event_loop: &ActiveEventLoop)`

Called when the application is ready to render (window created, context available).

**Initialization Steps:**
1. Create window with default attributes (800x600, titled "Velox Terminal")
2. Build Glutin display with alpha support
3. Create OpenGL context and surface
4. Wrap in Arc for shared ownership
5. Create renderer with OpenGL context
6. Create terminal state machine
7. Spawn shell process with PTY

### Event Handlers

**`window_event()`** - Route window events:
- Keyboard input → Terminal keyboard handler
- Mouse input → Terminal mouse handler
- Window resize → Resize PTY
- Request redraw → Trigger render

**`user_event()`** - Handle custom events:
- `PtyData` → Parse ANSI, update terminal/screen
- `PtyExit` → Shutdown application

**`about_to_wait()`** - Render if needed:
- Check if screen has damage
- Render frame
- Request next redraw

## Event Flow Examples

### Typing a Character

```
1. User presses 'a'
2. Winit detects KeyboardInput event
3. App::window_event() routes to input handler
4. Input handler generates keypress data
5. Data sent to PTY via pty_master.write()
6. Shell process receives data
7. Shell echoes character back to PTY
8. PTY thread detects data available
9. PTY thread sends CustomEvent::PtyData(bytes)
10. App::user_event() receives CustomEvent
11. ANSI parser processes bytes
12. Terminal processes parsed sequences
13. Screen buffer updated
14. Screen marks affected region as "dirty"
15. AboutToWait detects dirty screen
16. render() called
17. Renderer batches updated cells
18. OpenGL draws frame
19. Result appears on screen
```

### Window Resize

```
1. User resizes window
2. Winit sends WindowEvent::Resized
3. App::window_event() handles resize
4. Viewport/projection updated
5. PTY is resized to new dimensions
6. Shell process receives SIGWINCH
7. Shell's terminal size changes
8. Applications respond to new size
```

## Thread Model

### Main Thread
- Handles all event loop processing
- Performs all rendering
- Manages OpenGL state

### PTY Reading Thread
- Spawned in `resumed()`
- Continuously reads from PTY master
- Posts `CustomEvent::PtyData` events
- Detects PTY exit and posts `CustomEvent::PtyExit`

**Thread Safety:**
- Uses `EventLoopProxy` for thread-safe event posting
- PTY master access protected by `Arc`
- No shared mutable state between threads

## Key Invariants

1. **Uninitialized State Handling** - All subsystems start as `None`, only become `Some` after `resumed()`
2. **Single-Threaded Rendering** - All OpenGL calls happen on main thread
3. **Event Loop Ownership** - App owns references to subsystems but event loop owns App
4. **Custom Event Channel** - PTY thread communicates only via EventLoopProxy

## Performance Characteristics

- **Latency** - Minimal (event-driven)
- **CPU Usage** - Low (epoll-based I/O, no busy-waiting)
- **Memory** - Reuses buffers where possible (e.g., `render_cells_buf`)

## Testing

The app module can be tested by:

1. Creating mock subsystems (renderer, terminal, pty)
2. Simulating window events
3. Verifying correct routing and state changes

Example:
```rust
#[test]
fn test_keyboard_routing() {
    let proxy = create_mock_proxy();
    let mut app = App::new(proxy);
    // Simulate keyboard event
    // Verify terminal received input
}
```

## Future Improvements

- Async PTY handling (replace thread with async)
- Multiple window support
- Tab support
- Session management
- Configuration hot-reload
- Plugin/extension system
