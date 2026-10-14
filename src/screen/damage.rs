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

    #[inline(always)]
    pub fn mark_dirty(&mut self, row: usize) {
        if !self.full_redraw && row < self.dirty_rows.len() {
            self.dirty_rows[row] = true;
        }
    }

    #[inline(always)]
    pub fn mark_all(&mut self) {
        if !self.full_redraw {
            self.full_redraw = true;
            self.dirty_rows.fill(true);
        }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.full_redraw = false;
        self.dirty_rows.fill(false);
    }

    #[inline(always)]
    pub fn has_damage(&self) -> bool {
        if self.full_redraw {
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
