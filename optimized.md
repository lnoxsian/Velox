# Velox CPU and Overall Optimization Implementation Task

## Repository

Repository: `lnoxsian/Velox`

Target branch:

```text
software_render_only
```

Branch commit currently being optimized:

```text
973a1e7f7c401a7f9c39643d3bf1d8b894e14afe
```

Repository description:

Velox is a modern, high-performance terminal emulator for Linux, written in Rust and designed for speed, low resource usage, and strong terminal compatibility.

## Objective

Perform a careful, benchmark-driven optimization pass focused primarily on CPU usage and rendering throughput in the `software_render_only` branch.

The goal is to reduce CPU usage during:

1. Idle frames.
2. Single-row updates.
3. Cursor movement and blinking.
4. Selection changes.
5. Continuous terminal output.
6. Full-screen redraws.
7. Scrollback rendering.
8. Unicode and emoji rendering.
9. Multi-pane rendering.
10. High-frequency PTY input.

The implementation must preserve rendering correctness, terminal compatibility, Unicode behavior, selection behavior, cursor behavior, scrollback behavior, and software-only architecture.

Do not restore OpenGL or reintroduce `softbuffer`. The branch is intentionally software-rendering-only.

---

# Critical Engineering Rules

## 1. Profile before making large changes

Before changing performance-sensitive code:

- Inspect the current benchmark suite.
- Run the existing software renderer benchmarks.
- Run the test suite.
- Build using the optimized profile.
- Record baseline results.

Use commands similar to:

```bash
cargo test --all-targets
cargo build --profile optimized-release
cargo run --profile optimized-release --bin velox
```

Run the existing benchmark:

```bash
cargo bench --bench software_renderer
```

If the benchmark is not compatible with the current branch, fix the benchmark harness minimally or add an equivalent benchmark without changing application behavior.

Use Linux profiling where available:

```bash
perf stat -d cargo bench --bench software_renderer
perf record -g --call-graph dwarf cargo bench --bench software_renderer
perf report
```

If `perf` is unavailable, use the existing CPU measurement script and document that limitation.

Do not claim an optimization is beneficial without measuring it.

---

## 2. Preserve behavior

Do not change:

- ANSI/CSI/OSC protocol behavior.
- Terminal cursor semantics.
- Wide-character handling.
- Combining-character handling.
- Variation-selector handling.
- Scrollback ordering.
- Selection behavior.
- Mouse reporting behavior.
- Kitty keyboard protocol behavior.
- PTY data ordering.
- Window sizing behavior.
- CSD button behavior.
- Transparency semantics.

If a performance optimization changes behavior, revert it or add a compatibility-preserving implementation.

---

## 3. Avoid premature unsafe SIMD

Do not introduce unsafe SIMD in the first optimization pass unless:

- A profiler proves rasterization/blending is the dominant bottleneck.
- A safe scalar optimization is insufficient.
- The SIMD implementation has architecture guards.
- The fallback implementation is correct.
- Tests and benchmarks demonstrate a real improvement.

Prefer safe Rust, slice-based operations, `copy_within`, `fill`, lookup tables, cached values, and better data flow first.

---

# Priority 1: Optimize the CPU Rendering Hot Path

## Target files

Primary files:

```text
src/renderer/mod.rs
src/renderer/color.rs
src/renderer/framebuffer.rs
src/renderer/raster.rs
src/renderer/damage.rs
src/renderer/glyph.rs
src/screen/grid.rs
src/screen/damage.rs
```

---

## Task 1: Remove duplicate per-cell color resolution

The current renderer resolves colors repeatedly during the background and glyph passes.

In `src/renderer/mod.rs`, `render_single_pane()` currently performs color resolution for many cells more than once.

Implement a cached per-row or per-cell resolved representation.

A suitable design is:

```rust
struct ResolvedCell {
    fg: u32,
    bg: u32,
    glyph_key: Option<GlyphKey>,
    flags: CellFlags,
}
```

You may use a row-local scratch buffer to avoid allocations.

Requirements:

- Do not allocate for every cell.
- Reuse renderer-owned scratch storage.
- Only build the resolved row when the row is dirty.
- Reuse resolved foreground colors during both background and foreground rendering.
- Include selection, reverse video, cursor state, dimming, bold-bright remapping, and opacity in the resolved values.
- Ensure that cursor cells override foreground/background correctly.
- Ensure hidden and blinking cells still behave correctly.
- Ensure wide cells use the correct width.

