use crate::screen::cell::Cell;
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub wrapped: bool,
}

impl std::ops::Deref for Row {
    type Target = Vec<Cell>;
    fn deref(&self) -> &Self::Target {
        &self.cells
    }
}

impl std::ops::DerefMut for Row {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cells
    }
}

pub struct Scrollback {
    pub max_lines: usize,
    pub lines: VecDeque<Row>,
}

impl Scrollback {
    pub fn new(max_lines: usize) -> Self {
        Self {
            max_lines,
            lines: VecDeque::new(),
        }
    }

    pub fn push_line(&mut self, cells: &[Cell], wrapped: bool) {
        if self.max_lines == 0 {
            return;
        }
        if self.lines.len() >= self.max_lines
            && let Some(mut reused) = self.lines.pop_front() {
                reused.cells.clear();
                reused.cells.extend_from_slice(cells);
                reused.wrapped = wrapped;
                self.lines.push_back(reused);
                return;
            }
        self.lines.push_back(Row {
            cells: cells.to_vec(),
            wrapped,
        });
    }

    pub fn get_line(&self, index: usize) -> Option<&[Cell]> {
        self.lines.get(index).map(|v| v.cells.as_slice())
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
