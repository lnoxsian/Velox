use velox::renderer::CpuRenderer;
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::cursor::CursorShape;
use velox::screen::grid::Grid;
use velox::theme::theme::Theme;

fn setup_renderer(width: u32, height: u32) -> CpuRenderer {
    let theme = Theme::new();
    CpuRenderer::new("monospace", 14.0, 1.5, &theme, width, height, true, 1.0)
}

#[test]
fn test_software_renderer_idle_zero_work() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // First frame initializes and renders initial damage
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );
    assert!(!renderer.damage.has_damage());

    // Clear grid damage
    grid.damage.dirty_rows.fill(false);

    // Second frame: grid is idle, no damage
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );

    assert!(!renderer.damage.has_damage());
}

#[test]
fn test_software_renderer_damage_partial_row() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Initial frame
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );
    grid.damage.dirty_rows.fill(false);

    // Modify only row 5
    grid.damage.mark_dirty(5);
    grid.cells[5 * 80 + 10] = Cell::new(
        'A',
        Color { r: 255, g: 0, b: 0 },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::BOLD,
    );

    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );

    assert!(!renderer.damage.has_damage());
}

#[test]
fn test_software_renderer_selection_and_cursor() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    let cell_w = renderer.glyph_cache.cell_width;
    let cell_h = renderer.glyph_cache.cell_height;

    // Frame 1: Selection active (cols 2..10 at row 2) + cursor visible at (5, 2)
    grid.cursor.x = 5;
    grid.cursor.y = 2;
    grid.cursor.shape = CursorShape::Block;
    grid.cursor.visible = true;

    grid.selection.start_selection(2, 2);
    grid.selection.update_selection(10, 2);

    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );
    grid.clear_damage();

    let sample_x = 8 + 3 * cell_w + 2;
    let sample_y = 4 + 2 * cell_h + 2;
    let cursor_pixel_idx = ((4 + 2 * cell_h + 2) as usize) * 800 + ((8 + 5 * cell_w + 2) as usize);

    let default_bg = velox::renderer::color::PackedColor::from_color(grid.default_bg).to_u32();
    let inverted_bg = velox::renderer::color::PackedColor::from_color(grid.default_fg).to_u32();
    let cursor_color = velox::renderer::color::PackedColor::from_color(
        theme.resolve_cursor_color(grid.default_fg),
    )
    .to_u32();

    // Verify selected cell is inverted
    assert_eq!(
        target[(sample_y as usize) * 800 + (sample_x as usize)],
        inverted_bg,
        "Selected cell background must be inverted"
    );
    // Verify cursor is drawn
    assert_eq!(
        target[cursor_pixel_idx], cursor_color,
        "Cursor must be drawn at (col 5, row 2)"
    );

    // Frame 2: Cursor disappears / blinks off with clean grid damage
    grid.cursor.visible = false;
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        false,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );
    grid.clear_damage();

    // Since (5, 2) is still in selection, it should revert to selection color (inverted_bg), not cursor_color
    assert_eq!(
        target[cursor_pixel_idx], inverted_bg,
        "Cursor must disappear when cursor_visible is false even with clean grid damage"
    );

    // Frame 3: Clear selection and move cursor to (10, 3) with clean damage
    grid.selection.clear();
    grid.cursor.x = 10;
    grid.cursor.y = 3;
    grid.cursor.visible = true;

    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );
    grid.clear_damage();

    // Sample cell at (3, 2) must revert to default background
    assert_eq!(
        target[(sample_y as usize) * 800 + (sample_x as usize)],
        default_bg,
        "Previously selected cell must revert to default background"
    );
    // Old cursor position must be default background
    assert_eq!(
        target[cursor_pixel_idx], default_bg,
        "Old cursor position must revert to default background"
    );
    // New cursor position must be drawn
    let new_cursor_pixel = ((4 + 3 * cell_h + 2) as usize) * 800 + ((8 + 10 * cell_w + 2) as usize);
    assert_eq!(
        target[new_cursor_pixel], cursor_color,
        "New cursor position must be drawn at (col 10, row 3)"
    );
}

#[test]
fn test_software_renderer_scroll_selection() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
        80,
        10,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        1000,
        false,
    );
    let theme = Theme::new();

    // Populate lines 0..15 so scrollback has 5 lines and active grid has 10 lines
    for i in 0..15 {
        for c in format!("line-{}", i).chars() {
            grid.put_char(
                c,
                grid.default_fg,
                grid.default_bg,
                None,
                CellFlags::empty(),
            );
        }
        if i < 14 {
            grid.scroll_or_move_down(grid.default_bg);
            grid.cursor.x = 0;
        }
    }

    assert_eq!(grid.scrollback.len(), 5);

    // 1. Render at scroll_offset = 0 (showing active grid top row "line-5")
    grid.scroll_offset = 0;
    let mut target_live = vec![0u32; 800 * 600];
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target_live,
    );

    // 2. Select row 2 (which is in scrollback)
    grid.selection.start_selection(0, 2);
    grid.selection.update_selection(5, 2);

    // 3. Scroll up by 3 lines so row 2 is visible at viewport row 0 (since history_len=5, scroll_offset=3, abs_row=5-3+0=2)
    grid.scroll_offset = 3;
    grid.damage.mark_all();
    let mut target_scrollback = vec![0u32; 800 * 600];

    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target_scrollback,
    );

    // Render should succeed with damage resolved
    assert!(!renderer.damage.has_damage());
    assert_eq!(grid.extract_selection_text(), "line-2");

    // Pixels at top row when scrolled up must differ from live active grid top row
    let cell_h = renderer.glyph_cache.cell_height as usize;
    let row0_live_pixels: Vec<u32> = target_live[0..800 * cell_h].to_vec();
    let row0_scroll_pixels: Vec<u32> = target_scrollback[0..800 * cell_h].to_vec();
    assert_ne!(
        row0_live_pixels, row0_scroll_pixels,
        "Top row pixels when scrolled back to history must show scrollback line-2 instead of active grid line-5"
    );
}

#[test]
fn test_software_renderer_decorations() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    grid.cells[0] = Cell::new(
        'U',
        Color {
            r: 255,
            g: 255,
            b: 0,
        },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::UNDERLINE,
    );
    grid.cells[1] = Cell::new(
        'D',
        Color {
            r: 255,
            g: 255,
            b: 0,
        },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::DOUBLE_UNDERLINE,
    );
    grid.cells[2] = Cell::new(
        'C',
        Color {
            r: 255,
            g: 255,
            b: 0,
        },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::CURLY_UNDERLINE,
    );
    grid.cells[3] = Cell::new(
        'S',
        Color {
            r: 255,
            g: 255,
            b: 0,
        },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::STRIKE,
    );

    grid.damage.mark_dirty(0);
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );
    assert!(!renderer.damage.has_damage());
}