Prefer row-local stack storage for normal terminal widths where practical, with a reusable heap fallback for very wide terminals.

Example approach:

```rust
const STACK_CELL_LIMIT: usize = 512;
```

Do not expose internal scratch buffers publicly.

---

## Task 2: Precompute theme and ANSI color metadata

In `src/renderer/color.rs`:

- Precompute luminance values for:
  - Default foreground.
  - Default background.
  - All 16 ANSI colors.
  - Bright ANSI colors.
- Precompute direct bold-to-bright mappings.
- Avoid repeatedly comparing a cell color against eight ANSI colors.

Possible design:

```rust
pub struct PrecomputedPalette {
    pub ansi_colors: [u32; 16],
    pub ansi_colors_raw: [Color; 16],
    pub ansi_luminance: [u16; 16],
    pub bold_bright_map: [u8; 16],
    pub default_fg: u32,
    pub default_bg: u32,
    pub default_fg_luminance: u16,
    pub default_bg_luminance: u16,
    // Existing fields...
}
```

If changing `Cell` to hold an ANSI palette index is safe and practical, consider it. However, do not break serialized formats or terminal behavior. If a cell may contain arbitrary truecolor values, retain the current color fields and use a fast fallback path.

Use integer luminance where possible instead of floating-point calculations:

```text
luminance ≈ (77*r + 150*g + 29*b) >> 8
```

Validate selection inversion against the existing tests.

---

## Task 3: Move glyph-cache selection outside the cell loop

In `render_single_pane()`, the glyph cache is selected inside the per-cell path.

Instead:

- Resolve the correct `GlyphCache` once per pane.
- Pass it to row rendering.
- Avoid repeated font-size comparisons.
- Avoid repeated hash-map lookup by font-size for every cell.

Preferred design:

```rust
let glyph_cache = self.cache_for_font_size(font_size);
```

Then use that cache for all rows belonging to the pane.

If Rust borrowing rules make this difficult, split rendering into smaller helper methods or use a cache key/index resolved before entering the row loop.

Do not use unsafe pointers merely to bypass borrowing.

---

## Task 4: Improve primitive rendering classification

`try_render_primitive()` is already a fast path, but it still performs a broad character range check before matching.

Improve it without making the function difficult to maintain.

Possible options:

- Use a compact lookup table for common primitive characters.
- Keep the existing match but add fast handling for the most common block characters:
  - `█`
  - `▀`
  - `▄`
  - `▌`
  - `▐`
  - `▁` through `▇`
- Ensure ordinary ASCII characters exit quickly.

Benchmark primitive-heavy output separately from ASCII text.

---

## Task 5: Optimize blank-row and blank-cell rendering

Many terminal rows contain blank cells using the default background.

Implement fast paths for:

- Completely blank rows.
- Rows with only default-background spaces.
- Rows with no glyphs, decorations, selection, reverse video, or cursor.

For a blank dirty row:

1. Fill the row background once.
2. Skip both background cell resolution and glyph iteration.

For an unchanged blank row:

- Do not touch the framebuffer.

Do not skip rows that contain:

- Selection.
- Reverse-video cells.
- Cursor.
- Blink cells.
- Custom backgrounds.
- Underlines.
- Hidden text whose background still needs repainting.

---

# Priority 2: Improve Damage Tracking

## Task 6: Replace full dirty-row scans with indexed damage

Current damage tracking uses `Vec<bool>` and scans rows repeatedly.

Implement a more efficient damage structure while preserving the current public behavior where needed.

Preferred structure:

```rust
pub struct DamageTracker {
    pub dirty_rows: Vec<bool>,
    dirty_indices: Vec<usize>,
    pub full_redraw: bool,
}
```

Requirements:

- Marking an already-dirty row must not duplicate its index.
- `mark_all()` must avoid unnecessary repeated work.
- `clear()` must reset both the bit state and index list.
- Rendering should iterate `dirty_indices` when possible.
- Full redraw should still render every visible row.
- Existing callers using `dirty_rows` must continue to compile, or be migrated cleanly.

If maintaining `dirty_rows` is too costly, introduce methods:

