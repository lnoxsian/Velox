use velox::renderer::software::CpuRenderer;
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

    grid.cursor.x = 5;
    grid.cursor.y = 2;
    grid.cursor.shape = CursorShape::Block;
    grid.cursor.visible = true;

    // Start selection
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
    assert!(!renderer.damage.has_damage());
    // Verify that selected cell at (3, 2) has inverted background (grid.default_fg)
    let cell_w = renderer.glyph_cache.cell_width;
    let cell_h = renderer.glyph_cache.cell_height;
    let sample_x = 8 + 3 * cell_w + 2;
    let sample_y = 4 + 2 * cell_h + 2;
    let expected_inverted_bg =
        velox::renderer::software::color::PackedColor::from_color(grid.default_fg).to_u32();
    assert_eq!(
        target[(sample_y as usize) * 800 + (sample_x as usize)],
        expected_inverted_bg,
        "Selected cell background must be inverted to grid.default_fg"
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
        velox::renderer::software::color::PackedColor::from_color(grid.default_fg).to_u32();
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
        velox::renderer::software::color::PackedColor::from_color(grid.default_bg).to_u32();
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
        velox::renderer::software::color::PackedColor::from_color(theme.default_bg).to_u32();

    // Check row 0 (which is in scrollback) at column 40 (beyond text "line-1")
    let test_x = 40 * cell_w + 2;
    let test_y = 2;
    let actual_pixel = target[test_y * 800 + test_x];

    assert_eq!(
        actual_pixel, expected_bg_packed,
        "Trailing columns in scrollback must maintain the theme's default background color and not turn black"
    );
}
