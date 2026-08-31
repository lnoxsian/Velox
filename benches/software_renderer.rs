use std::time::Instant;
use velox::renderer::software::CpuRenderer;
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
    grid.damage.dirty_rows.fill(false);
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

    println!("=== Benchmark Complete ===");
}
