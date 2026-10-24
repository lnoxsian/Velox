use crate::screen::cell::{Cell, CellFlags, Color};
use crate::screen::cursor::Cursor;
use crate::screen::damage::DamageTracker;
use crate::screen::scrollback::Scrollback;
use crate::screen::selection::Selection;
use ahash::AHashMap;
use std::sync::{Mutex, OnceLock};
use unicode_width::UnicodeWidthChar;

thread_local! {
    static URL_SCRATCH: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

pub const MAX_COMBINING_SEQUENCES: usize = 4096;

pub struct CombiningRegistry {
    list: Vec<String>,
    map: AHashMap<String, u32>,
}

impl CombiningRegistry {
    pub fn new() -> Self {
        Self {
            list: Vec::with_capacity(64),
            map: AHashMap::with_capacity(64),
        }
    }

    #[inline]
    pub fn get_or_insert(&mut self, seq: &str) -> Option<u32> {
        if let Some(&id) = self.map.get(seq) {
            Some(id)
        } else if self.list.len() < MAX_COMBINING_SEQUENCES {
            let id = self.list.len() as u32;
            let s = seq.to_string();
            self.map.insert(s.clone(), id);
            self.list.push(s);
            Some(id)
        } else {
            None
        }
    }

    #[inline]
    pub fn append_to_entry(&mut self, reg_idx: usize, c: char) {
        if reg_idx < self.list.len() {
            let old_str = &self.list[reg_idx];
            self.map.remove(old_str.as_str());
            self.list[reg_idx].push(c);
            self.map.insert(self.list[reg_idx].clone(), reg_idx as u32);
        }
    }

    #[inline]
    pub fn get_sequence(&self, reg_idx: usize) -> Option<&str> {
        self.list.get(reg_idx).map(|s| s.as_str())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

impl Default for CombiningRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for CombiningRegistry {
    type Target = Vec<String>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

static COMBINING_REGISTRY: OnceLock<Mutex<CombiningRegistry>> = OnceLock::new();

pub fn get_combining_registry() -> &'static Mutex<CombiningRegistry> {
    COMBINING_REGISTRY.get_or_init(|| Mutex::new(CombiningRegistry::new()))
}

pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub row_wrapped: Vec<bool>,
    pub cursor: Cursor,
    pub saved_cursor: Cursor,
    pub saved_fg: Color,
    pub saved_bg: Color,
    pub saved_underline_color: Option<Color>,
    pub saved_flags: CellFlags,
    pub saved_g0_charset: u8,
    pub saved_g1_charset: u8,
    pub saved_active_charset: u8,
    pub damage: DamageTracker,
    pub scrollback: Scrollback,
    pub selection: Selection,
    pub default_fg: Color,
    pub default_bg: Color,
    pub scroll_region_top: usize,
    pub scroll_region_bottom: usize,
    pub scroll_offset: usize,
    pub row_offset: usize,
    pub hyperlinks: crate::hyperlink::osc8::HyperlinkStore,
    pub current_hyperlink: Option<std::sync::Arc<crate::hyperlink::osc8::Hyperlink>>,
    pub tab_stops: TabStops,
    pub auto_wrap: bool,
    pub insert_mode: bool,
    pub saved_origin_mode: bool,
    pub saved_auto_wrap: bool,
}

/// Compact bitset representing horizontal tab stops for up to 1024 terminal columns.
/// Uses 16 x 64-bit words (128 bytes inline), eliminating heap allocations and enabling
/// O(1) next/prev tab searches using CPU trailing_zeros / leading_zeros bit intrinsics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStops {
    pub words: [u64; 16],
    pub width: usize,
}

impl TabStops {
    pub const DEFAULT_PATTERN: u64 = 0x0101_0101_0101_0101; // Bits 0, 8, 16, 24, 32, 40, 48, 56

    #[inline]
    pub fn new(width: usize) -> Self {
        let mut stops = Self {
            words: [Self::DEFAULT_PATTERN; 16],
            width: width.min(1024),
        };
        stops.mask_width();
        stops
    }

    #[inline]
    fn mask_width(&mut self) {
        if self.width < 1024 {
            let last_word = self.width / 64;
            let bit = self.width % 64;
            if bit > 0 && last_word < 16 {
                let mask = (1u64 << bit) - 1;
                self.words[last_word] &= mask;
            }
            for w in (last_word + if bit > 0 { 1 } else { 0 })..16 {
                self.words[w] = 0;
            }
        }
    }

    #[inline]
    pub fn resize(&mut self, new_w: usize) {
        if new_w > self.width {
            let old_w = self.width;
            self.width = new_w.min(1024);
            let start_word = old_w / 64;
            let end_word = (self.width + 63) / 64;
            for w in start_word..end_word.min(16) {
                let base_col = w * 64;
                for bit in 0..64 {
                    let col = base_col + bit;
                    if col >= old_w && col < self.width && col % 8 == 0 {
                        self.words[w] |= 1u64 << bit;
                    }
                }
            }
        } else {
            self.width = new_w.min(1024);
        }
        self.mask_width();
    }

    #[inline]
    pub fn set(&mut self, col: usize) {
        if col < self.width && col < 1024 {
            self.words[col / 64] |= 1u64 << (col % 64);
        }
    }

    #[inline]
    pub fn clear(&mut self, col: usize) {
        if col < self.width && col < 1024 {
            self.words[col / 64] &= !(1u64 << (col % 64));
        }
    }

    #[inline]
    pub fn clear_all(&mut self) {
        self.words = [0u64; 16];
    }

    #[inline]
    pub fn reset(&mut self) {
        self.words = [Self::DEFAULT_PATTERN; 16];
        self.mask_width();
    }

    #[inline]
    pub fn next_stop(&self, col: usize) -> usize {
        let start_col = col + 1;
        if start_col >= self.width {
            return self.width.saturating_sub(1);
        }
        let start_word = start_col / 64;
        let start_bit = start_col % 64;

        if start_word < 16 {
            let mask = !((1u64 << start_bit) - 1);
            let active = self.words[start_word] & mask;
            if active != 0 {
                let stop = start_word * 64 + active.trailing_zeros() as usize;
                return stop.min(self.width.saturating_sub(1));
            }

            for w in (start_word + 1)..16.min((self.width + 63) / 64) {
                let word = self.words[w];
                if word != 0 {
                    let stop = w * 64 + word.trailing_zeros() as usize;
                    return stop.min(self.width.saturating_sub(1));
                }
            }
        }

        self.width.saturating_sub(1)
    }

    #[inline]
    pub fn prev_stop(&self, col: usize) -> usize {
        if col == 0 {
            return 0;
        }
        let target_col = col - 1;
        let start_word = (target_col / 64).min(15);
        let start_bit = target_col % 64;

        let mask = if start_bit == 63 {
            !0u64
        } else {
            (1u64 << (start_bit + 1)) - 1
        };
        let active = self.words[start_word] & mask;
        if active != 0 {
            let bit = 63 - active.leading_zeros() as usize;
            return start_word * 64 + bit;
        }

        for w in (0..start_word).rev() {
            let word = self.words[w];
            if word != 0 {
                let bit = 63 - word.leading_zeros() as usize;
                return w * 64 + bit;
            }
        }

        0
    }
}

impl Grid {
    pub fn new(
        width: usize,
        height: usize,
        fg: Color,
        bg: Color,
        scrollback_limit: usize,
        infinite_scrollback: bool,
    ) -> Self {
        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            underline_color: None,
            flags: CellFlags::empty(),
        };
        Self {
            width,
            height,
            cells: vec![default_cell; width * height],
            row_wrapped: vec![false; height],
            cursor: Cursor {
                x: 0,
                y: 0,
                shape: crate::screen::cursor::CursorShape::Block,
                visible: true,
            },
            saved_cursor: Cursor {
                x: 0,
                y: 0,
                shape: crate::screen::cursor::CursorShape::Block,
                visible: true,
            },
            saved_fg: fg,
            saved_bg: bg,
            saved_underline_color: None,
            saved_flags: CellFlags::empty(),
            saved_g0_charset: 0,
            saved_g1_charset: 0,
            saved_active_charset: 0,
            damage: DamageTracker::new(height),
            scrollback: {
                let mut sb = Scrollback::new(scrollback_limit, infinite_scrollback);
                sb.default_bg = Some(bg);
                sb
            },
            selection: Selection::new(),
            default_fg: fg,
            default_bg: bg,
            scroll_region_top: 0,
            scroll_region_bottom: height.saturating_sub(1),
            scroll_offset: 0,
            row_offset: 0,
            hyperlinks: crate::hyperlink::osc8::HyperlinkStore::new(),
            current_hyperlink: None,
            tab_stops: TabStops::new(width),
            auto_wrap: true,
            insert_mode: false,
            saved_origin_mode: false,
            saved_auto_wrap: true,
        }
    }

    #[inline(always)]
    pub fn physical_row(&self, y: usize) -> usize {
        if self.height == 0 {
            0
        } else {
            (self.row_offset + y) % self.height
        }
    }

    /// Maps a screen display row coordinate (0..height) to its absolute row coordinate
    /// across scrollback and active grid, accounting for the current `scroll_offset`.
    #[inline(always)]
    pub fn display_row_to_abs(&self, row_idx: usize) -> usize {
        (self.scrollback.len() + row_idx).saturating_sub(self.scroll_offset)
    }

    #[inline]
    pub fn set_tab_stop(&mut self, col: usize) {
        self.tab_stops.set(col);
    }

    #[inline]
    pub fn clear_tab_stop(&mut self, col: usize) {
        self.tab_stops.clear(col);
    }

    #[inline]
    pub fn clear_all_tab_stops(&mut self) {
        self.tab_stops.clear_all();
    }

    #[inline]
    pub fn reset_tab_stops(&mut self) {
        self.tab_stops.reset();
    }

    #[inline]
    pub fn next_tab_stop(&self, col: usize) -> usize {
        self.tab_stops.next_stop(col)
    }

    #[inline]
    pub fn prev_tab_stop(&self, col: usize) -> usize {
        self.tab_stops.prev_stop(col)
    }

    #[inline(always)]
    pub fn with_display_row_slice<R>(&self, y: usize, f: impl FnOnce(&[Cell]) -> R) -> Option<R> {
        let history_len = self.scrollback.len();
        let abs_y = y + history_len;
        if abs_y < self.scroll_offset {
            return None;
        }
        let abs_row = abs_y - self.scroll_offset;
        if abs_row < history_len {
            self.scrollback.with_row_slice(abs_row, |cells, _| f(cells))
        } else {
            let grid_y = abs_row - history_len;
            if grid_y < self.height {
                let physical_y = self.physical_row(grid_y);
                let start = physical_y * self.width;
                let end = (start + self.width).min(self.cells.len());
                if start < self.cells.len() {
                    Some(f(&self.cells[start..end]))
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    /// Fast-path row slice access for the visible active screen (when not viewing scrollback).
    /// Avoids closure overhead and branch dispatch.
    #[inline(always)]
    pub fn active_display_row_slice(&self, y: usize) -> Option<&[Cell]> {
        if self.scroll_offset != 0 {
            return None;
        }
        if y < self.height {
            let physical_y = self.physical_row(y);
            let start = physical_y * self.width;
            let end = (start + self.width).min(self.cells.len());
            if start < self.cells.len() {
                return Some(&self.cells[start..end]);
            }
        }
        None
    }

    pub fn normalize_row_offset(&mut self) {
        if self.row_offset == 0 || self.height <= 1 {
            return;
        }
        let split_point = self.row_offset * self.width;
        if split_point < self.cells.len() {
            self.cells.rotate_left(split_point);
        }
        if self.row_offset < self.row_wrapped.len() {
            self.row_wrapped.rotate_left(self.row_offset);
        }
        self.row_offset = 0;
    }

    #[inline(always)]
    pub fn hyperlink_at(
        &self,
        col: usize,
        abs_row: usize,
    ) -> Option<std::sync::Arc<crate::hyperlink::osc8::Hyperlink>> {
        self.hyperlinks.get(abs_row, col)
    }

    pub fn is_row_wrapped(&self, abs_row: usize) -> bool {
        let history_len = self.scrollback.len();
        if abs_row < history_len {
            self.scrollback
                .with_row_slice(abs_row, |_cells, wrapped| wrapped)
                .unwrap_or(false)
        } else {
            let grid_y = abs_row - history_len;
            if grid_y < self.height {
                let physical_y = self.physical_row(grid_y);
                self.row_wrapped.get(physical_y).copied().unwrap_or(false)
            } else {
                false
            }
        }
    }

    pub fn with_row_cells<R>(&self, abs_row: usize, f: impl FnOnce(&[Cell]) -> R) -> Option<R> {
        let history_len = self.scrollback.len();
        if abs_row < history_len {
            self.scrollback
                .with_row_slice(abs_row, |cells, _wrapped| f(cells))
        } else {
            let grid_y = abs_row - history_len;
            if grid_y < self.height {
                let physical_y = self.physical_row(grid_y);
                let start = physical_y * self.width;
                let end = (start + self.width).min(self.cells.len());
                if start < self.cells.len() {
                    Some(f(&self.cells[start..end]))
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    /// Determines the start and end row indices `(start_r, end_r)` for a logical wrapped line.
    #[inline]
    pub fn logical_line_range(&self, abs_row: usize, total_lines: usize) -> (usize, usize) {
        let mut start_r = abs_row;
        while start_r > 0 && self.is_row_wrapped(start_r - 1) {
            start_r -= 1;
        }
        let mut end_r = abs_row;
        while end_r + 1 < total_lines && self.is_row_wrapped(end_r) {
            end_r += 1;
        }
        (start_r, end_r)
    }

    fn fill_logical_line_scratch(&self, start_r: usize, end_r: usize, out: &mut String) {
        out.clear();
        for r in start_r..=end_r {
            let _ = self.with_row_cells(r, |cells| {
                for c in cells {
                    out.push(c.character);
                }
                for _ in cells.len()..self.width {
                    out.push(' ');
                }
            });
        }
    }

    /// Checks if a cell at `(col, abs_row)` has an explicit OSC-8 hyperlink or detected URL.
    /// Highly optimized for mouse hover: avoids string allocations for unwrapped rows without triggers.
    pub fn has_url_at(&self, col: usize, abs_row: usize) -> bool {
        if self.hyperlink_at(col, abs_row).is_some() {
            return true;
        }

        let total_lines = self.scrollback.len() + self.height;
        if abs_row >= total_lines || self.width == 0 {
            return false;
        }

        let is_prev_wrapped = abs_row > 0 && self.is_row_wrapped(abs_row - 1);
        let is_curr_wrapped = self.is_row_wrapped(abs_row);

        if !is_prev_wrapped && !is_curr_wrapped {
            let has_trigger = self
                .with_row_cells(abs_row, |cells| {
                    cells
                        .iter()
                        .any(|c| c.character == ':' || c.character == '.')
                })
                .unwrap_or(false);

            if !has_trigger {
                return false;
            }

            return URL_SCRATCH.with(|scratch| {
                let mut text = scratch.borrow_mut();
                text.clear();
                let _ = self.with_row_cells(abs_row, |cells| {
                    for c in cells {
                        text.push(c.character);
                    }
                });
                crate::hyperlink::detector::is_url_at(&text, col)
            });
        }

        let (start_r, end_r) = self.logical_line_range(abs_row, total_lines);

        URL_SCRATCH.with(|scratch| {
            let mut logical_text = scratch.borrow_mut();
            self.fill_logical_line_scratch(start_r, end_r, &mut logical_text);
            let logical_col = (abs_row - start_r) * self.width + col;
            crate::hyperlink::detector::is_url_at(&logical_text, logical_col)
        })
    }

    pub fn hyperlink_url_at(&self, col: usize, abs_row: usize) -> Option<String> {
        if let Some(link) = self.hyperlink_at(col, abs_row) {
            return Some(link.url.clone());
        }

        let total_lines = self.scrollback.len() + self.height;
        if abs_row >= total_lines || self.width == 0 {
            return None;
        }

        let is_prev_wrapped = abs_row > 0 && self.is_row_wrapped(abs_row - 1);
        let is_curr_wrapped = self.is_row_wrapped(abs_row);

        if !is_prev_wrapped && !is_curr_wrapped {
            let has_trigger = self
                .with_row_cells(abs_row, |cells| {
                    cells
                        .iter()
                        .any(|c| c.character == ':' || c.character == '.')
                })
                .unwrap_or(false);

            if !has_trigger {
                return None;
            }

            let mut text = String::with_capacity(self.width);
            let _ = self.with_row_cells(abs_row, |cells| {
                for c in cells {
                    text.push(c.character);
                }
            });

            let urls = crate::hyperlink::detector::detect(&text);
            for (start, end, url) in urls {
                if col >= start && col < end {
                    return Some(url.to_string());
                }
            }
            return None;
        }

        let (start_r, end_r) = self.logical_line_range(abs_row, total_lines);
        let mut logical_text = String::with_capacity((end_r - start_r + 1) * self.width);
        self.fill_logical_line_scratch(start_r, end_r, &mut logical_text);

        let logical_col = (abs_row - start_r) * self.width + col;
        let urls = crate::hyperlink::detector::detect(&logical_text);
        for (start, end, url) in urls {
            if logical_col >= start && logical_col < end {
                return Some(url.to_string());
            }
        }

        None
    }

    #[inline(always)]
    pub fn clear_damage(&mut self) {
        self.damage.clear();
    }

    #[inline(always)]
    pub fn mark_all_dirty(&mut self) {
        self.damage.mark_all();
    }

    pub fn clamp_cursor(&mut self) {
        if self.height > 0 {
            self.cursor.y = self.cursor.y.min(self.height - 1);
        } else {
            self.cursor.y = 0;
        }
        if self.width > 0 {
            self.cursor.x = self.cursor.x.min(self.width);
        } else {
            self.cursor.x = 0;
        }
    }

    #[inline(always)]
    pub fn put_ascii(
        &mut self,
        c: char,
        fg: Color,
        bg: Color,
        underline_color: Option<Color>,
        flags: CellFlags,
    ) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        if self.auto_wrap {
            if self.cursor.x >= self.width {
                let physical_y = self.physical_row(self.cursor.y);
                let row_start = physical_y * self.width;
                let start = (row_start + self.cursor.x).min(self.cells.len());
                let end = (row_start + self.width).min(self.cells.len());
                if start < end {
                    self.cells[start..end].fill(Cell {
                        character: ' ',
                        foreground: fg,
                        background: bg,
                        underline_color: None,
                        flags: CellFlags::empty(),
                    });
                }
                if physical_y < self.row_wrapped.len() {
                    self.row_wrapped[physical_y] = true;
                }
                self.cursor.x = 0;
                self.scroll_or_move_down(bg);
            }
        } else if self.cursor.x >= self.width {
            self.cursor.x = self.width.saturating_sub(1);
        }

        if self.insert_mode {
            self.insert_characters(1, fg, bg);
        }

        if let Some(ref link) = self.current_hyperlink {
            let abs_row = self.scrollback.len() + self.cursor.y;
            self.hyperlinks.insert(abs_row, self.cursor.x, link.clone());
        } else if !self.hyperlinks.rows.is_empty() {
            let abs_row = self.scrollback.len() + self.cursor.y;
            self.hyperlinks.remove_cell(abs_row, self.cursor.x);
        }

        let physical_y = self.physical_row(self.cursor.y);
        let idx = physical_y * self.width + self.cursor.x;
        if idx < self.cells.len() {
            // Clear paired wide character cell if overwriting a wide char or its continuation
            let cell_flags = self.cells[idx].flags;
            if cell_flags.intersects(CellFlags::WIDE | CellFlags::WIDE_CONTINUATION) {
                if cell_flags.contains(CellFlags::WIDE) && self.cursor.x + 1 < self.width {
                    let next_idx = idx + 1;
                    if next_idx < self.cells.len()
                        && self.cells[next_idx]
                            .flags
                            .contains(CellFlags::WIDE_CONTINUATION)
                    {
                        self.cells[next_idx] = Cell {
                            character: ' ',
                            foreground: fg,
                            background: bg,
                            underline_color: None,
                            flags: CellFlags::empty(),
                        };
                    }
                } else if self.cursor.x > 0 {
                    let prev_idx = idx - 1;
                    if self.cells[prev_idx].flags.contains(CellFlags::WIDE) {
                        self.cells[prev_idx] = Cell {
                            character: ' ',
                            foreground: fg,
                            background: bg,
                            underline_color: None,
                            flags: CellFlags::empty(),
                        };
                    }
                }
            }

            self.cells[idx] = Cell {
                character: c,
                foreground: fg,
                background: bg,
                underline_color,
                flags,
            };
        }
        if self.auto_wrap || self.cursor.x + 1 < self.width {
            self.cursor.x += 1;
        }
        self.damage.mark_dirty(self.cursor.y);
    }

    pub fn put_ascii_slice(
        &mut self,
        mut text: &[u8],
        fg: Color,
        bg: Color,
        underline_color: Option<Color>,
        flags: CellFlags,
    ) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        if self.insert_mode || !self.auto_wrap {
            for &b in text {
                self.put_ascii(b as char, fg, bg, underline_color, flags);
            }
            return;
        }

        let has_hyperlinks = !self.hyperlinks.rows.is_empty();
        let has_current_link = self.current_hyperlink.is_some();

        let template_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            underline_color,
            flags,
        };

        while !text.is_empty() {
            if self.cursor.x >= self.width {
                let physical_y = self.physical_row(self.cursor.y);
                if physical_y < self.row_wrapped.len() {
                    self.row_wrapped[physical_y] = true;
                }
                self.cursor.x = 0;
                self.scroll_or_move_down(bg);
            }

            let available_in_row = self.width.saturating_sub(self.cursor.x);
            if available_in_row == 0 {
                continue;
            }

            let chunk_len = text.len().min(available_in_row);
            let chunk = &text[..chunk_len];
            let physical_y = self.physical_row(self.cursor.y);
            let row_start = physical_y * self.width;
            let dest_start = row_start + self.cursor.x;
            let dest_end = (dest_start + chunk_len).min(self.cells.len());

            if dest_start < dest_end {
                // If starting cell is WIDE_CONTINUATION, clear the preceding WIDE cell
                if self.cells[dest_start]
                    .flags
                    .contains(CellFlags::WIDE_CONTINUATION)
                    && self.cursor.x > 0
                {
                    let prev = dest_start - 1;
                    if self.cells[prev].flags.contains(CellFlags::WIDE) {
                        self.cells[prev] = Cell {
                            character: ' ',
                            foreground: fg,
                            background: bg,
                            underline_color: None,
                            flags: CellFlags::empty(),
                        };
                    }
                }
                // If ending cell is WIDE, clear the subsequent WIDE_CONTINUATION cell
                let last_written = dest_end - 1;
                if self.cells[last_written].flags.contains(CellFlags::WIDE)
                    && last_written + 1 < self.cells.len()
                {
                    let next = last_written + 1;
                    if self.cells[next]
                        .flags
                        .contains(CellFlags::WIDE_CONTINUATION)
                    {
                        self.cells[next] = Cell {
                            character: ' ',
                            foreground: fg,
                            background: bg,
                            underline_color: None,
                            flags: CellFlags::empty(),
                        };
                    }
                }

                let actual_len = dest_end - dest_start;
                let dest_slice = &mut self.cells[dest_start..dest_end];
                for (cell, &b) in dest_slice.iter_mut().zip(&chunk[..actual_len]) {
                    *cell = Cell {
                        character: b as char,
                        ..template_cell
                    };
                }
            }

            if has_current_link {
                let link = self.current_hyperlink.as_ref().unwrap();
                let abs_row = self.scrollback.len() + self.cursor.y;
                for col in self.cursor.x..self.cursor.x + chunk_len {
                    self.hyperlinks.insert(abs_row, col, link.clone());
                }
            } else if has_hyperlinks {
                let abs_row = self.scrollback.len() + self.cursor.y;
                if self.hyperlinks.rows.contains_key(&abs_row) {
                    for col in self.cursor.x..self.cursor.x + chunk_len {
                        self.hyperlinks.remove_cell(abs_row, col);
                    }
                }
            }

            self.cursor.x += chunk_len;
            self.damage.mark_dirty(self.cursor.y);
            text = &text[chunk_len..];
        }
    }

    pub fn put_char(
        &mut self,
        c: char,
        fg: Color,
        bg: Color,
        underline_color: Option<Color>,
        mut flags: CellFlags,
    ) {
        let cp = c as u32;
        if (0x20..0x7f).contains(&cp) {
            self.put_ascii(c, fg, bg, underline_color, flags);
            return;
        }

        self.clamp_cursor();
        if self.width == 0 || self.height == 0 {
            return;
        }
        let is_combining = UnicodeWidthChar::width(c) == Some(0);
        if is_combining {
            // Non-combining zero-width characters (e.g. ZWSP U+200B, ZWNBSP/BOM U+FEFF, soft hyphen U+00AD)
            // or Variation Selectors
            if ('\u{fe00}'..='\u{fe0f}').contains(&c)
                || ('\u{e0100}'..='\u{e01ef}').contains(&c)
                || c == '\u{200b}'
                || c == '\u{feff}'
                || c == '\u{00ad}'
            {
                self.damage.mark_dirty(self.cursor.y);
                return;
            }

            if self.cursor.x > 0 {
                let physical_y = self.physical_row(self.cursor.y);
                let row_start = physical_y * self.width;
                let mut target_x = self.cursor.x - 1;
                let mut idx = row_start + target_x;
                if idx < self.cells.len()
                    && self.cells[idx].flags.contains(CellFlags::WIDE_CONTINUATION)
                    && target_x > 0
                {
                    target_x -= 1;
                    idx = row_start + target_x;
                }
                if idx < self.cells.len() {
                    let base_char = self.cells[idx].character;
                    if ('\u{100000}'..='\u{10ffff}').contains(&base_char) {
                        let reg_idx = (base_char as u32 - 0x100000) as usize;
                        if let Ok(mut registry) = get_combining_registry().lock() {
                            registry.append_to_entry(reg_idx, c);
                        }
                    } else {
                        let mut seq = String::with_capacity(8);
                        seq.push(base_char);
                        seq.push(c);
                        let new_char = if let Ok(mut registry) = get_combining_registry().lock() {
                            if let Some(id) = registry.get_or_insert(&seq) {
                                char::from_u32(0x100000 + id).unwrap_or(base_char)
                            } else {
                                base_char
                            }
                        } else {
                            base_char
                        };
                        self.cells[idx].character = new_char;
                    }
                }
            }
            self.damage.mark_dirty(self.cursor.y);
            return;
        }

        let mut w = UnicodeWidthChar::width(c).unwrap_or(1).max(1);
        if self.width < 2 {
            w = 1;
        }

        if self.auto_wrap {
            if self.cursor.x + w > self.width {
                // Fill rest of the row with spaces, then wrap
                let physical_y = self.physical_row(self.cursor.y);
                let row_start = physical_y * self.width;
                let start = (row_start + self.cursor.x).min(self.cells.len());
                let end = (row_start + self.width).min(self.cells.len());
                if start < end {
                    self.cells[start..end].fill(Cell {
                        character: ' ',
                        foreground: fg,
                        background: bg,
                        underline_color: None,
                        flags: CellFlags::empty(),
                    });
                }
                if physical_y < self.row_wrapped.len() {
                    self.row_wrapped[physical_y] = true;
                }
                self.cursor.x = 0;
                self.scroll_or_move_down(bg);
            }
        } else if self.cursor.x + w > self.width {
            self.cursor.x = self.width.saturating_sub(w);
        }

        if self.insert_mode {
            self.insert_characters(w, fg, bg);
        }

        let physical_y = self.physical_row(self.cursor.y);
        let row_start = physical_y * self.width;

        if w == 2 && self.width >= 2 {
            flags.insert(CellFlags::WIDE);
            if let Some(ref link) = self.current_hyperlink {
                let abs_row = self.scrollback.len() + self.cursor.y;
                self.hyperlinks.insert(abs_row, self.cursor.x, link.clone());
                self.hyperlinks
                    .insert(abs_row, self.cursor.x + 1, link.clone());
            } else if !self.hyperlinks.rows.is_empty() {
                let abs_row = self.scrollback.len() + self.cursor.y;
                self.hyperlinks.remove_cell(abs_row, self.cursor.x);
                self.hyperlinks.remove_cell(abs_row, self.cursor.x + 1);
            }

            let idx = row_start + self.cursor.x;
            if idx < self.cells.len() {
                self.cells[idx] = Cell {
                    character: c,
                    foreground: fg,
                    background: bg,
                    underline_color,
                    flags,
                };
            }
            if self.auto_wrap || self.cursor.x + 1 < self.width {
                self.cursor.x += 1;
            }

            let idx_next = row_start + self.cursor.x;
            if idx_next < self.cells.len() {
                self.cells[idx_next] = Cell {
                    character: ' ',
                    foreground: fg,
                    background: bg,
                    underline_color: None,
                    flags: CellFlags::WIDE_CONTINUATION,
                };
            }
            if self.auto_wrap || self.cursor.x + 1 < self.width {
                self.cursor.x += 1;
            }
        } else {
            if let Some(ref link) = self.current_hyperlink {
                let abs_row = self.scrollback.len() + self.cursor.y;
                self.hyperlinks.insert(abs_row, self.cursor.x, link.clone());
            } else if !self.hyperlinks.rows.is_empty() {
                let abs_row = self.scrollback.len() + self.cursor.y;
                self.hyperlinks.remove_cell(abs_row, self.cursor.x);
            }

            let idx = row_start + self.cursor.x;
            if idx < self.cells.len() {
                // Clear paired wide character cell if overwriting
                let cell_flags = self.cells[idx].flags;
                if cell_flags.intersects(CellFlags::WIDE | CellFlags::WIDE_CONTINUATION) {
                    if cell_flags.contains(CellFlags::WIDE) && self.cursor.x + 1 < self.width {
                        let next_idx = idx + 1;
                        if next_idx < self.cells.len()
                            && self.cells[next_idx]
                                .flags
                                .contains(CellFlags::WIDE_CONTINUATION)
                        {
                            self.cells[next_idx] = Cell {
                                character: ' ',
                                foreground: fg,
                                background: bg,
                                underline_color: None,
                                flags: CellFlags::empty(),
                            };
                        }
                    } else if self.cursor.x > 0 {
                        let prev_idx = idx - 1;
                        if self.cells[prev_idx].flags.contains(CellFlags::WIDE) {
                            self.cells[prev_idx] = Cell {
                                character: ' ',
                                foreground: fg,
                                background: bg,
                                underline_color: None,
                                flags: CellFlags::empty(),
                            };
                        }
                    }
                }

                self.cells[idx] = Cell {
                    character: c,
                    foreground: fg,
                    background: bg,
                    underline_color,
                    flags,
                };
            }
            if self.auto_wrap || self.cursor.x + 1 < self.width {
                self.cursor.x += 1;
            }
        }
        self.damage.mark_dirty(self.cursor.y);
    }

    /// Repeat the given character `n` times using current attributes (ECMA-48 REP).
    pub fn repeat_char(
        &mut self,
        n: usize,
        c: char,
        fg: Color,
        bg: Color,
        underline_color: Option<Color>,
        flags: CellFlags,
    ) {
        for _ in 0..n {
            self.put_char(c, fg, bg, underline_color, flags);
        }
    }

    pub fn erase_characters(&mut self, n: usize, fg: Color, bg: Color) {
        self.clamp_cursor();
        let cursor_y = self.cursor.y;
        let cursor_x = self.cursor.x;
        if cursor_y >= self.height || cursor_x >= self.width {
            return;
        }
        let physical_y = self.physical_row(cursor_y);
        let row_start = physical_y * self.width;
        let start = row_start + cursor_x;
        let end = (start + n).min(row_start + self.width);
        let default_cell = Cell::new(' ', fg, bg, CellFlags::empty());
        self.cells[start..end].fill(default_cell);
        self.damage.mark_dirty(cursor_y);
    }

    pub fn delete_characters(&mut self, n: usize, fg: Color, bg: Color) {
        self.clamp_cursor();
        let cursor_y = self.cursor.y;
        let cursor_x = self.cursor.x;
        if cursor_y >= self.height || cursor_x >= self.width {
            return;
        }
        let physical_y = self.physical_row(cursor_y);
        let row_start = physical_y * self.width;
        let n = n.min(self.width - cursor_x);
        let move_start = row_start + cursor_x + n;
        let move_end = row_start + self.width;
        let dest = row_start + cursor_x;
        self.cells.copy_within(move_start..move_end, dest);

        let default_cell = Cell::new(' ', fg, bg, CellFlags::empty());
        let fill_start = row_start + self.width - n;
        self.cells[fill_start..row_start + self.width].fill(default_cell);
        self.damage.mark_dirty(cursor_y);
    }

    pub fn insert_characters(&mut self, n: usize, fg: Color, bg: Color) {
        self.clamp_cursor();
        let cursor_y = self.cursor.y;
        let cursor_x = self.cursor.x;
        if cursor_y >= self.height || cursor_x >= self.width {
            return;
        }
        let physical_y = self.physical_row(cursor_y);
        let row_start = physical_y * self.width;
        let n = n.min(self.width - cursor_x);
        let move_start = row_start + cursor_x;
        let move_end = row_start + self.width - n;
        let dest = row_start + cursor_x + n;
        self.cells.copy_within(move_start..move_end, dest);

        let default_cell = Cell::new(' ', fg, bg, CellFlags::empty());
        self.cells[move_start..move_start + n].fill(default_cell);
        self.damage.mark_dirty(cursor_y);
    }

    pub fn erase_line(&mut self, mode: u8, fg: Color, bg: Color) {
        self.clamp_cursor();
        if self.height == 0 || self.width == 0 {
            return;
        }
        let default_cell = Cell::new(' ', fg, bg, CellFlags::empty());
        let cur_x = self.cursor.x.min(self.width.saturating_sub(1));
        let physical_y = self.physical_row(self.cursor.y);
        let row_start = physical_y * self.width;
        match mode {
            0 => {
                // Cursor to end of line
                let start_idx = row_start + cur_x;
                if cur_x > 0
                    && start_idx < self.cells.len()
                    && self.cells[start_idx]
                        .flags
                        .contains(CellFlags::WIDE_CONTINUATION)
                {
                    self.cells[start_idx - 1] = default_cell;
                }
                let end_idx = (row_start + self.width).min(self.cells.len());
                if start_idx < end_idx {
                    self.cells[start_idx..end_idx].fill(default_cell);
                }
            }
            1 => {
                // Start of line to cursor
                let start_idx = row_start;
                let end_idx = (row_start + cur_x + 1).min(self.cells.len());
                if cur_x + 1 < self.width
                    && end_idx < self.cells.len()
                    && self.cells[end_idx - 1].flags.contains(CellFlags::WIDE)
                    && self.cells[end_idx]
                        .flags
                        .contains(CellFlags::WIDE_CONTINUATION)
                {
                    self.cells[end_idx] = default_cell;
                }
                if start_idx < end_idx {
                    self.cells[start_idx..end_idx].fill(default_cell);
                }
            }
            2 => {
                // Entire line
                let start_idx = row_start;
                let end_idx = (row_start + self.width).min(self.cells.len());
                if start_idx < end_idx {
                    self.cells[start_idx..end_idx].fill(default_cell);
                }
            }
            _ => {}
        }
        if (mode == 2 || (mode == 0 && cur_x == 0)) && physical_y < self.row_wrapped.len() {
            self.row_wrapped[physical_y] = false;
        }
        self.damage.mark_dirty(self.cursor.y);
    }

    pub fn erase_display(&mut self, mode: u8, fg: Color, bg: Color) {
        self.clamp_cursor();
        if self.height == 0 || self.width == 0 {
            return;
        }
        let default_cell = Cell::new(' ', fg, bg, CellFlags::empty());
        let cur_y = self.cursor.y.min(self.height.saturating_sub(1));
        let cur_x = self.cursor.x.min(self.width.saturating_sub(1));
        match mode {
            0 => {
                // Cursor to end of display
                let phys_cur_y = self.physical_row(cur_y);
                let start = phys_cur_y * self.width + cur_x;
                if cur_x > 0
                    && start < self.cells.len()
                    && self.cells[start]
                        .flags
                        .contains(CellFlags::WIDE_CONTINUATION)
                {
                    self.cells[start - 1] = default_cell;
                }
                let end = (phys_cur_y + 1) * self.width;
                if start < end && end <= self.cells.len() {
                    self.cells[start..end].fill(default_cell);
                }
                self.damage.mark_dirty(cur_y);
                if phys_cur_y < self.row_wrapped.len() {
                    self.row_wrapped[phys_cur_y] = false;
                }
                for y in (cur_y + 1)..self.height {
                    let phys_y = self.physical_row(y);
                    let row_start = phys_y * self.width;
                    let row_end = row_start + self.width;
                    if row_end <= self.cells.len() {
                        self.cells[row_start..row_end].fill(default_cell);
                    }
                    self.damage.mark_dirty(y);
                    if phys_y < self.row_wrapped.len() {
                        self.row_wrapped[phys_y] = false;
                    }
                }
            }
            1 => {
                // Start of display to cursor
                for y in 0..cur_y {
                    let phys_y = self.physical_row(y);
                    let row_start = phys_y * self.width;
                    let row_end = row_start + self.width;
                    if row_end <= self.cells.len() {
                        self.cells[row_start..row_end].fill(default_cell);
                    }
                    self.damage.mark_dirty(y);
                    if phys_y < self.row_wrapped.len() {
                        self.row_wrapped[phys_y] = false;
                    }
                }
                let phys_cur_y = self.physical_row(cur_y);
                let start = phys_cur_y * self.width;
                let end = (start + cur_x + 1).min(self.cells.len());
                if cur_x + 1 < self.width
                    && end < self.cells.len()
                    && self.cells[end - 1].flags.contains(CellFlags::WIDE)
                    && self.cells[end].flags.contains(CellFlags::WIDE_CONTINUATION)
                {
                    self.cells[end] = default_cell;
                }
                if start < end {
                    self.cells[start..end].fill(default_cell);
                }
                self.damage.mark_dirty(cur_y);
                if phys_cur_y < self.row_wrapped.len() {
                    self.row_wrapped[phys_cur_y] = false;
                }
            }
            2 | 3 => {
                // Entire screen + scrollback buffer
                self.cells.fill(default_cell);
                self.damage.mark_all();
                self.row_wrapped.fill(false);
                self.row_offset = 0;
                self.scrollback.clear();
                self.scroll_offset = 0;
                self.selection.clear();
                self.hyperlinks.clear();
            }
            _ => {}
        }
    }

    pub fn extract_selection_text(&self) -> String {
        if !self.selection.active || self.width == 0 || self.height == 0 {
            return String::new();
        }

        let ((min_x, min_y), (max_x, max_y)) = self.selection.normalized_bounds();
        let history_len = self.scrollback.len();
        let total_lines = history_len + self.height;

        if min_y >= total_lines {
            return String::new();
        }
        let clamped_max_y = max_y.min(total_lines.saturating_sub(1));
        if min_y > clamped_max_y {
            return String::new();
        }

        let num_lines = clamped_max_y - min_y + 1;
        let estimated_capacity = (num_lines * self.width.min(80)).min(20 * 1024 * 1024);
        let mut result = String::with_capacity(estimated_capacity);

        for y in min_y..=clamped_max_y {
            if y > min_y {
                result.push('\n');
            }

            let start_col = if y == min_y { min_x } else { 0 };
            let end_col = if y == max_y {
                max_x.min(self.width.saturating_sub(1))
            } else {
                self.width.saturating_sub(1)
            };

            if start_col > end_col {
                continue;
            }

            if y < history_len {
                self.scrollback.with_row_slice(y, |cells, _| {
                    if cells.is_empty() {
                        return;
                    }
                    let upper = end_col.min(cells.len() - 1);
                    let mut last_col = None;
                    if upper >= start_col {
                        for x in (start_col..=upper).rev() {
                            if cells[x].character != ' ' {
                                last_col = Some(x);
                                break;
                            }
                        }
                    }
                    if let Some(last) = last_col {
                        for x in start_col..=last {
                            result.push(cells[x].character);
                        }
                    }
                });
            } else {
                let grid_y = y - history_len;
                if grid_y < self.height {
                    let physical_y = self.physical_row(grid_y);
                    let row_start = physical_y * self.width;
                    if row_start + self.width <= self.cells.len() {
                        let row_cells = &self.cells[row_start..row_start + self.width];
                        if !row_cells.is_empty() {
                            let upper = end_col.min(row_cells.len() - 1);
                            let mut last_col = None;
                            if upper >= start_col {
                                for x in (start_col..=upper).rev() {
                                    if row_cells[x].character != ' ' {
                                        last_col = Some(x);
                                        break;
                                    }
                                }
                            }
                            if let Some(last) = last_col {
                                for x in start_col..=last {
                                    result.push(row_cells[x].character);
                                }
                            }
                        }
                    }
                }
            }
        }

        result
    }

    pub fn select_word_at(&mut self, col: usize, abs_y: usize) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let history_len = self.scrollback.len();
        let total_lines = history_len + self.height;
        if abs_y >= total_lines || col >= self.width {
            return;
        }

        let is_word_char = |c: char| -> bool {
            c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' || c == '~'
        };

        let find_bounds = |row_slice: &[Cell]| -> Option<(usize, usize)> {
            if col >= row_slice.len() {
                return None;
            }
            let target_c = row_slice[col].character;
            if !is_word_char(target_c) {
                return None;
            }
            let mut start_col = col;
            while start_col > 0 && is_word_char(row_slice[start_col - 1].character) {
                start_col -= 1;
            }
            let mut end_col = col;
            while end_col + 1 < row_slice.len()
                && end_col + 1 < self.width
                && is_word_char(row_slice[end_col + 1].character)
            {
                end_col += 1;
            }
            Some((start_col, end_col))
        };

        let bounds = self.with_row_cells(abs_y, find_bounds).flatten();

        if let Some((start_col, end_col)) = bounds {
            self.selection.start_x = start_col;
            self.selection.start_y = abs_y;
            self.selection.end_x = end_col;
            self.selection.end_y = abs_y;
            self.selection.active = true;
        } else {
            self.selection.start_selection(col, abs_y);
        }
    }

    pub fn select_line_at(&mut self, abs_y: usize) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let history_len = self.scrollback.len();
        let total_lines = history_len + self.height;
        if abs_y >= total_lines {
            return;
        }

        self.selection.start_x = 0;
        self.selection.start_y = abs_y;
        self.selection.end_x = self.width.saturating_sub(1);
        self.selection.end_y = abs_y;
        self.selection.active = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_combining() {
        let mut grid = Grid::new(
            80,
            24,
            Color { r: 0, g: 0, b: 0 },
            Color { r: 0, g: 0, b: 0 },
            1000,
            false,
        );
        grid.put_char(
            'a',
            Color { r: 0, g: 0, b: 0 },
            Color { r: 0, g: 0, b: 0 },
            None,
            CellFlags::empty(),
        );
        grid.put_char(
            '\u{0301}',
            Color { r: 0, g: 0, b: 0 },
            Color { r: 0, g: 0, b: 0 },
            None,
            CellFlags::empty(),
        );

        let cell_char = grid.cells[0].character;
        println!("Cell character: {:?}", cell_char);
        assert!(cell_char >= '\u{100000}');
    }

    #[test]
    fn test_grid_reflow_narrow_and_expand() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 10, fg, bg, 1000, false);

        // Write a 70-character line (fits on 80 cols without wrapping)
        for c in "0123456789012345678901234567890123456789012345678901234567890123456789".chars() {
            grid.put_char(c, fg, bg, None, CellFlags::empty());
        }

        assert!(!grid.row_wrapped[0]);

        // Resize to 40 cols: 70 chars should reflow into Row 0 (40 chars, wrapped=true) and Row 1 (30 chars, wrapped=false)
        grid.resize(40, 10);

        assert!(grid.row_wrapped[0]);
        assert!(!grid.row_wrapped[1]);

        let row0_str: String = (0..40).map(|x| grid.cells[x].character).collect();
        let row1_str: String = (0..30).map(|x| grid.cells[40 + x].character).collect();
        assert_eq!(row0_str, "0123456789012345678901234567890123456789");
        assert_eq!(row1_str, "012345678901234567890123456789");

        // Resize back to 80 cols: should un-wrap back into Row 0 (70 chars, wrapped=false)
        grid.resize(80, 10);
        assert!(!grid.row_wrapped[0]);
        let restored_str: String = (0..70).map(|x| grid.cells[x].character).collect();
        assert_eq!(
            restored_str,
            "0123456789012345678901234567890123456789012345678901234567890123456789"
        );
    }

    #[test]
    fn test_grid_reflow_cursor_tracking() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 10, fg, bg, 1000, false);

        // Fill 50 characters, cursor will be at x=50, y=0
        for _ in 0..50 {
            grid.put_char('X', fg, bg, None, CellFlags::empty());
        }
        assert_eq!(grid.cursor.x, 50);
        grid.resize(30, 10);
        assert_eq!(grid.cursor.x, 20);
        assert_eq!(grid.cursor.y, 1);
    }

    #[test]
    fn test_grid_resize_shrink_erase_bounds_safety() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(98, 58, fg, bg, 1000, false);
        grid.cursor.y = 57;
        grid.cursor.x = 50;

        // Shrink grid size to 98x34 (len = 3332 cells)
        grid.resize(98, 34);

        // Erase operations should safely succeed without panic
        grid.erase_line(0, fg, bg);
        grid.erase_line(1, fg, bg);
        grid.erase_line(2, fg, bg);
        grid.erase_display(0, fg, bg);
        grid.erase_display(1, fg, bg);
        grid.erase_display(2, fg, bg);
    }

    #[test]
    fn test_grid_resize_after_clear_keeps_cleared_screen() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 5, fg, bg, 1000, false);

        // Fill history so scrollback has lines
        for i in 0..15 {
            for c in format!("line {}", i).chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            grid.scroll_or_move_down(bg);
            grid.cursor.x = 0;
        }

        assert!(!grid.scrollback.is_empty());

        // User clears screen and homes cursor
        grid.erase_display(2, fg, bg);
        grid.cursor.x = 0;
        grid.cursor.y = 0;

        // Scrollback should be cleared and scroll offset should be 0
        assert!(grid.scrollback.is_empty());
        assert_eq!(grid.scroll_offset, 0);

        // Resize window to be larger (e.g. 80x20)
        grid.resize(80, 20);

        // Grid row 0 should still be cleared (' '), NOT pulled from scrollback
        assert_eq!(grid.cells[0].character, ' ');
        assert_eq!(grid.cursor.y, 0);
    }
    #[test]
    fn test_reflow_empty_lines() {
        use crate::screen::cell::Color;
        let default_color = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(100, 24, default_color, default_color, 1000, false);

        // row 0: text
        grid.cells[0].character = 'a';

        // row 1: hard-broken empty line (like \n)
        grid.row_wrapped[1] = false;

        // row 2: text
        grid.cells[200].character = 'b';

        // row 3: wrapped empty line (like spaces to edge)
        grid.row_wrapped[3] = true;

        // row 4: prompt
        grid.cells[400].character = '>';

        grid.cursor.x = 1;
        grid.cursor.y = 4;

        for width in [50, 40, 100, 20].iter() {
            grid.resize(*width, 24);

            let mut prompt_y = 0;
            let mut b_y = 0;
            for y in 0..grid.height {
                let row_start = y * grid.width;
                if grid.cells[row_start..row_start + grid.width]
                    .iter()
                    .any(|c| c.character == 'b')
                {
                    b_y = y;
                }
                if grid.cells[row_start..row_start + grid.width]
                    .iter()
                    .any(|c| c.character == '>')
                {
                    prompt_y = y;
                }
            }
            assert_eq!(b_y, 2);
            assert_eq!(prompt_y, 3);
        }
    }

    #[test]
    fn test_extract_selection_text_scrollback_and_grid() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 5, fg, bg, 100, false);

        // Put 10 lines of text (5 scroll into scrollback, 5 remain in active grid)
        for i in 0..10 {
            for c in format!("line-{}", i).chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            if i < 9 {
                grid.scroll_or_move_down(bg);
                grid.cursor.x = 0;
            }
        }

        assert_eq!(grid.scrollback.len(), 5);

        // 1. Select lines 0..2 (entirely within scrollback)
        grid.selection.start_selection(0, 0);
        grid.selection.update_selection(5, 2);
        let text_scrollback = grid.extract_selection_text();
        assert_eq!(text_scrollback, "line-0\nline-1\nline-2");

        // 2. Select lines 3..7 (spanning scrollback and active grid: rows 3,4 from scrollback, 5,6,7 from grid)
        grid.selection.start_selection(0, 3);
        grid.selection.update_selection(5, 7);
        let text_spanning = grid.extract_selection_text();
        assert_eq!(text_spanning, "line-3\nline-4\nline-5\nline-6\nline-7");

        // 3. Select lines 7..9 (entirely within active grid)
        grid.selection.start_selection(0, 7);
        grid.selection.update_selection(5, 9);
        let text_grid = grid.extract_selection_text();
        assert_eq!(text_grid, "line-7\nline-8\nline-9");
    }

    #[test]
    fn test_select_word_and_line_in_scrollback() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 5, fg, bg, 100, false);

        for i in 0..10 {
            for c in format!("hello_world_{} foo-bar", i).chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            if i < 9 {
                grid.scroll_or_move_down(bg);
                grid.cursor.x = 0;
            }
        }

        // Line 0 is in scrollback: "hello_world_0 foo-bar"
        // Select word at col 2 in row 0 ("hello_world_0")
        grid.select_word_at(2, 0);
        assert_eq!(grid.extract_selection_text(), "hello_world_0");

        // Select word at col 15 in row 0 ("foo-bar")
        grid.select_word_at(15, 0);
        assert_eq!(grid.extract_selection_text(), "foo-bar");

        // Select entire line at row 0
        grid.select_line_at(0);
        assert_eq!(grid.extract_selection_text(), "hello_world_0 foo-bar");
    }

    #[test]
    fn test_selection_eviction_on_scroll() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        // Finite scrollback with capacity 5, height 5
        let mut grid = Grid::new(80, 5, fg, bg, 5, false);

        // Scroll 10 lines into scrollback (filling the 5-line capacity)
        for i in 0..10 {
            for c in format!("line-{}", i).chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            grid.scroll(1, bg);
        }

        assert_eq!(grid.scrollback.len(), 5);

        // Select line 1 (which is index 1 in the 5-element scrollback)
        grid.selection.start_selection(0, 1);
        grid.selection.update_selection(5, 1);
        assert_eq!(grid.selection.start_y, 1);
        assert!(grid.selection.active);

        // Scroll 1 more line: row 0 is evicted, old row 1 becomes new row 0
        grid.scroll(1, bg);
        assert!(grid.selection.active);
        assert_eq!(grid.selection.start_y, 0);

        // Scroll 1 more line: row 0 is evicted, selection is dropped
        grid.scroll(1, bg);
        assert!(!grid.selection.active);
    }

    #[test]
    fn test_variation_selector_does_not_corrupt_emoji_codepoint() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 5, fg, bg, 100, false);

        // Put emoji followed by VS16 (emoji presentation selector: \u{fe0f})
        grid.put_char('\u{2764}', fg, bg, None, CellFlags::empty());
        grid.put_char('\u{fe0f}', fg, bg, None, CellFlags::empty());

        let ch = grid
            .with_display_row_slice(0, |row| row[0].character)
            .unwrap();
        assert_eq!(
            ch, '\u{2764}',
            "Base emoji codepoint must be preserved, not converted to PUA"
        );
    }

    #[test]
    fn test_extract_selection_text_empty_scrollback_row() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 5, fg, bg, 100, false);

        // Put line 0: "hello"
        for c in "hello".chars() {
            grid.put_char(c, fg, bg, None, CellFlags::empty());
        }
        grid.scroll(1, bg);

        // Line 1: blank line (all spaces, trimmed to empty slice in scrollback)
        grid.scroll(1, bg);

        // Line 2: "world"
        grid.cursor.x = 0;
        for c in "world".chars() {
            grid.put_char(c, fg, bg, None, CellFlags::empty());
        }
        grid.scroll(1, bg);

        assert_eq!(grid.scrollback.len(), 3);

        // Verify line 1 in scrollback has len 0
        grid.scrollback.with_row_slice(1, |cells, _| {
            assert!(
                cells.is_empty(),
                "Blank line should have trimmed cells to empty slice"
            );
        });

        // Test 1: Selecting the empty row directly
        grid.selection.start_selection(0, 1);
        grid.selection.update_selection(10, 1);
        assert_eq!(grid.extract_selection_text(), "");

        // Test 2: Double-click / select_word_at on empty row
        grid.select_word_at(0, 1);
        assert_eq!(grid.extract_selection_text(), "");

        // Test 3: Triple-click / select_line_at on empty row
        grid.select_line_at(1);
        assert_eq!(grid.extract_selection_text(), "");

        // Test 4: Multi-line selection spanning the empty row (rows 0..=2)
        grid.selection.start_selection(0, 0);
        grid.selection.update_selection(5, 2);
        assert_eq!(grid.extract_selection_text(), "hello\n\nworld");
    }

    #[test]
    fn test_extract_selection_zero_dimensions() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(0, 0, fg, bg, 100, false);

        grid.selection.start_selection(0, 0);
        grid.selection.update_selection(5, 0);
        assert_eq!(grid.extract_selection_text(), "");

        grid.select_word_at(0, 0);
        assert_eq!(grid.extract_selection_text(), "");

        grid.select_line_at(0);
        assert_eq!(grid.extract_selection_text(), "");
    }

    #[test]
    fn test_physical_row_zero_height() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let grid = Grid::new(80, 0, fg, bg, 100, false);
        // Must not panic on % 0
        assert_eq!(grid.physical_row(0), 0);
        assert_eq!(grid.physical_row(5), 0);
    }

    #[test]
    fn test_grid_output_zero_dimensions_no_hang() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(0, 0, fg, bg, 100, false);
        grid.put_ascii('a', fg, bg, None, CellFlags::empty());
        grid.put_ascii_slice(b"hello world", fg, bg, None, CellFlags::empty());
        grid.put_char('🚀', fg, bg, None, CellFlags::empty());
    }

    #[test]
    fn test_wide_char_on_one_column_grid() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(1, 2, fg, bg, 100, false);
        // Writing emoji on 1-column grid should not corrupt next row
        grid.put_char('🚀', fg, bg, None, CellFlags::empty());
        assert_eq!(grid.cells[0].character, '🚀');
        assert!(!grid.cells[1].flags.contains(CellFlags::WIDE_CONTINUATION));
    }
}
