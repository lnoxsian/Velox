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
                self.scrollback.push_line(&self.cells[start..end]);
            }
        }

        if u_delta < height_of_region {
            let src = (top + u_delta) * self.width;
            let dst = top * self.width;
            let count = (height_of_region - u_delta) * self.width;
            self.cells.copy_within(src..src + count, dst);
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
                for cell in &mut self.cells {
                    *cell = default_cell;
                }
                for y in 0..self.height {
                    self.damage.mark_dirty(y);
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
        self.scroll_region_top = 0;
        self.scroll_region_bottom = new_h.saturating_sub(1);
        self.damage.resize(new_h);
        for y in 0..new_h {
            self.damage.mark_dirty(y);
        }
        self.cursor.x = self.cursor.x.min(new_w - 1);
        self.cursor.y = self.cursor.y.min(new_h - 1);
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
}