#[test]
fn test_software_renderer_scrollback_rendering() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        true,
    );
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Push historical row into scrollback
    let scroll_cell = Cell::new(
        'H',
        Color { r: 255, g: 0, b: 0 },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::empty(),
    );
    grid.scrollback.push_line(&[scroll_cell], false);
    grid.scroll_offset = 1;

    let rendered_cells = vec![scroll_cell; 80 * 24];
    renderer.render(
        &rendered_cells,
        &grid,
        &theme,
        8.0,
        4.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );
    assert!(!renderer.damage.has_damage());
}

#[test]
fn test_software_renderer_blinking_text() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Create a blinking green 'A' cell
    grid.cells[0] = Cell::new(
        'A',
        Color { r: 0, g: 255, b: 0 },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::BLINK,
    );
    grid.damage.mark_dirty(0);

    // 1. Phase 1: blink_on is true (start_time elapsed 0ms)
    renderer.start_time = std::time::Instant::now();
    renderer.prev_blink_on = false; // ensure transition triggers damage check
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    let has_green_when_on = target
        .iter()
        .any(|&p| (p & 0x0000FF00) != 0 && (p & 0x00FF0000) == 0);
    assert!(
        has_green_when_on,
        "Blinking text should be visible when blink_on is true"
    );

    // 2. Phase 2: blink_on is false (simulate 550ms elapsed)
    renderer.start_time = std::time::Instant::now() - std::time::Duration::from_millis(550);
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    // During off phase, foreground of cell 0 should not be drawn (only black bg)
    let cell_w = renderer.glyph_cache.cell_width as usize;
    let cell_h = renderer.glyph_cache.cell_height as usize;
    let mut cell_has_green_when_off = false;
    for y in 0..cell_h {
        for x in 0..cell_w {
            let pixel = target[y * 800 + x];
            if (pixel & 0x0000FF00) != 0 && (pixel & 0x00FF0000) == 0 {
                cell_has_green_when_off = true;
            }
        }
    }
    assert!(
        !cell_has_green_when_off,
        "Blinking text should be hidden when blink_on is false"
    );
}

#[test]
fn test_software_renderer_double_line_table() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Construct a double line table:
    // Row 0: ╔ ═ ╦ ═ ╗
    // Row 1: ║ A ║ B ║
    // Row 2: ╠ ═ ╬ ═ ╣
    // Row 3: ║ C ║ D ║
    // Row 4: ╚ ═ ╩ ═ ╝
    let table_rows = [
        ['╔', '═', '╦', '═', '╗'],
        ['║', 'A', '║', 'B', '║'],
        ['╠', '═', '╬', '═', '╣'],
        ['║', 'C', '║', 'D', '║'],
        ['╚', '═', '╩', '═', '╝'],
    ];

    for (y, row) in table_rows.iter().enumerate() {
        for (x, &ch) in row.iter().enumerate() {
            grid.cells[y * 80 + x] = Cell::new(
                ch,
                Color {
                    r: 0,
                    g: 255,
                    b: 255,
                },
                Color { r: 0, g: 0, b: 0 },
                CellFlags::empty(),
            );
        }
        grid.damage.mark_dirty(y);
    }

    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    // Verify cyan table pixels exist
    let has_cyan_pixels = target.iter().any(|&p| (p & 0x00FFFF) == 0x00FFFF);
    assert!(
        has_cyan_pixels,
        "Double line table should be rendered with cyan pixels"
    );
}

#[test]
fn test_software_renderer_all_box_drawing_chars() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Put all box drawing characters U+2500..=U+257F in the grid
    let mut char_idx = 0x2500u32;
    for y in 0..4 {
        for x in 0..32 {
            if let Some(ch) = char::from_u32(char_idx) {
                grid.cells[y * 80 + x] = Cell::new(
                    ch,
                    Color {
                        r: 255,
                        g: 255,
                        b: 0,
                    },
                    Color { r: 0, g: 0, b: 0 },
                    CellFlags::empty(),
                );
            }
            char_idx += 1;
        }
        grid.damage.mark_dirty(y);
    }

    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    let has_yellow_pixels = target.iter().any(|&p| (p & 0xFFFF00) == 0xFFFF00);
    assert!(
        has_yellow_pixels,
        "All box drawing chars should render yellow pixels"
    );
}

#[test]
fn test_software_renderer_transparency() {
    let mut renderer = setup_renderer(800, 600);
    let grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color {
            r: 30,
            g: 40,
            b: 50,
        },
        100,
        false,
    );
    let mut theme = Theme::new();
    theme.default_bg = Color {
        r: 30,
        g: 40,
        b: 50,
    };
    let mut target = vec![0u32; 800 * 600];

    // Render with 50% opacity (0.5)
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        0.5,
        &mut target,
    );

    // Alpha channel of cleared background should be ~128 (0x80)
    let bg_pixel = target[0];
    let bg_alpha = (bg_pixel >> 24) & 0xFF;
    assert_eq!(bg_alpha, 128);

    // Premultiplied RGB check:
    let bg_r = (bg_pixel >> 16) & 0xFF;
    let bg_g = (bg_pixel >> 8) & 0xFF;
    let bg_b = bg_pixel & 0xFF;
    assert_eq!(bg_r, (30 * 128) / 255);
    assert_eq!(bg_g, (40 * 128) / 255);
    assert_eq!(bg_b, (50 * 128) / 255);
}

#[test]
fn test_software_renderer_custom_cursor_color() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    grid.cursor.x = 0;
    grid.cursor.y = 0;
    grid.cursor.visible = true;

    let mut theme = Theme::new();
    theme.cursor_color = Some(Color { r: 255, g: 0, b: 0 }); // Red cursor
    theme.cursor_text_color = Some(Color { r: 0, g: 255, b: 0 }); // Green text

    let mut target = vec![0u32; 800 * 600];
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    // Pixel at (0, 0) should have the cursor cursor color (0xFFFF0000)
    let pixel = target[0];
    assert_eq!(pixel, 0xFFFF0000);
}

