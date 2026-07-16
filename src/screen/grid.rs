use crate::screen::cell::Cell;
use crate::screen::cursor::Cursor;
use crate::screen::damage::DamageTracker;
use crate::screen::scrollback::Scrollback;
use crate::screen::selection::Selection;

pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub cursor: Cursor,
    pub damage: DamageTracker,
    pub scrollback: Scrollback,
    pub selection: Selection,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: Vec::new(),
            cursor: Cursor {
                x: 0,
                y: 0,
                shape: crate::screen::cursor::CursorShape::Block,
                visible: true,
            },
            damage: DamageTracker::new(),
            scrollback: Scrollback::new(),
            selection: Selection::new(),
        }
    }

    pub fn put_char(&mut self, _c: char) {
        // stub
    }

    pub fn erase(&mut self) {
        // stub
    }

    pub fn scroll(&mut self, _delta: i32) {
        // stub
    }

    pub fn resize(&mut self, _cols: u32, _rows: u32) {
        // stub
    }

    pub fn clear(&mut self) {
        // stub
    }

    pub fn copy_region(&self) -> String {
        String::new()
    }

    pub fn mark_dirty(&mut self, _row: usize, _col: usize) {
        // stub
    }

    pub fn swap_alternate(&mut self) {
        // stub
    }

    pub fn restore_main(&mut self) {
        // stub
    }
}
