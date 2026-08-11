# Velox Roadmap

## Current Implementation

Velox currently ships a focused terminal stack in `src/`:

```text
src/
├── ansi/
├── app/
├── clipboard/
├── config/
├── font/
├── hyperlink/
├── input/
├── pty/
├── renderer/
├── screen/
├── terminal/
└── theme/
```

Implemented behavior includes:

- GPU-backed OpenGL rendering
- PTY shell spawning and terminal I/O
- ANSI/VT parsing for CSI, OSC, and ESC sequences
- Keyboard translation for printable, control, alt, function, and navigation keys
- Scrollback, selection, damage tracking, and alternate-screen handling
- Font loading with fallback discovery
- Hyperlink detection and OSC-8 parsing
- Clipboard copy/paste and OSC 52 helpers
- TOML configuration loading, saving, and defaults
- Theme, cursor, padding, scroll multiplier, and FPS configuration
- Bracketed paste, synchronized output, focus tracking, and semantic prompt markers

## Not Implemented Yet

These features are not present in `src/` yet and should stay marked as planned until code exists:

- In-terminal search UI and search navigation
- Regex search across the terminal buffer
- Dedicated window management modules
- Multiple window or tab session management
- Plugin or extension support
- Richer hyperlink interaction beyond detection and OSC-8 parsing
- More advanced mouse protocol coverage
- Additional terminal protocol handlers beyond the current parser set

## Near-Term Goals

1. Add a real search feature with a visible UI and key binding.
2. Expand mouse handling and selection interactions.
3. Improve renderer and font behavior for more glyph-heavy workloads.
4. Tighten config validation and reload behavior.
5. Broaden protocol support only after the current stack is stable.

## Longer-Term Goals

- Multi-window support
- Tab/session management
- Configuration hot reload
- More terminal mode coverage
- Better shaping for complex scripts
- Deeper renderer optimization and batching improvements
- Plugin or extension architecture

## Notes

The roadmap should only describe behavior that is either already implemented in `src/` or clearly planned. If a feature is not backed by code, it stays in this document as future work rather than being described as available.