```rust
pub fn iter_dirty_rows(&self) -> impl Iterator<Item = usize>;
pub fn is_dirty(&self, row: usize) -> bool;
pub fn has_damage(&self) -> bool;
```

Update the renderer to avoid scanning every row when damage is sparse.

Add tests for:

- Duplicate row marking.
- Full redraw.
- Clearing.
- Resize.
- Out-of-range rows.
- Iteration ordering.

---

## Task 7: Track dirty column ranges

Row-level damage is not enough for cursor movement and small updates.

Add optional per-row column ranges:

```rust
pub struct RowDamage {
    pub min_col: usize,
    pub max_col: usize,
}
```

At minimum, support:

- Marking a cell or column range dirty.
- Expanding an existing range.
- Clearing the range.
- Full-row invalidation.

Use this for:

- Cursor movement.
- Cursor blink changes.
- Selection changes.
- Single-cell terminal updates.

If a full column-range migration is too invasive, implement it only inside the renderer state first while preserving terminal damage APIs.

Do not overcomplicate the first version. A correct row-index optimization is more important than a partial range-tracking implementation.

---

## Task 8: Stop doing a complete damage pre-scan

`check_panes_damage()` currently scans pane rows to discover cursor and selection changes before rendering.

Replace this with explicit state invalidation:

- When cursor position changes, mark old and new cursor rows.
- When cursor visibility or shape changes, mark the cursor row.
- When selection changes, mark the old and new selection ranges.
- When focus changes, mark affected panes.
- When scroll offset changes, mark the affected viewport.

The renderer should not need to inspect every row just to discover that nothing changed.

If explicit invalidation cannot be completed safely in one pass, retain the old scan as a fallback behind a debug feature or use it only when state validity is unknown.

---

# Priority 3: Optimize Grid and Row Access

## Task 9: Avoid repeated display-row calculations

`Grid::with_display_row_slice()` is called many times per row.

Implement a frame-local visible-row mapping or helper that resolves each visible row once.

Possible structure:

```rust
enum VisibleRow<'a> {
    Scrollback(&'a [Cell]),
    Active(&'a [Cell]),
}
```

Or provide a direct iterator:

```rust
pub fn visible_rows(&self) -> impl Iterator<Item = Option<&[Cell]>>;
```

Requirements:

- Preserve scrollback ordering.
- Preserve `row_offset`.
- Handle active and alternate screens.
- Handle empty or partially populated rows.
- Avoid cloning row cells.

Use the mapping for:

- Blink detection.
- Background rendering.
- Glyph rendering.
- Cursor color lookup.

---

## Task 10: Avoid allocations during hyperlink hover detection

`Grid::has_url_at()` currently creates a `String` for rows containing URL trigger characters.

This is acceptable for occasional mouse movement but can become expensive when the pointer moves continuously.

Improve it by:

- Reusing a renderer/window-owned scratch string.
- Providing a callback-based URL scanner that works directly on row characters.
- Avoiding `String` allocation for non-wrapped rows where possible.
- Caching detected URL ranges per row and invalidating them when the row changes.

Do not run URL detection on every mouse movement if the mouse remains in the same cell.

---

# Priority 4: Glyph Cache Improvements

## Task 11: Replace whole-cache Unicode eviction

In `src/renderer/glyph.rs`, the current behavior clears the entire Unicode table when the cache limit is reached:

```rust
if self.unicode_table.len() >= self.max_unicode_entries {
    self.unicode_table.clear();
}
```

Replace this with a bounded eviction strategy.

Acceptable options:

1. A small LRU cache.
2. A generational cache with two maps.
3. A `VecDeque` of keys plus a map.
4. Separate stable caches for:
   - Common Unicode.
   - Emoji/color glyphs.
   - Powerline/Nerd Font glyphs.

Requirements:

- Do not re-rasterize common glyphs on every eviction.
- Preserve ASCII cache entries.
- Avoid unbounded memory.
- Add tests proving the cache remains bounded.
- Add a benchmark for repeated Unicode and emoji rendering.

Do not make the cache thread-safe; renderer state is thread-confined.

---

## Task 12: Avoid repeated font fallback parsing

Verify whether fallback glyph rendering repeatedly parses font faces or reloads font storage.

Cache:

