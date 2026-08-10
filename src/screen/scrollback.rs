use crate::screen::cell::Cell;
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Seek, SeekFrom, Write, Read};
use std::cell::RefCell;
use tempfile::tempfile;
use bincode;

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
    pub infinite: bool,
    disk_store: Option<RefCell<File>>,
    disk_index: Vec<(u64, usize)>,
}

impl Scrollback {
    pub fn new(max_lines: usize, infinite: bool) -> Self {
        Self {
            max_lines,
            lines: VecDeque::new(),
            infinite,
            disk_store: if infinite { tempfile().ok().map(RefCell::new) } else { None },
            disk_index: Vec::new(),
        }
    }

    pub fn push_line(&mut self, cells: &[Cell], wrapped: bool) {
        if self.max_lines == 0 && !self.infinite {
            return;
        }
        if self.lines.len() >= self.max_lines {
            if self.infinite {
                if let Some(file_ref) = self.disk_store.as_ref() {
                    let oldest = self.lines.pop_front().unwrap();
                    if let Ok(mut file) = file_ref.try_borrow_mut() {
                        if let Ok(bytes) = bincode::serialize(&oldest) {
                            if let Ok(offset) = file.stream_position() {
                                if file.write_all(&bytes).is_ok() {
                                    self.disk_index.push((offset, bytes.len()));
                                }
                            }
                        }
                    }
                } else {
                    self.lines.pop_front();
                }
            } else if let Some(mut reused) = self.lines.pop_front() {
                reused.cells.clear();
                reused.cells.extend_from_slice(cells);
                reused.wrapped = wrapped;
                self.lines.push_back(reused);
                return;
            }
        }
        self.lines.push_back(Row {
            cells: cells.to_vec(),
            wrapped,
        });
    }

    pub fn len(&self) -> usize {
        self.disk_index.len() + self.lines.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_row(&self, index: usize) -> Option<Row> {
        let disk_len = self.disk_index.len();
        if index < disk_len {
            if let Some(file_ref) = self.disk_store.as_ref() {
                if let Ok(mut file) = file_ref.try_borrow_mut() {
                    let (offset, size) = self.disk_index[index];
                    if file.seek(SeekFrom::Start(offset)).is_ok() {
                        let mut buffer = vec![0; size];
                        if file.read_exact(&mut buffer).is_ok() {
                            if let Ok(row) = bincode::deserialize(&buffer) {
                                return Some(row);
                            }
                        }
                    }
                }
            }
            None
        } else {
            let ram_index = index - disk_len;
            self.lines.get(ram_index).cloned()
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.disk_index.clear();
        if self.infinite {
            if let Some(file_ref) = self.disk_store.as_ref() {
                if let Ok(mut file) = file_ref.try_borrow_mut() {
                    let _ = file.set_len(0);
                    let _ = file.seek(SeekFrom::Start(0));
                }
            }
        }
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new(1000, false)
    }
}
