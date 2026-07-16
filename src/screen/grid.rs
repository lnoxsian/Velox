use crate::screen::cell::{Cell, Color, CellFlags};
use crate::screen::cursor::Cursor;
use crate::screen::damage::DamageTracker;
use crate::screen::scrollback::Scrollback;
use crate::screen::selection::Selection;
use unicode_width::UnicodeWidthChar;

pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub cursor: Cursor,
    pub saved_cursor: Cursor,
    pub damage: DamageTracker,
    pub scrollback: Scrollback,
    pub selection: Selection,
    pub default_fg: Color,
    pub default_bg: Color,
}

impl Grid {
    pub fn new(width: usize, height: usize, fg: Color, bg: Color) -> Self {
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
            damage: DamageTracker::new(height),
            scrollback: Scrollback::new(1000),
            selection: Selection::new(),
            default_fg: fg,
            default_bg: bg,
        }
    }

    pub fn put_char(&mut self, c: char, fg: Color, bg: Color, mut flags: CellFlags) {
        // Nerd Font icons and emojis are treated as double-width (w = 2) for rendering size, CJK characters are also w = 2
        let w = if (c >= '\u{e000}' && c <= '\u{f8ff}') || c >= '\u{1f000}' {
            2
        } else {
            UnicodeWidthChar::width(c).unwrap_or(1).max(1) as usize
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
            self.cursor.x = 0;
            self.scroll_or_move_down();
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

    pub fn scroll_or_move_down(&mut self) {
        if self.cursor.y + 1 < self.height {
            self.cursor.y += 1;
        } else {
            self.scroll(1);
        }
    }

    pub fn scroll(&mut self, delta: i32) {
        if delta <= 0 {
            return;
        }
        let u_delta = delta as usize;

        // Push lines scrolled off-screen to scrollback
        for y in 0..u_delta.min(self.height) {
            let start = y * self.width;
            let end = start + self.width;
            self.scrollback.push_line(self.cells[start..end].to_vec());
        }

        if u_delta >= self.height {
            self.clear();
            return;
        }

        // Shift cells up
        self.cells.copy_within((u_delta * self.width).., 0);

        // Clear bottom lines
        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: self.default_bg,
            flags: CellFlags::empty(),
        };
        let start = (self.height - u_delta) * self.width;
        for cell in &mut self.cells[start..] {
            *cell = default_cell;
        }

        // Mark all rows as damaged
        for y in 0..self.height {
            self.damage.mark_dirty(y);
        }
    }

    pub fn erase_line(&mut self, mode: u8) {
        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: self.default_bg,
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
        self.damage.mark_dirty(self.cursor.y);
    }

    pub fn erase_display(&mut self, mode: u8) {
        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: self.default_bg,
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
                }
            }
            2 | 3 => { // Entire screen
                self.clear();
            }
            _ => {}
        }
    }

    pub fn erase(&mut self) {
        self.clear();
    }

    pub fn clear(&mut self) {
        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: self.default_bg,
            flags: CellFlags::empty(),
        };
        for cell in &mut self.cells {
            *cell = default_cell;
        }
        for y in 0..self.height {
            self.damage.mark_dirty(y);
        }
    }

    pub fn copy_region(&self) -> String {
        let mut res = String::new();
        for y in 0..self.height {
            let row_start = y * self.width;
            let mut line = String::new();
            for x in 0..self.width {
                line.push(self.cells[row_start + x].character);
            }
            res.push_str(line.trim_end());
            res.push('\n');
        }
        res
    }

    pub fn mark_dirty(&mut self, row: usize, _col: usize) {
        self.damage.mark_dirty(row);
    }

    pub fn swap_alternate(&mut self) {
        // Handled in Terminal
    }

    pub fn restore_main(&mut self) {
        // Handled in Terminal
    }

    pub fn resize(&mut self, cols: u32, rows: u32) {
        let new_w = cols as usize;
        let new_h = rows as usize;
        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: self.default_bg,
            flags: CellFlags::empty(),
        };
        let mut new_cells = vec![default_cell; new_w * new_h];
        for y in 0..self.height.min(new_h) {
            for x in 0..self.width.min(new_w) {
                new_cells[y * new_w + x] = self.cells[y * self.width + x];
            }
        }
        self.cells = new_cells;
        self.width = new_w;
        self.height = new_h;
        self.damage.resize(new_h);
        for y in 0..new_h {
            self.damage.mark_dirty(y);
        }
        self.cursor.x = self.cursor.x.min(new_w - 1);
        self.cursor.y = self.cursor.y.min(new_h - 1);
    }
}
