/// Tracks dirty terminal rows and full-redraw conditions for the software renderer.
#[derive(Debug, Clone)]
pub struct DamageMap {
    pub dirty_rows: Vec<bool>,
    pub full_redraw: bool,
    pub prev_cursor_pos: Option<(usize, usize)>,
    pub prev_selection: Option<((usize, usize), (usize, usize))>,
}

impl DamageMap {
    pub fn new(rows: usize) -> Self {
        Self {
            dirty_rows: vec![true; rows.max(1)],
            full_redraw: true,
            prev_cursor_pos: None,
            prev_selection: None,
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
    pub fn sync_from_grid(&mut self, grid_dirty: &[bool], grid_full_redraw: bool) {
        if grid_full_redraw {
            self.full_redraw = true;
            self.dirty_rows.fill(true);
        } else {
            for (dst, &src) in self.dirty_rows.iter_mut().zip(grid_dirty.iter()) {
                if src {
                    *dst = true;
                }
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
    pub fn update_selection(
        &mut self,
        active: bool,
        bounds: ((usize, usize), (usize, usize)),
        history_len: usize,
        scroll_offset: usize,
        grid_h: usize,
    ) {
        let new_selection = if active { Some(bounds) } else { None };

        if self.prev_selection != new_selection {
            let mark_selection_rows = |dm: &mut DamageMap, min_abs_y: usize, max_abs_y: usize| {
                for abs_y in min_abs_y..=max_abs_y {
                    let total_y = abs_y + scroll_offset;
                    if total_y >= history_len {
                        let screen_y = total_y - history_len;
                        if screen_y < grid_h {
                            dm.mark_row(screen_y);
                        }
                    }
                }
            };

            if let Some(((_, old_min_y), (_, old_max_y))) = self.prev_selection {
                mark_selection_rows(self, old_min_y, old_max_y);
            }
            if let Some(((_, new_min_y), (_, new_max_y))) = new_selection {
                mark_selection_rows(self, new_min_y, new_max_y);
            }

            self.prev_selection = new_selection;
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