#[test]
fn test_software_renderer_alt_screen_clean_exit() {
    use velox::terminal::terminal::Terminal;

    let mut term = Terminal::new(80, 24);
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &term.theme, 800, 600, true, 1.0);
    let mut target = vec![0u32; 800 * 600];

    // 1. Initial shell screen
    term.feed(b"user@velox:~$ echo hello\r\nhello\r\nuser@velox:~$ ");
    renderer.render(
        &term.active_grid().cells,
        term.active_grid(),
        &term.theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        term.active_grid().cursor.x,
        true,
        1.0,
        &mut target,
    );
    term.active_grid_mut().clear_damage();

    // 2. Open full-screen CLI app (e.g. Neovim in alt screen)
    term.feed(b"\x1b[?1049h");
    term.feed(b"\x1b[12;10H=== NEOVIM BINARY RUNNING ===\x1b[24;1H[STATUSLINE: 100%]");
    renderer.render(
        &term.active_grid().cells,
        term.active_grid(),
        &term.theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        term.active_grid().cursor.x,
        true,
        1.0,
        &mut target,
    );
    term.active_grid_mut().clear_damage();

    // Verify Neovim rendered non-zero pixels around row 12 and row 24
    let cell_h = renderer.glyph_cache.cell_height;
    let row_12_pixels: u64 = target[((11 * cell_h) as usize * 800)..((12 * cell_h) as usize * 800)]
        .iter()
        .map(|&p| p as u64)
        .sum();
    assert!(
        row_12_pixels > 0,
        "Neovim row 12 content must be present in target buffer"
    );

    // 3. Exit CLI app (e.g. Neovim :q)
    term.feed(b"\x1b[?1049l");
    // Shell prints new prompt on row 3; rows 4..24 are untouched blank cells in primary grid
    term.feed(b"user@velox:~$ ");
    renderer.render(
        &term.active_grid().cells,
        term.active_grid(),
        &term.theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        term.active_grid().cursor.x,
        true,
        1.0,
        &mut target,
    );
    term.active_grid_mut().clear_damage();

    // 4. Verify that row 12 and row 24 in target buffer are completely clear (pure default_bg)
    let default_bg = renderer.palette.default_bg;
    let row_12_slice = &target[((11 * cell_h) as usize * 800)..((12 * cell_h) as usize * 800)];
    let row_12_all_bg = row_12_slice.iter().all(|&p| p == default_bg);
    assert!(
        row_12_all_bg,
        "Row 12 must contain ONLY default background color after exiting alt-screen CLI app"
    );

    let row_24_slice = &target[((23 * cell_h) as usize * 800)..((24 * cell_h) as usize * 800)];
    let row_24_all_bg = row_24_slice.iter().all(|&p| p == default_bg);
    assert!(
        row_24_all_bg,
        "Row 24 statusline must be completely wiped to default background color after exiting"
    );
}

#[test]
fn test_software_renderer_unfocused_dim() {
    let mut renderer = setup_renderer(800, 600);
    let theme = Theme::new();
    let mut grid = Grid::new(80, 24, theme.default_fg, theme.default_bg, 100, false);

    // Populate cell with bright text 'A'
    grid.cells[0] = Cell::new(
        'A',
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color {
            r: 50,
            g: 50,
            b: 50,
        },
        CellFlags::empty(),
    );
    grid.damage.mark_dirty(0);

    let mut focused_target = vec![0u32; 800 * 600];
    let mut unfocused_target = vec![0u32; 800 * 600];

    // Render focused with window_dim = 0.5 (effective_dim = 0.0)
    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true, // is_focused
        1.0,
        0.5, // window_dim
        &mut focused_target,
        None,
    );

    // Render unfocused with window_dim = 0.5 (effective_dim = 0.5)
    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        false, // is_focused = false -> dims by 50%
        1.0,
        0.5, // window_dim
        &mut unfocused_target,
        None,
    );

    // 1. Verify that the background pixel remains undimmed (50, 50, 50) in both focused and unfocused modes
    let focused_bg = focused_target[0];
    let unfocused_bg = unfocused_target[0];

    let focused_r = (focused_bg >> 16) & 0xFF;
    let focused_g = (focused_bg >> 8) & 0xFF;
    let focused_b = focused_bg & 0xFF;

    let unfocused_r = (unfocused_bg >> 16) & 0xFF;
    let unfocused_g = (unfocused_bg >> 8) & 0xFF;
    let unfocused_b = unfocused_bg & 0xFF;

    assert_eq!(focused_r, 50);
    assert_eq!(focused_g, 50);
    assert_eq!(focused_b, 50);

    // Background remains undimmed!
    assert_eq!(unfocused_r, 50);
    assert_eq!(unfocused_g, 50);
    assert_eq!(unfocused_b, 50);

    // 2. Verify that text glyph pixels (character 'A') are dimmed from 255 to ~128
    let cell_w = renderer.glyph_cache.cell_width as usize;
    let cell_h = renderer.glyph_cache.cell_height as usize;

    let mut max_focused_text_r = 0u32;
    let mut max_unfocused_text_r = 0u32;

    for y in 0..cell_h {
        for x in 0..cell_w {
            let f_px = focused_target[y * 800 + x];
            let u_px = unfocused_target[y * 800 + x];
            let f_r = (f_px >> 16) & 0xFF;
            let u_r = (u_px >> 16) & 0xFF;
            if f_r > max_focused_text_r {
                max_focused_text_r = f_r;
            }
            if u_r > max_unfocused_text_r {
                max_unfocused_text_r = u_r;
            }
        }
    }

    assert!(
        max_focused_text_r >= 240,
        "Focused text should have full brightness (got {})",
        max_focused_text_r
    );
    assert!(
        (100..=135).contains(&max_unfocused_text_r),
        "Unfocused text must be dimmed to ~125 (got {})",
        max_unfocused_text_r
    );
}

#[test]
fn test_software_renderer_bold_bright_unfocused_dim() {
    let mut renderer = setup_renderer(800, 600);
    let mut theme = Theme::new();
    // Use distinct base vs bright ANSI red colors to clearly verify remapping
    theme.ansi_colors[1] = Color { r: 100, g: 0, b: 0 };
    theme.ansi_colors[9] = Color { r: 240, g: 0, b: 0 };

    let mut grid = Grid::new(80, 24, theme.default_fg, theme.default_bg, 100, false);

    // Populate cell with bold ANSI red text 'X'
    grid.cells[0] = Cell::new(
        'X',
        theme.ansi_colors[1], // ANSI 1 (dark red)
        theme.default_bg,
        CellFlags::BOLD,
    );
    grid.damage.mark_dirty(0);

    let mut focused_target = vec![0u32; 800 * 600];
    let mut unfocused_target = vec![0u32; 800 * 600];

    // Render focused with window_dim = 0.5 (bold_is_bright remaps to bright red ~240)
    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true, // is_focused
        1.0,
        0.5, // window_dim
        &mut focused_target,
        None,
    );

    // Render unfocused with window_dim = 0.5 (bold_is_bright remaps to bright red then dims 50% -> ~120)
    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        false, // is_focused = false -> dims by 50%
        1.0,
        0.5, // window_dim
        &mut unfocused_target,
        None,
    );

    let cell_w = renderer.glyph_cache.cell_width as usize;
    let cell_h = renderer.glyph_cache.cell_height as usize;

    let mut max_focused_text_r = 0u32;
    let mut max_unfocused_text_r = 0u32;

    for y in 0..cell_h {
        for x in 0..cell_w {
            let f_px = focused_target[y * 800 + x];
            let u_px = unfocused_target[y * 800 + x];
            let f_r = (f_px >> 16) & 0xFF;
            let u_r = (u_px >> 16) & 0xFF;
            if f_r > max_focused_text_r {
                max_focused_text_r = f_r;
            }
            if u_r > max_unfocused_text_r {
                max_unfocused_text_r = u_r;
            }
        }
    }

    // Focused bold ANSI red must remap to bright red (around 240, not base 100)
    assert!(
        max_focused_text_r >= 200,
        "Focused bold text should remap to bright red (got {})",
        max_focused_text_r
    );

    // Unfocused bold ANSI red must remap to bright red (240) and dim by 50% -> around 120 (not 100 * 0.5 = 50)
    assert!(
        (95..=135).contains(&max_unfocused_text_r),
        "Unfocused bold text must remap to bright red and dim to ~120 (got {})",
        max_unfocused_text_r
    );
}

