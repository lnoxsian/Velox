use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::scrollback::{SCROLLBACK_CHUNK_ROWS, Scrollback};

#[test]
fn test_infinite_scrollback_million_lines_memory_bounded() {
    let hot_limit = 2000;
    let mut scrollback = Scrollback::new(hot_limit, true);

    let total_lines = 100_000usize; // 100,000 lines test
    for i in 0..total_lines {
        let cell = Cell {
            character: (b'0' + (i % 10) as u8) as char,
            foreground: Color {
                r: (i % 255) as u8,
                g: ((i * 3) % 255) as u8,
                b: ((i * 7) % 255) as u8,
                a: 255,
            },
            background: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            flags: if i % 2 == 0 {
                CellFlags::BOLD
            } else {
                CellFlags::UNDERLINE
            },
        };
        scrollback.push_line(&[cell], i % 3 == 0);
    }

    assert_eq!(scrollback.len(), total_lines);

    // Verify chunking on disk
    let expected_chunks = (total_lines - hot_limit) / SCROLLBACK_CHUNK_ROWS;
    assert_eq!(scrollback.chunk_count(), expected_chunks);
    assert_eq!(
        scrollback.disk_rows(),
        (expected_chunks * SCROLLBACK_CHUNK_ROWS) as u64
    );

    // Verify RAM resident rows is strictly bounded to hot rows + pending chunk + cache (4 * 512)
    let max_allowed_ram_rows = hot_limit + SCROLLBACK_CHUNK_ROWS + 4 * SCROLLBACK_CHUNK_ROWS;
    assert!(
        scrollback.resident_row_count() <= max_allowed_ram_rows,
        "Resident rows ({}) exceeded bounded limit ({})",
        scrollback.resident_row_count(),
        max_allowed_ram_rows
    );

    // Verify random historical lookups across disk chunks
    for &target in &[0, 511, 512, 1024, 25000, 50000, 75000, 99999] {
        let row = scrollback
            .get_row(target)
            .unwrap_or_else(|| panic!("Failed to get row {}", target));
        let expected_ch = (b'0' + (target % 10) as u8) as char;
        assert_eq!(
            row.cells[0].character, expected_ch,
            "Mismatch at line {}",
            target
        );
        let expected_flag = if target % 2 == 0 {
            CellFlags::BOLD
        } else {
            CellFlags::UNDERLINE
        };
        assert_eq!(
            row.cells[0].flags, expected_flag,
            "Flag mismatch at line {}",
            target
        );
        assert_eq!(row.wrapped, target % 3 == 0);
    }
}

#[test]
fn test_infinite_scrollback_all_cell_attributes_intact() {
    let mut scrollback = Scrollback::new(10, true);

    let test_cells = vec![
        Cell {
            character: 'W',
            foreground: Color {
                r: 12,
                g: 34,
                b: 56,
                a: 255,
            },
            background: Color {
                r: 78,
                g: 90,
                b: 12,
                a: 255,
            },
            flags: CellFlags::BOLD | CellFlags::ITALIC,
        },
        Cell {
            character: '🚀',
            foreground: Color {
                r: 255,
                g: 200,
                b: 100,
                a: 255,
            },
            background: Color {
                r: 50,
                g: 60,
                b: 70,
                a: 255,
            },
            flags: CellFlags::WIDE,
        },
        Cell {
            character: ' ',
            foreground: Color {
                r: 255,
                g: 200,
                b: 100,
                a: 255,
            },
            background: Color {
                r: 50,
                g: 60,
                b: 70,
                a: 255,
            },
            flags: CellFlags::WIDE_CONTINUATION,
        },
        Cell {
            character: 'U',
            foreground: Color {
                r: 11,
                g: 22,
                b: 33,
                a: 255,
            },
            background: Color {
                r: 44,
                g: 55,
                b: 66,
                a: 255,
            },
            flags: CellFlags::CURLY_UNDERLINE | CellFlags::STRIKE,
        },
    ];

    // Push 600 lines so that the first lines are written to a disk chunk
    for i in 0..600 {
        scrollback.push_line(&test_cells, i % 2 == 0);
    }

    assert!(scrollback.chunk_count() >= 1);

    // Retrieve line 0 from disk
    let row_0 = scrollback.get_row(0).expect("Line 0 should exist");
    assert_eq!(row_0.cells.len(), 4);
    assert_eq!(row_0.cells[0].character, 'W');
    assert_eq!(row_0.cells[0].flags, CellFlags::BOLD | CellFlags::ITALIC);
    assert_eq!(
        row_0.cells[0].foreground,
        Color {
            r: 12,
            g: 34,
            b: 56,
            a: 255
        }
    );

    assert_eq!(row_0.cells[1].character, '🚀');
    assert_eq!(row_0.cells[1].flags, CellFlags::WIDE);

    assert_eq!(row_0.cells[2].flags, CellFlags::WIDE_CONTINUATION);

    assert_eq!(row_0.cells[3].character, 'U');
    assert_eq!(
        row_0.cells[3].flags,
        CellFlags::CURLY_UNDERLINE | CellFlags::STRIKE
    );
    assert!(row_0.wrapped);
}
