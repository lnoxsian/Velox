use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::scrollback::{
    Chunk, MAX_PENDING_CHUNK_ROWS, MAX_SERIALIZED_CHUNK_BYTES, PersistenceState, RowMeta,
    SCROLLBACK_CHUNK_ROWS, Scrollback, ScrollbackStorage, clear_global_scrollback_cache,
};

fn make_test_cell(ch: char, r: u8) -> Cell {
    Cell::new(
        ch,
        Color { r, g: 150, b: 200 },
        Color { r: 0, g: 0, b: 0 },
        CellFlags::BOLD,
    )
}

#[test]
fn test_failed_disk_writes_bounded_memory_growth() {
    // Open /dev/full to guarantee write failures (ENOSPC - No space left on device)
    let dev_full = match OpenOptions::new().write(true).open("/dev/full") {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Skipping /dev/full test: {}", e);
            return;
        }
    };

    let hot_limit = 50;
    let storage = ScrollbackStorage::with_file(dev_full);
    let mut scrollback = Scrollback::with_storage(hot_limit, storage);

    assert_eq!(scrollback.persistence_state(), PersistenceState::Active);

    // Push 3,000 lines. With hot_limit 50 and chunk 512, multiple flushes will be attempted.
    for i in 0..3000 {
        let ch = (b'a' + (i % 26) as u8) as char;
        let cell = make_test_cell(ch, (i % 255) as u8);
        scrollback.push_line(&[cell], false);
    }

    // Persistence must have failed and transitioned to Disabled
    assert_eq!(scrollback.persistence_state(), PersistenceState::Disabled);
    assert!(scrollback.is_persistence_disabled());
    assert!(scrollback.last_error().is_some());

    // RAM resident rows must remain strictly bounded:
    // It must NOT have grown to 3000 rows.
    let resident = scrollback.resident_row_count();
    let max_allowed = hot_limit + MAX_PENDING_CHUNK_ROWS + 4 * SCROLLBACK_CHUNK_ROWS;
    assert!(
        resident <= max_allowed,
        "Resident rows ({}) exceeded bounded limit ({}) after disk write failure",
        resident,
        max_allowed
    );

    // Terminal must continue operating: recent lines in hot buffer are readable
    let last_row = scrollback
        .get_row(scrollback.len() - 1)
        .expect("Last pushed row should be accessible in bounded hot buffer");
    assert!(!last_row.cells.is_empty());
}

#[test]
fn test_corrupt_chunk_deserialization_recovery() {
    let mut scrollback = Scrollback::new(20, true);

    // Push 700 lines to produce 1 flushed disk chunk (512 rows) and 188 hot rows
    for i in 0..700 {
        let cell = make_test_cell('K', (i % 255) as u8);
        scrollback.push_line(&[cell], false);
    }

    assert_eq!(scrollback.chunk_count(), 1);

    // Verify row 0 is initially readable from disk
    let initial_row_0 = scrollback.get_row(0).expect("Row 0 should exist on disk");
    assert_eq!(initial_row_0.cells[0].character, 'K');

    // Overwrite the disk chunk with corrupted garbage bytes
    {
        let storage = scrollback
            .storage_mut()
            .expect("Storage must be initialized");
        let chunk_meta = storage.chunks()[0];
        let mut file = storage.file().borrow_mut();
        file.seek(SeekFrom::Start(chunk_meta.file_offset))
            .expect("Seek to chunk offset failed");
        let garbage = [0xFFu8; 128];
        file.write_all(&garbage).expect("Corrupt write failed");
    }

    // Clear caches so the next read misses the LRU cache and reads the corrupted file
    clear_global_scrollback_cache();
    scrollback.trim_memory();

    // Reading from the corrupted chunk must safely return None without panicking
    let corrupt_row = scrollback.get_row(0);
    assert!(
        corrupt_row.is_none(),
        "Corrupted chunk read should safely return None"
    );

    // Recent hot rows in RAM must continue to be completely accessible and unaffected
    let hot_row = scrollback
        .get_row(699)
        .expect("Recent hot row should remain accessible");
    assert_eq!(hot_row.cells[0].character, 'K');

    // New lines can continue to be pushed without issue
    let new_cell = make_test_cell('N', 200);
    scrollback.push_line(&[new_cell], false);
    let newest = scrollback.get_row(scrollback.len() - 1).unwrap();
    assert_eq!(newest.cells[0].character, 'N');
}

