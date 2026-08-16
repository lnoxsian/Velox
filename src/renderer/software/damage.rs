/// Tracks dirty terminal rows and full-redraw conditions for the software renderer.
#[derive(Debug, Clone)]
pub struct DamageMap {
    pub dirty_rows: Vec<bool>,
    pub full_redraw: bool,
    pub prev_cursor_pos: Option<(usize, usize)>,
    pub prev_selection_range: Option<(usize, usize)>,
}

impl DamageMap {
    pub fn new(rows: usize) -> Self {
        Self {
            dirty_rows: vec![true; rows.max(1)],
            full_redraw: true,
            prev_cursor_pos: None,
            prev_selection_range: None,
        }
    }

    pub fn resize(&mut self, rows: usize) {
        let r = rows.max(1);
        self.dirty_rows.resize(r, true);
        self.full_redraw = true;
    }

    #[inline(always)]
    pub fn mark_row(&mut self, row: usize) {
        if row < self.dirty_rows.len() {
            self.dirty_rows[row] = true;
        }
    }

    #[inline(always)]
    pub fn mark_all(&mut self) {
        self.full_redraw = true;
        self.dirty_rows.fill(true);
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.full_redraw = false;
        self.dirty_rows.fill(false);
    }

    #[inline(always)]
    pub fn is_dirty(&self, row: usize) -> bool {
        self.full_redraw || (row < self.dirty_rows.len() && self.dirty_rows[row])
    }

    #[inline(always)]
    pub fn has_damage(&self) -> bool {
        self.full_redraw || self.dirty_rows.iter().any(|&d| d)
    }

    /// Ingest damage flags from the active terminal grid.
    pub fn sync_from_grid(&mut self, grid_dirty: &[bool]) {
        for (dst, &src) in self.dirty_rows.iter_mut().zip(grid_dirty.iter()) {
            if src {
                *dst = true;
            }
        }
    }

    /// Mark old and new cursor rows dirty if cursor position moved.
    pub fn update_cursor(&mut self, cursor_x: usize, cursor_y: usize, visible: bool) {
        if !visible {
            if let Some((_, old_y)) = self.prev_cursor_pos.take() {
                self.mark_row(old_y);
            }
            return;
        }

        if let Some((old_x, old_y)) = self.prev_cursor_pos {
            if old_x != cursor_x || old_y != cursor_y {
                self.mark_row(old_y);
                self.mark_row(cursor_y);
                self.prev_cursor_pos = Some((cursor_x, cursor_y));
            }
        } else {
            self.mark_row(cursor_y);
            self.prev_cursor_pos = Some((cursor_x, cursor_y));
        }
    }

    /// Mark old and new selection rows dirty when selection changes.
    pub fn update_selection(&mut self, active: bool, min_y: usize, max_y: usize) {
        if !active {
            if let Some((old_min, old_max)) = self.prev_selection_range.take() {
                for y in old_min..=old_max {
                    self.mark_row(y);
                }
            }
            return;
        }

        let new_range = (min_y, max_y);
        if self.prev_selection_range != Some(new_range) {
            if let Some((old_min, old_max)) = self.prev_selection_range {
                for y in old_min..=old_max {
                    self.mark_row(y);
                }
            }
            for y in min_y..=max_y {
                self.mark_row(y);
            }
            self.prev_selection_range = Some(new_range);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_map_operations() {
        let mut dm = DamageMap::new(10);
        assert!(dm.has_damage());
        assert!(dm.full_redraw);

        dm.clear();
        assert!(!dm.has_damage());
        assert!(!dm.is_dirty(3));

        dm.mark_row(3);
        assert!(dm.has_damage());
        assert!(dm.is_dirty(3));
        assert!(!dm.is_dirty(4));
    }
}