- Parsed face metadata.
- Font face index.
- Color-image availability.
- Glyph IDs where practical.

Do not parse the same TTF face for every emoji glyph lookup.

Focus especially on:

```rust
owned_ttf_parser::Face::parse(...)
```

and PNG extraction in `extract_color_png()`.

---

## Task 13: Fix scratch-buffer capacity policy

Current memory cleanup aggressively resets scratch vectors to empty:

```rust
self.scratch.clear_and_release(0);
```

This may reduce memory, but it can cause repeated allocations after every cleanup cycle.

Use bounded retention instead:

- Keep small alpha scratch capacity.
- Keep moderate PNG scratch capacity.
- Release only buffers exceeding a high-water threshold.
- Keep separate thresholds for normal text and emoji images.

Suggested defaults:

```text
alpha mask: retain up to 64 KiB
color pixels: retain up to 128 KiB
PNG decode buffer: retain up to 256 KiB
```

Measure memory and CPU impact before choosing final values.

---

# Priority 5: Framebuffer and Rasterization

## Task 14: Add opaque rendering fast paths

Most terminal content is opaque.

Implement separate paths where possible:

```rust
blit_alpha_glyph_opaque(...)
blit_color_glyph_premultiplied(...)
```

For opaque text:

- Skip destination alpha calculation.
- Skip alpha-channel blending when the mask is fully opaque.
- Use direct stores for mask values of 255.
- Use a simpler RGB blend for partial alpha.

Do not change transparent-window behavior.

Add tests for:

- Fully opaque text.
- Partially transparent glyph edges.
- Transparent window backgrounds.
- Emoji alpha.

---

## Task 15: Verify and optimize framebuffer presentation

Inspect the `PlatformWindow::present_frame()` implementation.

Ensure that:

- The framebuffer is not copied more than once unnecessarily.
- Damage rectangles are honored.
- X11 and Wayland presentation paths do not upload the entire frame for a small update.
- Shared-memory buffers are reused.
- Buffer allocation does not happen every frame.
- Window resizing does not retain excessive old buffers.

Add instrumentation or debug counters for:

```text
full_frame_bytes_presented
damage_bytes_presented
buffer_allocations
buffer_reuses
```

Expose these only in debug builds or benchmarks.

---

## Task 16: Improve scroll rendering

For a one-line terminal scroll:

- Move existing framebuffer rows using `copy_within` or the existing scroll helper.
- Clear only newly exposed rows.
- Render only the new row and any affected cursor/selection rows.

Do not use this optimization when:

- The viewport is scrolled into scrollback.
- Selection spans the affected region.
- The pane layout changed.
- Transparency/compositing requires repainting the entire region.
- The theme or dim state changed.

Add a benchmark for:

```text
continuous 1-line scrolling
full-screen colored block scrolling
scrollback navigation
```

---

# Priority 6: PTY and Main Loop Optimizations

## Task 17: Coalesce PTY events per pane

The PTY reactor currently sends each batch as an individual `CustomEvent::PtyData`.

Implement event coalescing where safe:

- Drain all currently available PTY data for a pane.
- Send one event containing the combined data, or
- Let the main loop drain multiple events and parse them in one pane update.

Requirements:

- Preserve byte ordering.
- Preserve pane ordering.
- Do not starve other panes.
- Bound maximum coalesced data size.
- Do not allocate unbounded buffers.

Suggested limit:

```text
maximum coalesced event: 256 KiB
```

Use a per-loop budget if needed.

---

## Task 18: Avoid repeated redraw scheduling

Ensure that many PTY events for the same pane result in one pending redraw, not one redraw per event.

Use existing `needs_redraw` and `content_dirty` state, but verify that:

- A pane receives one redraw request per loop iteration.
- Multiple PTY events are parsed before rendering.
- Inactive tabs do not trigger unnecessary visible-window redraws.
- Background tabs still retain terminal state correctly.

---

## Task 19: Avoid rebuilding poll vectors repeatedly

The event loop rebuilds the `pollfd` vector on every cycle.

Use a reusable buffer owned by `App`, or a `SmallVec` for the common case:

```rust
SmallVec<[libc::pollfd; 8]>
```

This is lower priority than renderer work, but should be implemented if profiling shows measurable allocation activity.

---

# Benchmark Requirements

