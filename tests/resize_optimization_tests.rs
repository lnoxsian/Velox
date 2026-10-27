use std::fs::File;
use std::os::fd::FromRawFd;
use std::sync::{Arc, Mutex};
use velox::app::app::WindowState;
use velox::diagnostics::ResizeStats;
use velox::pty::master::PtyMaster;
use velox::renderer::framebuffer::Framebuffer;
use velox::renderer::{CpuPaneState, CpuRenderer, CpuRowState};
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::cursor::CursorShape;
use velox::screen::grid::Grid;
use velox::terminal::terminal::Terminal;
use velox::theme::theme::Theme;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn create_test_pipe() -> (Arc<PtyMaster>, File) {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(res, 0);

    let pty = Arc::new(PtyMaster::new(fds[0], None));
    let write_end = unsafe { File::from_raw_fd(fds[1]) };
    (pty, write_end)
}

fn setup_renderer(width: u32, height: u32) -> CpuRenderer {
    let theme = Theme::new();
    CpuRenderer::new("monospace", 14.0, 1.0, &theme, width, height, true, 1.0)
}

#[test]
fn test_1_pixel_resizing_does_not_reflow_unchanged_terminal_dimensions() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let (pty, _w) = create_test_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Initial resize to establish 800x600 layout and terminal dimensions
    ws.resize_state.last_applied_width = 0;
    ws.resize_state.last_applied_height = 0;
    ws.resize_renderer(800, 600);
    let initial_cols = ws.active_pane().terminal.grid.width;
    let initial_rows = ws.active_pane().terminal.grid.height;

    ResizeStats::reset();

    // 1-pixel steps that stay well within cell dimensions (8x16)
    for dx in 1..=6 {
        ws.resize_renderer(800 + dx, 600);
    }

    let stats = ResizeStats::get();
    assert_eq!(stats.applied_viewport_resizes, 6);
    // Terminal dimensions did not change, so no terminal reflow or PTY resize
    assert_eq!(stats.terminal_resize_calls, 0);
    assert_eq!(stats.normal_grid_reflows, 0);
    assert_eq!(stats.pty_resize_calls, 0);

    assert_eq!(ws.active_pane().terminal.grid.width, initial_cols);
    assert_eq!(ws.active_pane().terminal.grid.height, initial_rows);
}

#[test]
fn test_2_cell_boundary_crossing_triggers_one_terminal_resize() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let (pty, _w) = create_test_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    ws.resize_state.last_applied_width = 0;
    ws.resize_state.last_applied_height = 0;
    ws.resize_renderer(800, 600);
    let old_cols = ws.active_pane().terminal.grid.width;
    let old_rows = ws.active_pane().terminal.grid.height;

    // Feed some identifiable text
    ws.active_pane_mut()
        .terminal
        .feed(b"Hello world from Velox!");

    ResizeStats::reset();

    // Cross cell boundaries (80px wider is 10 cols, 64px taller is 4 rows)
    ws.resize_renderer(880, 664);

    let stats = ResizeStats::get();
    assert_eq!(stats.terminal_resize_calls, 1);
    assert_eq!(stats.normal_grid_reflows, 1);
    assert_eq!(stats.pty_resize_calls, 1);

    let new_cols = ws.active_pane().terminal.grid.width;
    let new_rows = ws.active_pane().terminal.grid.height;
    assert!(new_cols > old_cols);
    assert!(new_rows > old_rows);

    // Verify content and cursor are valid
    let grid = ws.active_pane().terminal.active_grid();
    assert_eq!(grid.cells[0].character, 'H');
    assert_eq!(grid.cells[4].character, 'o');
}

#[test]
fn test_3_repeated_identical_viewport_resize_is_a_no_op() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let (pty, _w) = create_test_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    ws.resize_renderer(800, 600);

    ResizeStats::reset();

    // Call resize repeatedly with the exact same dimensions
    for _ in 0..5 {
        ws.resize_renderer(800, 600);
    }

    let stats = ResizeStats::get();
    assert_eq!(stats.applied_viewport_resizes, 0);
    assert_eq!(stats.framebuffer_resize_calls, 0);
    assert_eq!(stats.terminal_resize_calls, 0);
    assert_eq!(stats.normal_grid_reflows, 0);
    assert_eq!(stats.pty_resize_calls, 0);
    assert_eq!(stats.pane_state_clears, 0);
}

