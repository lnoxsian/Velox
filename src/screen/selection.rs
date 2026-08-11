#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start_x: usize,
    pub start_y: usize,
    pub end_x: usize,
    pub end_y: usize,
    pub active: bool,
}

impl Selection {
    pub fn new() -> Self {
        Self {
            start_x: 0,
            start_y: 0,
            end_x: 0,
            end_y: 0,
            active: false,
        }
    }

    pub fn clear(&mut self) {
        self.active = false;
    }

    pub fn start_selection(&mut self, x: usize, y: usize) {
        self.start_x = x;
        self.start_y = y;
        self.end_x = x;
        self.end_y = y;
        self.active = true;
    }

    pub fn update_selection(&mut self, x: usize, y: usize) {
        self.end_x = x;
        self.end_y = y;
        self.active = true;
    }

    pub fn normalized_bounds(&self) -> ((usize, usize), (usize, usize)) {
        if self.start_y < self.end_y || (self.start_y == self.end_y && self.start_x <= self.end_x) {
            ((self.start_x, self.start_y), (self.end_x, self.end_y))
        } else {
            ((self.end_x, self.end_y), (self.start_x, self.start_y))
        }
    }

    #[inline(always)]
    pub fn contains_fast(
        &self,
        min_x: usize,
        min_y: usize,
        max_x: usize,
        max_y: usize,
        x: usize,
        y: usize,
    ) -> bool {
        if y < min_y || y > max_y {
            return false;
        }
        if min_y == max_y {
            return x >= min_x && x <= max_x;
        }
        if y == min_y {
            return x >= min_x;
        }
        if y == max_y {
            return x <= max_x;
        }
        true
    }

    #[allow(dead_code)]
    pub fn contains(&self, x: usize, y: usize) -> bool {
        if !self.active {
            return false;
        }

        let ((min_x, min_y), (max_x, max_y)) = self.normalized_bounds();
        self.contains_fast(min_x, min_y, max_x, max_y, x, y)
    }

    pub fn extract_text(
        &self,
        width: usize,
        height: usize,
        cells: &[crate::screen::cell::Cell],
    ) -> String {
        if !self.active {
            return String::new();
        }

        let ((min_x, min_y), (max_x, max_y)) = self.normalized_bounds();
        let mut lines = Vec::new();

        for y in min_y..=max_y {
            if y >= height {
                break;
            }

            let start_col = if y == min_y { min_x } else { 0 };
            let end_col = if y == max_y {
                max_x.min(width.saturating_sub(1))
            } else {
                width.saturating_sub(1)
            };

            let mut line = String::new();
            for x in start_col..=end_col {
                let idx = y * width + x;
                if idx < cells.len() {
                    line.push(cells[idx].character);
                }
            }
            lines.push(line.trim_end().to_string());
        }

        lines.join("\n")
    }

    pub fn select_word(
        &mut self,
        width: usize,
        height: usize,
        cells: &[crate::screen::cell::Cell],
        x: usize,
        y: usize,
    ) {
        if y >= height || x >= width {
            return;
        }

        let is_word_char = |c: char| -> bool {
            c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' || c == '~'
        };

        let target_idx = y * width + x;
        if target_idx >= cells.len() {
            return;
        }

        let target_c = cells[target_idx].character;

        if !is_word_char(target_c) {
            self.start_selection(x, y);
            return;
        }

        let mut start_col = x;
        while start_col > 0 {
            let idx = y * width + (start_col - 1);
            if is_word_char(cells[idx].character) {
                start_col -= 1;
            } else {
                break;
            }
        }

        let mut end_col = x;
        while end_col + 1 < width {
            let idx = y * width + (end_col + 1);
            if is_word_char(cells[idx].character) {
                end_col += 1;
            } else {
                break;
            }
        }

        self.start_x = start_col;
        self.start_y = y;
        self.end_x = end_col;
        self.end_y = y;
        self.active = true;
    }

    pub fn select_line(&mut self, width: usize, height: usize, y: usize) {
        if y >= height {
            return;
        }

        self.start_x = 0;
        self.start_y = y;
        self.end_x = width.saturating_sub(1);
        self.end_y = y;
        self.active = true;
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_bounds_and_contains() {
        let mut sel = Selection::new();
        sel.start_selection(2, 5);
        sel.update_selection(8, 5);
        assert!(sel.contains(2, 5));
        assert!(sel.contains(5, 5));
        assert!(sel.contains(8, 5));
        assert!(!sel.contains(1, 5));
        assert!(!sel.contains(9, 5));
    }
}
