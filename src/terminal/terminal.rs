use crate::screen::grid::Grid;

pub struct Terminal {
    pub grid: Grid,
}

impl Terminal {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            grid: Grid::new(width, height),
        }
    }

    pub fn execute(&mut self) {
        // stub
    }

    pub fn handle_input(&mut self) {
        // stub
    }

    pub fn handle_mouse(&mut self) {
        // stub
    }

    pub fn send_to_shell(&mut self, _data: &[u8]) {
        // stub
    }

    pub fn update_cursor(&mut self) {
        // stub
    }

    pub fn paste(&mut self) {
        // stub
    }

    pub fn copy(&mut self) {
        // stub
    }

    pub fn select(&mut self) {
        // stub
    }
}
