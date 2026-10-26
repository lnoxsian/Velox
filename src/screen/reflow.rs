use crate::screen::cell::{Cell, CellFlags};
use crate::screen::grid::Grid;

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

        self.normalize_row_offset();
        self.selection.clear();

        let default_cell = Cell::new(' ', self.default_fg, self.default_bg, CellFlags::empty());

        let len = self.scrollback.len();
        let reflow_window = self.scrollback.max_lines.max(1000);
        let start_idx = len.saturating_sub(reflow_window);
        let scrollback_count = len - start_idx;
        let old_w = self.width;
        let old_h = self.height;

        let mut last_used_y = self.cursor.y;
        for y in (0..old_h).rev() {
            let start = y * old_w;
            let end = start + old_w;
            if end <= self.cells.len() {
                let row_cells = &self.cells[start..end];
                if row_cells.iter().any(|c| {
                    c.character != ' ' || !c.flags.is_empty() || c.background != self.default_bg
                }) {
                    last_used_y = last_used_y.max(y);
                    break;
                }
            }
        }
        let active_grid_rows = last_used_y.min(old_h.saturating_sub(1)) + 1;
        let total_input_rows = scrollback_count + active_grid_rows;

        let old_cursor_row_idx = scrollback_count + self.cursor.y;
        let old_cursor_col = self.cursor.x;

        struct LogicalLineSpan {
            start: usize,
            len: usize,
            hard_break: bool,
        }

        let mut all_logical_cells: Vec<Cell> = Vec::with_capacity(total_input_rows * old_w);
        let mut logical_spans: Vec<LogicalLineSpan> = Vec::with_capacity(total_input_rows);

        let mut cursor_logical_line_idx: usize = 0;
        let mut cursor_logical_cell_idx: usize = 0;
        let mut cursor_found = false;
        let mut active_screen_start_log_idx: Option<usize> = None;

        let mut current_line_start = 0;
        let mut consecutive_empty_count = 0;

        let mut process_row = |row_idx: usize, cells: &[Cell], wrapped: bool| {
            if active_screen_start_log_idx.is_none() && row_idx == scrollback_count {
                active_screen_start_log_idx = Some(logical_spans.len());
            }

            let start_len = all_logical_cells.len() - current_line_start;

            let mut last_non_default = 0;
            for (i, cell) in cells.iter().enumerate() {
                if cell.character != ' '
                    || !cell.flags.is_empty()
                    || cell.background != self.default_bg
                {
                    last_non_default = i + 1;
                }
            }
            let keep_len = if row_idx == old_cursor_row_idx {
                last_non_default.max(old_cursor_col + 1)
            } else if wrapped {
                cells.len()
            } else {
                last_non_default
            };
            all_logical_cells.extend_from_slice(&cells[..keep_len.min(cells.len())]);

            if !cursor_found && row_idx == old_cursor_row_idx {
                cursor_logical_line_idx = logical_spans.len();
                cursor_logical_cell_idx = start_len + old_cursor_col.min(cells.len());
                cursor_found = true;
            }

            if !wrapped {
                let current_len = all_logical_cells.len() - current_line_start;
                let is_empty = current_len == 0;
                let is_cursor_row = row_idx == old_cursor_row_idx;

                if is_empty && !is_cursor_row && row_idx >= scrollback_count {
                    consecutive_empty_count += 1;
                } else {
                    consecutive_empty_count = 0;
                }

                if consecutive_empty_count <= 1 {
                    logical_spans.push(LogicalLineSpan {
                        start: current_line_start,
                        len: current_len,
                        hard_break: true,
                    });
                    current_line_start = all_logical_cells.len();
                } else {
                    all_logical_cells.truncate(current_line_start);
                }
            }
        };

        for i in start_idx..len {
            self.scrollback.with_row_slice(i, |cells, wrapped| {
                process_row(i - start_idx, cells, wrapped);
            });
        }

        for y in 0..active_grid_rows {
            let start = y * old_w;
            let end = start + old_w;
            if end <= self.cells.len() {
                let wrapped = self.row_wrapped.get(y).copied().unwrap_or(false);
                process_row(scrollback_count + y, &self.cells[start..end], wrapped);
            }
        }

        let trailing_len = all_logical_cells.len() - current_line_start;
        if trailing_len > 0 || logical_spans.is_empty() {
            if active_screen_start_log_idx.is_none() && total_input_rows == scrollback_count {
                active_screen_start_log_idx = Some(logical_spans.len());
            }
            if !cursor_found {
                cursor_logical_line_idx = logical_spans.len();
                cursor_logical_cell_idx = trailing_len + old_cursor_col;
            }
            logical_spans.push(LogicalLineSpan {
                start: current_line_start,
                len: trailing_len,
                hard_break: false,
            });
        }

        let mut reflowed_cells: Vec<Cell> = Vec::with_capacity(logical_spans.len() * new_w * 2);
        let mut reflowed_wrapped: Vec<bool> = Vec::with_capacity(logical_spans.len() * 2);
        let mut new_cursor_row_idx: usize = 0;
        let mut new_cursor_col: usize = 0;
        let mut new_cursor_found = false;
        let mut active_screen_start_reflowed_row_idx: usize = 0;
        let mut active_reflowed_found = false;

        let active_log_target = active_screen_start_log_idx.unwrap_or(0);
        let mut current_chunk_start = 0;

        for (log_idx, log_span) in logical_spans.iter().enumerate() {
            if !active_reflowed_found && log_idx == active_log_target {
                active_screen_start_reflowed_row_idx = reflowed_wrapped.len();
                active_reflowed_found = true;
            }

            let cells = &all_logical_cells[log_span.start..log_span.start + log_span.len];

            if cells.is_empty() {
                if !new_cursor_found && log_idx == cursor_logical_line_idx {
                    new_cursor_row_idx = reflowed_wrapped.len();
                    new_cursor_col = 0;
                    new_cursor_found = true;
                }
                reflowed_cells.extend(std::iter::repeat_n(default_cell, new_w));
                reflowed_wrapped.push(!log_span.hard_break);
                current_chunk_start = reflowed_cells.len();
                continue;
            }

            let start_reflowed_count = reflowed_wrapped.len();
            let mut i = 0;

            while i < cells.len() {
                let cell = cells[i];

                if !new_cursor_found
                    && log_idx == cursor_logical_line_idx
                    && i == cursor_logical_cell_idx
                {
                    new_cursor_row_idx = reflowed_wrapped.len();
                    new_cursor_col = reflowed_cells.len() - current_chunk_start;
                    new_cursor_found = true;
                }

                let is_wide = cell.flags.contains(CellFlags::WIDE);

                if is_wide && (reflowed_cells.len() - current_chunk_start) + 2 > new_w {
                    if new_w < 2 {
                        if !new_cursor_found
                            && log_idx == cursor_logical_line_idx
                            && i == cursor_logical_cell_idx
                        {
                            new_cursor_row_idx = reflowed_wrapped.len();
                            new_cursor_col = 0;
                            new_cursor_found = true;
                        }
                        reflowed_cells.extend(std::iter::repeat_n(cell, new_w));
                        reflowed_wrapped.push(true);
                        current_chunk_start = reflowed_cells.len();
                        if i + 1 < cells.len()
                            && cells[i + 1].flags.contains(CellFlags::WIDE_CONTINUATION)
                        {
                            i += 1;
                        }
                        i += 1;
                        continue;
                    }
                    if reflowed_cells.len() - current_chunk_start < new_w {
                        reflowed_cells.push(default_cell);
                    }
                    let chunk = &reflowed_cells[current_chunk_start..];
                    let is_all_spaces = chunk.iter().all(|c| {
                        c.character == ' ' && c.flags.is_empty() && c.background == self.default_bg
                    });
                    if !is_all_spaces {
                        let pad = new_w.saturating_sub(chunk.len());
                        reflowed_cells.extend(std::iter::repeat_n(default_cell, pad));
                        reflowed_wrapped.push(true);
                        current_chunk_start = reflowed_cells.len();
                    } else {
                        reflowed_cells.truncate(current_chunk_start);
                    }
                    continue;
                }

                reflowed_cells.push(cell);
                if is_wide
                    && i + 1 < cells.len()
                    && cells[i + 1].flags.contains(CellFlags::WIDE_CONTINUATION)
                {
                    i += 1;
                    reflowed_cells.push(cells[i]);
                }

                i += 1;

                if reflowed_cells.len() - current_chunk_start >= new_w {
                    let is_last = i >= cells.len();
                    let wrapped = if is_last { !log_span.hard_break } else { true };
                    let chunk = &reflowed_cells[current_chunk_start..];
                    let is_all_spaces = chunk.iter().all(|c| {
                        c.character == ' ' && c.flags.is_empty() && c.background == self.default_bg
                    });
                    if !is_all_spaces {
                        let pad = new_w.saturating_sub(chunk.len());
                        reflowed_cells.extend(std::iter::repeat_n(default_cell, pad));
                        reflowed_wrapped.push(wrapped);
                        current_chunk_start = reflowed_cells.len();
                    } else {
                        reflowed_cells.truncate(current_chunk_start);
                    }
                }
            }

            if !new_cursor_found && log_idx == cursor_logical_line_idx {
                let chunk_len = reflowed_cells.len() - current_chunk_start;
                new_cursor_row_idx = if chunk_len == 0 && !reflowed_wrapped.is_empty() {
                    reflowed_wrapped.len() - 1
                } else {
                    reflowed_wrapped.len()
                };
                new_cursor_col = chunk_len.min(new_w.saturating_sub(1));
                new_cursor_found = true;
            }

            let chunk_len = reflowed_cells.len() - current_chunk_start;
            if chunk_len > 0 {
                let chunk = &reflowed_cells[current_chunk_start..];
                let is_all_spaces = chunk.iter().all(|c| {
                    c.character == ' ' && c.flags.is_empty() && c.background == self.default_bg
                });
                if !is_all_spaces || reflowed_wrapped.len() == start_reflowed_count {
                    let pad = new_w.saturating_sub(chunk_len);
                    reflowed_cells.extend(std::iter::repeat_n(default_cell, pad));
                    reflowed_wrapped.push(!log_span.hard_break);
                    current_chunk_start = reflowed_cells.len();
                } else {
                    reflowed_cells.truncate(current_chunk_start);
                }
            } else if reflowed_wrapped.len() == start_reflowed_count {
                reflowed_cells.extend(std::iter::repeat_n(default_cell, new_w));
                reflowed_wrapped.push(!log_span.hard_break);
                current_chunk_start = reflowed_cells.len();
            }
        }

        if !active_reflowed_found {
            active_screen_start_reflowed_row_idx = reflowed_wrapped.len();
        }

        if !new_cursor_found {
            new_cursor_row_idx = reflowed_wrapped.len().saturating_sub(1);
            new_cursor_col = 0;
        }

        let total_rows = reflowed_wrapped.len();

        let grid_start = if total_rows == 0 {
            0
        } else {
            active_screen_start_reflowed_row_idx
                .min(total_rows - 1)
                .max(new_cursor_row_idx.saturating_sub(new_h - 1))
        };

        if start_idx == 0 {
            self.scrollback.clear();
        } else {
            self.scrollback.clear_hot();
        }
        for r in 0..grid_start {
            let row_slice = &reflowed_cells[r * new_w..(r + 1) * new_w];
            self.scrollback.push_line(row_slice, reflowed_wrapped[r]);
        }

        let mut new_grid_cells = Vec::with_capacity(new_w * new_h);
        let mut new_row_wrapped = Vec::with_capacity(new_h);

        for y in 0..new_h {
            let row_idx = grid_start + y;
            if row_idx < total_rows {
                new_grid_cells.extend_from_slice(&reflowed_cells[row_idx * new_w..(row_idx + 1) * new_w]);
                new_row_wrapped.push(reflowed_wrapped[row_idx]);
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
        self.row_offset = 0;

        self.tab_stops.resize(new_w);

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

#[cfg(test)]
mod tests {
    use crate::terminal::terminal::Terminal;

    #[test]
    fn test_reflow_narrow_inspection() {
        let mut term = Terminal::new(80, 24);
        for i in 0..30 {
            term.feed(format!("line {:02} some text here that is quite long and definitely exceeds 40 characters\r\n", i).as_bytes());
        }
        term.feed(b"user@host:~$ ");

        // Step by step narrowing from 80 down to 40, then back up to 80, then down to 40
        for _ in 0..3 {
            for w in (40..80).rev() {
                term.resize(w, 24);
            }
            for w in 41..=80 {
                term.resize(w, 24);
            }
        }
        for w in (40..80).rev() {
            term.resize(w, 24);
        }

        assert!(!term.grid.scrollback.is_empty());

        // Verify that in the visible grid, every line is distinct and non-duplicated
        let mut non_empty_rows: Vec<String> = Vec::new();
        for y in 0..24 {
            let row: String = (0..40)
                .map(|x| term.grid.cells[y * 40 + x].character)
                .collect();
            let trimmed = row.trim_end().to_string();
            if !trimmed.is_empty() {
                non_empty_rows.push(trimmed);
            }
        }
        for i in 1..non_empty_rows.len() {
            assert_ne!(
                non_empty_rows[i - 1],
                non_empty_rows[i],
                "Consecutive duplicated row detected: {:?}",
                non_empty_rows[i]
            );
        }
    }

    #[test]
    fn test_reflow_width_one_with_wide_characters() {
        let mut term = Terminal::new(80, 24);
        term.feed("🌟 Hello 世界 🚀\r\n".as_bytes());
        // Resize down to width 1: must complete immediately without infinite hang
        term.resize(1, 10);
        assert_eq!(term.grid.width, 1);
        assert_eq!(term.grid.height, 10);
    }
}