Add or improve benchmarks covering at least:

## 1. Idle rendering

Expected behavior:

- No framebuffer redraw after the initial frame.
- Minimal CPU use.
- No repeated allocations.

## 2. Single dirty row

Modify one cell in one row and render repeatedly.

Measure:

- CPU time per frame.
- Bytes written to framebuffer.
- Number of rows visited.
- Number of glyphs rasterized.
- Number of color resolutions.

## 3. Full-screen ASCII

Render a full screen of ASCII characters.

Measure:

- Frames per second.
- CPU time.
- Allocations.
- Raster cache hit rate.

## 4. Full-screen color blocks

Render colored `█` characters.

This must exercise:

- Background coalescing.
- Solid-block fast paths.
- Span merging.
- Framebuffer fills.

## 5. Unicode and emoji

Render:

- CJK.
- Combining characters.
- Nerd Font symbols.
- Powerline glyphs.
- Color emoji.
- Mixed Unicode and ASCII.

Measure cache misses and rasterization time.

## 6. Continuous scroll

Measure one-line scroll throughput and CPU usage.

## 7. Multi-pane rendering

Test:

- Four panes with one dirty row.
- Four panes with one active pane.
- Four panes with all panes dirty.
- Different pane font sizes.

## 8. Transparency

Compare opacity:

```text
1.0
0.8
0.5
```

Ensure transparent rendering does not regress significantly.

---

# Instrumentation

Add optional counters behind a feature or compile-time configuration.

Useful counters:

```rust
struct RenderStats {
    frames_attempted: u64,
    frames_rendered: u64,
    idle_frames: u64,
    rows_scanned: u64,
    rows_rendered: u64,
    cells_scanned: u64,
    cells_rendered: u64,
    glyph_cache_hits: u64,
    glyph_cache_misses: u64,
    color_resolutions: u64,
    framebuffer_pixels_written: u64,
    full_present_count: u64,
    damage_present_count: u64,
}
```

Do not add logging inside the innermost per-pixel or per-cell loop in normal builds.

Use feature-gated or explicitly enabled diagnostics.

---

# Testing Requirements

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Add regression tests for:

- Idle renderer does not redraw.
- Single-row damage only touches the intended row.
- Cursor movement repaints old and new cursor cells.
- Selection changes repaint old and new selection ranges.
- Scrollback rendering remains correct.
- Wide characters remain correct.
- Combining characters remain correct.
- Variation selectors do not corrupt base codepoints.
- Emoji still render correctly.
- Transparent rendering remains correct.
- Multi-pane damage isolation remains correct.
- PTY data ordering remains correct.
- Unicode cache remains bounded.
- Glyph cache does not repeatedly rasterize stable glyphs.

---

# Correctness Requirements

The final implementation must preserve:

- Existing public APIs unless changes are necessary and migrated consistently.
- Existing terminal rendering tests.
- Existing PTY reactor tests.
- Existing keyboard and mouse tests.
- Existing scrollback tests.
- Existing CSD tests.

If any existing test fails:

1. Determine whether the failure is caused by the optimization.
2. Fix the implementation without weakening the test.
3. Do not simply delete or relax the test.

---

# Required Final Report

At the end, provide a concise report containing:

## Files changed

List every changed file and summarize the change.

## Benchmark comparison

Provide before/after results for:

- Idle frame.
- Single dirty row.
- Full-screen ASCII.
- Color blocks.
- Unicode/emoji.
- Scrolling.
- Four-pane isolated damage.

Include CPU time and throughput where available.

## Memory comparison

Report:

- Renderer memory before/after.
- Glyph cache memory before/after.
- Peak memory during Unicode/emoji workload.
- PTY buffer memory.

## Correctness

Report the results of:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

## Remaining bottlenecks

If profiling still shows major hotspots, list them and explain why they were not changed.

---

# Definition of Done

The task is complete only when:

- The code compiles in the optimized profile.
- Existing tests pass.
- New optimization tests pass.
- Benchmarks demonstrate measurable improvement in at least two workloads.
- Idle CPU usage does not regress.
- Rendering correctness is preserved.
- No unnecessary allocations remain in the common dirty-row path.
- No OpenGL or `softbuffer` dependency is reintroduced.
- The final report includes benchmark results and remaining bottlenecks.
