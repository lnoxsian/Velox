#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowDamage {
    pub min_col: usize,
    pub max_col: usize,
}

impl RowDamage {
    #[inline(always)]
    pub const fn empty() -> Self {
        Self {
            min_col: usize::MAX,
            max_col: 0,
        }
    }

    #[inline(always)]
    pub const fn full() -> Self {
        Self {
            min_col: 0,
            max_col: usize::MAX,
        }
    }

    #[inline(always)]
    pub fn expand(&mut self, min: usize, max: usize) {
        self.min_col = self.min_col.min(min);
        self.max_col = self.max_col.max(max);
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.min_col > self.max_col
    }
}

#[derive(Debug, Clone)]
pub struct DamageTracker {
    pub dirty_rows: Vec<bool>,
    dirty_indices: Vec<usize>,
    pub full_redraw: bool,
    pub row_damage: Vec<RowDamage>,
    pub scrolled_lines: usize,
}

impl DamageTracker {
    pub fn new(rows: usize) -> Self {
        let r = rows.max(1);
        Self {
            dirty_rows: vec![true; r],
            dirty_indices: Vec::new(),
            full_redraw: true,
            row_damage: vec![RowDamage::full(); r],
            scrolled_lines: 0,
        }
    }

    pub fn resize(&mut self, rows: usize) {
        let r = rows.max(1);
        self.dirty_rows.resize(r, true);
        self.row_damage.resize(r, RowDamage::full());
        self.full_redraw = true;
        self.dirty_indices.clear();
        self.scrolled_lines = 0;
    }

    #[inline(always)]
    pub fn mark_dirty(&mut self, row: usize) {
        if row < self.dirty_rows.len() {
            if !self.dirty_rows[row] {
                self.dirty_rows[row] = true;
                self.dirty_indices.push(row);
            }
            if row < self.row_damage.len() {
                self.row_damage[row] = RowDamage::full();
            }
        }
    }

    #[inline(always)]
    pub fn mark_range_dirty(&mut self, row: usize, min_col: usize, max_col: usize) {
        if row < self.dirty_rows.len() {
            if !self.dirty_rows[row] {
                self.dirty_rows[row] = true;
                self.dirty_indices.push(row);
            }
            if row < self.row_damage.len() {
                self.row_damage[row].expand(min_col, max_col);
            }
        }
    }

    #[inline(always)]
    pub fn mark_cell_dirty(&mut self, row: usize, col: usize) {
        self.mark_range_dirty(row, col, col);
    }

    #[inline(always)]
    pub fn mark_all(&mut self) {
        self.full_redraw = true;
        self.dirty_rows.fill(true);
        self.dirty_indices.clear();
        self.scrolled_lines = 0;
        for rd in &mut self.row_damage {
            *rd = RowDamage::full();
        }
    }

    #[inline(always)]
    pub fn mark_scrolled(&mut self, lines: usize) {
        self.scrolled_lines = self.scrolled_lines.saturating_add(lines);
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.full_redraw = false;
        self.dirty_rows.fill(false);
        self.dirty_indices.clear();
        self.scrolled_lines = 0;
        for rd in &mut self.row_damage {
            *rd = RowDamage::empty();
        }
    }

    #[inline(always)]
    pub fn is_dirty(&self, row: usize) -> bool {
        self.full_redraw || (row < self.dirty_rows.len() && self.dirty_rows[row])
    }

    #[inline(always)]
    pub fn dirty_indices(&self) -> &[usize] {
        &self.dirty_indices
    }

    #[inline(always)]
    pub fn has_damage(&self) -> bool {
        if self.full_redraw {
            return true;
        }
        if !self.dirty_indices.is_empty() {
            return true;
        }
        let slice = self.dirty_rows.as_slice();
        let (prefix, words, suffix) = unsafe { slice.align_to::<usize>() };
        prefix.iter().any(|&d| d) || words.iter().any(|&w| w != 0) || suffix.iter().any(|&d| d)
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new(24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_tracker_operations() {
        let mut tracker = DamageTracker::new(10);
        assert!(tracker.has_damage());
        assert!(tracker.full_redraw);
        assert!(tracker.is_dirty(0));
        assert!(tracker.is_dirty(9));

        tracker.clear();
        assert!(!tracker.has_damage());
        assert!(!tracker.full_redraw);
        assert!(!tracker.is_dirty(3));
        assert_eq!(tracker.dirty_indices().len(), 0);

        // Mark single row
        tracker.mark_dirty(3);
        assert!(tracker.has_damage());
        assert!(tracker.is_dirty(3));
        assert!(!tracker.is_dirty(4));
        assert_eq!(tracker.dirty_indices(), &[3]);

        // Duplicate marking does not duplicate index
        tracker.mark_dirty(3);
        assert_eq!(tracker.dirty_indices(), &[3]);

        // Mark second row
        tracker.mark_dirty(7);
        assert_eq!(tracker.dirty_indices(), &[3, 7]);

        // Out-of-bounds marking is safe
        tracker.mark_dirty(99);
        assert_eq!(tracker.dirty_indices(), &[3, 7]);

        // Clear resets everything
        tracker.clear();
        assert!(!tracker.has_damage());
        assert_eq!(tracker.dirty_indices().len(), 0);

        // Column range tracking
        tracker.mark_cell_dirty(2, 5);
        assert!(tracker.is_dirty(2));
        assert_eq!(tracker.row_damage[2].min_col, 5);
        assert_eq!(tracker.row_damage[2].max_col, 5);

        tracker.mark_range_dirty(2, 2, 8);
        assert_eq!(tracker.row_damage[2].min_col, 2);
        assert_eq!(tracker.row_damage[2].max_col, 8);
    }

    #[test]
    fn test_damage_tracker_resize() {
        let mut tracker = DamageTracker::new(10);
        tracker.clear();
        assert!(!tracker.has_damage());

        tracker.resize(20);
        assert!(tracker.has_damage());
        assert!(tracker.full_redraw);
        assert_eq!(tracker.dirty_rows.len(), 20);
        assert_eq!(tracker.row_damage.len(), 20);
    }
}
