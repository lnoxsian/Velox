use crate::app::app::WindowState;
use crate::app::tab::TabBarHitResult;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::ModifiersState;

impl WindowState {
    pub fn handle_cursor_moved(
        &mut self,
        position: PhysicalPosition<f64>,
        modifiers: ModifiersState,
    ) {
        self.mouse_x = position.x;
        self.mouse_y = position.y;

        let tab_bar_h = self.tab_bar_height() as f64;

        // ── Tab Bar Hover & Hit-Testing ──────────────────────────────────────
        if tab_bar_h > 0.0 && self.mouse_y < tab_bar_h {
            let win_w = self.window.inner_size().width as f32;
            let hit = self.tab_bar.hit_test(
                self.mouse_x as f32,
                self.mouse_y as f32,
                win_w,
                self.cell_height(),
                self.tabs.len(),
            );

            let old_hovered_tab = self.tab_bar.hovered_tab;
            let old_hovered_close = self.tab_bar.hovered_close;
            let old_hovered_new = self.tab_bar.hovered_new_tab;

            match hit {
                TabBarHitResult::Tab(idx) => {
                    self.tab_bar.hovered_tab = Some(idx);
                    self.tab_bar.hovered_close = None;
                    self.tab_bar.hovered_new_tab = false;
                    self.window.set_cursor(winit::window::CursorIcon::Pointer);
                }
                TabBarHitResult::CloseTab(idx) => {
                    self.tab_bar.hovered_tab = Some(idx);
                    self.tab_bar.hovered_close = Some(idx);
                    self.tab_bar.hovered_new_tab = false;
                    self.window.set_cursor(winit::window::CursorIcon::Pointer);
                }
                TabBarHitResult::NewTab => {
                    self.tab_bar.hovered_tab = None;
                    self.tab_bar.hovered_close = None;
                    self.tab_bar.hovered_new_tab = true;
                    self.window.set_cursor(winit::window::CursorIcon::Pointer);
                }
                _ => {
                    self.tab_bar.hovered_tab = None;
                    self.tab_bar.hovered_close = None;
                    self.tab_bar.hovered_new_tab = false;
                    self.window.set_cursor(winit::window::CursorIcon::Default);
                }
            }

            if old_hovered_tab != self.tab_bar.hovered_tab
                || old_hovered_close != self.tab_bar.hovered_close
                || old_hovered_new != self.tab_bar.hovered_new_tab
            {
                self.tab_bar_dirty = true;
                self.needs_redraw = true;
            }
            return;
        } else if self.tab_bar.hovered_tab.is_some()
            || self.tab_bar.hovered_close.is_some()
            || self.tab_bar.hovered_new_tab
        {
            self.tab_bar.hovered_tab = None;
            self.tab_bar.hovered_close = None;
            self.tab_bar.hovered_new_tab = false;
            self.tab_bar_dirty = true;
            self.needs_redraw = true;
        }

        // ── Active Terminal Grid Hover & Selection ───────────────────────────
        let px = self.padding_x as f64;
        let py = (self.padding_y + self.tab_bar_height()) as f64;
        let cw = self.cell_width() as f64;
        let ch = self.cell_height() as f64;

        let grid_width = self.active_tab().terminal.grid.width;
        let grid_height = self.active_tab().terminal.grid.height;

        let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize)
            .min(grid_width.saturating_sub(1));
        let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize)
            .min(grid_height.saturating_sub(1));

        if (col_idx, row_idx) != self.last_mouse_cell {
            self.last_mouse_cell = (col_idx, row_idx);

            let active_tab = self.active_tab();
            let active_grid = if active_tab.terminal.is_alt_screen {
                &active_tab.terminal.alt_grid
            } else {
                &active_tab.terminal.grid
            };
            let offset = active_grid.scroll_offset;
            let history_len = active_grid.scrollback.len();

            let mut is_link = false;
            let y_offset = (row_idx + history_len).saturating_sub(offset);
            let line_text: String = if y_offset < history_len {
                active_grid
                    .scrollback
                    .with_row_slice(y_offset, |cells, _| {
                        cells.iter().map(|c| c.character).collect()
                    })
                    .unwrap_or_default()
            } else {
                let y = y_offset - history_len;
                if y < active_grid.height {
                    let src_start = y * active_grid.width;
                    let src_end = src_start + active_grid.width;
                    active_grid.cells[src_start..src_end.min(active_grid.cells.len())]
                        .iter()
                        .map(|c| c.character)
                        .collect()
                } else {
                    String::new()
                }
            };
            crate::hyperlink::detector::for_each_url(&line_text, |start_col, end_col| {
                if col_idx >= start_col && col_idx < end_col {
                    is_link = true;
                }
            });

            let mouse_mode = self.active_tab().terminal.mouse_mode;
            if is_link {
                self.window.set_cursor(winit::window::CursorIcon::Pointer);
            } else if mouse_mode > 0 {
                self.window.set_cursor(winit::window::CursorIcon::Default);
            } else {
                self.window.set_cursor(winit::window::CursorIcon::Text);
            }

            if self.is_mouse_down {
                let active_tab = self.active_tab();
                let should_report_motion = active_tab.terminal.mouse_mode == 1003
                    || (active_tab.terminal.mouse_mode == 1002 && self.is_mouse_down);
                if should_report_motion && !modifiers.shift_key() {
                    let btn_code = if self.is_mouse_down { 32 } else { 35 };
                    let seq = if active_tab.terminal.mouse_sgr {
                        format!("\x1b[<{};{};{}M", btn_code, col_idx + 1, row_idx + 1)
                    } else {
                        let cb = 32 + btn_code;
                        let cx = 32 + col_idx + 1;
                        let cy = 32 + row_idx + 1;
                        if cx <= 255 && cy <= 255 {
                            format!(
                                "\x1b[M{}{}{}",
                                cb as u8 as char, cx as u8 as char, cy as u8 as char
                            )
                        } else {
                            String::new()
                        }
                    };
                    if !seq.is_empty() {
                        let _ = active_tab.pty_master.write(seq.as_bytes());
                    }
                } else {
                    let active_tab = self.active_tab_mut();
                    let active_grid = active_tab.terminal.active_grid_mut();
                    if active_grid.selection.active {
                        let offset = active_grid.scroll_offset;
                        let history_len = active_grid.scrollback.len();
                        let abs_y = (history_len + row_idx).saturating_sub(offset);
                        active_grid.selection.update_selection(col_idx, abs_y);
                        self.needs_redraw = true;
                    }
                }
            }
        }
    }

    pub fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta, _modifiers: ModifiersState) {
        let lines_f = match delta {
            MouseScrollDelta::LineDelta(_, y) => y as f64,
            MouseScrollDelta::PixelDelta(pos) => pos.y / 15.0,
        };
        let lines = (lines_f * self.scroll_multiplier).round() as i32;
        if lines != 0 {
            let tab_bar_h = self.tab_bar_height() as f64;

            // Scroll over tab bar switches tabs
            if tab_bar_h > 0.0 && self.mouse_y < tab_bar_h {
                if lines > 0 {
                    self.prev_tab();
                } else {
                    self.next_tab();
                }
                return;
            }

            let px = self.padding_x as f64;
            let py = (self.padding_y + self.tab_bar_height()) as f64;
            let cw = self.cell_width() as f64;
            let ch = self.cell_height() as f64;
            let col = (((self.mouse_x - px).max(0.0) / cw).floor() as i32 + 1).max(1);
            let row = (((self.mouse_y - py).max(0.0) / ch).floor() as i32 + 1).max(1);

            let active_tab = self.active_tab();
            if active_tab.terminal.mouse_mode > 0 {
                let btn = if lines > 0 { 64 } else { 65 };
                for _ in 0..lines.abs() {
                    let seq = if active_tab.terminal.mouse_sgr {
                        format!("\x1b[<{};{};{}M", btn, col, row)
                    } else {
                        let cb = 32 + btn;
                        let cx = 32 + col;
                        let cy = 32 + row;
                        if cx <= 255 && cy <= 255 {
                            format!(
                                "\x1b[M{}{}{}",
                                cb as u8 as char, cx as u8 as char, cy as u8 as char
                            )
                        } else {
                            String::new()
                        }
                    };
                    if !seq.is_empty() {
                        let _ = active_tab.pty_master.write(seq.as_bytes());
                    }
                }
            } else if active_tab.terminal.is_alt_screen {
                let key_seq = if lines > 0 {
                    if active_tab.terminal.cursor_keys_mode {
                        b"\x1bOA"
                    } else {
                        b"\x1b[A"
                    }
                } else {
                    if active_tab.terminal.cursor_keys_mode {
                        b"\x1bOB"
                    } else {
                        b"\x1b[B"
                    }
                };
                for _ in 0..lines.abs() {
                    let _ = active_tab.pty_master.write(key_seq);
                }
            } else {
                let active_tab = self.active_tab_mut();
                let active_grid = if active_tab.terminal.is_alt_screen {
                    &mut active_tab.terminal.alt_grid
                } else {
                    &mut active_tab.terminal.grid
                };
                let history_len = active_grid.scrollback.len();
                if lines > 0 {
                    active_grid.scroll_offset =
                        (active_grid.scroll_offset + lines as usize).min(history_len);
                } else if lines < 0 {
                    active_grid.scroll_offset = active_grid
                        .scroll_offset
                        .saturating_sub(lines.unsigned_abs() as usize);
                }
                self.needs_redraw = true;
            }
        }
    }

    pub fn handle_mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        modifiers: ModifiersState,
    ) {
        let tab_bar_h = self.tab_bar_height() as f64;

        // ── Tab Bar Mouse Clicks ─────────────────────────────────────────────
        if tab_bar_h > 0.0 && self.mouse_y < tab_bar_h {
            if state.is_pressed() {
                let win_w = self.window.inner_size().width as f32;
                let hit = self.tab_bar.hit_test(
                    self.mouse_x as f32,
                    self.mouse_y as f32,
                    win_w,
                    self.cell_height(),
                    self.tabs.len(),
                );

                match hit {
                    TabBarHitResult::Tab(idx) => {
                        if button == MouseButton::Left {
                            self.switch_tab(idx);
                        } else if button == MouseButton::Middle {
                            self.close_tab(idx);
                        }
                    }
                    TabBarHitResult::CloseTab(idx) => {
                        if button == MouseButton::Left || button == MouseButton::Middle {
                            self.close_tab(idx);
                        }
                    }
                    TabBarHitResult::NewTab => {
                        if button == MouseButton::Left {
                            self.create_tab(None, None, None, None);
                        }
                    }
                    TabBarHitResult::EmptyArea => {
                        if button == MouseButton::Left {
                            let now = std::time::Instant::now();
                            let is_double_click = if let Some(last_time) = self.last_click_instant {
                                self.last_click_pos == (0, 0)
                                    && last_time.elapsed().as_millis() < 400
                            } else {
                                false
                            };
                            self.last_click_instant = Some(now);
                            self.last_click_pos = (0, 0);
                            if is_double_click {
                                self.create_tab(None, None, None, None);
                            }
                        }
                    }
                    TabBarHitResult::None => {}
                }
            }
            return;
        }

        // ── Terminal Content Area Clicks ─────────────────────────────────────
        let px = self.padding_x as f64;
        let py = (self.padding_y + self.tab_bar_height()) as f64;
        let cw = self.cell_width() as f64;
        let ch = self.cell_height() as f64;

        let grid_width = self.active_tab().terminal.grid.width;
        let grid_height = self.active_tab().terminal.grid.height;

        let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize)
            .min(grid_width.saturating_sub(1));
        let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize)
            .min(grid_height.saturating_sub(1));

        let mouse_mode = self.active_tab().terminal.mouse_mode;
        let mouse_sgr = self.active_tab().terminal.mouse_sgr;

        let btn_code = match button {
            MouseButton::Left => Some(0),
            MouseButton::Middle => Some(1),
            MouseButton::Right => Some(2),
            _ => None,
        };

        // 1. Application Mouse Reporting (when mouse tracking is active and Shift is NOT held)
        if mouse_mode > 0 && !modifiers.shift_key() {
            if let Some(btn) = btn_code {
                let pty_master = self.active_tab().pty_master.clone();
                if state.is_pressed() {
                    let seq = if mouse_sgr {
                        format!("\x1b[<{};{};{}M", btn, col_idx + 1, row_idx + 1)
                    } else {
                        let cb = 32 + btn;
                        let cx = 32 + col_idx + 1;
                        let cy = 32 + row_idx + 1;
                        if cx <= 255 && cy <= 255 {
                            format!(
                                "\x1b[M{}{}{}",
                                cb as u8 as char, cx as u8 as char, cy as u8 as char
                            )
                        } else {
                            String::new()
                        }
                    };
                    if !seq.is_empty() {
                        let _ = pty_master.write(seq.as_bytes());
                    }
                } else {
                    let seq = if mouse_sgr {
                        format!("\x1b[<{};{};{}m", btn, col_idx + 1, row_idx + 1)
                    } else {
                        let cb = 32 + 3;
                        let cx = 32 + col_idx + 1;
                        let cy = 32 + row_idx + 1;
                        if cx <= 255 && cy <= 255 {
                            format!(
                                "\x1b[M{}{}{}",
                                cb as u8 as char, cx as u8 as char, cy as u8 as char
                            )
                        } else {
                            String::new()
                        }
                    };
                    if !seq.is_empty() {
                        let _ = pty_master.write(seq.as_bytes());
                    }
                }
            }
            return;
        }

        // 2. Terminal Local Mouse Behavior (Selection / URL clicking / Middle-paste)
        match button {
            MouseButton::Left => {
                if state.is_pressed() {
                    self.is_mouse_down = true;
                    if col_idx < grid_width && row_idx < grid_height {
                        let now = std::time::Instant::now();
                        let is_double_click = if let Some(last_time) = self.last_click_instant {
                            self.last_click_pos == (col_idx, row_idx)
                                && last_time.elapsed().as_millis() < 400
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
                        let click_count = self.click_count;

                        let mut url_opened = false;
                        let active_tab = self.active_tab_mut();
                        let active_grid = active_tab.terminal.active_grid_mut();
                        let offset = active_grid.scroll_offset;
                        let history_len = active_grid.scrollback.len();
                        let y_offset = (row_idx + history_len).saturating_sub(offset);

                        let line_text: String = if y_offset < history_len {
                            active_grid
                                .scrollback
                                .with_row_slice(y_offset, |cells, _| {
                                    cells.iter().map(|c| c.character).collect()
                                })
                                .unwrap_or_default()
                        } else {
                            let y = y_offset - history_len;
                            if y < active_grid.height {
                                let src_start = y * grid_width;
                                let src_end = src_start + grid_width;
                                active_grid.cells[src_start..src_end.min(active_grid.cells.len())]
                                    .iter()
                                    .map(|c| c.character)
                                    .collect()
                            } else {
                                String::new()
                            }
                        };
                        let urls = crate::hyperlink::detector::detect(&line_text);
                        for (start, end, url) in urls {
                            if col_idx >= start && col_idx < end {
                                let _ = crate::hyperlink::detector::open(url);
                                url_opened = true;
                                break;
                            }
                        }

                        if !url_opened {
                            match click_count {
                                1 => {
                                    if modifiers.shift_key() && active_grid.selection.active {
                                        active_grid.selection.update_selection(col_idx, y_offset);
                                    } else {
                                        active_grid.selection.start_selection(col_idx, y_offset);
                                    }
                                }
                                2 => {
                                    active_grid.select_word_at(col_idx, y_offset);
                                    let text = active_grid.extract_selection_text();
                                    if !text.is_empty() {
                                        crate::clipboard::clipboard::copy(text);
                                    }
                                }
                                3 => {
                                    active_grid.select_line_at(y_offset);
                                    let text = active_grid.extract_selection_text();
                                    if !text.is_empty() {
                                        crate::clipboard::clipboard::copy(text);
                                    }
                                }
                                _ => {}
                            }

                            if click_count == 1 && row_idx == active_grid.cursor.y {
                                let cursor_x = active_grid.cursor.x;
                                let pty_master = active_tab.pty_master.clone();
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

                        self.needs_redraw = true;
                    }
                } else {
                    self.is_mouse_down = false;
                    let active_tab = self.active_tab_mut();
                    let active_grid = active_tab.terminal.active_grid_mut();
                    if active_grid.selection.active {
                        let text = active_grid.extract_selection_text();
                        if !text.is_empty()
                            && (active_grid.selection.start_x != active_grid.selection.end_x
                                || active_grid.selection.start_y != active_grid.selection.end_y)
                        {
                            crate::clipboard::clipboard::copy(text);
                        }
                    }
                }
            }
            MouseButton::Middle => {
                if state.is_pressed() {
                    let mut text = crate::clipboard::clipboard::primary_selection();
                    if text.is_empty() {
                        text = crate::clipboard::clipboard::paste();
                    }
                    if !text.is_empty() {
                        let scroll_on_keystroke = self.active_tab().terminal.scroll_on_keystroke;
                        let formatted = self.active_tab().terminal.format_paste(&text);
                        let active_tab = self.active_tab_mut();
                        if scroll_on_keystroke {
                            active_tab.terminal.active_grid_mut().scroll_offset = 0;
                        }
                        let _ = active_tab.pty_master.write(formatted.as_bytes());
                        self.needs_redraw = true;
                    }
                }
            }
            MouseButton::Right if state.is_pressed() => {
                let active_tab = self.active_tab_mut();
                let active_grid = active_tab.terminal.active_grid_mut();
                if active_grid.selection.active {
                    active_grid.selection.active = false;
                    self.needs_redraw = true;
                }
            }
            _ => {}
        }
    }
}
