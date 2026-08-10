use crate::screen::grid::Grid;
use crate::screen::cell::{Cell, Color, CellFlags};

impl Grid {
    pub fn scroll_or_move_down(&mut self, bg: Color) {
        self.clamp_cursor();
        let bottom = self.scroll_region_bottom.min(self.height.saturating_sub(1));
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

    pub fn scroll_or_move_up(&mut self, bg: Color) {
        self.clamp_cursor();
        let top = self.scroll_region_top.min(self.height.saturating_sub(1));
        if self.cursor.y > top {
            self.cursor.y -= 1;
        } else if self.cursor.y == top {
            self.scroll_down(1, bg);
        } else {
            if self.cursor.y > 0 {
                self.cursor.y -= 1;
            }
        }
    }

    pub fn scroll(&mut self, delta: i32, bg: Color) {
        if delta <= 0 {
            return;
        }
        let top = self.scroll_region_top.min(self.height.saturating_sub(1));
        let bottom = self.scroll_region_bottom.min(self.height.saturating_sub(1));
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
        let top = self.scroll_region_top.min(self.height.saturating_sub(1));
        let bottom = self.scroll_region_bottom.min(self.height.saturating_sub(1));
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

    pub fn insert_lines(&mut self, n: usize, fg: Color, bg: Color) {
        self.clamp_cursor();
        let top = self.cursor.y;
        let bottom = self.scroll_region_bottom.min(self.height.saturating_sub(1));
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
        self.clamp_cursor();
        let top = self.cursor.y;
        let bottom = self.scroll_region_bottom.min(self.height.saturating_sub(1));
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
}