#[test]
fn test_4_framebuffer_capacity_retention() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut fb = Framebuffer::new(800, 600);
    assert_eq!(fb.pixels.len(), 480_000);

    // Grow framebuffer to 1200x900
    ResizeStats::reset();
    let changed = fb.resize(1200, 900);
    assert!(changed);
    assert_eq!(fb.pixels.len(), 1_080_000);
    let peak_capacity = fb.pixels.capacity();
    assert!(peak_capacity >= 1_080_000);

    let stats = ResizeStats::get();
    assert_eq!(stats.framebuffer_growth_events, 1);

    // Shrink framebuffer down to 600x400 during interactive resizing
    ResizeStats::reset();
    let changed = fb.resize(600, 400);
    assert!(changed);
    assert_eq!(fb.pixels.len(), 240_000);
    // Capacity MUST NOT shrink during interactive resizing!
    assert_eq!(fb.pixels.capacity(), peak_capacity);
    assert_eq!(ResizeStats::get().framebuffer_shrink_events, 0);

    // Re-expand to 1000x800 (smaller than peak capacity)
    ResizeStats::reset();
    let changed = fb.resize(1000, 800);
    assert!(changed);
    assert_eq!(fb.pixels.len(), 800_000);
    assert_eq!(fb.pixels.capacity(), peak_capacity);
    // Did NOT need to reallocate because capacity was retained!
    assert_eq!(ResizeStats::get().framebuffer_shrink_events, 0);

    // Idle memory release shrinks capacity
    fb.release_memory();
    assert_eq!(fb.pixels.capacity(), fb.pixels.len());
    assert_eq!(ResizeStats::get().framebuffer_shrink_events, 1);
}

#[test]
fn test_5_pane_state_retention() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut renderer = setup_renderer(800, 600);

    // Insert mock pane states with allocated row states
    renderer.pane_states.insert(
        1,
        CpuPaneState {
            row_states: vec![CpuRowState::default(); 24],
            ..Default::default()
        },
    );
    renderer.pane_states.insert(
        2,
        CpuPaneState {
            row_states: vec![CpuRowState::default(); 24],
            ..Default::default()
        },
    );

    ResizeStats::reset();

    // Resize the renderer
    renderer.resize(1024, 768);

    let stats = ResizeStats::get();
    assert_eq!(stats.pane_state_clears, 0);
    assert_eq!(stats.pane_state_invalidations, 2);

    // Verify pane state objects were retained and not destroyed
    assert!(renderer.pane_states.contains_key(&1));
    assert!(renderer.pane_states.contains_key(&2));
    assert_eq!(renderer.pane_states.get(&1).unwrap().row_states.len(), 24);
    assert_eq!(renderer.pane_states.get(&2).unwrap().row_states.len(), 24);
    assert!(renderer.pane_states.get(&1).unwrap().last_rect.is_none());
}

#[test]
fn test_6_normal_and_alternate_screen_preservation() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut term = Terminal::new(80, 24);

    // 1. Write text to normal screen
    term.feed(b"Normal Screen Content Line 1\r\nNormal Screen Content Line 2");
    assert!(!term.is_alt_screen);
    assert_eq!(term.grid.cells[0].character, 'N');

    // 2. Switch to alternate screen
    term.feed(b"\x1b[?1049h");
    assert!(term.is_alt_screen);
    term.feed(b"Alt Screen Content Line 1");
    assert_eq!(term.alt_grid.cells[0].character, 'A');

    // 3. Resize terminal
    term.resize(100, 30);
    assert_eq!(term.grid.width, 100);
    assert_eq!(term.grid.height, 30);
    assert_eq!(term.alt_grid.width, 100);
    assert_eq!(term.alt_grid.height, 30);

    // Verify alt screen content preserved
    assert_eq!(term.alt_grid.cells[0].character, 'A');

    // 4. Switch back to normal screen
    term.feed(b"\x1b[?1049l");
    assert!(!term.is_alt_screen);
    assert_eq!(term.grid.cells[0].character, 'N');

    // 5. Repeated resize with unchanged dimensions is a no-op
    ResizeStats::reset();
    term.resize(100, 30);
    let stats = ResizeStats::get();
    assert_eq!(stats.terminal_resize_calls, 0);
    assert_eq!(stats.normal_grid_reflows, 0);
    assert_eq!(stats.alternate_grid_reflows, 0);
}

