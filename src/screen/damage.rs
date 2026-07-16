pub struct DamageTracker {
    // damage tracking state
}

impl DamageTracker {
    pub fn new() -> Self {
        Self {}
    }

    pub fn mark_dirty(&mut self, _row: usize, _col: usize) {
        // stub
    }

    pub fn clear(&mut self) {
        // stub
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new()
    }
}
