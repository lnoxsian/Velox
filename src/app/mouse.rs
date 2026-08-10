use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::dpi::PhysicalPosition;
use crate::app::app::App;

impl App {
    pub fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        self.mouse_x = position.x;
        self.mouse_y = position.y;

        if let (Some(window), Some(renderer), Some(terminal)) = (&self.window, &self.renderer, &self.terminal) {
            let px = self.padding_x as f64;
            let py = self.padding_y as f64;
            let cw = renderer.font_loader.cell_width as f64;
            let ch = renderer.font_loader.cell_height as f64;
            let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize).min(terminal.grid.width.saturating_sub(1));
            let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize).min(terminal.grid.height.saturating_sub(1));

            let active_grid = if terminal.is_alt_screen { &terminal.alt_grid } else { &terminal.grid };
            let offset = active_grid.scroll_offset;
            let history_len = active_grid.scrollback.len();

            let mut is_link = false;
            let y_offset = (row_idx + history_len).saturating_sub(offset);
            let line_text: String = if y_offset < history_len {
                active_grid.scrollback.get_row(y_offset).unwrap_or_else(|| crate::screen::scrollback::Row { cells: vec![], wrapped: false })
                    .iter().map(|c| c.character).collect()
            } else {
                let y = y_offset - history_len;
                if y < active_grid.height {
                    let src_start = y * active_grid.width;
                    let src_end = src_start + active_grid.width;
                    active_grid.cells[src_start..src_end.min(active_grid.cells.len())]
                        .iter().map(|c| c.character).collect()
                } else {
                    String::new()
                }
            };
            let urls = crate::hyperlink::detector::detect(&line_text);
            for (start_col, end_col, _) in urls {
                if col_idx >= start_col && col_idx < end_col {
                    is_link = true;
                    break;
                }
            }

            if is_link {
                window.set_cursor(winit::window::CursorIcon::Pointer);
            } else if terminal.mouse_mode > 0 {
                window.set_cursor(winit::window::CursorIcon::Default);
            } else {
                window.set_cursor(winit::window::CursorIcon::Text);
            }
        }

        if self.is_mouse_down
            && let (Some(renderer), Some(terminal)) = (&self.renderer, &mut self.terminal) {
                let px = self.padding_x as f64;
                let py = self.padding_y as f64;
                let cw = renderer.font_loader.cell_width as f64;
                let ch = renderer.font_loader.cell_height as f64;
                let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize).min(terminal.grid.width.saturating_sub(1));
                let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize).min(terminal.grid.height.saturating_sub(1));

                if (col_idx, row_idx) != self.last_mouse_cell {
                    self.last_mouse_cell = (col_idx, row_idx);

                    let should_report_motion = terminal.mouse_mode == 1003 || (terminal.mouse_mode == 1002 && self.is_mouse_down);
                    if should_report_motion && !self.modifiers.shift_key() {
                        if let Some(pty_master) = &self.pty_master {
                            let btn_code = if self.is_mouse_down { 32 } else { 35 };
                            let seq = if terminal.mouse_sgr {
                                format!("\x1b[<{};{};{}M", btn_code, col_idx + 1, row_idx + 1)
                            } else {
                                let cb = 32 + btn_code;
                                let cx = 32 + col_idx + 1;
                                let cy = 32 + row_idx + 1;
                                if cx <= 255 && cy <= 255 {
                                    format!("\x1b[M{}{}{}", cb as u8 as char, cx as u8 as char, cy as u8 as char)
                                } else {
                                    String::new()
                                }
                            };
                            if !seq.is_empty() {
                                let _ = pty_master.write(seq.as_bytes());
                            }
                        }
                    } else {
                        let active_grid = terminal.active_grid_mut();
                        if active_grid.selection.active {
                            active_grid.selection.update_selection(col_idx, row_idx);
                            self.needs_redraw = true;
                        }
                    }
                }
            }
    }

    pub fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let lines_f = match delta {
            MouseScrollDelta::LineDelta(_, y) => y as f64,
            MouseScrollDelta::PixelDelta(pos) => pos.y / 15.0,
        };
        let lines = (lines_f * self.scroll_multiplier).round() as i32;
        if lines != 0
            && let Some(pty_master) = &self.pty_master
                && let (Some(terminal), Some(renderer)) = (&mut self.terminal, &self.renderer) {
                    let px = self.padding_x as f64;
                    let py = self.padding_y as f64;
                    let cw = renderer.font_loader.cell_width as f64;
                    let ch = renderer.font_loader.cell_height as f64;
                    let col = (((self.mouse_x - px).max(0.0) / cw).floor() as i32 + 1).max(1);
                    let row = (((self.mouse_y - py).max(0.0) / ch).floor() as i32 + 1).max(1);

                    if terminal.mouse_mode > 0 {
                        let btn = if lines > 0 { 64 } else { 65 };
                        for _ in 0..lines.abs() {
                            let seq = if terminal.mouse_sgr {
                                format!("\x1b[<{};{};{}M", btn, col, row)
                            } else {
                                let cb = 32 + btn;
                                let cx = 32 + col;
                                let cy = 32 + row;
                                if cx <= 255 && cy <= 255 {
                                    format!("\x1b[M{}{}{}", cb as u8 as char, cx as u8 as char, cy as u8 as char)
                                } else {
                                    String::new()
                                }
                            };
                            if !seq.is_empty() {
                                let _ = pty_master.write(seq.as_bytes());
                            }
                        }
                    } else if terminal.is_alt_screen {
                        let key_seq = if lines > 0 {
                            if terminal.cursor_keys_mode { b"\x1bOA" } else { b"\x1b[A" }
                        } else {
                            if terminal.cursor_keys_mode { b"\x1bOB" } else { b"\x1b[B" }
                        };
                        for _ in 0..lines.abs() {
                            let _ = pty_master.write(key_seq);
                        }
                    } else {
                        let active_grid = if terminal.is_alt_screen { &mut terminal.alt_grid } else { &mut terminal.grid };
                        let history_len = active_grid.scrollback.len();
                        if lines > 0 {
                            active_grid.scroll_offset = (active_grid.scroll_offset + lines as usize).min(history_len);
                        } else if lines < 0 {
                            active_grid.scroll_offset = active_grid.scroll_offset.saturating_sub(lines.unsigned_abs() as usize);
                        }
                        self.needs_redraw = true;
                    }
                }
    }

    pub fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if button == MouseButton::Left {
            if state.is_pressed() {
                self.is_mouse_down = true;
                if let (Some(renderer), Some(terminal)) = (&self.renderer, &mut self.terminal) {
                    let px = self.padding_x as f64;
                    let py = self.padding_y as f64;
                    let cw = renderer.font_loader.cell_width as f64;
                    let ch = renderer.font_loader.cell_height as f64;
                    let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize).min(terminal.grid.width.saturating_sub(1));
                    let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize).min(terminal.grid.height.saturating_sub(1));

                    let mouse_mode = terminal.mouse_mode;
                    let mouse_sgr = terminal.mouse_sgr;
                    let active_grid = terminal.active_grid_mut();
                    let grid_width = active_grid.width;
                    let grid_height = active_grid.height;

                    if col_idx < grid_width && row_idx < grid_height {
                        let now = std::time::Instant::now();
                        let is_double_click = if let Some(last_time) = self.last_click_instant {
                            self.last_click_pos == (col_idx, row_idx) && last_time.elapsed().as_millis() < 400
                        } else {
                            false
                        };

                        if is_double_click {
                            self.click_count = (self.click_count % 3) + 1;
                        } else {
                            self.click_count = 1;
                        }
                        self.last_click_instant = Some(now);
                        self.last_click_pos = (col_idx, row_idx);

                        if mouse_mode > 0 && !self.modifiers.shift_key() {
                            if let Some(pty_master) = &self.pty_master {
                                let seq = if mouse_sgr {
                                    format!("\x1b[<0;{};{}M", col_idx + 1, row_idx + 1)
                                } else {
                                    let cb = 32;
                                    let cx = 32 + col_idx + 1;
                                    let cy = 32 + row_idx + 1;
                                    if cx <= 255 && cy <= 255 {
                                        format!("\x1b[M{}{}{}", cb as u8 as char, cx as u8 as char, cy as u8 as char)
                                    } else {
                                        String::new()
                                    }
                                };
                                if !seq.is_empty() {
                                    let _ = pty_master.write(seq.as_bytes());
                                }
                            }
                        } else {
                            let mut url_opened = false;
                            let offset = active_grid.scroll_offset;
                            let history_len = active_grid.scrollback.len();
                            let y_offset = (row_idx + history_len).saturating_sub(offset);
                            
                            let line_text: String = if y_offset < history_len {
                                active_grid.scrollback.get_row(y_offset).unwrap_or_else(|| crate::screen::scrollback::Row { cells: vec![], wrapped: false })
                                    .iter().map(|c| c.character).collect()
                            } else {
                                let y = y_offset - history_len;
                                if y < active_grid.height {
                                    let src_start = y * grid_width;
                                    let src_end = src_start + grid_width;
                                    active_grid.cells[src_start..src_end.min(active_grid.cells.len())]
                                        .iter().map(|c| c.character).collect()
                                } else {
                                    String::new()
                                }
                            };
                            let urls = crate::hyperlink::detector::detect(&line_text);
                            for (start, end, url) in urls {
                                if col_idx >= start && col_idx < end {
                                    let _ = crate::hyperlink::detector::open(&url);
                                    url_opened = true;
                                    break;
                                }
                            }

                            if !url_opened {
                                match self.click_count {
                                    1 => {
                                        if self.modifiers.shift_key() && active_grid.selection.active {
                                            active_grid.selection.update_selection(col_idx, row_idx);
                                        } else {
                                            active_grid.selection.start_selection(col_idx, row_idx);
                                        }
                                    }
                                    2 => {
                                        active_grid.selection.select_word(grid_width, grid_height, &active_grid.cells, col_idx, row_idx);
                                        let text = active_grid.selection.extract_text(grid_width, grid_height, &active_grid.cells);
                                        if !text.is_empty() {
                                            crate::clipboard::clipboard::copy(&text);
                                        }
                                    }
                                    3 => {
                                        active_grid.selection.select_line(grid_width, grid_height, row_idx);
                                        let text = active_grid.selection.extract_text(grid_width, grid_height, &active_grid.cells);
                                        if !text.is_empty() {
                                            crate::clipboard::clipboard::copy(&text);
                                        }
                                    }
                                    _ => {}
                                }

                                if self.click_count == 1 && row_idx == active_grid.cursor.y {
                                    let cursor_x = active_grid.cursor.x;
                                    if let Some(pty_master) = &self.pty_master {
                                        if col_idx < cursor_x {
                                            let diff = cursor_x - col_idx;
                                            let seq = b"\x1b[D".repeat(diff);
                                            let _ = pty_master.write(&seq);
                                        } else if col_idx > cursor_x {
                                            let diff = col_idx - cursor_x;
                                            let seq = b"\x1b[C".repeat(diff);
                                            let _ = pty_master.write(&seq);
                                        }
                                    }
                                }
                            }

                            self.needs_redraw = true;
                        }
                    }
                }
            } else {
                self.is_mouse_down = false;
                if let (Some(renderer), Some(terminal)) = (&self.renderer, &mut self.terminal) {
                    let px = self.padding_x as f64;
                    let py = self.padding_y as f64;
                    let cw = renderer.font_loader.cell_width as f64;
                    let ch = renderer.font_loader.cell_height as f64;
                    let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize).min(terminal.grid.width.saturating_sub(1));
                    let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize).min(terminal.grid.height.saturating_sub(1));

                    let mouse_mode = terminal.mouse_mode;
                    let mouse_sgr = terminal.mouse_sgr;
                    if mouse_mode > 0 && !self.modifiers.shift_key() {
                        if let Some(pty_master) = &self.pty_master {
                            let seq = if mouse_sgr {
                                format!("\x1b[<0;{};{}m", col_idx + 1, row_idx + 1)
                            } else {
                                let cb = 32 + 3;
                                let cx = 32 + col_idx + 1;
                                let cy = 32 + row_idx + 1;
                                if cx <= 255 && cy <= 255 {
                                    format!("\x1b[M{}{}{}", cb as u8 as char, cx as u8 as char, cy as u8 as char)
                                } else {
                                    String::new()
                                }
                            };
                            if !seq.is_empty() {
                                let _ = pty_master.write(seq.as_bytes());
                            }
                        }
                    } else {
                        let active_grid = terminal.active_grid_mut();
                        if active_grid.selection.active {
                            let text = active_grid.selection.extract_text(active_grid.width, active_grid.height, &active_grid.cells);
                            if !text.is_empty() && (active_grid.selection.start_x != active_grid.selection.end_x || active_grid.selection.start_y != active_grid.selection.end_y) {
                                crate::clipboard::clipboard::copy(&text);
                            }
                        }
                    }
                }
            }
        }
    }
}
