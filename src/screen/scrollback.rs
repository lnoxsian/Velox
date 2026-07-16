use crate::screen::cell::Cell;
use std::collections::VecDeque;

pub struct Scrollback {
    pub max_lines: usize,
    pub lines: VecDeque<Vec<Cell>>,
}

impl Scrollback {
    pub fn new(max_lines: usize) -> Self {
        Self {
            max_lines,
            lines: VecDeque::new(),
        }
    }

    pub fn push_line(&mut self, cells: &[Cell]) {
        if self.max_lines == 0 {
            return;
        }
        if self.lines.len() >= self.max_lines {
            if let Some(mut reused) = self.lines.pop_front() {
                reused.clear();
                reused.extend_from_slice(cells);
                self.lines.push_back(reused);
                return;
            }
        }
        self.lines.push_back(cells.to_vec());
    }

    pub fn get_line(&self, index: usize) -> Option<&[Cell]> {
        self.lines.get(index).map(|v| v.as_slice())
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new(1000)
    }
}
