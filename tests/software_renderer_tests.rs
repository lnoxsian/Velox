use velox::renderer::software::CpuRenderer;
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::cursor::CursorShape;
use velox::screen::grid::Grid;
use velox::theme::theme::Theme;

fn setup_renderer(width: u32, height: u32) -> CpuRenderer {
    let theme = Theme::new();
    CpuRenderer::new("monospace", 14.0, 1.5, &theme, width, height, true)
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
            a: 255,
        },
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
            a: 255,
        },
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
            a: 255,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
            a: 255,
        },
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
                a: 255,
            },
            background: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
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
        &mut target,
    );

    // Check that some green pixels (0x0000FF00) were drawn in the framebuffer
    let has_green_pixels = target.contains(&0x0000FF00);
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
            a: 255,
        },
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
        &mut target,
    );
    assert!(!renderer.damage.has_damage());
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
            a: 255,
        },
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
            a: 255,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        flags: CellFlags::UNDERLINE,
    };
    grid.cells[1] = Cell {
        character: 'D',
        foreground: Color {
            r: 255,
            g: 255,
            b: 0,
            a: 255,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        flags: CellFlags::DOUBLE_UNDERLINE,
    };
    grid.cells[2] = Cell {
        character: 'C',
        foreground: Color {
            r: 255,
            g: 255,
            b: 0,
            a: 255,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        flags: CellFlags::CURLY_UNDERLINE,
    };
    grid.cells[3] = Cell {
        character: 'S',
        foreground: Color {
            r: 255,
            g: 255,
            b: 0,
            a: 255,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
            a: 255,
        },
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
            a: 255,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
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
        &mut target,
    );
    assert!(!renderer.damage.has_damage());
}

#[test]
fn test_software_renderer_scroll_down() {
    let mut fb = velox::renderer::software::Framebuffer::new(10, 10);
    fb.fill_span(0, 0, 10, 1, 0x00FF0000);
    fb.scroll_region_down(0, 10, 1, 0);
    assert_eq!(fb.pixels[10], 0x00FF0000);
    assert_eq!(fb.pixels[0], 0);
}