#[test]
fn test_software_renderer_dynamic_selection_drag_and_scrollback() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Push 10 lines of scrollback history
    for i in 0..10 {
        let line = vec![
            Cell {
                character: (b'0' + (i as u8)) as char,
                foreground: Color {
                    r: 255,
                    g: 255,
                    b: 255
                },
                background: Color { r: 0, g: 0, b: 0 },
                underline_color: None,
                flags: CellFlags::empty(),
            };
            80
        ];
        grid.scrollback.push_line(&line, false);
    }
    assert_eq!(grid.scrollback.len(), 10);

    // Initial render (clears initial full_redraw)
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );
    assert!(!renderer.damage.has_damage());

    // 1. Start selection on screen row 2 (abs_y = 10 + 2 = 12)
    grid.selection.start_selection(0, 12);
    grid.selection.update_selection(5, 12);

    // Frame 2: should detect damage on row 2 even without full_redraw
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );
    let cell_w = renderer.glyph_cache.cell_width;
    let cell_h = renderer.glyph_cache.cell_height;
    let sample_x = 2 * cell_w + 2;
    let sample_y = 2 * cell_h + 2;
    let expected_inverted_bg =
        velox::renderer::color::PackedColor::from_color(grid.default_fg).to_u32();
    assert_eq!(
        target[(sample_y as usize) * 800 + (sample_x as usize)],
        expected_inverted_bg,
        "Row 2 cell 2 must be highlighted as selected"
    );

    // 2. Drag selection horizontally to col 15 on same row 2
    grid.selection.update_selection(15, 12);
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );
    let sample_x_10 = 10 * cell_w + 2;
    assert_eq!(
        target[(sample_y as usize) * 800 + (sample_x_10 as usize)],
        expected_inverted_bg,
        "Horizontal drag expansion to col 10 must be highlighted"
    );

    // 3. Clear selection
    grid.selection.clear();
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );
    let default_bg_packed =
        velox::renderer::color::PackedColor::from_color(grid.default_bg).to_u32();
    assert_eq!(
        target[(sample_y as usize) * 800 + (sample_x as usize)],
        default_bg_packed,
        "Row 2 cell 2 must revert to default background when selection cleared"
    );
}

#[test]
fn test_scrollback_background_preserves_theme_background_color() {
    let mut theme = Theme::new();
    // Non-black theme background (e.g. Catppuccin Mocha #1e1e2e -> r:30, g:30, b:46)
    theme.default_bg = Color {
        r: 30,
        g: 30,
        b: 46,
    };
    theme.default_fg = Color {
        r: 205,
        g: 214,
        b: 244,
    };

    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let mut grid = Grid::new(80, 10, theme.default_fg, theme.default_bg, 100, false);

    // Write short lines so lines 0..15 have trailing trimmed blank spaces
    for i in 0..15 {
        for c in format!("line-{}", i).chars() {
            grid.put_char(
                c,
                grid.default_fg,
                grid.default_bg,
                None,
                CellFlags::empty(),
            );
        }
        if i < 14 {
            grid.scroll_or_move_down(grid.default_bg);
            grid.cursor.x = 0;
        }
    }

    assert_eq!(grid.scrollback.len(), 5);

    // Scroll up by 4 lines so scrollback lines 1..5 and active grid lines 0..6 are displayed
    grid.scroll_offset = 4;
    grid.damage.mark_all();
    let mut target = vec![0u32; 800 * 600];

    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    let cell_w = renderer.glyph_cache.cell_width as usize;
    let _cell_h = renderer.glyph_cache.cell_height as usize;
    let expected_bg_packed =
        velox::renderer::color::PackedColor::from_color(theme.default_bg).to_u32();

    // Check row 0 (which is in scrollback) at column 40 (beyond text "line-1")
    let test_x = 40 * cell_w + 2;
    let test_y = 2;
    let actual_pixel = target[test_y * 800 + test_x];

    assert_eq!(
        actual_pixel, expected_bg_packed,
        "Trailing columns in scrollback must maintain the theme's default background color and not turn black"
    );
}

#[test]
fn test_software_renderer_unified_background_and_padding_no_dual_shading() {
    let mut theme = Theme::new();
    theme.default_bg = Color {
        r: 30,
        g: 30,
        b: 46,
    };
    theme.default_fg = Color {
        r: 205,
        g: 214,
        b: 244,
    };

    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let mut grid = Grid::new(80, 24, theme.default_fg, theme.default_bg, 100, false);
    grid.damage.mark_all();

    let mut target = vec![0u32; 800 * 600];

    let padding_x = 12.0;
    let padding_y = 10.0;

    // Render with unfocused dimming (window_dim = 0.5, is_focused = false)
    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        padding_x,
        padding_y,
        false,
        CursorShape::Block,
        0,
        false, // is_focused
        1.0,   // opacity
        0.5,   // window_dim
        &mut target,
        None,
    );

    let expected_bg = velox::renderer::color::PackedColor::from_color(theme.default_bg).to_u32();

    // 1. Verify padding pixel (e.g. top-left margin (2, 2))
    let padding_pixel = target[2 * 800 + 2];
    assert_eq!(
        padding_pixel, expected_bg,
        "Top-left window padding must match theme default_bg and not be dimmed"
    );

    // 2. Verify inner grid cell background pixel (inside cell at col 0, row 0)
    let cell_w = renderer.glyph_cache.cell_width as usize;
    let cell_h = renderer.glyph_cache.cell_height as usize;
    let grid_x = padding_x as usize + cell_w / 2;
    let grid_y = padding_y as usize + cell_h / 2;
    let cell_pixel = target[grid_y * 800 + grid_x];
    assert_eq!(
        cell_pixel, expected_bg,
        "Grid cell background must match theme default_bg exactly"
    );

    // 3. Verify right window margin beyond terminal grid
    let right_margin_pixel = target[300 * 800 + 798];
    assert_eq!(
        right_margin_pixel, expected_bg,
        "Right margin must match theme default_bg"
    );

    // 4. Verify bottom window margin beyond terminal grid
    let bottom_margin_pixel = target[598 * 800 + 400];
    assert_eq!(
        bottom_margin_pixel, expected_bg,
        "Bottom margin must match theme default_bg"
    );

    // 5. Test with transparency (opacity = 0.8)
    let mut trans_target = vec![0u32; 800 * 600];
    let mut trans_renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 0.8);
    let expected_trans_bg = velox::renderer::color::PackedColor::from_premultiplied(
        theme.default_bg,
        (0.8f32 * 255.0).round() as u8,
    )
    .to_u32();

    trans_renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        padding_x,
        padding_y,
        false,
        CursorShape::Block,
        0,
        false,
        0.8,
        0.5,
        &mut trans_target,
        None,
    );

    assert_eq!(
        trans_target[2 * 800 + 2],
        expected_trans_bg,
        "Translucent padding pixel must have unified premultiplied background color"
    );
    assert_eq!(
        trans_target[grid_y * 800 + grid_x],
        expected_trans_bg,
        "Translucent grid cell pixel must match translucent padding pixel exactly"
    );
}

