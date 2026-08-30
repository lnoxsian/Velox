# Velox Terminal Overview

Velox is an ultra-fast, lightweight Linux terminal emulator written in Rust. It offers hardware-accelerated OpenGL rendering alongside a pure-Rust CPU software renderer fallback (`softbuffer`), multi-tab workflows with isolated font zooming, ANSI/VT protocol parsing, asynchronous PTY management, synthetic italic typography fallbacks, memory compaction, clipboard integration, and single-instance IPC.

## Implemented Features

- **Dual Rendering Pipelines**: Hardware OpenGL 3.3+ texture atlas glyph renderer (120–240 FPS) and native CPU software renderer via `softbuffer` with fine-grained `DamageMap` row tracking.
- **Tab Management (`src/app/tab.rs`)**: Full multi-tab support with `Auto`, `Always`, and `Never` visibility, interactive tab creation/closing, middle-click close, per-tab font size isolation, and custom tab accent colors.
- **Zero-Flicker Startup Sequence**: Window created initially hidden, first frame rendered synchronously before window reveal, transparency enabled conditionally based on opacity, eliminating cold-start transparent flickering.
- **Synthetic Italic Typography (`src/font/resolved.rs`)**: Dynamic glyph outline shearing (`shear_outline`) and point transformation when native italic font variants are absent.
- **Infinite Scrollback & Memory Control (`src/screen/scrollback.rs`, `src/memory.rs`)**: Chunked scrollback history with disk paging, bounded RAM usage, and automatic idle memory trimming (PTY inactivity allocator cleanup).
- **Single-Process IPC Architecture (`src/ipc.rs`)**: Unix domain socket server supporting `create-window` and `create-tab` requests with `-w` (working dir), `-e` (command), `-t` (title), and `--hold`.
- **ANSI / VT Protocol Engine**: High-performance CSI, OSC, DCS, and ESC byte parser with TrueColor (24-bit), 256 colors, OSC-7 working directory, OSC-8 explicit hyperlinks, OSC-52 clipboard, and OSC-133 shell integration markers.
- **Screen Buffer & Typography**: Double-width emojis, box-drawing primitives, line decorations (single, double, curly, dotted, dashed underlines; strikethrough), dimming, and bold-as-bright remapping.
- **Terminal Interaction**: Word/line text selection, SGR 1006 mouse tracking, drag selection, and DECSCUSR cursor shapes (block, beam, underline, hollow block).
- **Visual Customizations**: Window background opacity / transparency, unfocused window dimming (`window_dim`), customizable cursor colors / text colors, and theme presets.

## Module Tree

```text
src/
├── main.rs                 # CLI entry point, option routing & single-instance IPC check
├── lib.rs                  # Crate root and module exports
├── cli.rs                  # Command line argument parser & action protocol
├── ipc.rs                  # Unix domain socket single-instance IPC server & client
├── memory.rs               # Allocator memory trimming & heap compaction helpers
├── ansi/                   # High-speed VT byte stream parsing
│   ├── mod.rs
│   ├── csi.rs              # CSI control sequence handlers
│   ├── esc.rs              # ESC escape sequence handlers
│   ├── osc.rs              # OSC operating system command handlers
│   ├── parser.rs           # Byte-stream dispatch & state machine
│   └── state.rs            # Parser state tracking
├── app/                    # Multi-window orchestrator & event loops
│   ├── mod.rs
│   ├── app.rs              # WindowState, App, and winit ApplicationHandler
│   ├── tab.rs              # Tab, TabBar, hit-testing & tab bar render model
│   ├── keyboard.rs         # Keyboard event translation & shortcuts
│   └── mouse.rs            # Mouse clicks, drags, selections & tab interactions
├── clipboard/              # System clipboard integration
│   ├── mod.rs
│   └── clipboard.rs        # Wayland (wl-clipboard), X11 (xclip/xsel) & OSC-52
├── config/                 # TOML configuration engine
│   ├── mod.rs
│   ├── config.rs           # Config data structures & serde definitions
│   ├── defaults.rs         # Default fallback configurations
│   └── loader.rs           # Disk loading, validation & persistence
├── font/                   # Typography, atlas rasterizer & fallbacks
│   ├── mod.rs
│   ├── fallback.rs         # System font fallback LRU with byte budgeting
│   ├── loader.rs           # OpenGL atlas texture packing & glyph caching
│   ├── resolved.rs         # ResolvedFontSet & synthetic italic outline shearing
│   └── storage.rs          # Shared Arc-backed font data storage
├── hyperlink/              # Hyperlink detection & activation
│   ├── mod.rs
│   ├── detector.rs         # URL regex detector & system browser opener
│   └── osc8.rs             # OSC-8 explicit hyperlink parsing
├── input/                  # Input translations
│   ├── mod.rs
│   └── keyboard.rs         # Key translation table & modifier keys
├── pty/                    # Asynchronous PTY process streams
│   ├── mod.rs
│   ├── master.rs           # PTY master read/write/resize helpers
│   └── process.rs          # Shell process spawning & fork execution
├── renderer/               # Dual rendering backends
│   ├── mod.rs
│   ├── renderer.rs         # Hardware OpenGL 3.3+ shader atlas renderer
│   └── software/           # Pure-Rust CPU software renderer
│       ├── mod.rs          # CpuRenderer & blitting pipeline
│       ├── atlas.rs        # Software glyph bitmap atlas
│       ├── color.rs        # Packed ARGB colors & precomputed palettes
│       ├── damage.rs       # DamageMap dirty-row tracking
│       ├── decorations.rs  # Underlines, curly lines, cursor & strikethrough
│       ├── framebuffer.rs  # CPU pixel framebuffer
│       ├── glyph.rs        # Glyph raster cache & font wrappers
│       ├── primitives.rs   # Box & block drawing fast paths
│       └── raster.rs       # Alpha & color glyph blitters
├── screen/                 # Terminal screen grid & text storage
│   ├── mod.rs
│   ├── cell.rs             # Cell attributes, colors & style bitflags
│   ├── cursor.rs           # Cursor state, shapes & visibility
│   ├── damage.rs           # Dirty row tracker
│   ├── grid.rs             # Active screen grid & line reflow
│   ├── reflow.rs           # Window resize text reflow
│   ├── scroll.rs           # Scrolling calculations
│   ├── scrollback.rs       # Chunked scrollback history with disk paging
│   └── selection.rs        # Mouse text selection regions
├── terminal/               # Terminal emulator state machine
│   ├── mod.rs
│   └── terminal.rs         # VT state machine, alternate screen & modes
└── theme/                  # Theme presets & palette resolver
    ├── mod.rs
    └── theme.rs            # ANSI color palettes & color parsing
```

