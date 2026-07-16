use crate::screen::cell::Cell;

pub struct Scrollback {
    // scrollback history
}

impl Scrollback {
    pub fn new() -> Self {
        Self {}
    }

    pub fn push_line(&mut self, _line: Vec<Cell>) {
        // stub
    }

    pub fn get_line(&self, _index: usize) -> Option<&[Cell]> {
        None
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new()
    }
}