#[test]
fn test_software_renderer_colorblocks_and_block_coalescing() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
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
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    let cw = renderer.glyph_cache.cell_width;
    let ch = renderer.glyph_cache.cell_height;

    // Fill row 0 with coalesced full blocks '█' of pure red (col 0..4) and pure blue (col 4..8)
    let red_fg = Color { r: 255, g: 0, b: 0 };
    let blue_fg = Color { r: 0, g: 0, b: 255 };
    let default_bg = Color { r: 0, g: 0, b: 0 };

    for col in 0..4 {
        grid.cells[col] = Cell::new('█', red_fg, default_bg, CellFlags::empty());
    }
    for col in 4..8 {
        grid.cells[col] = Cell::new('█', blue_fg, default_bg, CellFlags::empty());
    }

    // Fill row 1 with coalesced upper half blocks '▀'
    let green_fg = Color { r: 0, g: 255, b: 0 };
    let yellow_bg = Color {
        r: 255,
        g: 255,
        b: 0,
    };
    for col in 0..4 {
        grid.cells[80 + col] = Cell::new('▀', green_fg, yellow_bg, CellFlags::empty());
    }

    // Fill row 2 with shade characters
    for col in 0..4 {
        grid.cells[160 + col] = Cell::new('▒', red_fg, default_bg, CellFlags::empty());
    }

    grid.damage.mark_all();
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    // Verify row 0 full blocks: col 0..4 must be 0xFFFF0000 (red)
    let expected_red = 0xFFFF0000u32;
    let expected_blue = 0xFF0000FFu32;
    let expected_green = 0xFF00FF00u32;
    let expected_yellow = 0xFFFFFF00u32;

    for col in 0..4 {
        let center_x = (col as u32 * cw + cw / 2) as usize;
        let center_y = (ch / 2) as usize;
        let pixel = target[center_y * 800 + center_x];
        assert_eq!(pixel, expected_red, "Col {} must be red full block", col);
    }
    for col in 4..8 {
        let center_x = (col as u32 * cw + cw / 2) as usize;
        let center_y = (ch / 2) as usize;
        let pixel = target[center_y * 800 + center_x];
        assert_eq!(pixel, expected_blue, "Col {} must be blue full block", col);
    }

    // Verify row 1 upper half block: top half is green, bottom half is yellow
    for col in 0..4 {
        let center_x = (col as u32 * cw + cw / 2) as usize;
        let top_y = (ch + ch / 4) as usize;
        let bottom_y = (ch + (ch * 3) / 4) as usize;
        let top_pixel = target[top_y * 800 + center_x];
        let bottom_pixel = target[bottom_y * 800 + center_x];
        assert_eq!(
            top_pixel, expected_green,
            "Col {} top half must be green",
            col
        );
        assert_eq!(
            bottom_pixel, expected_yellow,
            "Col {} bottom half must be yellow",
            col
        );
    }

    // Verify scrollback scroll of colorblocks
    grid.scrollback.push_line(&grid.cells[0..80], false);
    grid.scroll_offset = 1;
    grid.damage.mark_all();
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        false,
        CursorShape::Block,
        0,
        true,
        1.0,
        &mut target,
    );

    // After scroll_offset = 1, row 0 in scrollback is displayed at y = 0
    let pixel = target[(ch / 2) as usize * 800 + (cw / 2) as usize];
    assert_eq!(
        pixel, expected_red,
        "Scrollback row 0 must be red full block"
    );
}

#[test]
fn test_software_renderer_csd_title_bar() {
    let mut renderer = setup_renderer(400, 200);
    let csd_h = 38u32;

    let layout = velox::window::csd::CsdButtonLayout::default();
    let csd_palette = velox::window::csd::CsdPalette::dark();

    // 1. Render focused title bar with Close hovered (not maximized -> rounded corners)
    renderer.render_csd_title_bar(
        "Velox - Shell",
        csd_h,
        true,
        false,
        Some(velox::window::csd::CsdButtonType::Close),
        None,
        &layout,
        &csd_palette,
    );

    let border_color = csd_palette.border;

    // Check bottom border at y = 37
    assert_eq!(
        renderer.framebuffer.pixels[37 * 400 + 50],
        border_color,
        "Row 37 must be the CSD bottom separator line"
    );

    // Top-left outer corner at (0, 0) is outside the 10px radius, so it must be transparent (0)
    assert_eq!(
        renderer.framebuffer.pixels[0], 0x00000000,
        "Row 0 col 0 should be transparent (rounded corner)"
    );

    // Inside the titlebar (e.g. x = 50, y = 19), background should be drawn
    let bg_focused = csd_palette.bg;
    assert_eq!(
        renderer.framebuffer.pixels[19 * 400 + 50],
        bg_focused,
        "Inside titlebar must have focused title bar background"
    );

    // Close button center is at x = 400 - 22 = 378, y = 19.
    // It is hovered, so it must render the GTK close red background (0x00E01B24)
    // and white icon pixels (0x00FFFFFF).
    let close_cx = 400 - 22;
    let close_cy = 19;
    let close_center_pixel = renderer.framebuffer.pixels[close_cy * 400 + close_cx];
    assert!(
        close_center_pixel == 0x00E01B24 || close_center_pixel == 0x00FFFFFF,
        "Hovered close button must show GTK red pill or white cross glyph"
    );

    // Row 38 (outside CSD title bar) must remain untouched (0)
    assert_eq!(
        renderer.framebuffer.pixels[38 * 400 + 10],
        0,
        "Row 38 must be untouched by CSD title bar"
    );

    // 2. Render unfocused title bar when maximized (square corners)
    renderer.render_csd_title_bar(
        "Velox - Shell",
        csd_h,
        false,
        true, // maximized
        None,
        None,
        &layout,
        &csd_palette,
    );
    // Row 0 col 0 when maximized has top border line
    assert_eq!(
        renderer.framebuffer.pixels[0], border_color,
        "Row 0 col 0 should have top border line when maximized"
    );
    assert_eq!(
        renderer.framebuffer.pixels[37 * 400 + 50],
        border_color,
        "Row 37 must still have border line when unfocused"
    );

    // 3. Render CSD window borders
    renderer.csd_height = csd_h;
    renderer.render_csd_window_borders(csd_palette.border);
    // Bottom-most row (y = 199) must have border color
    assert_eq!(
        renderer.framebuffer.pixels[199 * 400 + 50],
        border_color,
        "Bottom window edge must have border line"
    );
}