## Performance Targets

| Metric | Target / Benchmark |
| :--- | :--- |
| **Startup** | `< 15ms` (Zero-flicker cold start) |
| **Idle Memory** | `< 30MB` (Standalone) / `~3–5MB` (IPC sub-window/tab) |
| **Frame Rate** | `120–240 FPS` (GPU) / `60 FPS` (Software) |
| **Rendering** | Dirty-region driven with frame throttler |
| **Allocations** | Reused frame buffers, bounded fallback cache & allocator trimming |
| **IPC Creation** | `< 3ms` |

## Configuration Keys

Velox reads configuration from `~/.config/velox/config.toml` (or `$XDG_CONFIG_HOME/velox/config.toml`).

| Section | Key | Type | Default | Description |
| :--- | :--- | :--- | :--- | :--- |
| **Root** | `shell` | string | `$SHELL` or `/bin/sh` | Default shell executable |
| **Root** | `app_title` | string | `"{program}"` | Window title template |
| **Root** | `single_instance` | bool | `true` | Enable single-process IPC window/tab creation |
| **`[font]`** | `font_family` | string | `"monospace"` | Primary font family name |
| **`[font]`** | `font_size` | float | `12.0` | Default terminal font size |
| **`[font]`** | `font_scale_multiplier`| float | `1.5` | Font rasterization scale multiplier |
| **`[font]`** | `bold_is_bright` | bool | `true` | Map bold text to bright ANSI colors |
| **`[window]`**| `scrollback_limit` | integer | `2000` | Finite scrollback line limit |
| **`[window]`**| `infinite_scrollback`| bool | `true` | Enable chunked disk-backed infinite history |
| **`[window]`**| `gpu_acceleration` | bool | `true` | Use OpenGL (true) or native CPU software (false) |
| **`[window]`**| `scroll_multiplier` | float | `5.0` | Mouse wheel scroll speed multiplier |
| **`[window]`**| `fps_limit` | integer | `120` (GPU) / `60` (CPU)| Maximum render frames per second |
| **`[window]`**| `padding_x` | float | `8.0` | Horizontal window padding in pixels |
| **`[window]`**| `padding_y` | float | `4.0` | Vertical window padding in pixels |
| **`[window]`**| `cursor_shape` | string | `"beam"` | Cursor shape: `block`, `beam`, `underline`, `hollow_block` |
| **`[window]`**| `cursor_blink` | bool | `true` | Toggle cursor blinking animation |
| **`[window]`**| `cursor_color` | string | `"default"` | Cursor background color (hex, `default`, `inverted`) |
| **`[window]`**| `cursor_text_color` | string | `"default"` | Cursor character color (hex, `default`, `inverted`) |
| **`[window]`**| `scroll_on_output` | bool | `true` | Scroll to bottom when PTY produces output |
| **`[window]`**| `scroll_on_keystroke` | bool | `true` | Scroll to bottom when typing |
| **`[window]`**| `opacity` | float | `1.0` | Background opacity (`0.0` transparent to `1.0` opaque) |
| **`[window]`**| `window_dim` | float | `0.0` | Dimming factor when window loses focus (`0.0` to `1.0`) |
| **`[tabs]`** | `show_tab_bar` | string | `"auto"` | Tab bar visibility: `auto`, `always`, `never` |
| **`[tabs]`** | `tab_bar_height` | integer | `None` (derived) | Custom tab bar height in pixels |
| **`[tabs]`** | `show_close_button` | bool | `true` | Render `×` close button on tabs |
| **`[tabs]`** | `show_new_tab_button`| bool | `false` | Render `+` button on tab bar |
| **`[tabs]`** | `font_size` | float | `None` (derived) | Custom font size for tab headers |
| **`[tabs]`** | `tab_accent_color` | string | `"blue"` | Active tab accent color name or hex |
| **`[colors]`** | `default_fg` / `bg` | string | Hex strings | Default foreground and background colors |
| **`[colors]`** | 16 ANSI colors | string | Hex strings | Black, red, green, yellow, blue, etc. |

## Building & Running

```bash
# Standard release
cargo build --release

# Optimized release
cargo build --profile optimized-release

# Run terminal
./target/release/velox

# Create window via single-instance IPC
./target/release/velox msg create-window -w ~/projects -t "Velox" -e htop

# Create tab via single-instance IPC
./target/release/velox msg create-tab -w ~/projects -t "Logs"
```

## Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [App Module Documentation](APP_MODULE.md)
- [Development Plan](PLAN.md)

