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

    #[allow(dead_code)]
    #[inline(always)]
    pub fn contains_coords(
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
        Self::contains_coords(min_x, min_y, max_x, max_y, x, y)
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
