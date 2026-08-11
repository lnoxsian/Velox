use crate::screen::cell::{Cell, CellFlags, Color};
use crate::screen::cursor::Cursor;
use crate::screen::damage::DamageTracker;
use crate::screen::scrollback::Scrollback;
use crate::screen::selection::Selection;
use std::sync::{Mutex, OnceLock};
use unicode_width::UnicodeWidthChar;

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
    pub scroll_region_top: usize,
    pub scroll_region_bottom: usize,
    pub scroll_offset: usize,
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
            scrollback: Scrollback::new(scrollback_limit, infinite_scrollback),
            selection: Selection::new(),
            default_fg: fg,
            default_bg: bg,
            scroll_region_top: 0,
            scroll_region_bottom: height.saturating_sub(1),
            scroll_offset: 0,
        }
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

    pub fn put_char(&mut self, c: char, fg: Color, bg: Color, mut flags: CellFlags) {
        self.clamp_cursor();
        if self.width == 0 || self.height == 0 {
            return;
        }
        let is_combining = UnicodeWidthChar::width(c) == Some(0);
        if is_combining && self.cursor.x > 0 {
            let mut target_x = self.cursor.x - 1;
            let mut idx = self.cursor.y * self.width + target_x;
            if idx < self.cells.len()
                && self.cells[idx].flags.contains(CellFlags::WIDE_CONTINUATION)
                && target_x > 0
            {
                target_x -= 1;
                idx = self.cursor.y * self.width + target_x;
            }
            if idx < self.cells.len() {
                let base_char = self.cells[idx].character;
                if ('\u{100000}'..='\u{10ffff}').contains(&base_char) {
                    let reg_idx = (base_char as u32 - 0x100000) as usize;
                    if let Ok(mut registry) = get_combining_registry().lock()
                        && reg_idx < registry.len()
                    {
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

        let w = UnicodeWidthChar::width(c).unwrap_or(1).max(1);

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

    pub fn erase_characters(&mut self, n: usize, fg: Color, bg: Color) {
        self.clamp_cursor();
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
        self.clamp_cursor();
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
        self.clamp_cursor();
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

    pub fn erase_line(&mut self, mode: u8, fg: Color, bg: Color) {
        self.clamp_cursor();
        if self.height == 0 || self.width == 0 {
            return;
        }
        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        let cur_x = self.cursor.x.min(self.width.saturating_sub(1));
        let row_start = self.cursor.y * self.width;
        match mode {
            0 => {
                // Cursor to end of line
                let start_idx = row_start + cur_x;
                let end_idx = (row_start + self.width).min(self.cells.len());
                if start_idx < end_idx {
                    for cell in &mut self.cells[start_idx..end_idx] {
                        *cell = default_cell;
                    }
                }
            }
            1 => {
                // Start of line to cursor
                let start_idx = row_start;
                let end_idx = (row_start + cur_x + 1).min(self.cells.len());
                if start_idx < end_idx {
                    for cell in &mut self.cells[start_idx..end_idx] {
                        *cell = default_cell;
                    }
                }
            }
            2 => {
                // Entire line
                let start_idx = row_start;
                let end_idx = (row_start + self.width).min(self.cells.len());
                if start_idx < end_idx {
                    for cell in &mut self.cells[start_idx..end_idx] {
                        *cell = default_cell;
                    }
                }
            }
            _ => {}
        }
        if (mode == 2 || (mode == 0 && cur_x == 0)) && self.cursor.y < self.row_wrapped.len() {
            self.row_wrapped[self.cursor.y] = false;
        }
        self.damage.mark_dirty(self.cursor.y);
    }

    pub fn erase_display(&mut self, mode: u8, fg: Color, bg: Color) {
        self.clamp_cursor();
        let default_cell = Cell {
            character: ' ',
            foreground: fg,
            background: bg,
            flags: CellFlags::empty(),
        };
        let cur_y = self.cursor.y.min(self.height.saturating_sub(1));
        let cur_x = self.cursor.x.min(self.width.saturating_sub(1));
        match mode {
            0 => {
                // Cursor to end of display
                let start = (cur_y * self.width + cur_x).min(self.cells.len());
                for cell in &mut self.cells[start..] {
                    *cell = default_cell;
                }
                for y in cur_y..self.height {
                    self.damage.mark_dirty(y);
                    if y < self.row_wrapped.len() {
                        self.row_wrapped[y] = false;
                    }
                }
            }
            1 => {
                // Start of display to cursor
                let len = self.cells.len();
                if len > 0 {
                    let end = (cur_y * self.width + cur_x).min(len - 1);
                    for cell in &mut self.cells[0..=end] {
                        *cell = default_cell;
                    }
                    for y in 0..=cur_y {
                        self.damage.mark_dirty(y);
                        if y < self.row_wrapped.len() {
                            self.row_wrapped[y] = false;
                        }
                    }
                }
            }
            2 | 3 => {
                // Entire screen + scrollback buffer
                for cell in &mut self.cells {
                    *cell = default_cell;
                }
                for y in 0..self.height {
                    self.damage.mark_dirty(y);
                    if y < self.row_wrapped.len() {
                        self.row_wrapped[y] = false;
                    }
                }
                self.scrollback.clear();
                self.scroll_offset = 0;
            }
            _ => {}
        }
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
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            1000,
            false,
        );
        grid.put_char(
            'a',
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            CellFlags::empty(),
        );
        grid.put_char(
            '\u{0301}',
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
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
            a: 255,
        };
        let bg = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        let mut grid = Grid::new(80, 10, fg, bg, 1000, false);

        // Write a 70-character line (fits on 80 cols without wrapping)
        for c in "0123456789012345678901234567890123456789012345678901234567890123456789".chars() {
            grid.put_char(c, fg, bg, CellFlags::empty());
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
            a: 255,
        };
        let bg = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        let mut grid = Grid::new(80, 10, fg, bg, 1000, false);

        // Fill 50 characters, cursor will be at x=50, y=0
        for _ in 0..50 {
            grid.put_char('X', fg, bg, CellFlags::empty());
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
            a: 255,
        };
        let bg = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
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
            a: 255,
        };
        let bg = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        let mut grid = Grid::new(80, 5, fg, bg, 1000, false);

        // Fill history so scrollback has lines
        for i in 0..15 {
            for c in format!("line {}", i).chars() {
                grid.put_char(c, fg, bg, CellFlags::empty());
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
        let default_color = Color { r: 0, g: 0, b: 0, a: 255 };
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
                let row_start = y as usize * grid.width;
                if grid.cells[row_start..row_start + grid.width].iter().any(|c| c.character == 'b') {
                    b_y = y;
                }
                if grid.cells[row_start..row_start + grid.width].iter().any(|c| c.character == '>') {
                    prompt_y = y;
                }
            }
            assert_eq!(b_y, 2);
            assert_eq!(prompt_y, 3);
        }
    }
}
