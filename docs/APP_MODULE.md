# App Module Documentation

## Overview

The `app` module is the runtime orchestrator for Velox. It implements the Winit `ApplicationHandler` lifecycle, manages multiple native windows and tabs, configures dual rendering backends (OpenGL and CPU Software), routes keyboard and mouse events, executes single-instance IPC requests, and schedules redraws and idle memory trimming.

## Module Structure

```text
src/app/
├── mod.rs        # App module root and re-exports
├── app.rs        # App, WindowState, WindowRendererBackend & ApplicationHandler
├── tab.rs        # Tab, TabBar, TabHeaderInfo, TabBarRenderInfo & hit-testing
├── keyboard.rs   # Winit keyboard event dispatch, shortcuts & tab navigation
└── mouse.rs      # Mouse tracking, click count, drag selection & tab bar interactions
```

## Main Concepts

### 1. `App` Struct

`App` manages top-level application state across all windows and IPC communication:

- **`windows: HashMap<WindowId, WindowState>`**: Map of active application windows.
- **`gl_display`, `gl_config`, `gl`**: Shared OpenGL display and context resources.
- **`ipc_listener: Option<IpcListenerHandle>`**: Background thread listener for single-instance Unix domain socket requests.
- **`modifiers: ModifiersState`**: Current keyboard modifier key states.
- **`single_instance_mode`, `daemon_mode`**: Process lifecycle configuration.

### 2. `WindowState` & `WindowRendererBackend`

`WindowState` encapsulates all state for a single native window:

- **`backend: WindowRendererBackend`**: Enum dispatching to either:
  - `WindowRendererBackend::OpenGL`: Owns `Renderer`, `gl_surface`, and `gl_context`.
  - `WindowRendererBackend::Software`: Owns `CpuRenderer` and `softbuffer::Surface`.
- **`tabs: Vec<Tab>` & `active_tab_index: usize`**: List of active terminal tabs.
- **`tab_bar: TabBar`**: Visual configuration, dimensions, accent color, and hit-testing cache.
- **`render_cells_buf: Vec<Cell>`**: Reused contiguous cell buffer passed to the renderer.
- **`opacity: f32` & `window_dim: f32`**: Transparency and unfocused dimming factors.
- **`fps_limit: Option<u32>` & `last_frame_instant: Instant`**: Frame rate throttler.
- **`needs_redraw: bool` & `content_dirty: bool`**: Redraw flags for synchronized and damaged rendering.

### 3. `Tab` & `TabBar` (`app/tab.rs`)

- **`Tab`**:
  - `id: u64`: Unique tab identifier.
  - `pty_master: Arc<PtyMaster>`: PTY master descriptor for reading/writing/resizing.
  - `terminal: Terminal`: Isolated VT state machine, alternate screen, and character grids.
  - `font_size: f32`: Per-tab isolated font zoom level.
  - `custom_title: Option<String>` & `current_title: String`: Dynamically updated process/tab title.
  - `hold: bool`: Retains tab open on process exit with `[Process exited]` indicator.
  - `last_activity: Instant` & `last_cleanup: Instant`: Idle memory trimming timestamps.
- **`TabBar`**:
  - `show_tab_bar: TabBarVisibility`: `Auto` (visible when > 1 tab), `Always`, or `Never`.
  - `tab_accent_color: Option<Color>`: Active tab top border color.
  - `show_close_button: bool` & `show_new_tab_button: bool`: Button toggles.
  - `hit_test(x, y, ...)`: Hit-testing for tab selection, tab close buttons, and new-tab button.

### 4. `CustomEvent`

```rust
pub enum CustomEvent {
    PtyData {
        window_id: WindowId,
        tab_id: u64,
        data: Vec<u8>,
    },
    PtyExit {
        window_id: WindowId,
        tab_id: u64,
    },
    IpcCreateWindow {
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        title: Option<String>,
        hold: Option<bool>,
    },
    IpcCreateTab {
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        title: Option<String>,
        hold: Option<bool>,
    },
}
```

---

## Application Lifecycle

### 1. Resumed (`ApplicationHandler::resumed`)

1. Loads user configuration from `~/.config/velox/config.toml`.
2. Initializes OpenGL display/context if `gpu_acceleration = true`.
3. Starts IPC server if in single-instance or daemon mode.
4. Invokes `create_window()` for initial command-line arguments.

### 2. Window Creation (`create_window`)

1. Creates native window with `.with_visible(false)` and `.with_transparent(opacity < 1.0)`.
2. Configures rendering backend (`OpenGL` with `glow` or `Software` with `softbuffer`).
3. Calculates cell dimensions and spawns initial shell process inside PTY.
4. Spawns dedicated background reader thread (`spawn_pty_reader`).
5. Constructs `WindowState` with `last_frame_instant` initialized in the past.
6. **Zero-Flicker Presentation**: Synchronously calls `window_state.draw()` to render background and prompt, then reveals the window via `window_state.window.set_visible(true)`.

### 3. Event Loop & Routing

- **`WindowEvent::KeyboardInput`**: Dispatches to `handle_keyboard_input` for tab shortcuts, clipboard keys (`Ctrl+Shift+C`/`V`), zoom (`Ctrl+Plus`/`Minus`/`0`), and writes translated escape sequences to the active PTY.
- **`WindowEvent::MouseInput` / `CursorMoved` / `MouseWheel`**: Handled by `app::mouse`, managing tab bar clicking, middle-click close, hyperlink opening (`Ctrl+Click`), text selection, and mouse tracking reporting (SGR 1006).
- **`WindowEvent::Resized`**: Calls `resize_renderer()`, adjusts viewport surfaces, recalculates grid columns/rows, and sends `TIOCSWINSZ` to the active PTY.
- **`WindowEvent::RedrawRequested`**: Calls `ws.draw()` to render terminal cells and tab bar.

### 4. Idle Processing (`about_to_wait`)

- **Idle Memory Trimming**: Calls `ws.release_memory()` and `malloc_trim` when 2.5 seconds of PTY inactivity elapse following burst terminal output.
- **Cursor Blinking**: Toggles cursor blink every 500ms when enabled.
- **Frame Rate Throttling**: Checks `fps_limit` and requests redraws only when `now >= last_frame_instant + frame_duration`.

---

## Keyboard Shortcuts

| Shortcut | Description |
| :--- | :--- |
| `Ctrl + Shift + T` | Open new tab in the active window |
| `Ctrl + Shift + W` | Close the active tab |
| `Ctrl + Tab` / `Ctrl + Shift + ]` | Switch to next tab |
| `Ctrl + Shift + Tab` / `Ctrl + Shift + [` | Switch to previous tab |
| `Alt + 1` .. `Alt + 9` | Switch directly to tab 1 through 9 |
| `Ctrl + Shift + C` | Copy current selection to clipboard |
| `Ctrl + Shift + V` | Paste from system clipboard |
| `Ctrl + Plus` / `Ctrl + =` | Zoom in font size (isolated to active tab) |
| `Ctrl + Minus` | Zoom out font size (isolated to active tab) |
| `Ctrl + 0` | Reset font size to default (isolated to active tab) |
| `Shift + PageUp` / `PageDown` | Scroll terminal history up / down |
| `Shift + Home` / `End` | Scroll to top / bottom of history |

