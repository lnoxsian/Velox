use crate::app::app::{DraggingSeparator, WindowState};
use crate::app::split::SplitDirection;
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
        self.mark_interaction();
        if self.hide_mouse_on_typing {
            self.window.set_cursor_visible(true);
        }
        self.mouse_x = position.x;
        self.mouse_y = position.y;

        let tab_bar_h = self.tab_bar_height() as f64;

        // ── 1. Tab Bar Hover & Hit-Testing ───────────────────────────────────
        if tab_bar_h > 0.0 && self.mouse_y < tab_bar_h {
            let win_w = self.window.inner_size().width as f32;
            let hit = self.tab_bar.hit_test(
                self.mouse_x as f32,
                self.mouse_y as f32,
                win_w,
                self.base_cell_height,
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
                    self.set_cursor_cached(winit::window::CursorIcon::Pointer);
                }
                TabBarHitResult::CloseTab(idx) => {
                    self.tab_bar.hovered_tab = Some(idx);
                    self.tab_bar.hovered_close = Some(idx);
                    self.tab_bar.hovered_new_tab = false;
                    self.set_cursor_cached(winit::window::CursorIcon::Pointer);
                }
                TabBarHitResult::NewTab => {
                    self.tab_bar.hovered_tab = None;
                    self.tab_bar.hovered_close = None;
                    self.tab_bar.hovered_new_tab = true;
                    self.set_cursor_cached(winit::window::CursorIcon::Pointer);
                }
                _ => {
                    self.tab_bar.hovered_tab = None;
                    self.tab_bar.hovered_close = None;
                    self.tab_bar.hovered_new_tab = false;
                    self.set_cursor_cached(winit::window::CursorIcon::Default);
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

        // ── 2. Separator Dragging ────────────────────────────────────────────
        if let Some(dragging) = self.dragging_separator {
            if self.is_mouse_down {
                let ratio = match dragging.direction {
                    SplitDirection::Horizontal => {
                        let rel_y = (self.mouse_y as f32) - dragging.bounds_y;
                        (rel_y / dragging.bounds_h.max(1.0)).clamp(0.05, 0.95)
                    }
                    SplitDirection::Vertical => {
                        let rel_x = (self.mouse_x as f32) - dragging.bounds_x;
                        (rel_x / dragging.bounds_w.max(1.0)).clamp(0.05, 0.95)
                    }
                };
                let tab = self.active_tab_mut();
                if tab.tree.set_split_ratio(dragging.split_id, ratio) {
                    self.sync_active_tab_layout();
                    self.needs_redraw = true;
                    self.content_dirty = true;
                }
                return;
            } else {
                self.dragging_separator = None;
            }
        }

        // ── 3. Separator Hover Hit-testing ───────────────────────────────────
        if self.tabs.is_empty() {
            return;
        }
        let (pane_rects, sep_rects) = self.recalculate_panes_layout(self.active_tab_index);
        let mut hit_sep = None;
        for sep in &sep_rects {
            if sep.contains(self.mouse_x as f32, self.mouse_y as f32) {
                hit_sep = Some(*sep);
                break;
            }
        }

        if let Some(sep) = hit_sep {
            if self.hovered_separator != Some(sep.split_id) {
                self.hovered_separator = Some(sep.split_id);
                self.needs_redraw = true;
            }
            match sep.direction {
                SplitDirection::Horizontal => {
                    self.set_cursor_cached(winit::window::CursorIcon::RowResize)
                }
                SplitDirection::Vertical => {
                    self.set_cursor_cached(winit::window::CursorIcon::ColResize)
                }
            }
            return;
        } else if self.hovered_separator.is_some() {
            self.hovered_separator = None;
            self.needs_redraw = true;
        }

        // ── 4. Pane Hover & Selection Dragging ───────────────────────────────
        let hit_pane_rect = pane_rects
            .iter()
            .find(|r| r.contains(self.mouse_x as f32, self.mouse_y as f32))
            .copied()
            .or_else(|| pane_rects.first().copied());

        if let Some(pane_rect) = hit_pane_rect {
            let cw = pane_rect.cell_width as f64;
            let ch = pane_rect.cell_height as f64;
            let px = (pane_rect.x + pane_rect.padding_x) as f64;
            let py = (pane_rect.y + pane_rect.padding_y) as f64;
            let grid_width = pane_rect.cols;
            let grid_height = pane_rect.rows;

            let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize)
                .min(grid_width.saturating_sub(1));
            let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize)
                .min(grid_height.saturating_sub(1));

            if (col_idx, row_idx) != self.last_mouse_cell {
                self.last_mouse_cell = (col_idx, row_idx);

                let (mouse_mode, mouse_sgr, is_link, pty_master) = {
                    let tab = self.active_tab();
                    if let Some(pane) = tab.tree.find_pane(pane_rect.pane_id) {
                        let mouse_mode = pane.terminal.mouse_mode;
                        let mouse_sgr = pane.terminal.mouse_sgr;

                        let is_link = if mouse_mode == 0 {
                            let active_grid = if pane.terminal.is_alt_screen {
                                &pane.terminal.alt_grid
                            } else {
                                &pane.terminal.grid
                            };
                            let offset = active_grid.scroll_offset;
                            let history_len = active_grid.scrollback.len();
                            let y_offset = (row_idx + history_len).saturating_sub(offset);
                            let mut link_found =
                                active_grid.hyperlink_at(col_idx, y_offset).is_some();
                            if !link_found {
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
                                        let physical_y = active_grid.physical_row(y);
                                        let src_start = physical_y * active_grid.width;
                                        let src_end = src_start + active_grid.width;
                                        active_grid.cells
                                            [src_start..src_end.min(active_grid.cells.len())]
                                            .iter()
                                            .map(|c| c.character)
                                            .collect()
                                    } else {
                                        String::new()
                                    }
                                };
                                crate::hyperlink::detector::for_each_url(
                                    &line_text,
                                    |start_col, end_col| {
                                        if col_idx >= start_col && col_idx < end_col {
                                            link_found = true;
                                        }
                                    },
                                );
                            }
                            link_found
                        } else {
                            false
                        };
                        (
                            Some(mouse_mode),
                            Some(mouse_sgr),
                            is_link,
                            Some(pane.pty_master.clone()),
                        )
                    } else {
                        (None, None, false, None)
                    }
                };

                if let (Some(mouse_mode), Some(mouse_sgr), Some(pty_master)) =
                    (mouse_mode, mouse_sgr, pty_master)
                {
                    if mouse_mode == 0 {
                        if is_link {
                            self.set_cursor_cached(winit::window::CursorIcon::Pointer);
                        } else {
                            self.set_cursor_cached(winit::window::CursorIcon::Text);
                        }
                    } else {
                        self.set_cursor_cached(winit::window::CursorIcon::Default);
                    }

                    let should_report_motion =
                        mouse_mode == 1003 || (mouse_mode == 1002 && self.is_mouse_down);

                    if should_report_motion && !modifiers.shift_key() {
                        let base_code = if self.is_mouse_down {
                            32 + self.last_mouse_button
                        } else {
                            35
                        };
                        let mut btn_code = base_code;
                        if modifiers.shift_key() {
                            btn_code += 4;
                        }
                        if modifiers.alt_key() {
                            btn_code += 8;
                        }
                        if modifiers.control_key() {
                            btn_code += 16;
                        }

                        let mut buf = [0u8; 32];
                        let written = if mouse_sgr {
                            use std::io::Write;
                            let mut cur = std::io::Cursor::new(&mut buf[..]);
                            let _ =
                                write!(cur, "\x1b[<{};{};{}M", btn_code, col_idx + 1, row_idx + 1);
                            cur.position() as usize
                        } else {
                            let cb = 32 + btn_code;
                            let cx = 32 + col_idx + 1;
                            let cy = 32 + row_idx + 1;
                            if cx <= 255 && cy <= 255 {
                                buf[0] = 0x1b;
                                buf[1] = b'M';
                                buf[2] = cb;
                                buf[3] = cx as u8;
                                buf[4] = cy as u8;
                                5
                            } else {
                                0
                            }
                        };
                        if written > 0 {
                            let _ = pty_master.write(&buf[..written]);
                        }
                    } else if self.is_mouse_down {
                        let tab = self.active_tab_mut();
                        let active_id = tab.active_pane_id;
                        if let Some(pane) = tab.tree.find_pane_mut(active_id) {
                            let active_grid = pane.terminal.active_grid_mut();
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
        }
    }

    pub fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta, modifiers: ModifiersState) {
        self.mark_interaction();
        let lines_f = match delta {
            MouseScrollDelta::LineDelta(_, y) => y as f64,
            MouseScrollDelta::PixelDelta(pos) => pos.y / 15.0,
        };
        let lines = (lines_f * self.scroll_multiplier).round() as i32;
        if lines != 0 {
            if modifiers.control_key() {
                let current_size = self.active_pane().font_size;
                let step = if lines > 0 { 1.0 } else { -1.0 };
                let new_size = (current_size + step).max(1.0);
                if (new_size - current_size).abs() > 0.01 {
                    self.set_font_size(new_size);
                }
                return;
            }

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

            if self.tabs.is_empty() {
                return;
            }

            let (pane_rects, _) = self.recalculate_panes_layout(self.active_tab_index);
            let hit_pane_rect = pane_rects
                .iter()
                .find(|r| r.contains(self.mouse_x as f32, self.mouse_y as f32))
                .copied()
                .or_else(|| pane_rects.first().copied());

            if let Some(pane_rect) = hit_pane_rect {
                let px = (pane_rect.x + pane_rect.padding_x) as f64;
                let py = (pane_rect.y + pane_rect.padding_y) as f64;
                let cw = pane_rect.cell_width as f64;
                let ch = pane_rect.cell_height as f64;
                let col = (((self.mouse_x - px).max(0.0) / cw).floor() as i32 + 1).max(1);
                let row = (((self.mouse_y - py).max(0.0) / ch).floor() as i32 + 1).max(1);

                let tab = self.active_tab();
                if let Some(pane) = tab.tree.find_pane(pane_rect.pane_id) {
                    if pane.terminal.mouse_mode > 0 {
                        let btn = if lines > 0 { 64 } else { 65 };
                        let mut buf = [0u8; 32];
                        let written = if pane.terminal.mouse_sgr {
                            use std::io::Write;
                            let mut cur = std::io::Cursor::new(&mut buf[..]);
                            let _ = write!(cur, "\x1b[<{};{};{}M", btn, col, row);
                            cur.position() as usize
                        } else {
                            let cb = 32 + btn;
                            let cx = 32 + col;
                            let cy = 32 + row;
                            if cb <= 255 && cx <= 255 && cy <= 255 {
                                buf[0] = 0x1b;
                                buf[1] = b'[';
                                buf[2] = b'M';
                                buf[3] = cb as u8;
                                buf[4] = cx as u8;
                                buf[5] = cy as u8;
                                6
                            } else {
                                0
                            }
                        };
                        if written > 0 {
                            for _ in 0..lines.abs() {
                                let _ = pane.pty_master.write(&buf[..written]);
                            }
                        }
                    } else if pane.terminal.is_alt_screen {
                        let key_seq: &[u8] = if lines > 0 {
                            if pane.terminal.cursor_keys_mode {
                                b"\x1bOA"
                            } else {
                                b"\x1b[A"
                            }
                        } else if pane.terminal.cursor_keys_mode {
                            b"\x1bOB"
                        } else {
                            b"\x1b[B"
                        };
                        for _ in 0..lines.abs() {
                            let _ = pane.pty_master.write(key_seq);
                        }
                    } else {
                        let tab_mut = self.active_tab_mut();
                        if let Some(pane_mut) = tab_mut.tree.find_pane_mut(pane_rect.pane_id) {
                            let active_grid = if pane_mut.terminal.is_alt_screen {
                                &mut pane_mut.terminal.alt_grid
                            } else {
                                &mut pane_mut.terminal.grid
                            };
                            let history_len = active_grid.scrollback.len();
                            if lines > 0 {
                                active_grid.scroll_offset =
                                    (active_grid.scroll_offset + lines as usize).min(history_len);
                                active_grid.damage.mark_all();
                            } else if lines < 0 {
                                active_grid.scroll_offset = active_grid
                                    .scroll_offset
                                    .saturating_sub(lines.unsigned_abs() as usize);
                                active_grid.damage.mark_all();
                            }
                            self.needs_redraw = true;
                        }
                    }
                }
            }
        }
    }

    pub fn handle_mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        modifiers: ModifiersState,
    ) {
        self.mark_interaction();
        if self.hide_mouse_on_typing {
            self.window.set_cursor_visible(true);
        }
        let tab_bar_h = self.tab_bar_height() as f64;

        // ── 1. Tab Bar Mouse Clicks ──────────────────────────────────────────
        if tab_bar_h > 0.0 && self.mouse_y < tab_bar_h {
            if state.is_pressed() {
                let win_w = self.window.inner_size().width as f32;
                let hit = self.tab_bar.hit_test(
                    self.mouse_x as f32,
                    self.mouse_y as f32,
                    win_w,
                    self.base_cell_height,
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

        if self.tabs.is_empty() {
            return;
        }

        // ── 2. Separator Clicks & Drag Initiation ────────────────────────────
        let (pane_rects, sep_rects) = self.recalculate_panes_layout(self.active_tab_index);

        if button == MouseButton::Left {
            if state.is_pressed() {
                for sep in &sep_rects {
                    if sep.contains(self.mouse_x as f32, self.mouse_y as f32) {
                        self.is_mouse_down = true;
                        self.dragging_separator = Some(DraggingSeparator {
                            split_id: sep.split_id,
                            direction: sep.direction,
                            bounds_x: sep.bounds_x,
                            bounds_y: sep.bounds_y,
                            bounds_w: sep.bounds_w,
                            bounds_h: sep.bounds_h,
                        });
                        return;
                    }
                }
            } else {
                self.dragging_separator = None;
            }
        }

        // ── 3. Pane Clicks & Focus ───────────────────────────────────────────
        let hit_pane_rect = pane_rects
            .iter()
            .find(|r| r.contains(self.mouse_x as f32, self.mouse_y as f32))
            .copied()
            .or_else(|| pane_rects.first().copied());

        let pane_rect = match hit_pane_rect {
            Some(r) => r,
            None => return,
        };

        // Switch active pane to clicked pane if different
        if state.is_pressed() {
            let clicked_pane_id = pane_rect.pane_id;
            if self.active_tab().active_pane_id != clicked_pane_id {
                self.active_tab_mut().set_active_pane(clicked_pane_id);
                self.sync_active_pane_font_size();
                self.needs_redraw = true;
                self.content_dirty = true;
            }
            self.active_tab_mut().clear_unfocused_selections();
        }

        let px = (pane_rect.x + pane_rect.padding_x) as f64;
        let py = (pane_rect.y + pane_rect.padding_y) as f64;
        let cw = pane_rect.cell_width as f64;
        let ch = pane_rect.cell_height as f64;
        let grid_width = pane_rect.cols;
        let grid_height = pane_rect.rows;

        let col_idx = (((self.mouse_x - px).max(0.0) / cw).floor() as usize)
            .min(grid_width.saturating_sub(1));
        let row_idx = (((self.mouse_y - py).max(0.0) / ch).floor() as usize)
            .min(grid_height.saturating_sub(1));

        let (mouse_mode, mouse_sgr) = {
            let pane = match self.active_tab().tree.find_pane(pane_rect.pane_id) {
                Some(p) => p,
                None => return,
            };
            (pane.terminal.mouse_mode, pane.terminal.mouse_sgr)
        };

        let btn_code = match button {
            MouseButton::Left => Some(0),
            MouseButton::Middle => Some(1),
            MouseButton::Right => Some(2),
            _ => None,
        };

        // Application Mouse Reporting
        if mouse_mode > 0 && !modifiers.shift_key() {
            if let Some(btn) = btn_code {
                self.is_mouse_down = state.is_pressed();
                if state.is_pressed() {
                    self.last_mouse_button = btn;
                }

                let mut report_btn = btn;
                if modifiers.shift_key() {
                    report_btn += 4;
                }
                if modifiers.alt_key() {
                    report_btn += 8;
                }
                if modifiers.control_key() {
                    report_btn += 16;
                }

                let pty_master = self
                    .active_tab()
                    .tree
                    .find_pane(pane_rect.pane_id)
                    .map(|p| p.pty_master.clone());
                if let Some(pty) = pty_master {
                    let mut buf = [0u8; 32];
                    let written = if mouse_sgr {
                        use std::io::Write;
                        let mut cur = std::io::Cursor::new(&mut buf[..]);
                        let terminator = if state.is_pressed() { 'M' } else { 'm' };
                        let _ = write!(
                            cur,
                            "\x1b[<{};{};{}{}",
                            report_btn,
                            col_idx + 1,
                            row_idx + 1,
                            terminator
                        );
                        cur.position() as usize
                    } else {
                        let cb = if state.is_pressed() {
                            32 + report_btn
                        } else {
                            32 + 3
                        };
                        let cx = 32 + col_idx + 1;
                        let cy = 32 + row_idx + 1;
                        if cx <= 255 && cy <= 255 {
                            buf[0] = 0x1b;
                            buf[1] = b'M';
                            buf[2] = cb;
                            buf[3] = cx as u8;
                            buf[4] = cy as u8;
                            5
                        } else {
                            0
                        }
                    };
                    if written > 0 {
                        let _ = pty.write(&buf[..written]);
                    }
                }
            }
            return;
        }

        // Terminal Local Mouse Behavior (Selection / URL clicking / Middle-paste)
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
                        let tab = self.active_tab_mut();
                        if let Some(pane) = tab.tree.find_pane_mut(pane_rect.pane_id) {
                            let active_grid = pane.terminal.active_grid_mut();
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
                                    let physical_y = active_grid.physical_row(y);
                                    let src_start = physical_y * grid_width;
                                    let src_end = src_start + grid_width;
                                    active_grid.cells
                                        [src_start..src_end.min(active_grid.cells.len())]
                                        .iter()
                                        .map(|c| c.character)
                                        .collect()
                                } else {
                                    String::new()
                                }
                            };
                            if modifiers.control_key() {
                                if let Some(link) = active_grid.hyperlink_at(col_idx, y_offset) {
                                    let _ = crate::hyperlink::detector::open(&link.url);
                                    url_opened = true;
                                } else {
                                    let urls = crate::hyperlink::detector::detect(&line_text);
                                    for (start, end, url) in urls {
                                        if col_idx >= start && col_idx < end {
                                            let _ = crate::hyperlink::detector::open(url);
                                            url_opened = true;
                                            break;
                                        }
                                    }
                                }
                            }

                            if !url_opened {
                                match click_count {
                                    1 => {
                                        if modifiers.shift_key() && active_grid.selection.active {
                                            active_grid
                                                .selection
                                                .update_selection(col_idx, y_offset);
                                        } else {
                                            active_grid
                                                .selection
                                                .start_selection(col_idx, y_offset);
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
                                    let pty_master = pane.pty_master.clone();
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
                    }
                } else {
                    self.is_mouse_down = false;
                    let tab = self.active_tab_mut();
                    if let Some(pane) = tab.tree.find_pane_mut(pane_rect.pane_id) {
                        let active_grid = pane.terminal.active_grid_mut();
                        if active_grid.selection.active {
                            if active_grid.selection.is_empty() {
                                active_grid.selection.clear();
                                self.needs_redraw = true;
                            } else {
                                let text = active_grid.extract_selection_text();
                                if !text.is_empty() {
                                    crate::clipboard::clipboard::copy(text);
                                }
                            }
                        }
                    }
                }
            }
            MouseButton::Middle => {
                if state.is_pressed() {
                    self.is_mouse_down = true;
                    self.last_mouse_button = 1;
                    let mut text = crate::clipboard::clipboard::primary_selection();
                    if text.is_empty() {
                        text = crate::clipboard::clipboard::paste();
                    }
                    if !text.is_empty() {
                        let tab = self.active_tab_mut();
                        if let Some(pane) = tab.tree.find_pane_mut(pane_rect.pane_id) {
                            let scroll_on_keystroke = pane.terminal.scroll_on_keystroke;
                            let formatted = pane.terminal.format_paste(&text);
                            if scroll_on_keystroke {
                                pane.terminal.active_grid_mut().scroll_offset = 0;
                            }
                            let _ = pane.pty_master.write(formatted.as_bytes());
                            self.needs_redraw = true;
                        }
                    }
                } else {
                    self.is_mouse_down = false;
                }
            }
            MouseButton::Right => {
                if state.is_pressed() {
                    self.is_mouse_down = true;
                    self.last_mouse_button = 2;
                    let tab = self.active_tab_mut();
                    if let Some(pane) = tab.tree.find_pane_mut(pane_rect.pane_id) {
                        let active_grid = pane.terminal.active_grid_mut();
                        if active_grid.selection.active {
                            active_grid.selection.active = false;
                            self.needs_redraw = true;
                        }
                    }
                } else {
                    self.is_mouse_down = false;
                }
            }
            _ => {}
        }
    }
}
