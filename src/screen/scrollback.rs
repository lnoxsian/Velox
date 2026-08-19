use crate::screen::cell::Cell;
use bincode;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use tempfile::tempfile;

pub const SCROLLBACK_CHUNK_ROWS: usize = 512;
pub const CHUNK_CACHE_CAPACITY: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub wrapped: bool,
}

impl std::ops::Deref for Row {
    type Target = Vec<Cell>;
    fn deref(&self) -> &Self::Target {
        &self.cells
    }
}

impl std::ops::DerefMut for Row {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cells
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowMeta {
    pub start: u32,
    pub len: u32,
    pub wrapped: bool,
}

/// Contiguous memory storage for a chunk of serialized scrollback rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub cells: Vec<Cell>,
    pub rows: Vec<RowMeta>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            cells: Vec::with_capacity(SCROLLBACK_CHUNK_ROWS * 80),
            rows: Vec::with_capacity(SCROLLBACK_CHUNK_ROWS),
        }
    }

    pub fn push_row(&mut self, cells: &[Cell], wrapped: bool) {
        let start = self.cells.len() as u32;
        let len = cells.len() as u32;
        self.cells.extend_from_slice(cells);
        self.rows.push(RowMeta {
            start,
            len,
            wrapped,
        });
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.rows.clear();
    }

    #[inline(always)]
    pub fn get_row_view(&self, row_offset: usize) -> Option<(&[Cell], bool)> {
        let meta = self.rows.get(row_offset)?;
        let start = meta.start as usize;
        let end = start + meta.len as usize;
        Some((&self.cells[start..end], meta.wrapped))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkIndex {
    pub first_line: u64,
    pub line_count: u32,
    pub file_offset: u64,
    pub byte_len: u32,
}

struct CachedChunk {
    pub chunk_id: usize,
    pub chunk: Chunk,
    pub access_tick: u64,
}

pub struct ScrollbackStorage {
    file: RefCell<File>,
    chunks: Vec<ChunkIndex>,
    pending_chunk: Chunk,
    cache: RefCell<Vec<CachedChunk>>,
    tick_counter: RefCell<u64>,
    serialize_buf: RefCell<Vec<u8>>,
    read_buf: RefCell<Vec<u8>>,
    total_disk_lines: u64,
}

impl ScrollbackStorage {
    pub fn new() -> Option<Self> {
        tempfile().ok().map(|f| Self {
            file: RefCell::new(f),
            chunks: Vec::new(),
            pending_chunk: Chunk::new(),
            cache: RefCell::new(Vec::with_capacity(CHUNK_CACHE_CAPACITY)),
            tick_counter: RefCell::new(0),
            serialize_buf: RefCell::new(Vec::with_capacity(128 * 1024)),
            read_buf: RefCell::new(Vec::with_capacity(128 * 1024)),
            total_disk_lines: 0,
        })
    }

    fn flush_pending_chunk(&mut self) {
        if self.pending_chunk.is_empty() {
            return;
        }

        let line_count = self.pending_chunk.len() as u32;
        let first_line = self.total_disk_lines;

        let mut serialize_buf = self.serialize_buf.borrow_mut();
        serialize_buf.clear();

        if let Ok(mut file) = self.file.try_borrow_mut()
            && let Ok(offset) = file.seek(SeekFrom::End(0))
            && bincode::serialize_into(&mut *serialize_buf, &self.pending_chunk).is_ok()
            && file.write_all(&serialize_buf).is_ok()
        {
            self.chunks.push(ChunkIndex {
                first_line,
                line_count,
                file_offset: offset,
                byte_len: serialize_buf.len() as u32,
            });
            self.total_disk_lines += line_count as u64;
            self.pending_chunk.clear();
        }

        // Release serialization buffer if it grew abnormally large
        if serialize_buf.capacity() > 1024 * 1024 {
            *serialize_buf = Vec::with_capacity(128 * 1024);
        }
    }

    fn next_tick(&self) -> u64 {
        let mut tick = self.tick_counter.borrow_mut();
        *tick = tick.wrapping_add(1);
        *tick
    }

    fn with_disk_row_slice<R>(
        &self,
        chunk_idx: usize,
        row_offset: usize,
        f: impl FnOnce(&[Cell], bool) -> R,
    ) -> Option<R> {
        let tick = self.next_tick();

        // 1. Check in-memory chunk cache
        {
            let mut cache = self.cache.borrow_mut();
            if let Some(pos) = cache.iter().position(|c| c.chunk_id == chunk_idx) {
                cache[pos].access_tick = tick;
                let (cells, wrapped) = cache[pos].chunk.get_row_view(row_offset)?;
                return Some(f(cells, wrapped));
            }
        }

        // 2. Cache miss: Read chunk from disk using reusable buffer
        let chunk_meta = *self.chunks.get(chunk_idx)?;
        let mut file = self.file.try_borrow_mut().ok()?;
        file.seek(SeekFrom::Start(chunk_meta.file_offset)).ok()?;

        let mut read_buf = self.read_buf.borrow_mut();
        let needed_len = chunk_meta.byte_len as usize;
        if read_buf.len() < needed_len {
            read_buf.resize(needed_len, 0);
        }
        file.read_exact(&mut read_buf[..needed_len]).ok()?;

        let chunk: Chunk = bincode::deserialize(&read_buf[..needed_len]).ok()?;
        let (cells, wrapped) = chunk.get_row_view(row_offset)?;
        let result = f(cells, wrapped);

        // 3. Insert into bounded LRU cache
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= CHUNK_CACHE_CAPACITY
            && let Some((oldest_idx, _)) =
                cache.iter().enumerate().min_by_key(|(_, c)| c.access_tick)
        {
            cache.remove(oldest_idx);
        }
        cache.push(CachedChunk {
            chunk_id: chunk_idx,
            chunk,
            access_tick: tick,
        });

        // Release read buffer if it grew abnormally large
        if read_buf.capacity() > 1024 * 1024 {
            *read_buf = Vec::with_capacity(128 * 1024);
        }

        Some(result)
    }
}

pub struct Scrollback {
    pub max_lines: usize,
    pub infinite: bool,
    hot_rows: VecDeque<Row>,
    storage: Option<ScrollbackStorage>,
}

impl Scrollback {
    pub fn new(max_lines: usize, infinite: bool) -> Self {
        Self {
            max_lines,
            infinite,
            hot_rows: VecDeque::new(),
            storage: if infinite {
                ScrollbackStorage::new()
            } else {
                None
            },
        }
    }

    pub fn push_line(&mut self, cells: &[Cell], wrapped: bool) {
        if self.max_lines == 0 && !self.infinite {
            return;
        }

        if self.infinite {
            if self.storage.is_none() {
                self.storage = ScrollbackStorage::new();
            }

            if self.max_lines == 0 {
                // Directly accumulate in pending chunk when hot RAM buffer limit is 0
                if let Some(storage) = self.storage.as_mut() {
                    storage.pending_chunk.push_row(cells, wrapped);
                    if storage.pending_chunk.len() >= SCROLLBACK_CHUNK_ROWS {
                        storage.flush_pending_chunk();
                    }
                }
                return;
            }

            if self.hot_rows.len() >= self.max_lines {
                let mut oldest = self.hot_rows.pop_front().unwrap();
                if let Some(storage) = self.storage.as_mut() {
                    storage
                        .pending_chunk
                        .push_row(&oldest.cells, oldest.wrapped);
                    if storage.pending_chunk.len() >= SCROLLBACK_CHUNK_ROWS {
                        storage.flush_pending_chunk();
                    }
                }
                // Recycle the popped row's allocation instead of dropping and reallocating
                oldest.cells.clear();
                oldest.cells.extend_from_slice(cells);
                oldest.wrapped = wrapped;
                self.hot_rows.push_back(oldest);
            } else {
                self.hot_rows.push_back(Row {
                    cells: cells.to_vec(),
                    wrapped,
                });
            }
        } else {
            // Finite mode: Recycled ring buffer in RAM
            if self.hot_rows.len() >= self.max_lines
                && let Some(mut reused) = self.hot_rows.pop_front()
            {
                reused.cells.clear();
                reused.cells.extend_from_slice(cells);
                reused.wrapped = wrapped;
                self.hot_rows.push_back(reused);
                return;
            }
            self.hot_rows.push_back(Row {
                cells: cells.to_vec(),
                wrapped,
            });
        }
    }

    pub fn len(&self) -> usize {
        let storage_lines = match &self.storage {
            Some(s) => s.total_disk_lines as usize + s.pending_chunk.len(),
            None => 0,
        };
        storage_lines + self.hot_rows.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Access row by reference slice without allocating a heap Row.
    pub fn with_row_slice<R>(&self, index: usize, f: impl FnOnce(&[Cell], bool) -> R) -> Option<R> {
        if let Some(storage) = &self.storage {
            let total_disk = storage.total_disk_lines as usize;
            let pending_len = storage.pending_chunk.len();

            if index < total_disk {
                let chunk_idx = match storage.chunks.binary_search_by(|chunk| {
                    let start = chunk.first_line as usize;
                    let end = start + chunk.line_count as usize;
                    if index < start {
                        std::cmp::Ordering::Greater
                    } else if index >= end {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                }) {
                    Ok(idx) => idx,
                    Err(_) => return None,
                };

                let chunk = &storage.chunks[chunk_idx];
                let row_offset = index - chunk.first_line as usize;
                return storage.with_disk_row_slice(chunk_idx, row_offset, f);
            } else if index < total_disk + pending_len {
                let pending_idx = index - total_disk;
                let (cells, wrapped) = storage.pending_chunk.get_row_view(pending_idx)?;
                return Some(f(cells, wrapped));
            } else {
                let hot_idx = index - total_disk - pending_len;
                return self.hot_rows.get(hot_idx).map(|r| f(&r.cells, r.wrapped));
            }
        }

        self.hot_rows.get(index).map(|r| f(&r.cells, r.wrapped))
    }

    #[allow(dead_code)]
    pub fn get_row(&self, index: usize) -> Option<Row> {
        self.with_row_slice(index, |cells, wrapped| Row {
            cells: cells.to_vec(),
            wrapped,
        })
    }

    pub fn copy_row_to_slice(&self, index: usize, dest: &mut [Cell], default_cell: Cell) -> bool {
        self.with_row_slice(index, |cells, _| {
            let copy_len = cells.len().min(dest.len());
            dest[..copy_len].copy_from_slice(&cells[..copy_len]);
            if copy_len < dest.len() {
                dest[copy_len..].fill(default_cell);
            }
            true
        })
        .unwrap_or(false)
    }

    pub fn clear(&mut self) {
        self.hot_rows.clear();
        if let Some(storage) = self.storage.as_mut() {
            storage.pending_chunk.clear();
            storage.chunks.clear();
            storage.cache.borrow_mut().clear();
            storage.total_disk_lines = 0;
            if let Ok(mut file) = storage.file.try_borrow_mut() {
                let _ = file.set_len(0);
                let _ = file.seek(SeekFrom::Start(0));
            }
        }
    }

    /// Clears only the active hot RAM buffer and pending chunk without truncating cold disk chunks.
    pub fn clear_hot(&mut self) {
        self.hot_rows.clear();
        if let Some(storage) = self.storage.as_mut() {
            storage.pending_chunk.clear();
        }
    }

    /// Number of rows currently resident in RAM (hot rows + pending chunk + cache).
    #[allow(dead_code)]
    pub fn resident_row_count(&self) -> usize {
        let hot = self.hot_rows.len();
        let pending = self.storage.as_ref().map_or(0, |s| s.pending_chunk.len());
        let cached: usize = self
            .storage
            .as_ref()
            .map_or(0, |s| s.cache.borrow().iter().map(|c| c.chunk.len()).sum());
        hot + pending + cached
    }

    /// Number of rows committed to disk.
    #[allow(dead_code)]
    pub fn disk_rows(&self) -> u64 {
        self.storage.as_ref().map_or(0, |s| s.total_disk_lines)
    }

    /// Total number of chunk index entries.
    #[allow(dead_code)]
    pub fn chunk_count(&self) -> usize {
        self.storage.as_ref().map_or(0, |s| s.chunks.len())
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new(2000, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::cell::{Cell, CellFlags, Color};

    fn make_test_cell(ch: char, fg_r: u8) -> Cell {
        Cell {
            character: ch,
            foreground: Color {
                r: fg_r,
                g: 200,
                b: 100,
            },
            background: Color {
                r: 10,
                g: 20,
                b: 30,
            },
            flags: CellFlags::BOLD | CellFlags::UNDERLINE,
        }
    }

    #[test]
    fn test_infinite_scrollback_chunk_flushing_and_bounded_ram() {
        let mut scrollback = Scrollback::new(100, true);

        // Push 1,500 rows. (With hot limit 100 and chunk size 512, this will produce 2 flushed disk chunks)
        for i in 0..1500 {
            let ch = (b'A' + (i % 26) as u8) as char;
            let cell = make_test_cell(ch, (i % 256) as u8);
            scrollback.push_line(&[cell], i % 2 == 0);
        }

        assert_eq!(scrollback.len(), 1500);
        assert_eq!(scrollback.chunk_count(), 2);
        assert_eq!(scrollback.disk_rows(), 1024);

        // RAM resident rows must be strictly bounded:
        // hot_rows (100) + pending_chunk (1500 - 100 - 1024 = 376) + cache (0) = 476 rows
        assert!(scrollback.resident_row_count() <= 100 + SCROLLBACK_CHUNK_ROWS);

        // Retrieve row 0 (from first disk chunk)
        let row_0 = scrollback.get_row(0).expect("Row 0 should exist");
        assert_eq!(row_0.cells[0].character, 'A');
        assert!(row_0.wrapped);
        assert_eq!(row_0.cells[0].flags, CellFlags::BOLD | CellFlags::UNDERLINE);

        // Retrieve row 512 (from second disk chunk)
        let row_512 = scrollback.get_row(512).expect("Row 512 should exist");
        let expected_ch = (b'A' + (512 % 26) as u8) as char;
        assert_eq!(row_512.cells[0].character, expected_ch);
        assert!(row_512.wrapped);

        // Retrieve row 1499 (from hot RAM)
        let row_1499 = scrollback.get_row(1499).expect("Row 1499 should exist");
        let expected_ch_1499 = (b'A' + (1499 % 26) as u8) as char;
        assert_eq!(row_1499.cells[0].character, expected_ch_1499);
        assert!(!row_1499.wrapped);
    }

    #[test]
    fn test_infinite_scrollback_cache_behavior() {
        let mut scrollback = Scrollback::new(50, true);

        for _ in 0..1200 {
            let cell = make_test_cell('X', 255);
            scrollback.push_line(&[cell], false);
        }

        assert_eq!(scrollback.chunk_count(), 2);

        // Repeated reads from chunk 0 should hit the cache without error
        for _ in 0..10 {
            let row = scrollback.get_row(10).unwrap();
            assert_eq!(row.cells[0].character, 'X');
        }

        // Read from chunk 1
        let row_chunk1 = scrollback.get_row(600).unwrap();
        assert_eq!(row_chunk1.cells[0].character, 'X');
    }

    #[test]
    fn test_infinite_scrollback_clear() {
        let mut scrollback = Scrollback::new(50, true);

        for i in 0..2000 {
            let cell = make_test_cell('C', (i % 255) as u8);
            scrollback.push_line(&[cell], false);
        }

        assert_eq!(scrollback.len(), 2000);
        assert!(scrollback.chunk_count() > 0);

        scrollback.clear();

        assert_eq!(scrollback.len(), 0);
        assert!(scrollback.is_empty());
        assert_eq!(scrollback.resident_row_count(), 0);
        assert_eq!(scrollback.disk_rows(), 0);
        assert_eq!(scrollback.chunk_count(), 0);
    }

    #[test]
    fn test_finite_scrollback_mode() {
        let mut scrollback = Scrollback::new(10, false);

        for i in 0..25 {
            let cell = make_test_cell((b'0' + (i % 10) as u8) as char, 255);
            scrollback.push_line(&[cell], false);
        }

        assert_eq!(scrollback.len(), 10);
        assert_eq!(scrollback.disk_rows(), 0);
        assert_eq!(scrollback.chunk_count(), 0);

        // The oldest row in 10-line buffer should be line 15 ('5')
        let row_0 = scrollback.get_row(0).unwrap();
        assert_eq!(row_0.cells[0].character, '5');

        // The newest row should be line 24 ('4')
        let row_9 = scrollback.get_row(9).unwrap();
        assert_eq!(row_9.cells[0].character, '4');
    }

    #[test]
    fn test_scrollback_limit_zero() {
        // Finite mode with limit 0 -> no scrollback
        let mut finite_zero = Scrollback::new(0, false);
        finite_zero.push_line(&[make_test_cell('A', 255)], false);
        assert_eq!(finite_zero.len(), 0);

        // Infinite mode with limit 0 -> disk accumulation
        let mut infinite_zero = Scrollback::new(0, true);
        for _ in 0..600 {
            infinite_zero.push_line(&[make_test_cell('Z', 255)], false);
        }
        assert_eq!(infinite_zero.len(), 600);
        assert_eq!(infinite_zero.chunk_count(), 1);
        assert_eq!(infinite_zero.disk_rows(), 512);
        assert_eq!(infinite_zero.get_row(0).unwrap().cells[0].character, 'Z');
    }

    #[test]
    fn test_copy_row_to_slice_zero_allocation() {
        let mut scrollback = Scrollback::new(10, false);
        let default_cell = make_test_cell(' ', 0);
        let cell_a = make_test_cell('A', 255);
        let cell_b = make_test_cell('B', 255);

        scrollback.push_line(&[cell_a, cell_b], false);

        let mut target_slice = vec![default_cell; 5];
        let copied = scrollback.copy_row_to_slice(0, &mut target_slice, default_cell);
        assert!(copied);
        assert_eq!(target_slice[0].character, 'A');
        assert_eq!(target_slice[1].character, 'B');
        assert_eq!(target_slice[2].character, ' ');
    }
}
