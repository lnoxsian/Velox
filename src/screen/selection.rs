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
}

impl Default for Selection {
    fn default() -> Self {
        Self::new()
    }
}
