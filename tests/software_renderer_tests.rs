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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
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
    renderer.enable_stats = true;
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

    // Should touch 0 dirty rows
    assert_eq!(renderer.stats.dirty_rows, 0);
    assert_eq!(renderer.stats.dirty_cells, 0);
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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
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
    grid.cells[5 * 80 + 10] = Cell {
        character: 'A',
        foreground: Color {
            r: 255,
            g: 0,
            b: 0,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        flags: CellFlags::BOLD,
    };

    renderer.enable_stats = true;
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

    // Only 1 dirty row rendered
    assert_eq!(renderer.stats.dirty_rows, 1);
    assert_eq!(renderer.stats.dirty_cells, 80);
}

#[test]
fn test_software_renderer_box_and_block_primitives() {
    let mut renderer = setup_renderer(800, 600);
    let mut grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
        100,
        false,
    );
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    let test_primitives = [
        '█', '▀', '▄', '▌', '▐', '─', '│', '┌', '┐', '└', '┘', '┼', '═', '║',
    ];
    for (i, &ch) in test_primitives.iter().enumerate() {
        grid.cells[i] = Cell {
            character: ch,
            foreground: Color {
                r: 0,
                g: 255,
                b: 0,
            },
            background: Color {
                r: 0,
                g: 0,
                b: 0,
            },
            flags: CellFlags::empty(),
        };
    }

    grid.damage.mark_dirty(0);
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );

    // Check that some green pixels (0xFF00FF00) were drawn in the framebuffer
    let has_green_pixels = target.iter().any(|&p| (p & 0x00FFFFFF) == 0x0000FF00);
    assert!(has_green_pixels);
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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
        100,
        false,
    );
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Populate lines 0..15 so scrollback has 5 lines and active grid has 10 lines
    for i in 0..15 {
        for c in format!("line-{}", i).chars() {
            grid.put_char(c, grid.default_fg, grid.default_bg, CellFlags::empty());
        }
        if i < 14 {
            grid.scroll_or_move_down(grid.default_bg);
            grid.cursor.x = 0;
        }
    }

    assert_eq!(grid.scrollback.len(), 5);

    // Select row 2 (which is in scrollback)
    grid.selection.start_selection(0, 2);
    grid.selection.update_selection(5, 2);

    // Scroll up by 3 lines so row 2 is visible at viewport row 0 (since history_len=5, scroll_offset=3, abs_row=5-3+0=2)
    grid.scroll_offset = 3;

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

    // Render should succeed with damage resolved
    assert!(!renderer.damage.has_damage());
    assert_eq!(grid.extract_selection_text(), "line-2");
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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
        100,
        false,
    );
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    grid.cells[0] = Cell {
        character: 'U',
        foreground: Color {
            r: 255,
            g: 255,
            b: 0,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        flags: CellFlags::UNDERLINE,
    };
    grid.cells[1] = Cell {
        character: 'D',
        foreground: Color {
            r: 255,
            g: 255,
            b: 0,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        flags: CellFlags::DOUBLE_UNDERLINE,
    };
    grid.cells[2] = Cell {
        character: 'C',
        foreground: Color {
            r: 255,
            g: 255,
            b: 0,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        flags: CellFlags::CURLY_UNDERLINE,
    };
    grid.cells[3] = Cell {
        character: 'S',
        foreground: Color {
            r: 255,
            g: 255,
            b: 0,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        flags: CellFlags::STRIKE,
    };

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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
        100,
        true,
    );
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Push historical row into scrollback
    let scroll_cell = Cell {
        character: 'H',
        foreground: Color {
            r: 255,
            g: 0,
            b: 0,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        flags: CellFlags::empty(),
    };
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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
        100,
        false,
    );
    let theme = Theme::new();
    let mut target = vec![0u32; 800 * 600];

    // Create a blinking green 'A' cell
    grid.cells[0] = Cell {
        character: 'A',
        foreground: Color {
            r: 0,
            g: 255,
            b: 0,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
        },
        flags: CellFlags::BLINK,
    };
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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
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
            grid.cells[y * 80 + x] = Cell {
                character: ch,
                foreground: Color {
                    r: 0,
                    g: 255,
                    b: 255,
                },
                background: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                },
                flags: CellFlags::empty(),
            };
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
        Color {
            r: 0,
            g: 0,
            b: 0,
        },
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
                grid.cells[y * 80 + x] = Cell {
                    character: ch,
                    foreground: Color {
                        r: 255,
                        g: 255,
                        b: 0,
                    },
                    background: Color {
                        r: 0,
                        g: 0,
                        b: 0,
                    },
                    flags: CellFlags::empty(),
                };
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
    let mut grid = Grid::new(
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
