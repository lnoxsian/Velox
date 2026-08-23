pub struct DamageTracker {
    pub dirty_rows: Vec<bool>,
    pub full_redraw: bool,
}

impl DamageTracker {
    pub fn new(rows: usize) -> Self {
        Self {
            dirty_rows: vec![true; rows.max(1)],
            full_redraw: true,
        }
    }

    pub fn resize(&mut self, rows: usize) {
        self.dirty_rows.resize(rows.max(1), true);
        self.full_redraw = true;
    }

    pub fn mark_dirty(&mut self, row: usize) {
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
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new(24)
    }
}
