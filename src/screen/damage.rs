pub struct DamageTracker {
    pub dirty_rows: Vec<bool>,
}

impl DamageTracker {
    pub fn new(rows: usize) -> Self {
        Self {
            dirty_rows: vec![true; rows],
        }
    }

    pub fn resize(&mut self, rows: usize) {
        self.dirty_rows.resize(rows, true);
    }

    pub fn mark_dirty(&mut self, row: usize) {
        if row < self.dirty_rows.len() {
            self.dirty_rows[row] = true;
        }
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new(24)
    }
}
