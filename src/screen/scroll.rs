use crate::screen::cell::{Cell, CellFlags, Color};
use crate::screen::grid::Grid;

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
        let default_cell = Cell::new(' ', self.default_fg, bg, CellFlags::empty());

        // Fast path: Full-screen scroll via circular row offset rotation O(1) with ZERO memory copying
        if top == 0 && bottom == self.height - 1 {
            for y in 0..u_delta.min(self.height) {
                let physical_y = (self.row_offset + y) % self.height;
                let start = physical_y * self.width;
                let end = start + self.width;
                let wrapped = self.row_wrapped.get(physical_y).copied().unwrap_or(false);
                let prev_len = self.scrollback.len();
                self.scrollback.push_line(&self.cells[start..end], wrapped);
                let new_len = self.scrollback.len();
                let evicted = (prev_len + 1).saturating_sub(new_len);
                if evicted > 0 && self.selection.active {
                    let max_y = self.selection.start_y.max(self.selection.end_y);
                    if max_y < evicted {
                        self.selection.clear();
                    } else {
                        self.selection.start_y = self.selection.start_y.saturating_sub(evicted);
                        self.selection.end_y = self.selection.end_y.saturating_sub(evicted);
                    }
                }
                // Clear the scrolled-off physical row so it becomes the fresh bottom row
                self.cells[start..end].fill(default_cell);
                if physical_y < self.row_wrapped.len() {
                    self.row_wrapped[physical_y] = false;
                }
            }

            self.row_offset = (self.row_offset + u_delta) % self.height;

            if self.scroll_offset > 0 {
                self.scroll_offset = (self.scroll_offset + u_delta).min(self.scrollback.len());
            }

            self.damage.mark_all();
            return;
        }

        // Sub-region scrolling (e.g. vim/htop): Normalize row offset first, then standard copy_within
        self.normalize_row_offset();

        if u_delta < height_of_region {
            let src = (top + u_delta) * self.width;
            let dst = top * self.width;
            let count = (height_of_region - u_delta) * self.width;
            self.cells.copy_within(src..src + count, dst);

            if top + u_delta < self.row_wrapped.len() {
                let last_valid = bottom.min(self.row_wrapped.len() - 1);
                self.row_wrapped
                    .copy_within((top + u_delta)..=last_valid, top);
            }
        }

        let clear_start = (bottom + 1 - u_delta) * self.width;
        let clear_end = (bottom + 1) * self.width;
        self.cells[clear_start..clear_end].fill(default_cell);

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

    pub fn scroll_down(&mut self, delta: usize, bg: Color) {
        let top = self.scroll_region_top.min(self.height.saturating_sub(1));
        let bottom = self.scroll_region_bottom.min(self.height.saturating_sub(1));
        if top >= bottom || bottom >= self.height {
            return;
        }
        let height_of_region = bottom - top + 1;
        let u_delta = delta.min(height_of_region);
        let default_cell = Cell::new(' ', self.default_fg, bg, CellFlags::empty());

        if top == 0 && bottom == self.height - 1 {
            // Full-screen reverse scroll via circular row offset
            for y in 0..u_delta {
                let physical_y = (self.row_offset + self.height - 1 - y) % self.height;
                let start = physical_y * self.width;
                let end = start + self.width;
                self.cells[start..end].fill(default_cell);
                if physical_y < self.row_wrapped.len() {
                    self.row_wrapped[physical_y] = false;
                }
            }
            self.row_offset = (self.row_offset + self.height - (u_delta % self.height)) % self.height;
            self.damage.mark_all();
            return;
        }

        self.normalize_row_offset();
        if u_delta < height_of_region {
            let src = top * self.width;
            let dst = (top + u_delta) * self.width;
            let count = (height_of_region - u_delta) * self.width;
            self.cells.copy_within(src..src + count, dst);

            if top < self.row_wrapped.len() {
                let last_valid = (bottom - u_delta).min(self.row_wrapped.len() - 1);
                self.row_wrapped
                    .copy_within(top..=last_valid, top + u_delta);
            }
        }

        let clear_start = top * self.width;
        let clear_end = (top + u_delta) * self.width;
        self.cells[clear_start..clear_end].fill(default_cell);

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
        self.normalize_row_offset();
        let top_idx = if top == 0 {
            0
        } else {
            (top - 1).min(self.height - 1)
        };
        let bottom_idx = if bottom == 0 {
            self.height - 1
        } else {
            (bottom - 1).min(self.height - 1)
        };

        if top_idx < bottom_idx {
            self.scroll_region_top = top_idx;
            self.scroll_region_bottom = bottom_idx;
        }
        // When scroll region is changed, standard terminal behavior is to home the cursor
        self.cursor.x = 0;
        self.cursor.y = 0;
    }

    pub fn insert_lines(&mut self, n: usize, fg: Color, bg: Color) {
        self.normalize_row_offset();
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
                    self.row_wrapped
                        .copy_within(top..=last_valid, top + u_delta);
                }
            }
            let default_cell = Cell::new(' ', fg, bg, CellFlags::empty());
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
        self.normalize_row_offset();
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
                    self.row_wrapped
                        .copy_within((top + u_delta)..=last_valid, top);
                }
            }
            let default_cell = Cell::new(' ', fg, bg, CellFlags::empty());
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