#[test]
fn test_corrupt_chunk_metadata_out_of_bounds() {
    let mut chunk = Chunk::new();
    chunk.cells = vec![make_test_cell('A', 10); 10]; // only 10 cells

    // RowMeta where start + len exceeds cells.len()
    chunk.rows.push(RowMeta {
        start: 5,
        len: 20, // 5 + 20 = 25 > 10
        wrapped: false,
    });

    assert!(
        chunk.get_row_view(0).is_none(),
        "Out of bounds RowMeta should return None"
    );

    // RowMeta where start exceeds cells.len()
    chunk.rows.push(RowMeta {
        start: 15,
        len: 2,
        wrapped: false,
    });
    assert!(
        chunk.get_row_view(1).is_none(),
        "Out of bounds start offset should return None"
    );

    // RowMeta with arithmetic overflow
    chunk.rows.push(RowMeta {
        start: u32::MAX,
        len: 10,
        wrapped: false,
    });
    assert!(
        chunk.get_row_view(2).is_none(),
        "Overflowing RowMeta should return None"
    );
}

#[test]
fn test_oversized_chunk_metadata_rejection() {
    let mut scrollback = Scrollback::new(20, true);

    // Push 600 lines (produces 1 chunk on disk)
    for _ in 0..600 {
        scrollback.push_line(&[make_test_cell('O', 123)], false);
    }
    assert_eq!(scrollback.chunk_count(), 1);

    // Invalidate caches
    clear_global_scrollback_cache();
    scrollback.trim_memory();

    // Mutate chunk 0 byte length to be oversized (> MAX_SERIALIZED_CHUNK_BYTES)
    {
        let storage = scrollback.storage_mut().unwrap();
        storage.chunks_mut()[0].byte_len = (MAX_SERIALIZED_CHUNK_BYTES + 1024) as u32;
    }

    // Accessing row 0 must reject oversized chunk without trying to allocate 64MB+
    assert!(
        scrollback.get_row(0).is_none(),
        "Oversized chunk metadata should be rejected"
    );
}

#[test]
fn test_scrollback_recovery_on_clear() {
    let dev_full = match OpenOptions::new().write(true).open("/dev/full") {
        Ok(file) => file,
        Err(_) => return,
    };

    let storage = ScrollbackStorage::with_file(dev_full);
    let mut scrollback = Scrollback::with_storage(20, storage);

    // Trigger failure by pushing enough lines
    for _ in 0..600 {
        scrollback.push_line(&[make_test_cell('X', 100)], false);
    }
    assert!(scrollback.is_persistence_disabled());

    // Clear scrollback
    scrollback.clear();
    assert_eq!(scrollback.len(), 0);
    assert_eq!(scrollback.resident_row_count(), 0);

    // try_recover_persistence creates a fresh working tempfile
    let recovered = scrollback.try_recover_persistence();
    assert!(recovered.is_ok(), "Recovery should allocate a new tempfile");
    assert_eq!(scrollback.persistence_state(), PersistenceState::Active);
    assert!(!scrollback.is_persistence_disabled());

    // Push new lines and verify normal persistence is restored
    for _ in 0..600 {
        scrollback.push_line(&[make_test_cell('Y', 200)], false);
    }
    assert_eq!(scrollback.chunk_count(), 1);
    assert_eq!(scrollback.get_row(0).unwrap().cells[0].character, 'Y');
}

#[test]
fn test_arithmetic_protection_underflow_overflow() {
    let scrollback = Scrollback::new(100, false);
    // Out of bounds accesses
    assert!(scrollback.get_row(usize::MAX).is_none());
    assert!(scrollback.get_row(1000).is_none());

    let infinite = Scrollback::new(100, true);
    assert!(infinite.get_row(usize::MAX).is_none());
    assert_eq!(infinite.len(), 0);
}
