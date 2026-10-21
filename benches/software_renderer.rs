use std::time::Instant;
use velox::renderer::CpuRenderer;
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::grid::Grid;
use velox::theme::theme::Theme;

fn main() {
    println!("=== Velox CPU Software Renderer Benchmark Suite ===");
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 1920, 1080, true, 1.0);
    let mut target = vec![0u32; 1920 * 1080];

    let cols = 1920 / renderer.glyph_cache.cell_width;
    let rows = 1080 / renderer.glyph_cache.cell_height;
    println!(
        "Resolution: 1920x1080 (Grid: {} cols x {} rows, Cell: {}x{} px)",
        cols, rows, renderer.glyph_cache.cell_width, renderer.glyph_cache.cell_height
    );

    let mut grid = Grid::new(
        cols as usize,
        rows as usize,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        1000,
        false,
    );

    // Warm up
    renderer.render(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        true,
        velox::screen::cursor::CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        &mut target,
    );

    // 1. Idle Benchmark (0-damage)
    grid.damage.clear();
    let iterations = 5_000u32;
    let start = Instant::now();
    for _ in 0..iterations {
        renderer.render(
            &grid.cells,
            &grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "1. Idle 0-Damage Frame: {:?} per frame ({:.2} million fps)",
        elapsed / iterations,
        (iterations as f64) / elapsed.as_secs_f64() / 1_000_000.0
    );

    // 1b. Single Dirty Row Update Benchmark (Task 6 & Task 8)
    let single_row_iters = 500u32;
    let start = Instant::now();
    for i in 0..single_row_iters {
        grid.damage.clear();
        grid.damage.mark_dirty(5);
        grid.cells[5 * cols as usize + 10].character = ((33 + i % 94) as u8) as char;
        renderer.render(
            &grid.cells,
            &grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "1b. Single Dirty Row 1080p Update: {:?} per frame ({:.2} fps)",
        elapsed / single_row_iters,
        (single_row_iters as f64) / elapsed.as_secs_f64()
    );

    // 2. ASCII Full-Screen Redraw Benchmark
    for y in 0..rows as usize {
        for x in 0..cols as usize {
            grid.cells[y * cols as usize + x] = Cell::new(
                ((33 + (x + y) % 94) as u8) as char,
                Color {
                    r: 220,
                    g: 220,
                    b: 220,
                },
                Color {
                    r: 15,
                    g: 15,
                    b: 15,
                },
                CellFlags::empty(),
            );
        }
    }
    let iters = 50u32;
    let start = Instant::now();
    for _ in 0..iters {
        grid.damage.dirty_rows.fill(true);
        renderer.render(
            &grid.cells,
            &grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "2. Full-Screen ASCII 1080p Redraw: {:?} per frame ({:.2} fps)",
        elapsed / iters,
        (iters as f64) / elapsed.as_secs_f64()
    );

    // 3. Span-Merged Backgrounds (ANSI TrueColor Blocks)
    for y in 0..rows as usize {
        for x in 0..cols as usize {
            let bg_r = ((x * 255) / cols as usize) as u8;
            let bg_g = ((y * 255) / rows as usize) as u8;
            let bg_b = 128u8;
            grid.cells[y * cols as usize + x] = Cell::new(
                ' ',
                Color {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Color {
                    r: bg_r,
                    g: bg_g,
                    b: bg_b,
                },
                CellFlags::empty(),
            );
        }
    }
    let start = Instant::now();
    for _ in 0..iters {
        grid.damage.dirty_rows.fill(true);
        renderer.render(
            &grid.cells,
            &grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "3. Full-Screen Background Span Merging: {:?} per frame ({:.2} fps)",
        elapsed / iters,
        (iters as f64) / elapsed.as_secs_f64()
    );

    // 4. Box Drawing & Block Element Geometric Primitives
    let box_chars = [
        '─', '│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼', '█', '▀', '▄',
    ];
    for y in 0..rows as usize {
        for x in 0..cols as usize {
            grid.cells[y * cols as usize + x] = Cell::new(
                box_chars[(x + y) % box_chars.len()],
                Color {
                    r: 0,
                    g: 255,
                    b: 128,
                },
                Color {
                    r: 20,
                    g: 20,
                    b: 30,
                },
                CellFlags::empty(),
            );
        }
    }
    let start = Instant::now();
    for _ in 0..iters {
        grid.damage.dirty_rows.fill(true);
        renderer.render(
            &grid.cells,
            &grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "4. Box Drawing & Block Primitives (TUI): {:?} per frame ({:.2} fps)",
        elapsed / iters,
        (iters as f64) / elapsed.as_secs_f64()
    );

    // 4b. Unicode & Emoji Rendering (Mixed CJK, Nerd Fonts, and Emoji)
    let unicode_chars = [
        '你', '好', '世', '界', '🦀', '📦', '🚀', '😀', '\u{e0a0}', '\u{e0b0}', 'A', 'Z',
    ];
    for y in 0..rows as usize {
        for x in 0..cols as usize {
            grid.cells[y * cols as usize + x] = Cell::new(
                unicode_chars[(x + y) % unicode_chars.len()],
                Color {
                    r: 255,
                    g: 200,
                    b: 100,
                },
                Color {
                    r: 20,
                    g: 20,
                    b: 30,
                },
                CellFlags::empty(),
            );
        }
    }
    let u_iters = 20u32;
    let start = Instant::now();
    for _ in 0..u_iters {
        grid.damage.mark_all();
        renderer.render(
            &grid.cells,
            &grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "4b. Unicode & Emoji Rendering (CJK, Nerd Font, Color Emoji): {:?} per frame ({:.2} fps)",
        elapsed / u_iters,
        (u_iters as f64) / elapsed.as_secs_f64()
    );

    // 5. High-Throughput Scrolling Benchmark (1 line scroll + 1 new dirty row)
    let scroll_iters = 500u32;
    let start = Instant::now();
    for i in 0..scroll_iters {
        // Fast shift in framebuffer memory
        renderer
            .framebuffer
            .scroll_region_up(0, 1080, renderer.glyph_cache.cell_height, 0);
        // Only mark bottom row dirty
        grid.damage.dirty_rows.fill(false);
        grid.damage.dirty_rows[(rows - 1) as usize] = true;
        for x in 0..cols as usize {
            grid.cells[(rows - 1) as usize * cols as usize + x] = Cell::new(
                ((33 + (i + x as u32) % 94) as u8) as char,
                Color {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Color { r: 0, g: 0, b: 0 },
                CellFlags::empty(),
            );
        }
        renderer.render(
            &grid.cells,
            &grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "5. High-Throughput Terminal Scrolling: {:?} per row ({:.2} lines/sec)",
        elapsed / scroll_iters,
        (scroll_iters as f64) / elapsed.as_secs_f64()
    );

    // 6. Split Pane Isolated Damage Rendering (4 Panes, only 1 pane damaged)
    let pane_w = 960.0;
    let pane_h = 540.0;
    let p_cols = (pane_w / renderer.glyph_cache.cell_width as f32) as usize;
    let p_rows = (pane_h / renderer.glyph_cache.cell_height as f32) as usize;

    let mut p_grid1 = Grid::new(
        p_cols,
        p_rows,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    let p_grid2 = Grid::new(
        p_cols,
        p_rows,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    let p_grid3 = Grid::new(
        p_cols,
        p_rows,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    let p_grid4 = Grid::new(
        p_cols,
        p_rows,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );

    let rect1 = velox::app::split::PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: pane_w,
        height: pane_h,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: p_cols,
        rows: p_rows,
        cell_width: renderer.glyph_cache.cell_width as f32,
        cell_height: renderer.glyph_cache.cell_height as f32,
    };
    let rect2 = velox::app::split::PaneRect {
        pane_id: 2,
        x: pane_w,
        y: 0.0,
        width: pane_w,
        height: pane_h,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: p_cols,
        rows: p_rows,
        cell_width: renderer.glyph_cache.cell_width as f32,
        cell_height: renderer.glyph_cache.cell_height as f32,
    };
    let rect3 = velox::app::split::PaneRect {
        pane_id: 3,
        x: 0.0,
        y: pane_h,
        width: pane_w,
        height: pane_h,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: p_cols,
        rows: p_rows,
        cell_width: renderer.glyph_cache.cell_width as f32,
        cell_height: renderer.glyph_cache.cell_height as f32,
    };
    let rect4 = velox::app::split::PaneRect {
        pane_id: 4,
        x: pane_w,
        y: pane_h,
        width: pane_w,
        height: pane_h,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: p_cols,
        rows: p_rows,
        cell_width: renderer.glyph_cache.cell_width as f32,
        cell_height: renderer.glyph_cache.cell_height as f32,
    };

    // Initial frame
    {
        let split_panes = [
            velox::renderer::CpuPaneRenderData {
                pane_id: 1,
                rect: rect1,
                cells: &p_grid1.cells,
                grid: &p_grid1,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: true,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: true,
            },
            velox::renderer::CpuPaneRenderData {
                pane_id: 2,
                rect: rect2,
                cells: &p_grid2.cells,
                grid: &p_grid2,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: false,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: false,
            },
            velox::renderer::CpuPaneRenderData {
                pane_id: 3,
                rect: rect3,
                cells: &p_grid3.cells,
                grid: &p_grid3,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: false,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: false,
            },
            velox::renderer::CpuPaneRenderData {
                pane_id: 4,
                rect: rect4,
                cells: &p_grid4.cells,
                grid: &p_grid4,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: false,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: false,
            },
        ];
        renderer.render_splits(
            &split_panes,
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

    let multi_iters = 500u32;
    let start = Instant::now();
    for _ in 0..multi_iters {
        p_grid1.damage.dirty_rows.fill(false);
        p_grid1.damage.dirty_rows[5] = true;
        let split_panes = [
            velox::renderer::CpuPaneRenderData {
                pane_id: 1,
                rect: rect1,
                cells: &p_grid1.cells,
                grid: &p_grid1,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: true,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: true,
            },
            velox::renderer::CpuPaneRenderData {
                pane_id: 2,
                rect: rect2,
                cells: &p_grid2.cells,
                grid: &p_grid2,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: false,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: false,
            },
            velox::renderer::CpuPaneRenderData {
                pane_id: 3,
                rect: rect3,
                cells: &p_grid3.cells,
                grid: &p_grid3,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: false,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: false,
            },
            velox::renderer::CpuPaneRenderData {
                pane_id: 4,
                rect: rect4,
                cells: &p_grid4.cells,
                grid: &p_grid4,
                font_size: 14.0,
                theme: &theme,
                bold_is_bright: true,
                cursor_visible: false,
                cursor_shape: velox::screen::cursor::CursorShape::Block,
                display_cursor_x: 0,
                is_active: false,
            },
        ];
        renderer.render_splits(
            &split_panes,
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
    let elapsed = start.elapsed();
    println!(
        "6. 4-Split Panes Isolated Row Damage: {:?} per frame ({:.2} fps)",
        elapsed / multi_iters,
        (multi_iters as f64) / elapsed.as_secs_f64()
    );

    // 7. Scrolling Through Colorblocks in Scrollback
    let mut cb_grid = Grid::new(
        cols as usize,
        rows as usize,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        1000,
        false,
    );
    for _row_idx in 0..500 {
        let mut row_cells = Vec::with_capacity(cols as usize);
        for col_idx in 0..cols as usize {
            let color = match (col_idx / 4) % 8 {
                0 => Color { r: 0, g: 0, b: 0 },
                1 => Color { r: 205, g: 0, b: 0 },
                2 => Color { r: 0, g: 205, b: 0 },
                3 => Color {
                    r: 205,
                    g: 205,
                    b: 0,
                },
                4 => Color { r: 0, g: 0, b: 238 },
                5 => Color {
                    r: 205,
                    g: 0,
                    b: 205,
                },
                6 => Color {
                    r: 0,
                    g: 205,
                    b: 205,
                },
                _ => Color {
                    r: 229,
                    g: 229,
                    b: 229,
                },
            };
            row_cells.push(Cell::new(
                '█',
                color,
                Color { r: 0, g: 0, b: 0 },
                CellFlags::empty(),
            ));
        }
        cb_grid.scrollback.push_line(&row_cells, false);
    }

    let scroll_iters = 100u32;
    let start = Instant::now();
    for i in 0..scroll_iters {
        cb_grid.scroll_offset = (i as usize * 2) % 400;
        cb_grid.damage.mark_all();
        renderer.render(
            &cb_grid.cells,
            &cb_grid,
            &theme,
            0.0,
            0.0,
            true,
            velox::screen::cursor::CursorShape::Block,
            cb_grid.cursor.x,
            true,
            1.0,
            &mut target,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "7. Scrolling Through Colorblocks (Scrollback): {:?} per frame ({:.2} fps)",
        elapsed / scroll_iters,
        (scroll_iters as f64) / elapsed.as_secs_f64()
    );

    // 7b. Transparency Comparison (opacity 1.0 vs 0.8 vs 0.5)
    for &test_opacity in &[1.0f32, 0.8f32, 0.5f32] {
        let t_iters = 20u32;
        let start = Instant::now();
        for _ in 0..t_iters {
            cb_grid.damage.mark_all();
            renderer.render(
                &cb_grid.cells,
                &cb_grid,
                &theme,
                0.0,
                0.0,
                true,
                velox::screen::cursor::CursorShape::Block,
                cb_grid.cursor.x,
                true,
                test_opacity,
                &mut target,
            );
        }
        let elapsed = start.elapsed();
        println!(
            "7b. Transparency Render (opacity {:.1}): {:?} per frame ({:.2} fps)",
            test_opacity,
            elapsed / t_iters,
            (t_iters as f64) / elapsed.as_secs_f64()
        );
    }

    // 8. CSD Titlebar Rendering and Hit-Testing Throughput
    let csd_palette = velox::window::csd::CsdPalette::dark();
    let button_layout = velox::window::csd::CsdButtonLayout::default();
    let csd_iters = 10_000u32;
    let start = Instant::now();
    for _ in 0..csd_iters {
        renderer.render_csd_title_bar(
            "Velox Terminal - /home/lnoxsian/projects/velox",
            38,
            true,
            false,
            Some(velox::window::csd::CsdButtonType::Close),
            None,
            &button_layout,
            &csd_palette,
        );
    }
    let elapsed = start.elapsed();
    println!(
        "8a. CSD Titlebar Render (Table Mask & Scanlines): {:?} per call ({:.2} kfps)",
        elapsed / csd_iters,
        (csd_iters as f64) / elapsed.as_secs_f64() / 1_000.0
    );

    let hit_test_iters = 1_000_000u32;
    let start = Instant::now();
    let mut hit_count = 0usize;
    for i in 0..hit_test_iters {
        let px = (i % 1920) as f32;
        let py = (i % 38) as f32;
        if button_layout.hit_test(px, py, 1920, 38).is_some() {
            hit_count += 1;
        }
    }
    let elapsed = start.elapsed();
    println!(
        "8b. CSD Button Hit-Testing (Zero Heap Alloc): {:?} per test ({:.2} million ops/sec, hits: {})",
        elapsed / hit_test_iters,
        (hit_test_iters as f64) / elapsed.as_secs_f64() / 1_000_000.0,
        hit_count
    );

    let resize_iters = 1_000_000u32;
    let start = Instant::now();
    let mut edge_count = 0usize;
    for i in 0..resize_iters {
        let px = (i % 1920) as f32;
        let py = (i % 1080) as f32;
        if velox::window::csd::detect_resize_edge(px, py, 1920, 1080, 6.0).is_some() {
            edge_count += 1;
        }
    }
    let elapsed = start.elapsed();
    println!(
        "8c. Window Resize Edge Detection (Bitmask): {:?} per test ({:.2} million ops/sec, hits: {})",
        elapsed / resize_iters,
        (resize_iters as f64) / elapsed.as_secs_f64() / 1_000_000.0,
        edge_count
    );

    println!("=== Benchmark Complete ===");
}
