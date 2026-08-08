use crate::screen::cell::{Cell, Color, CellFlags};
use crate::screen::cursor::Cursor;
use crate::screen::damage::DamageTracker;
use crate::screen::scrollback::Scrollback;
use crate::screen::selection::Selection;
use unicode_width::UnicodeWidthChar;
use std::sync::{Mutex, OnceLock};

static COMBINING_REGISTRY: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub fn get_combining_registry() -> &'static Mutex<Vec<String>> {
    COMBINING_REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
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
    pub saved_flags: CellFlags,
    pub saved_g0_charset: u8,
    pub saved_g1_charset: u8,
    pub saved_active_charset: u8,
    pub damage: DamageTracker,
    pub scrollback: Scrollback,
    pub selection: Selection,
    pub default_fg: Color,
    pub default_bg: Color,
    pub enable_nerdfont: bool,
    pub scroll_region_top: usize,
    pub scroll_region_bottom: usize,
    pub scroll_offset: usize,
}

impl Grid {
    pub fn new(width: usize, height: usize, fg: Color, bg: Color, enable_nerdfont: bool, scrollback_limit: usize) -> Self {
        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
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
            saved_flags: CellFlags::empty(),
            saved_g0_charset: 0,
            saved_g1_charset: 0,
            saved_active_charset: 0,
            damage: DamageTracker::new(height),
            scrollback: Scrollback::new(scrollback_limit),
            selection: Selection::new(),
            default_fg: fg,
            default_bg: bg,
            enable_nerdfont,
            scroll_region_top: 0,
            scroll_region_bottom: height.saturating_sub(1),
            scroll_offset: 0,
        }
    }

    pub fn put_char(&mut self, c: char, fg: Color, bg: Color, mut flags: CellFlags) {
        let is_combining = UnicodeWidthChar::width(c) == Some(0);
        if is_combining && self.cursor.x > 0 {
            let mut target_x = self.cursor.x - 1;
            let mut idx = self.cursor.y * self.width + target_x;
            if idx < self.cells.len() && self.cells[idx].flags.contains(CellFlags::WIDE_CONTINUATION)
                && target_x > 0 {
                    target_x -= 1;
                    idx = self.cursor.y * self.width + target_x;
                }
            if idx < self.cells.len() {
                let base_char = self.cells[idx].character;
                if ('\u{100000}'..='\u{10ffff}').contains(&base_char) {
                    let reg_idx = (base_char as u32 - 0x100000) as usize;
                    if let Ok(mut registry) = get_combining_registry().lock()
                        && reg_idx < registry.len() {
                            registry[reg_idx].push(c);
                        }
                } else {
                    let mut seq = String::new();
                    seq.push(base_char);
                    seq.push(c);
                    let new_char = if let Ok(mut registry) = get_combining_registry().lock() {
                        let reg_idx = registry.len();
                        registry.push(seq);
                        char::from_u32(0x100000 + reg_idx as u32).unwrap_or(base_char)
                    } else {
                        base_char
                    };
                    self.cells[idx].character = new_char;
                }
            }
            return;
        }

        // Nerd Font icons and emojis are treated as double-width (w = 2) for rendering size if enabled, CJK characters are also w = 2
        let w = if (self.enable_nerdfont && ('\u{e000}'..='\u{f8ff}').contains(&c)) || c >= '\u{1f000}' {
            2
        } else {
            UnicodeWidthChar::width(c).unwrap_or(1).max(1)
        };

        if self.cursor.x + w > self.width {
            // Fill rest of the row with spaces, then wrap
            for x in self.cursor.x..self.width {
                let idx = self.cursor.y * self.width + x;
                if idx < self.cells.len() {
                    self.cells[idx] = Cell {
                        character: ' ',
                        foreground: fg,
                        background: bg,
                        flags: CellFlags::empty(),
                    };
                }
            }
            if self.cursor.y < self.row_wrapped.len() {
                self.row_wrapped[self.cursor.y] = true;
            }
            self.cursor.x = 0;
            self.scroll_or_move_down(bg);
        }

        if w == 2 {
            flags.insert(CellFlags::WIDE);
            let idx = self.cursor.y * self.width + self.cursor.x;
            if idx < self.cells.len() {
                self.cells[idx] = Cell {
                    character: c,
                    foreground: fg,
                    background: bg,
                    flags,
                };
            }
            self.cursor.x += 1;

            let idx_next = self.cursor.y * self.width + self.cursor.x;
            if idx_next < self.cells.len() {
                self.cells[idx_next] = Cell {
                    character: ' ',
                    foreground: fg,
                    background: bg,
                    flags: CellFlags::WIDE_CONTINUATION,
                };
            }
            self.cursor.x += 1;
        } else {
            let idx = self.cursor.y * self.width + self.cursor.x;
            if idx < self.cells.len() {
                self.cells[idx] = Cell {
                    character: c,
                    foreground: fg,
                    background: bg,
                    flags,
                };
            }
            self.cursor.x += 1;
        }
        self.damage.mark_dirty(self.cursor.y);
    }

    pub fn scroll_or_move_down(&mut self, bg: Color) {
        let bottom = self.scroll_region_bottom;
        if self.cursor.y < bottom {
            self.cursor.y += 1;
        } else if self.cursor.y == bottom {
            self.scroll(1, bg);
        } else {
            if self.cursor.y + 1 < self.height {
                self.cursor.y += 1;
            }
        }
    }

    pub fn scroll(&mut self, delta: i32, bg: Color) {
        if delta <= 0 {
            return;
        }
        let top = self.scroll_region_top;
        let bottom = self.scroll_region_bottom;
        if top >= bottom || bottom >= self.height {
            return;
        }
        let height_of_region = bottom - top + 1;
        let u_delta = (delta as usize).min(height_of_region);

        // Push lines scrolled off-screen to scrollback only if scrolling the entire screen
        if top == 0 && bottom == self.height - 1 {
            for y in 0..u_delta.min(self.height) {
                let start = y * self.width;
                let end = start + self.width;
                let wrapped = self.row_wrapped.get(y).copied().unwrap_or(false);
                self.scrollback.push_line(&self.cells[start..end], wrapped);
            }
        }

        if u_delta < height_of_region {
            let src = (top + u_delta) * self.width;
            let dst = top * self.width;
            let count = (height_of_region - u_delta) * self.width;
            self.cells.copy_within(src..src + count, dst);

            if top + u_delta < self.row_wrapped.len() {
                let last_valid = bottom.min(self.row_wrapped.len() - 1);
                self.row_wrapped.copy_within((top + u_delta)..=last_valid, top);
            }
        }

        // Clear bottom lines of the scrolling region
        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        let clear_start = (bottom + 1 - u_delta) * self.width;
        let clear_end = (bottom + 1) * self.width;
        for cell in &mut self.cells[clear_start..clear_end] {
            *cell = default_cell;
        }

        let clear_start_y = bottom + 1 - u_delta;
        for y in clear_start_y..=bottom {
            if y < self.row_wrapped.len() {
                self.row_wrapped[y] = false;
            }
        }

        // Mark rows in scrolling region as damaged
        for y in top..=bottom {
            self.damage.mark_dirty(y);
        }
    }

    pub fn scroll_down(&mut self, delta: usize, bg: Color) {
        let top = self.scroll_region_top;
        let bottom = self.scroll_region_bottom;
        if top >= bottom || bottom >= self.height {
            return;
        }
        let height_of_region = bottom - top + 1;
        let u_delta = delta.min(height_of_region);

        if u_delta < height_of_region {
            let src = top * self.width;
            let dst = (top + u_delta) * self.width;
            let count = (height_of_region - u_delta) * self.width;
            self.cells.copy_within(src..src + count, dst);

            if top < self.row_wrapped.len() {
                let last_valid = (bottom - u_delta).min(self.row_wrapped.len() - 1);
                self.row_wrapped.copy_within(top..=last_valid, top + u_delta);
            }
        }

        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        let clear_start = top * self.width;
        let clear_end = (top + u_delta) * self.width;
        for cell in &mut self.cells[clear_start..clear_end] {
            *cell = default_cell;
        }

        for y in top..(top + u_delta) {
            if y < self.row_wrapped.len() {
                self.row_wrapped[y] = false;
            }
        }

        for y in top..=bottom {
            self.damage.mark_dirty(y);
        }
    }

    pub fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let top_idx = if top == 0 { 0 } else { (top - 1).min(self.height - 1) };
        let bottom_idx = if bottom == 0 { self.height - 1 } else { (bottom - 1).min(self.height - 1) };

        if top_idx < bottom_idx {
            self.scroll_region_top = top_idx;
            self.scroll_region_bottom = bottom_idx;
        }
        // When scroll region is changed, standard terminal behavior is to home the cursor
        self.cursor.x = 0;
        self.cursor.y = 0;
    }

    pub fn erase_characters(&mut self, n: usize, fg: Color, bg: Color) {
        let cursor_y = self.cursor.y;
        let cursor_x = self.cursor.x;
        if cursor_y >= self.height || cursor_x >= self.width {
            return;
        }
        let start = cursor_y * self.width + cursor_x;
        let end = (start + n).min(cursor_y * self.width + self.width);
        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        for cell in &mut self.cells[start..end] {
            *cell = default_cell;
        }
        self.damage.mark_dirty(cursor_y);
    }

    pub fn delete_characters(&mut self, n: usize, fg: Color, bg: Color) {
        let cursor_y = self.cursor.y;
        let cursor_x = self.cursor.x;
        if cursor_y >= self.height || cursor_x >= self.width {
            return;
        }
        let row_start = cursor_y * self.width;
        let n = n.min(self.width - cursor_x);
        let move_start = row_start + cursor_x + n;
        let move_end = row_start + self.width;
        let dest = row_start + cursor_x;
        self.cells.copy_within(move_start..move_end, dest);

        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        let fill_start = row_start + self.width - n;
        for cell in &mut self.cells[fill_start..row_start + self.width] {
            *cell = default_cell;
        }
        self.damage.mark_dirty(cursor_y);
    }

    pub fn insert_characters(&mut self, n: usize, fg: Color, bg: Color) {
        let cursor_y = self.cursor.y;
        let cursor_x = self.cursor.x;
        if cursor_y >= self.height || cursor_x >= self.width {
            return;
        }
        let row_start = cursor_y * self.width;
        let n = n.min(self.width - cursor_x);
        let move_start = row_start + cursor_x;
        let move_end = row_start + self.width - n;
        let dest = row_start + cursor_x + n;
        self.cells.copy_within(move_start..move_end, dest);

        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        for cell in &mut self.cells[move_start..move_start + n] {
            *cell = default_cell;
        }
        self.damage.mark_dirty(cursor_y);
    }

    pub fn insert_lines(&mut self, n: usize, fg: Color, bg: Color) {
        let top = self.cursor.y;
        let bottom = self.scroll_region_bottom;
        if top <= bottom && bottom < self.height {
            let height_of_region = bottom - top + 1;
            let u_delta = n.min(height_of_region);
            if u_delta < height_of_region {
                let src = top * self.width;
                let dst = (top + u_delta) * self.width;
                let count = (height_of_region - u_delta) * self.width;
                self.cells.copy_within(src..src + count, dst);

                if top < self.row_wrapped.len() {
                    let last_valid = (bottom - u_delta).min(self.row_wrapped.len() - 1);
                    self.row_wrapped.copy_within(top..=last_valid, top + u_delta);
                }
            }
            let default_cell = Cell {
                character: ' ',
                foreground: fg,
                background: bg,
                flags: CellFlags::empty(),
            };
            let clear_start = top * self.width;
            let clear_end = (top + u_delta) * self.width;
            for cell in &mut self.cells[clear_start..clear_end] {
                *cell = default_cell;
            }
            for y in top..(top + u_delta) {
                if y < self.row_wrapped.len() {
                    self.row_wrapped[y] = false;
                }
            }
            for y in top..=bottom {
                self.damage.mark_dirty(y);
            }
        }
    }

    pub fn delete_lines(&mut self, n: usize, fg: Color, bg: Color) {
        let top = self.cursor.y;
        let bottom = self.scroll_region_bottom;
        if top <= bottom && bottom < self.height {
            let height_of_region = bottom - top + 1;
            let u_delta = n.min(height_of_region);
            if u_delta < height_of_region {
                let src = (top + u_delta) * self.width;
                let dst = top * self.width;
                let count = (height_of_region - u_delta) * self.width;
                self.cells.copy_within(src..src + count, dst);

                if top + u_delta < self.row_wrapped.len() {
                    let last_valid = bottom.min(self.row_wrapped.len() - 1);
                    self.row_wrapped.copy_within((top + u_delta)..=last_valid, top);
                }
            }
            let default_cell = Cell {
                character: ' ',
                foreground: fg,
                background: bg,
                flags: CellFlags::empty(),
            };
            let clear_start = (bottom + 1 - u_delta) * self.width;
            let clear_end = (bottom + 1) * self.width;
            for cell in &mut self.cells[clear_start..clear_end] {
                *cell = default_cell;
            }
            let clear_start_y = bottom + 1 - u_delta;
            for y in clear_start_y..=bottom {
                if y < self.row_wrapped.len() {
                    self.row_wrapped[y] = false;
                }
            }
            for y in top..=bottom {
                self.damage.mark_dirty(y);
            }
        }
    }

    pub fn erase_line(&mut self, mode: u8, fg: Color, bg: Color) {
        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        let row_start = self.cursor.y * self.width;
        match mode {
            0 => { // Cursor to end of line
                for x in self.cursor.x..self.width {
                    self.cells[row_start + x] = default_cell;
                }
            }
            1 => { // Start of line to cursor
                for x in 0..=self.cursor.x.min(self.width - 1) {
                    self.cells[row_start + x] = default_cell;
                }
            }
            2 => { // Entire line
                for x in 0..self.width {
                    self.cells[row_start + x] = default_cell;
                }
            }
            _ => {}
        }
        if mode == 2 || (mode == 0 && self.cursor.x == 0) {
            if self.cursor.y < self.row_wrapped.len() {
                self.row_wrapped[self.cursor.y] = false;
            }
        }
        self.damage.mark_dirty(self.cursor.y);
    }

    pub fn erase_display(&mut self, mode: u8, fg: Color, bg: Color) {
        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        match mode {
            0 => { // Cursor to end of display
                let start = self.cursor.y * self.width + self.cursor.x;
                for cell in &mut self.cells[start..] {
                    *cell = default_cell;
                }
                for y in self.cursor.y..self.height {
                    self.damage.mark_dirty(y);
                    if y < self.row_wrapped.len() {
                        self.row_wrapped[y] = false;
                    }
                }
            }
            1 => { // Start of display to cursor
                let len = self.cells.len();
                let end = (self.cursor.y * self.width + self.cursor.x).min(len - 1);
                for cell in &mut self.cells[0..=end] {
                    *cell = default_cell;
                }
                for y in 0..=self.cursor.y {
                    self.damage.mark_dirty(y);
                    if y < self.row_wrapped.len() {
                        self.row_wrapped[y] = false;
                    }
                }
            }
            2 | 3 => { // Entire screen
                for cell in &mut self.cells {
                    *cell = default_cell;
                }
                for y in 0..self.height {
                    self.damage.mark_dirty(y);
                    if y < self.row_wrapped.len() {
                        self.row_wrapped[y] = false;
                    }
                }
            }
            _ => {}
        }
    }
    pub fn mark_dirty(&mut self, row: usize, _col: usize) {
        self.damage.mark_dirty(row);
    }

    pub fn resize(&mut self, cols: u32, rows: u32) {
        let new_w = cols as usize;
        let new_h = rows as usize;
        if new_w == 0 || new_h == 0 {
            return;
        }

        if new_w == self.width && new_h == self.height {
            return;
        }

        self.selection.clear();

        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: self.default_bg,
            flags: CellFlags::empty(),
        };

        struct CombinedRow {
            cells: Vec<Cell>,
            wrapped: bool,
        }

        let mut combined_rows: Vec<CombinedRow> = Vec::new();

        for row in &self.scrollback.lines {
            combined_rows.push(CombinedRow {
                cells: row.cells.clone(),
                wrapped: row.wrapped,
            });
        }

        let scrollback_count = combined_rows.len();
        let old_w = self.width;
        let old_h = self.height;

        let mut last_used_y = self.cursor.y;
        for y in (0..old_h).rev() {
            let start = y * old_w;
            let end = start + old_w;
            if end <= self.cells.len() {
                let row_cells = &self.cells[start..end];
                if row_cells.iter().any(|c| c.character != ' ' || !c.flags.is_empty() || c.background != self.default_bg) {
                    last_used_y = last_used_y.max(y);
                    break;
                }
            }
        }

        for y in 0..=last_used_y.min(old_h.saturating_sub(1)) {
            let start = y * old_w;
            let end = start + old_w;
            if end <= self.cells.len() {
                let cells = self.cells[start..end].to_vec();
                let wrapped = self.row_wrapped.get(y).copied().unwrap_or(false);
                combined_rows.push(CombinedRow { cells, wrapped });
            }
        }

        let old_cursor_row_idx = scrollback_count + self.cursor.y;
        let old_cursor_col = self.cursor.x;

        struct LogicalLine {
            cells: Vec<Cell>,
            hard_break: bool,
        }

        let mut logical_lines: Vec<LogicalLine> = Vec::new();
        let mut cursor_logical_line_idx: usize = 0;
        let mut cursor_logical_cell_idx: usize = 0;
        let mut cursor_found = false;

        let mut current_cells: Vec<Cell> = Vec::new();

        for (row_idx, row) in combined_rows.iter().enumerate() {
            let start_len = current_cells.len();

            if row.wrapped {
                current_cells.extend_from_slice(&row.cells);
            } else {
                let mut last_non_default = 0;
                for (i, cell) in row.cells.iter().enumerate() {
                    if cell.character != ' ' || !cell.flags.is_empty() || cell.background != self.default_bg {
                        last_non_default = i + 1;
                    }
                }
                let keep_len = if row_idx == old_cursor_row_idx {
                    last_non_default.max(old_cursor_col + 1)
                } else {
                    last_non_default
                };
                current_cells.extend_from_slice(&row.cells[..keep_len.min(row.cells.len())]);
            }

            if !cursor_found && row_idx == old_cursor_row_idx {
                cursor_logical_line_idx = logical_lines.len();
                cursor_logical_cell_idx = start_len + old_cursor_col.min(row.cells.len());
                cursor_found = true;
            }

            if !row.wrapped {
                logical_lines.push(LogicalLine {
                    cells: std::mem::take(&mut current_cells),
                    hard_break: true,
                });
            }
        }

        if !current_cells.is_empty() || logical_lines.is_empty() {
            if !cursor_found {
                cursor_logical_line_idx = logical_lines.len();
                cursor_logical_cell_idx = current_cells.len() + old_cursor_col;
            }
            logical_lines.push(LogicalLine {
                cells: current_cells,
                hard_break: false,
            });
        }

        struct ReflowedRow {
            cells: Vec<Cell>,
            wrapped: bool,
        }

        fn pad_row(mut chunk: Vec<Cell>, new_w: usize, default_cell: Cell) -> Vec<Cell> {
            while chunk.len() < new_w {
                chunk.push(default_cell);
            }
            chunk
        }

        let mut reflowed_rows: Vec<ReflowedRow> = Vec::new();
        let mut new_cursor_row_idx: usize = 0;
        let mut new_cursor_col: usize = 0;
        let mut new_cursor_found = false;

        for (log_idx, log_line) in logical_lines.iter().enumerate() {
            let cells = &log_line.cells;

            if cells.is_empty() {
                if !new_cursor_found && log_idx == cursor_logical_line_idx {
                    new_cursor_row_idx = reflowed_rows.len();
                    new_cursor_col = 0;
                    new_cursor_found = true;
                }
                reflowed_rows.push(ReflowedRow {
                    cells: vec![default_cell; new_w],
                    wrapped: !log_line.hard_break,
                });
                continue;
            }

            let mut chunk: Vec<Cell> = Vec::with_capacity(new_w);
            let mut i = 0;

            while i < cells.len() {
                let cell = cells[i];

                if !new_cursor_found && log_idx == cursor_logical_line_idx && i == cursor_logical_cell_idx {
                    new_cursor_row_idx = reflowed_rows.len();
                    new_cursor_col = chunk.len();
                    new_cursor_found = true;
                }

                let is_wide = cell.flags.contains(CellFlags::WIDE);

                if is_wide && chunk.len() + 2 > new_w {
                    if chunk.len() < new_w {
                        chunk.push(default_cell);
                    }
                    reflowed_rows.push(ReflowedRow {
                        cells: pad_row(chunk, new_w, default_cell),
                        wrapped: true,
                    });
                    chunk = Vec::with_capacity(new_w);
                    continue;
                }

                chunk.push(cell);
                if is_wide && i + 1 < cells.len() && cells[i + 1].flags.contains(CellFlags::WIDE_CONTINUATION) {
                    i += 1;
                    chunk.push(cells[i]);
                }

                i += 1;

                if chunk.len() >= new_w {
                    let is_last = i >= cells.len();
                    let wrapped = if is_last { !log_line.hard_break } else { true };
                    reflowed_rows.push(ReflowedRow {
                        cells: chunk,
                        wrapped,
                    });
                    chunk = Vec::with_capacity(new_w);
                }
            }

            if !new_cursor_found && log_idx == cursor_logical_line_idx {
                new_cursor_row_idx = if chunk.is_empty() && !reflowed_rows.is_empty() {
                    reflowed_rows.len() - 1
                } else {
                    reflowed_rows.len()
                };
                new_cursor_col = chunk.len().min(new_w.saturating_sub(1));
                new_cursor_found = true;
            }

            if !chunk.is_empty() {
                reflowed_rows.push(ReflowedRow {
                    cells: pad_row(chunk, new_w, default_cell),
                    wrapped: !log_line.hard_break,
                });
            }
        }

        if !new_cursor_found {
            new_cursor_row_idx = reflowed_rows.len().saturating_sub(1);
            new_cursor_col = 0;
        }

        let total_rows = reflowed_rows.len();

        let grid_start = if total_rows <= new_h {
            0
        } else {
            let max_start = total_rows - new_h;
            let min_start = new_cursor_row_idx.saturating_sub(new_h - 1);
            max_start.min(min_start)
        };

        self.scrollback.clear();
        for row in &reflowed_rows[..grid_start] {
            self.scrollback.push_line(&row.cells, row.wrapped);
        }

        let mut new_grid_cells = Vec::with_capacity(new_w * new_h);
        let mut new_row_wrapped = Vec::with_capacity(new_h);

        for y in 0..new_h {
            let row_idx = grid_start + y;
            if row_idx < total_rows {
                new_grid_cells.extend_from_slice(&reflowed_rows[row_idx].cells);
                new_row_wrapped.push(reflowed_rows[row_idx].wrapped);
            } else {
                new_grid_cells.extend(std::iter::repeat(default_cell).take(new_w));
                new_row_wrapped.push(false);
            }
        }

        self.cells = new_grid_cells;
        self.row_wrapped = new_row_wrapped;
        self.width = new_w;
        self.height = new_h;
        self.scroll_region_top = 0;
        self.scroll_region_bottom = new_h.saturating_sub(1);

        self.cursor.x = new_cursor_col.min(new_w.saturating_sub(1));
        self.cursor.y = if new_cursor_row_idx >= grid_start {
            (new_cursor_row_idx - grid_start).min(new_h.saturating_sub(1))
        } else {
            0
        };

        self.damage.resize(new_h);
        for y in 0..new_h {
            self.damage.mark_dirty(y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_combining() {
        let mut grid = Grid::new(80, 24, Color { r:0, g:0, b:0, a:255 }, Color { r:0, g:0, b:0, a:255 }, false, 1000);
        grid.put_char('a', Color { r:0, g:0, b:0, a:255 }, Color { r:0, g:0, b:0, a:255 }, CellFlags::empty());
        grid.put_char('\u{0301}', Color { r:0, g:0, b:0, a:255 }, Color { r:0, g:0, b:0, a:255 }, CellFlags::empty());
        
        let cell_char = grid.cells[0].character;
        println!("Cell character: {:?}", cell_char);
        assert!(cell_char >= '\u{100000}');
    }

    #[test]
    fn test_grid_reflow_narrow_and_expand() {
        let fg = Color { r: 255, g: 255, b: 255, a: 255 };
        let bg = Color { r: 0, g: 0, b: 0, a: 255 };
        let mut grid = Grid::new(80, 10, fg, bg, false, 1000);

        // Write a 70-character line (fits on 80 cols without wrapping)
        for c in "0123456789012345678901234567890123456789012345678901234567890123456789".chars() {
            grid.put_char(c, fg, bg, CellFlags::empty());
        }

        assert_eq!(grid.row_wrapped[0], false);

        // Resize to 40 cols: 70 chars should reflow into Row 0 (40 chars, wrapped=true) and Row 1 (30 chars, wrapped=false)
        grid.resize(40, 10);

        assert_eq!(grid.row_wrapped[0], true);
        assert_eq!(grid.row_wrapped[1], false);

        let row0_str: String = (0..40).map(|x| grid.cells[0 * 40 + x].character).collect();
        let row1_str: String = (0..30).map(|x| grid.cells[1 * 40 + x].character).collect();
        assert_eq!(row0_str, "0123456789012345678901234567890123456789");
        assert_eq!(row1_str, "012345678901234567890123456789");

        // Resize back to 80 cols: should un-wrap back into Row 0 (70 chars, wrapped=false)
        grid.resize(80, 10);
        assert_eq!(grid.row_wrapped[0], false);
        let restored_str: String = (0..70).map(|x| grid.cells[0 * 80 + x].character).collect();
        assert_eq!(restored_str, "0123456789012345678901234567890123456789012345678901234567890123456789");
    }

    #[test]
    fn test_grid_reflow_cursor_tracking() {
        let fg = Color { r: 255, g: 255, b: 255, a: 255 };
        let bg = Color { r: 0, g: 0, b: 0, a: 255 };
        let mut grid = Grid::new(80, 10, fg, bg, false, 1000);

        // Fill 50 characters, cursor will be at x=50, y=0
        for _ in 0..50 {
            grid.put_char('X', fg, bg, CellFlags::empty());
        }
        assert_eq!(grid.cursor.x, 50);
        assert_eq!(grid.cursor.y, 0);

        // Resize to 30 cols: 50 characters reflow into Row 0 (30 chars) + Row 1 (20 chars). Cursor at x=20, y=1
        grid.resize(30, 10);
        assert_eq!(grid.cursor.x, 20);
        assert_eq!(grid.cursor.y, 1);
    }
}

