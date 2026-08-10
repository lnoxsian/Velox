use crate::screen::grid::Grid;
use crate::screen::cell::{Cell, CellFlags};

impl Grid {
    pub fn resize(&mut self, cols: u32, rows: u32) {
        let new_w = cols as usize;
        let new_h = rows as usize;
        if new_w == 0 || new_h == 0 {
            return;
        }

        if new_w == self.width && new_h == self.height {
            return;
        }

        self.selection.clear();

        let default_cell = Cell {
            character: ' ',
            foreground: self.default_fg,
            background: self.default_bg,
            flags: CellFlags::empty(),
        };

        struct CombinedRow {
            cells: Vec<Cell>,
            wrapped: bool,
        }

        let mut combined_rows: Vec<CombinedRow> = Vec::new();

        for row in &self.scrollback.lines {
            combined_rows.push(CombinedRow {
                cells: row.cells.clone(),
                wrapped: row.wrapped,
            });
        }

        let scrollback_count = combined_rows.len();
        let old_w = self.width;
        let old_h = self.height;

        let mut last_used_y = self.cursor.y;
        for y in (0..old_h).rev() {
            let start = y * old_w;
            let end = start + old_w;
            if end <= self.cells.len() {
                let row_cells = &self.cells[start..end];
                if row_cells.iter().any(|c| c.character != ' ' || !c.flags.is_empty() || c.background != self.default_bg) {
                    last_used_y = last_used_y.max(y);
                    break;
                }
            }
        }

        for y in 0..=last_used_y.min(old_h.saturating_sub(1)) {
            let start = y * old_w;
            let end = start + old_w;
            if end <= self.cells.len() {
                let cells = self.cells[start..end].to_vec();
                let wrapped = self.row_wrapped.get(y).copied().unwrap_or(false);
                combined_rows.push(CombinedRow { cells, wrapped });
            }
        }

        let old_cursor_row_idx = scrollback_count + self.cursor.y;
        let old_cursor_col = self.cursor.x;

        struct LogicalLine {
            cells: Vec<Cell>,
            hard_break: bool,
        }

        let mut logical_lines: Vec<LogicalLine> = Vec::new();
        let mut cursor_logical_line_idx: usize = 0;
        let mut cursor_logical_cell_idx: usize = 0;
        let mut cursor_found = false;
        let mut active_screen_start_log_idx: Option<usize> = None;

        let mut current_cells: Vec<Cell> = Vec::new();

        for (row_idx, row) in combined_rows.iter().enumerate() {
            if active_screen_start_log_idx.is_none() && row_idx == scrollback_count {
                active_screen_start_log_idx = Some(logical_lines.len());
            }

            let start_len = current_cells.len();

            if row.wrapped {
                current_cells.extend_from_slice(&row.cells);
            } else {
                let mut last_non_default = 0;
                for (i, cell) in row.cells.iter().enumerate() {
                    if cell.character != ' ' || !cell.flags.is_empty() || cell.background != self.default_bg {
                        last_non_default = i + 1;
                    }
                }
                let keep_len = if row_idx == old_cursor_row_idx {
                    last_non_default.max(old_cursor_col + 1)
                } else {
                    last_non_default
                };
                current_cells.extend_from_slice(&row.cells[..keep_len.min(row.cells.len())]);
            }

            if !cursor_found && row_idx == old_cursor_row_idx {
                cursor_logical_line_idx = logical_lines.len();
                cursor_logical_cell_idx = start_len + old_cursor_col.min(row.cells.len());
                cursor_found = true;
            }

            if !row.wrapped {
                logical_lines.push(LogicalLine {
                    cells: std::mem::take(&mut current_cells),
                    hard_break: true,
                });
            }
        }

        if !current_cells.is_empty() || logical_lines.is_empty() {
            if active_screen_start_log_idx.is_none() && combined_rows.len() == scrollback_count {
                active_screen_start_log_idx = Some(logical_lines.len());
            }
            if !cursor_found {
                cursor_logical_line_idx = logical_lines.len();
                cursor_logical_cell_idx = current_cells.len() + old_cursor_col;
            }
            logical_lines.push(LogicalLine {
                cells: current_cells,
                hard_break: false,
            });
        }

        struct ReflowedRow {
            cells: Vec<Cell>,
            wrapped: bool,
        }

        fn pad_row(mut chunk: Vec<Cell>, new_w: usize, default_cell: Cell) -> Vec<Cell> {
            while chunk.len() < new_w {
                chunk.push(default_cell);
            }
            chunk
        }

        let mut reflowed_rows: Vec<ReflowedRow> = Vec::new();
        let mut new_cursor_row_idx: usize = 0;
        let mut new_cursor_col: usize = 0;
        let mut new_cursor_found = false;
        let mut active_screen_start_reflowed_row_idx: usize = 0;
        let mut active_reflowed_found = false;

        let active_log_target = active_screen_start_log_idx.unwrap_or(0);

        for (log_idx, log_line) in logical_lines.iter().enumerate() {
            if !active_reflowed_found && log_idx == active_log_target {
                active_screen_start_reflowed_row_idx = reflowed_rows.len();
                active_reflowed_found = true;
            }

            let cells = &log_line.cells;

            if cells.is_empty() {
                if !new_cursor_found && log_idx == cursor_logical_line_idx {
                    new_cursor_row_idx = reflowed_rows.len();
                    new_cursor_col = 0;
                    new_cursor_found = true;
                }
                reflowed_rows.push(ReflowedRow {
                    cells: vec![default_cell; new_w],
                    wrapped: !log_line.hard_break,
                });
                continue;
            }

            let mut chunk: Vec<Cell> = Vec::with_capacity(new_w);
            let mut i = 0;

            while i < cells.len() {
                let cell = cells[i];

                if !new_cursor_found && log_idx == cursor_logical_line_idx && i == cursor_logical_cell_idx {
                    new_cursor_row_idx = reflowed_rows.len();
                    new_cursor_col = chunk.len();
                    new_cursor_found = true;
                }

                let is_wide = cell.flags.contains(CellFlags::WIDE);

                if is_wide && chunk.len() + 2 > new_w {
                    if chunk.len() < new_w {
                        chunk.push(default_cell);
                    }
                    reflowed_rows.push(ReflowedRow {
                        cells: pad_row(chunk, new_w, default_cell),
                        wrapped: true,
                    });
                    chunk = Vec::with_capacity(new_w);
                    continue;
                }

                chunk.push(cell);
                if is_wide && i + 1 < cells.len() && cells[i + 1].flags.contains(CellFlags::WIDE_CONTINUATION) {
                    i += 1;
                    chunk.push(cells[i]);
                }

                i += 1;

                if chunk.len() >= new_w {
                    let is_last = i >= cells.len();
                    let wrapped = if is_last { !log_line.hard_break } else { true };
                    reflowed_rows.push(ReflowedRow {
                        cells: chunk,
                        wrapped,
                    });
                    chunk = Vec::with_capacity(new_w);
                }
            }

            if !new_cursor_found && log_idx == cursor_logical_line_idx {
                new_cursor_row_idx = if chunk.is_empty() && !reflowed_rows.is_empty() {
                    reflowed_rows.len() - 1
                } else {
                    reflowed_rows.len()
                };
                new_cursor_col = chunk.len().min(new_w.saturating_sub(1));
                new_cursor_found = true;
            }

            if !chunk.is_empty() {
                reflowed_rows.push(ReflowedRow {
                    cells: pad_row(chunk, new_w, default_cell),
                    wrapped: !log_line.hard_break,
                });
            }
        }

        if !active_reflowed_found {
            active_screen_start_reflowed_row_idx = reflowed_rows.len();
        }

        if !new_cursor_found {
            new_cursor_row_idx = reflowed_rows.len().saturating_sub(1);
            new_cursor_col = 0;
        }

        let total_rows = reflowed_rows.len();

        let grid_start = if total_rows == 0 {
            0
        } else {
            active_screen_start_reflowed_row_idx
                .min(total_rows - 1)
                .max(new_cursor_row_idx.saturating_sub(new_h - 1))
        };


        self.scrollback.clear();
        for row in &reflowed_rows[..grid_start] {
            self.scrollback.push_line(&row.cells, row.wrapped);
        }

        let mut new_grid_cells = Vec::with_capacity(new_w * new_h);
        let mut new_row_wrapped = Vec::with_capacity(new_h);

        for y in 0..new_h {
            let row_idx = grid_start + y;
            if row_idx < total_rows {
                new_grid_cells.extend_from_slice(&reflowed_rows[row_idx].cells);
                new_row_wrapped.push(reflowed_rows[row_idx].wrapped);
            } else {
                new_grid_cells.extend(std::iter::repeat_n(default_cell, new_w));
                new_row_wrapped.push(false);
            }
        }

        self.cells = new_grid_cells;
        self.row_wrapped = new_row_wrapped;
        self.width = new_w;
        self.height = new_h;
        self.scroll_region_top = 0;
        self.scroll_region_bottom = new_h.saturating_sub(1);

        self.cursor.x = new_cursor_col.min(new_w.saturating_sub(1));
        self.cursor.y = if new_cursor_row_idx >= grid_start {
            (new_cursor_row_idx - grid_start).min(new_h.saturating_sub(1))
        } else {
            0
        };

        self.damage.resize(new_h);
        for y in 0..new_h {
            self.damage.mark_dirty(y);
        }
    }
}