#[test]
fn test_7_reflow_correctness_edge_cases() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut term = Terminal::new(80, 24);

    // Long wrapped lines
    let long_text = "A".repeat(160);
    term.feed(format!("{}\r\n", long_text).as_bytes());

    // Wide unicode characters (Chinese + emoji)
    term.feed("你好世界 🦀\r\n".as_bytes());

    // Combining characters: 'e' with combining acute accent
    term.feed("e\u{0301}cole\r\n".as_bytes());

    // Empty lines
    term.feed(b"\r\n\r\n");

    // Scrollback generation: push 50 lines
    for i in 0..50 {
        term.feed(format!("Scrollback line {:03}\r\n", i).as_bytes());
    }

    // Narrow down to 40 columns
    term.resize(40, 24);
    assert_eq!(term.active_grid().width, 40);
    assert_eq!(term.active_grid().height, 24);
    assert_eq!(term.active_grid().cells.len(), 40 * 24);
    assert_eq!(term.active_grid().row_wrapped.len(), 24);

    // Expand to 120 columns
    term.resize(120, 24);
    assert_eq!(term.active_grid().width, 120);
    assert_eq!(term.active_grid().height, 24);
    assert_eq!(term.active_grid().cells.len(), 120 * 24);
    assert_eq!(term.active_grid().row_wrapped.len(), 24);

    // Cursor at right edge
    term.active_grid_mut().cursor.x = 119;
    term.active_grid_mut().cursor.y = 10;
    term.resize(60, 24);
    assert!(term.active_grid().cursor.x < 60);

    // One-column terminal width
    term.resize(1, 24);
    assert_eq!(term.active_grid().width, 1);
    assert_eq!(term.active_grid().height, 24);
    assert_eq!(term.active_grid().cells.len(), 24);
    assert_eq!(term.active_grid().row_wrapped.len(), 24);

    // One-row terminal height
    term.resize(80, 1);
    assert_eq!(term.active_grid().width, 80);
    assert_eq!(term.active_grid().height, 1);
    assert_eq!(term.active_grid().cells.len(), 80);
    assert_eq!(term.active_grid().row_wrapped.len(), 1);
}

#[test]
fn test_8_resize_event_coalescing() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let (pty, _w) = create_test_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    ws.resize_state.last_applied_width = 0;
    ws.resize_state.last_applied_height = 0;
    ws.resize_renderer(800, 600);

    // Submit multiple pending resize requests in rapid succession
    ws.request_resize(850, 650);
    ws.request_resize(900, 700);
    ws.request_resize(1024, 768);

    assert!(ws.resize_state.is_pending);
    assert_eq!(ws.resize_state.pending_width, 1024);
    assert_eq!(ws.resize_state.pending_height, 768);

    ResizeStats::reset();

    // Now apply pending resize: only the final coalesced dimensions are applied!
    ws.apply_pending_resize();

    assert!(!ws.resize_state.is_pending);
    assert_eq!(ws.resize_state.last_applied_width, 1024);
    assert_eq!(ws.resize_state.last_applied_height, 768);
    assert_eq!(ResizeStats::get().applied_viewport_resizes, 1);
}

#[test]
fn test_9_blank_terminal_resize_invariants() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut term = Terminal::new(80, 24);

    let test_dimensions = [
        (1, 1),
        (5, 5),
        (10, 100),
        (120, 40),
        (1, 50),
        (50, 1),
        (200, 80),
        (80, 24),
    ];

    for (cols, rows) in test_dimensions {
        term.resize(cols, rows);

        // Verify normal grid invariant
        assert_eq!(
            term.grid.cells.len(),
            term.grid.width.saturating_mul(term.grid.height)
        );
        assert_eq!(term.grid.row_wrapped.len(), term.grid.height);

        // Verify alternate grid invariant
        assert_eq!(
            term.alt_grid.cells.len(),
            term.alt_grid.width.saturating_mul(term.alt_grid.height)
        );
        assert_eq!(term.alt_grid.row_wrapped.len(), term.alt_grid.height);
    }
}

#[test]
fn test_10_renderer_safety() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut renderer = setup_renderer(800, 600);
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // 1. Normal grid rendering
    let normal_grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    renderer.render(
        &normal_grid.cells,
        &normal_grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        normal_grid.cursor.x,
        true,
        1.0,
        &mut target,
    );

    // 2. One-cell grid (width 1, height 1)
    let one_cell_grid = Grid::new(
        1,
        1,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    renderer.render(
        &one_cell_grid.cells,
        &one_cell_grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        one_cell_grid.cursor.x,
        true,
        1.0,
        &mut target,
    );

    // 3. Blank grid (empty text, all default space cells)
    let blank_cells = vec![
        Cell::new(
            ' ',
            Color {
                r: 255,
                g: 255,
                b: 255
            },
            Color { r: 0, g: 0, b: 0 },
            CellFlags::empty()
        );
        80 * 24
    ];
    renderer.render(
        &blank_cells,
        &normal_grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        normal_grid.cursor.x,
        true,
        1.0,
        &mut target,
    );

    // 4. Resized framebuffer (larger and smaller)
    renderer.resize(1024, 768);
    let mut new_target = vec![0u32; 1024 * 768];
    renderer.render(
        &normal_grid.cells,
        &normal_grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        normal_grid.cursor.x,
        true,
        1.0,
        &mut new_target,
    );

    renderer.resize(400, 300);
    let mut small_target = vec![0u32; 400 * 300];
    renderer.render(
        &normal_grid.cells,
        &normal_grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        normal_grid.cursor.x,
        true,
        1.0,
        &mut small_target,
    );
}
