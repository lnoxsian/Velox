# Velox Roadmap

## Current Implementation (v0.1.9)

Velox ships a modular, high-performance terminal stack in `src/`:

```text
src/
├── main.rs           # Entry point & CLI routing
├── lib.rs            # Core library root
├── cli.rs            # CLI argument parser & subcommands
├── ipc.rs            # Single-instance Unix socket IPC server/client
├── memory.rs         # Allocator memory trimming & compaction
├── ansi/             # High-speed CSI / OSC / DCS / ESC byte parsers
├── app/              # Window orchestrator, tab manager & event loop
├── clipboard/        # System clipboard integration (Wayland / X11 / OSC-52)
├── config/           # TOML configuration engine & loader
├── font/             # FontDB, resolved font sets, synthetic italics & fallback LRU
├── hyperlink/        # OSC-8 & regex hyperlink detector and browser opener
├── input/            # Keyboard translations & modifier handling
├── pty/              # Asynchronous PTY process streams & fork execution
├── renderer/         # Dual backends: OpenGL 3.3+ & pure-Rust CPU softbuffer
├── screen/           # Character grid, cursor, selection & chunked scrollback
├── terminal/         # VT state machine, alternate screen & protocol engine
└── theme/            # Theme presets, palette resolver & hex parser
```

### Implemented Features in v0.1.9:

- **Dual Rendering Backends**: Hardware OpenGL 3.3+ texture atlas glyph rendering and pure CPU software rendering via `softbuffer` with `DamageMap` row tracking.
- **Multi-Tab Workflows**: Built-in tab bar (`Auto`, `Always`, `Never` visibility), per-tab isolated font zoom, close button, middle-click close, new tab button, tab navigation shortcuts, and custom tab accent colors.
- **Zero-Flicker Cold Startup**: Window created hidden, first frame drawn synchronously before window reveal, conditional alpha visuals, and throttler past-timestamp initialization.
- **Single-Process IPC**: Unix domain socket server with CLI commands `velox msg create-window` and `velox msg create-tab` for instant sub-3ms window/tab spawning.
- **Infinite Scrollback & Memory Control**: Chunked scrollback paging with bounded RAM footprint and automatic allocator memory trimming (`malloc_trim`) after PTY inactivity.
- **Synthetic Italic Typography**: Dynamic glyph outline shearing (`shear_outline`) and point transformation when native italic font variants are absent.
- **Advanced Text Styling**: Underlines (single, double, curly, dotted, dashed), SGR underline color customization, strikethrough, dimming, and bold-as-bright remapping.
- **VT Protocols**: 24-bit TrueColor, 256 colors, OSC-7 directory tracking, OSC-8 hyperlinks, OSC-52 clipboard, OSC-133 semantic prompt markers, DECSCUSR cursor shapes, and SGR 1006 mouse tracking.
- **Visual Options**: Background opacity / transparency, unfocused window dimming (`window_dim`), and customizable cursor colors / text colors.

---

## Planned Future Features

These features are planned for future releases:

### 1. In-Terminal Search UI
- Interactive search overlay with forward/backward search navigation (`Ctrl + Shift + F`).
- Case-sensitive and regular expression pattern matching across the screen grid and scrollback history.
- Highlight matching search occurrences in the visible viewport.

### 2. Configuration Live Reload
- Filesystem watcher on `~/.config/velox/config.toml` (via `notify` or `inotify`).
- Live updates to color schemes, font sizes, cursor styles, and tab bar settings without restarting the terminal.

### 3. Complex Text Shaping & Bidirectional Text
- Integration with HarfBuzz (`rustybuzz`) for complex script ligatures, Arabic/Hebrew bidirectional text shaping, and emoji grapheme cluster composition.

### 4. Custom Keybinding System
- User-configurable shortcut bindings in `config.toml` for tab actions, clipboard operations, font resizing, and custom escape sequence macros.

### 5. Multi-Split Panes
- Horizontal and vertical pane tiling inside individual tabs with keyboard navigation (`Ctrl + Shift + D` / `Ctrl + Shift + E`).

---

## Notes

The roadmap strictly distinguishes between code that is implemented and tested in `src/` versus planned future enhancements.