#[test]
fn test_software_renderer_tab_bar_with_csd_offset() {
    let mut renderer = setup_renderer(400, 200);
    let csd_h = 32u32;
    renderer.csd_height = csd_h;

    let tab_info = velox::app::tab::TabBarRenderInfo {
        height: 28.0,
        tabs: vec![
            velox::app::tab::TabHeaderInfo {
                title: "Tab 1".to_string(),
                is_active: true,
                is_hovered: false,
                is_close_hovered: false,
            },
            velox::app::tab::TabHeaderInfo {
                title: "Tab 2".to_string(),
                is_active: false,
                is_hovered: false,
                is_close_hovered: false,
            },
        ],
        show_new_tab: true,
        is_new_tab_hovered: false,
        show_close_button: true,
    };

    let grid = Grid::new(
        40,
        10,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        10,
        false,
    );
    let theme = Theme::new();
    let pane_rect = velox::app::split::PaneRect {
        pane_id: 1,
        x: 0.0,
        y: csd_h as f32 + 28.0,
        width: 400.0,
        height: 140.0,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: 40,
        rows: 10,
        cell_width: renderer.glyph_cache.cell_width as f32,
        cell_height: renderer.glyph_cache.cell_height as f32,
    };

    let pane_data = velox::renderer::CpuPaneRenderData {
        pane_id: 1,
        rect: pane_rect,
        cells: &grid.cells,
        grid: &grid,
        font_size: renderer.glyph_cache.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    // Render splits with tab bar and CSD offset
    renderer.render_splits(
        &[pane_data],
        &[],
        1.0,
        0.0,
        true,
        None,
        Some(&tab_info),
        None,
        None,
    );

    // Active tab top 2px has tab accent line at y = 32..34
    assert_eq!(
        renderer.framebuffer.pixels[32 * 400 + 10],
        renderer.palette.tab_accent,
        "Active tab accent line should start at y = csd_h (32)"
    );
    // Active tab body below accent line at y = 35 has active tab background
    assert_eq!(
        renderer.framebuffer.pixels[35 * 400 + 10],
        renderer.palette.default_bg,
        "Active tab body at y = 35 should have default_bg"
    );
}

#[test]
fn test_csd_gtk_window_buttons_and_restore_icon() {
    let mut renderer = setup_renderer(500, 200);
    let csd_h = 38u32;
    let layout = velox::window::csd::CsdButtonLayout::default();
    let csd_palette = velox::window::csd::CsdPalette::dark();

    // 1. Render when window is NOT maximized with Maximize button hovered
    renderer.render_csd_title_bar(
        "Velox",
        csd_h,
        true,
        false, // not maximized
        Some(velox::window::csd::CsdButtonType::Maximize),
        None,
        &layout,
        &csd_palette,
    );

    let max_cx = 500 - 22 - 32; // 446
    let max_cy = 19;
    // Square outline has a border at y0 = 14, x in 442..=450
    let border_px = renderer.framebuffer.pixels[14 * 500 + max_cx];
    assert_eq!(
        border_px, 0x00FFFFFF,
        "Maximize button outline must be white when hovered"
    );
    // Center of square (446, 19) is hollow and has highlight background
    let center_px = renderer.framebuffer.pixels[max_cy * 500 + max_cx];
    assert_ne!(
        center_px, 0x00FFFFFF,
        "Center of unmaximized square must be hollow"
    );

    // 2. Render when window IS maximized (Restore state)
    renderer.render_csd_title_bar(
        "Velox",
        csd_h,
        true,
        true, // maximized!
        None,
        None,
        &layout,
        &csd_palette,
    );
    // Back square top edge at by0 = 14, x = 447
    let restore_top = renderer.framebuffer.pixels[14 * 500 + (max_cx + 1)];
    assert_eq!(
        restore_top, csd_palette.fg,
        "Restore icon back square top must be drawn"
    );
}

#[test]
fn test_csd_theme_customization_and_light_dark_modes() {
    let mut renderer = setup_renderer(500, 200);
    let csd_h = 38u32;
    let layout = velox::window::csd::CsdButtonLayout::default();

    // 1. Light Mode Palette (GNOME Adwaita Light Headerbar)
    let light_palette = velox::window::csd::CsdPalette::light();
    assert!(light_palette.is_light);
    assert_eq!(light_palette.bg, 0xFFEBEBEB);
    assert_eq!(light_palette.fg, 0xFF2E3436);
    assert_eq!(light_palette.border, 0xFFD1D1D1);

    renderer.render_csd_title_bar(
        "Velox Light",
        csd_h,
        true,
        false,
        Some(velox::window::csd::CsdButtonType::Maximize),
        None,
        &layout,
        &light_palette,
    );

    // Titlebar background must be light Adwaita headerbar #EBEBEB
    assert_eq!(
        renderer.framebuffer.pixels[19 * 500 + 50],
        0xFFEBEBEB,
        "Light mode must render #EBEBEB headerbar background"
    );

    // In light mode, hovered maximize button outline must be dark/black (0x00000000)
    let max_cx = 500 - 22 - 32;
    let max_border_px = renderer.framebuffer.pixels[14 * 500 + max_cx];
    assert_eq!(
        max_border_px, 0x00000000,
        "Maximize button outline must be black in light mode"
    );

    // 2. Custom Config Overrides
    let custom_config = velox::config::config::WindowDecorationsConfig {
        theme: Some("custom".to_string()),
        bg: Some("#334455".to_string()),
        fg: Some("#ffffff".to_string()),
        inactive_bg: Some("#223344".to_string()),
        inactive_fg: Some("#8899aa".to_string()),
        border: Some("#112233".to_string()),
        close_fg: Some("#ff5555".to_string()),
    };

    let resolved = velox::window::csd::CsdPalette::resolve(Some(&custom_config), &renderer.palette);

    assert_eq!(resolved.bg, 0xFF334455, "Custom bg must be parsed");
    assert_eq!(resolved.fg, 0xFFFFFFFF, "Custom fg must be parsed");
    assert_eq!(
        resolved.inactive_bg, 0xFF223344,
        "Custom inactive_bg must be parsed"
    );
    assert_eq!(resolved.border, 0xFF112233, "Custom border must be parsed");
    assert_eq!(
        resolved.close_fg, 0xFFFF5555,
        "Custom close_fg must be parsed"
    );

    // 3. Seamless Terminal Theme Mode
    let terminal_palette = velox::window::csd::CsdPalette::terminal(&renderer.palette);
    assert_eq!(terminal_palette.bg, renderer.palette.default_bg);
    assert_eq!(terminal_palette.border, renderer.palette.default_bg);
    assert_eq!(terminal_palette.fg, renderer.palette.default_fg);
    assert_eq!(
        terminal_palette.close_fg, renderer.palette.ansi_colors[1],
        "Terminal CSD close_fg must use theme red color"
    );
    assert_eq!(
        terminal_palette.button_hover_bg,
        renderer.palette.tab_accent
    );

    renderer.render_csd_title_bar(
        "Velox Terminal Theme",
        csd_h,
        true,
        false,
        None,
        None,
        &layout,
        &terminal_palette,
    );

    // Row 19 inside titlebar and row 37 (bottom of titlebar) must match default_bg (no dividing gap line)
    assert_eq!(
        renderer.framebuffer.pixels[19 * 500 + 50],
        renderer.palette.default_bg,
        "Titlebar background must match terminal background"
    );
    assert_eq!(
        renderer.framebuffer.pixels[37 * 500 + 50],
        renderer.palette.default_bg,
        "Row 37 must match terminal background with zero gap/divider line"
    );

    // Close button icon (cross center at cx=478, cy=19) must be drawn with red theme color (close_fg)
    let close_px = renderer.framebuffer.pixels[19 * 500 + 478];
    assert_eq!(
        close_px, terminal_palette.close_fg,
        "Close button icon must use the terminal theme's red color instead of fg"
    );

    // Minimize button icon (line at cx=414, cy=22) must be drawn with standard fg
    let min_px = renderer.framebuffer.pixels[22 * 500 + 414];
    assert_eq!(
        min_px, terminal_palette.fg,
        "Minimize button icon must use standard fg"
    );
}

#[test]
fn test_gtk_theme_detection_and_css_palette() {
    let system_palette = velox::window::csd::CsdPalette::detect_system();
    // System palette must have valid opaque background and non-zero colors
    assert_ne!(system_palette.bg, 0);
    assert_ne!(system_palette.fg, 0);
    assert_eq!(
        (system_palette.bg >> 24) & 0xFF,
        0xFF,
        "Titlebar background must be fully opaque"
    );

    // Test resolving with theme = "terminal"
    let renderer = setup_renderer(500, 200);
    let terminal_config = velox::config::config::WindowDecorationsConfig {
        theme: Some("terminal".to_string()),
        ..Default::default()
    };
    let resolved =
        velox::window::csd::CsdPalette::resolve(Some(&terminal_config), &renderer.palette);
    assert_eq!(resolved.bg, renderer.palette.default_bg);
    assert_eq!(resolved.border, renderer.palette.default_bg);

    // If Mint-Y-Dark-Purple theme is on the machine, verify load_gtk_theme_palette directly
    if let Some(mint_palette) =
        velox::window::csd::load_gtk_theme_palette("Mint-Y-Dark-Purple", true)
    {
        assert_eq!(
            mint_palette.bg, 0xFF222226,
            "Mint-Y-Dark-Purple titlebar background must match CSS #222226"
        );
        assert_eq!(
            mint_palette.border, 0xFF18181B,
            "Mint-Y-Dark-Purple titlebar border must match CSS #18181b"
        );
        assert!(!mint_palette.is_light);
    }
}

#[test]
fn test_csd_optimizations_and_truncation() {
    let mut renderer = setup_renderer(300, 100);
    let csd_h = 38u32;
    let layout = velox::window::csd::CsdButtonLayout::default();
    let dark_palette = velox::window::csd::CsdPalette::dark();

    // 1. Title truncation verification with long title
    renderer.render_csd_title_bar(
        "A Very Long Title That Definitely Exceeds Safe Width Bounds And Needs Truncation",
        csd_h,
        true,
        false,
        None,
        None,
        &layout,
        &dark_palette,
    );

    // Verify top-left corner transparent outside (0, 0)
    assert_eq!(
        renderer.framebuffer.pixels[0], 0x00000000,
        "Corner pixel (0,0) must be transparent (outside window)"
    );

    // Verify top-left border arc at (9, 0) is border color
    assert_eq!(
        renderer.framebuffer.pixels[9], dark_palette.border,
        "Pixel (9,0) must be border arc color"
    );

    // Verify top-right corner pixel (299, 0) is transparent
    assert_eq!(
        renderer.framebuffer.pixels[299], 0x00000000,
        "Corner pixel (299,0) must be transparent (outside window)"
    );

    // Verify button hit testing returns None in dead space and Some on buttons
    assert_eq!(
        layout.hit_test(10.0, 10.0, 300, csd_h),
        None,
        "Hit test at (10, 10) should be in titlebar dead space"
    );

    // Close button is at rightmost: cx = 300 - 22 = 278, cy = 19
    assert_eq!(
        layout.hit_test(278.0, 19.0, 300, csd_h),
        Some(velox::window::csd::CsdButtonType::Close),
        "Hit test at (278, 19) should hit Close button"
    );

    // 2. Resize edge bitmask tests
    use velox::window::WindowResizeEdge;
    use velox::window::csd::detect_resize_edge;
    assert_eq!(
        detect_resize_edge(2.0, 2.0, 300, 100, 6.0),
        Some(WindowResizeEdge::TopLeft)
    );
    assert_eq!(
        detect_resize_edge(298.0, 2.0, 300, 100, 6.0),
        Some(WindowResizeEdge::TopRight)
    );
    assert_eq!(
        detect_resize_edge(2.0, 98.0, 300, 100, 6.0),
        Some(WindowResizeEdge::BottomLeft)
    );
    assert_eq!(
        detect_resize_edge(298.0, 98.0, 300, 100, 6.0),
        Some(WindowResizeEdge::BottomRight)
    );
    assert_eq!(
        detect_resize_edge(150.0, 2.0, 300, 100, 6.0),
        Some(WindowResizeEdge::Top)
    );
    assert_eq!(
        detect_resize_edge(150.0, 98.0, 300, 100, 6.0),
        Some(WindowResizeEdge::Bottom)
    );
    assert_eq!(
        detect_resize_edge(2.0, 50.0, 300, 100, 6.0),
        Some(WindowResizeEdge::Left)
    );
    assert_eq!(
        detect_resize_edge(298.0, 50.0, 300, 100, 6.0),
        Some(WindowResizeEdge::Right)
    );
    assert_eq!(detect_resize_edge(150.0, 50.0, 300, 100, 6.0), None);
}

#[test]
fn test_software_renderer_damage_rect_bounding_box() {
    let theme = velox::theme::theme::Theme::new();
    let mut renderer =
        velox::renderer::CpuRenderer::new("monospace", 14.0, 1.0, &theme, 400, 300, true, 1.0);
    let mut target = vec![0u32; 400 * 300];
    let mut grid =
        velox::screen::grid::Grid::new(20, 15, theme.default_fg, theme.default_bg, 100, false);

    let cell_h = renderer.glyph_cache.cell_height;

    let pane_rect = velox::app::split::PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 300.0,
        cols: 20,
        rows: 15,
        padding_x: 0.0,
        padding_y: 0.0,
        cell_width: renderer.glyph_cache.cell_width as f32,
        cell_height: cell_h as f32,
    };

    // 1. Initial render of pane 1
    let pane_data = velox::renderer::CpuPaneRenderData {
        pane_id: 1,
        rect: pane_rect,
        cells: &grid.cells,
        grid: &grid,
        font_size: 14.0,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: velox::screen::cursor::CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    let first_damage = renderer.render_splits(
        &[pane_data],
        &[],
        1.0,
        0.0,
        true,
        Some(&mut target),
        None,
        None,
        None,
    );
    assert!(first_damage.is_some(), "First frame should have damage");

    grid.damage.clear();

    // 2. Second frame: zero damage -> returns None
    let pane_data = velox::renderer::CpuPaneRenderData {
        pane_id: 1,
        rect: pane_rect,
        cells: &grid.cells,
        grid: &grid,
        font_size: 14.0,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: velox::screen::cursor::CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    let damage = renderer.render_splits(
        &[pane_data],
        &[],
        1.0,
        0.0,
        true,
        Some(&mut target),
        None,
        None,
        None,
    );
    assert_eq!(damage, None, "Zero damage frame should return None");

    // 3. Third frame: single row damage at row 5
    grid.damage.dirty_rows[5] = true;
    let pane_data = velox::renderer::CpuPaneRenderData {
        pane_id: 1,
        rect: pane_rect,
        cells: &grid.cells,
        grid: &grid,
        font_size: 14.0,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: velox::screen::cursor::CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    let damage = renderer.render_splits(
        &[pane_data],
        &[],
        1.0,
        0.0,
        true,
        Some(&mut target),
        None,
        None,
        None,
    );
    assert!(
        damage.is_some(),
        "Damaged row should return Some(DamageRect)"
    );
    let rect = damage.unwrap();
    let expected_y = 5 * cell_h;
    assert_eq!(
        rect.y, expected_y,
        "DamageRect y should match dirty row top"
    );
    assert_eq!(
        rect.height, cell_h,
        "DamageRect height should match dirty row height"
    );
}

#[test]
fn test_renderer_scrollback_row_wider_than_pane() {
    let theme = Theme::default();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let fg = theme.default_fg;
    let bg = theme.default_bg;

    // Grid was originally 80 columns wide
    let mut grid = Grid::new(80, 24, fg, bg, 100, false);
    for c in "This is a long line that was stored in scrollback when window was wide".chars() {
        grid.put_char(c, fg, bg, None, CellFlags::empty());
    }
    grid.scroll(1, bg);
    assert_eq!(grid.scrollback.len(), 1);

    // Scrollback row has 70 chars. Now pane is narrowed to 40 columns.
    grid.width = 40;
    grid.scroll_offset = 1;
    grid.damage.mark_all();

    let cell_w = renderer.glyph_cache.cell_width;
    let cell_h = renderer.glyph_cache.cell_height;
    let pane_rect = velox::app::split::PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 40.0 * cell_w as f32,
        height: 24.0 * cell_h as f32,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: 40,
        rows: 24,
        cell_width: cell_w as f32,
        cell_height: cell_h as f32,
    };

    let pane_data = velox::renderer::CpuPaneRenderData {
        pane_id: 1,
        rect: pane_rect,
        cells: &grid.cells,
        grid: &grid,
        font_size: 14.0,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: true,
        cursor_shape: velox::screen::cursor::CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    let mut target = vec![0u32; 800 * 600];
    // Must render without panicking even though scrollback row (70 cells) is wider than pane (40 cells)
    let _ = renderer.render_splits(
        &[pane_data],
        &[],
        1.0,
        0.0,
        true,
        Some(&mut target),
        None,
        None,
        None,
    );
}

#[test]
fn test_software_renderer_hidden_cursor_with_decscusr_menu_navigation() {
    use velox::renderer::CpuRenderer;
    use velox::screen::cursor::CursorShape;
    use velox::terminal::terminal::Terminal;

    let mut term = Terminal::new(80, 24);
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &term.theme, 800, 600, true, 1.0);
    let mut target = vec![0u32; 800 * 600];
    let cell_w = renderer.glyph_cache.cell_width;
    let cell_h = renderer.glyph_cache.cell_height;

    // Simulate gh cs ssh / fzf startup: enter alternate screen, hide cursor, set block shape
    term.feed(b"\x1b[?1049h\x1b[?25l\x1b[2 q");
    assert!(term.is_alt_screen);
    assert_eq!(term.active_grid().cursor.shape, CursorShape::Block);
    assert!(!term.active_grid().cursor.visible);

    // Write a menu item
    term.feed(b"> Item 1: codespace-alpha\r\n  Item 2: codespace-beta");

    // Calculate cursor visibility as in App::prepare_pane_render_data
    let is_active = true;
    let offset = term.active_grid().scroll_offset;
    let cursor_blink_enabled = true;
    let cursor_blink_on = true;
    let cursor_visible = is_active
        && offset == 0
        && term.active_grid().cursor.visible
        && (!cursor_blink_enabled || cursor_blink_on);
    assert!(
        !cursor_visible,
        "Cursor must not be visible when DECTCEM is reset"
    );

    renderer.render(
        &term.active_grid().cells,
        term.active_grid(),
        &term.theme,
        0.0,
        0.0,
        cursor_visible,
        term.active_grid().cursor.shape,
        term.active_grid().cursor.x,
        true,
        1.0,
        &mut target,
    );
    term.active_grid_mut().clear_damage();

    // Verify cursor color is NOT drawn at cursor position (x=24, y=1)
    let cur_x = term.active_grid().cursor.x as u32;
    let cur_y = term.active_grid().cursor.y as u32;
    let cur_px = cur_x * cell_w + cell_w / 2;
    let cur_py = cur_y * cell_h + cell_h / 2;
    let pixel = target[(cur_py as usize) * 800 + (cur_px as usize)];
    let cursor_color = velox::renderer::color::PackedColor::from_color(
        term.theme.resolve_cursor_color(term.theme.default_fg),
    )
    .to_u32();
    assert_ne!(
        pixel, cursor_color,
        "Cursor color must NOT be rendered when cursor is hidden"
    );

    // Simulate arrow down to move selection: cursor moves to row 1, col 0 and re-emits CSI 2 q
    term.feed(b"\x1b[2;1H\x1b[2 q");
    assert_eq!(term.active_grid().cursor.y, 1);
    assert_eq!(term.active_grid().cursor.x, 0);
    assert!(
        !term.active_grid().cursor.visible,
        "DECSCUSR must not show cursor"
    );

    let cursor_visible = is_active
        && offset == 0
        && term.active_grid().cursor.visible
        && (!cursor_blink_enabled || cursor_blink_on);

    renderer.render(
        &term.active_grid().cells,
        term.active_grid(),
        &term.theme,
        0.0,
        0.0,
        cursor_visible,
        term.active_grid().cursor.shape,
        term.active_grid().cursor.x,
        true,
        1.0,
        &mut target,
    );
    term.active_grid_mut().clear_damage();

    // Sample at new cursor position (0, 1): text is ' ' or default_bg, NOT cursor color
    let new_px = 2;
    let new_py = cell_h + 2;
    let new_pixel = target[(new_py as usize) * 800 + (new_px as usize)];
    assert_ne!(
        new_pixel, cursor_color,
        "Cursor color must NOT be rendered at new cursor position when cursor is hidden"
    );
}
