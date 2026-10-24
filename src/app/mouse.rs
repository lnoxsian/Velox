use crate::app::app::{DraggingSeparator, WindowState};
use crate::app::split::SplitDirection;
use crate::app::tab::TabBarHitResult;
use crate::window::event::{ElementState, ModifiersState, MouseButton};

#[inline(always)]
fn pane_cell_coords(
    pane_rect: &crate::app::split::PaneRect,
    mouse_x: f64,
    mouse_y: f64,
) -> (usize, usize) {
    let cw = pane_rect.cell_width as f64;
    let ch = pane_rect.cell_height as f64;
    let px = (pane_rect.x + pane_rect.padding_x) as f64;
    let py = (pane_rect.y + pane_rect.padding_y) as f64;
    let grid_width = pane_rect.cols;
    let grid_height = pane_rect.rows;

    let col_idx = if cw > 0.0 {
        let raw = ((mouse_x - px) / cw).floor() as i64;
        raw.clamp(0, grid_width.saturating_sub(1) as i64) as usize
    } else {
        0
    };
    let row_idx = if ch > 0.0 {
        let raw = ((mouse_y - py) / ch).floor() as i64;
        raw.clamp(0, grid_height.saturating_sub(1) as i64) as usize
    } else {
        0
    };
    (col_idx, row_idx)
}

#[inline(always)]
fn pane_coords_if_in_text(
    pane_rect: &crate::app::split::PaneRect,
    mouse_x: f64,
    mouse_y: f64,
) -> Option<(usize, usize)> {
    let cw = pane_rect.cell_width as f64;
    let ch = pane_rect.cell_height as f64;
    if cw <= 0.0 || ch <= 0.0 {
        return None;
    }
    let px = (pane_rect.x + pane_rect.padding_x) as f64;
    let py = (pane_rect.y + pane_rect.padding_y) as f64;
    let max_w = cw * (pane_rect.cols as f64);
    let max_h = ch * (pane_rect.rows as f64);

    let rel_x = mouse_x - px;
    let rel_y = mouse_y - py;

    if rel_x >= 0.0 && rel_x < max_w && rel_y >= 0.0 && rel_y < max_h {
        let col = (rel_x / cw).floor() as usize;
        let row = (rel_y / ch).floor() as usize;
        Some((
            col.min(pane_rect.cols.saturating_sub(1)),
            row.min(pane_rect.rows.saturating_sub(1)),
        ))
    } else {
        None
    }
}

#[inline(always)]
fn apply_mouse_modifiers(btn: u8, modifiers: ModifiersState) -> usize {
    let mut btn_code = btn as usize;
    if modifiers.shift_key() {
        btn_code += 4;
    }
    if modifiers.alt_key() {
        btn_code += 8;
    }
    if modifiers.control_key() {
        btn_code += 16;
    }
    btn_code
}

#[derive(Debug, Clone, Copy, Default)]
struct MouseProtocol {
    sgr: bool,
    utf8: bool,
    pixel: bool,
}

#[inline(always)]
fn pane_pixel_coords(
    pane_rect: &crate::app::split::PaneRect,
    mouse_x: f64,
    mouse_y: f64,
) -> (usize, usize) {
    let px = (pane_rect.x + pane_rect.padding_x) as f64;
    let py = (pane_rect.y + pane_rect.padding_y) as f64;
    let x = (mouse_x - px).max(0.0) as usize;
    let y = (mouse_y - py).max(0.0) as usize;
    (x, y)
}

#[inline]
fn format_usize_into(mut n: usize, buf: &mut [u8]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut temp = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        temp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    for j in 0..i {
        buf[j] = temp[i - 1 - j];
    }
    i
}

#[inline]
fn format_mouse_report(
    proto: MouseProtocol,
    btn_code: usize,
    col_idx: usize,
    row_idx: usize,
    pixel_x: usize,
    pixel_y: usize,
    is_release: bool,
    buf: &mut [u8; 64],
) -> usize {
    if proto.pixel || proto.sgr {
        let suffix = if is_release { b'm' } else { b'M' };
        let (x_coord, y_coord) = if proto.pixel {
            (pixel_x + 1, pixel_y + 1)
        } else {
            (col_idx + 1, row_idx + 1)
        };
        buf[0] = 0x1b;
        buf[1] = b'[';
        buf[2] = b'<';
        let mut offset = 3;
        offset += format_usize_into(btn_code, &mut buf[offset..]);
        buf[offset] = b';';
        offset += 1;
        offset += format_usize_into(x_coord, &mut buf[offset..]);
        buf[offset] = b';';
        offset += 1;
        offset += format_usize_into(y_coord, &mut buf[offset..]);
        buf[offset] = suffix;
        offset + 1
    } else if proto.utf8 {
        let cb = if is_release { 32 + 3 } else { 32 + btn_code };
        let cx = 32 + col_idx + 1;
        let cy = 32 + row_idx + 1;
        if let (Some(cb_char), Some(cx_char), Some(cy_char)) = (
            char::from_u32(cb as u32),
            char::from_u32(cx as u32),
            char::from_u32(cy as u32),
        ) {
            let mut offset = 0;
            buf[offset..offset + 3].copy_from_slice(b"\x1b[M");
            offset += 3;
            offset += cb_char.encode_utf8(&mut buf[offset..]).len();
            offset += cx_char.encode_utf8(&mut buf[offset..]).len();
            offset += cy_char.encode_utf8(&mut buf[offset..]).len();
            offset
        } else {
            0
        }
    } else {
        let cb = if is_release { 32 + 3 } else { 32 + btn_code };
        let cx = 32 + col_idx + 1;
        let cy = 32 + row_idx + 1;
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
    }
}

fn send_mouse_report(
    pty: &crate::pty::master::PtyMaster,
    proto: MouseProtocol,
    btn_code: usize,
    col_idx: usize,
    row_idx: usize,
    pixel_x: usize,
    pixel_y: usize,
    is_release: bool,
) {
    let mut buf = [0u8; 64];
    let written = format_mouse_report(
        proto, btn_code, col_idx, row_idx, pixel_x, pixel_y, is_release, &mut buf,
    );
    if written > 0 {
        let _ = pty.write(&buf[..written]);
    }
}

impl WindowState {
    pub fn handle_cursor_moved(&mut self, position: (f64, f64), modifiers: ModifiersState) {
        self.mark_interaction();
        self.mouse_x = position.0;
        self.mouse_y = position.1;

        // ── 1. Separator Dragging ────────────────────────────────────────────
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
                    let now = std::time::Instant::now();
                    let should_sync = match self.last_separator_drag_sync {
                        Some(last) => {
                            now.duration_since(last) >= std::time::Duration::from_millis(60)
                        }
                        None => true,
                    };
                    if should_sync {
                        self.sync_active_tab_layout();
                        self.last_separator_drag_sync = Some(now);
                    }
                    self.needs_redraw = true;
                    self.content_dirty = true;
                }
                return;
            } else {
                self.dragging_separator = None;
                self.last_separator_drag_sync = None;
            }
        }

        if self.tabs.is_empty() {
            return;
        }

        let (pane_rects, sep_rects) = self.recalculate_panes_layout(self.active_tab_index);

        // ── 2. Pane Mouse Dragging (Active Selection or App Mouse Drag) ──────
        if self.is_mouse_down
            && let Some(drag_pane_id) = self.mouse_down_pane_id
            && let Some(pane_rect) = pane_rects
                .iter()
                .find(|r| r.pane_id == drag_pane_id)
                .copied()
        {
            let (col_idx, row_idx) = pane_cell_coords(&pane_rect, self.mouse_x, self.mouse_y);

            if self.last_mouse_pane_id != Some(drag_pane_id)
                || (col_idx, row_idx) != self.last_mouse_cell
            {
                self.last_mouse_pane_id = Some(drag_pane_id);
                self.last_mouse_cell = (col_idx, row_idx);

                let (mouse_mode, mouse_proto, pty_master) = {
                    let tab = self.active_tab();
                    if let Some(pane) = tab.tree.find_pane(drag_pane_id) {
                        (
                            pane.terminal.mouse_mode,
                            MouseProtocol {
                                sgr: pane.terminal.mouse_sgr,
                                utf8: pane.terminal.mouse_utf8,
                                pixel: pane.terminal.mouse_pixel,
                            },
                            Some(pane.pty_master.clone()),
                        )
                    } else {
                        (
                            0,
                            MouseProtocol {
                                sgr: false,
                                utf8: false,
                                pixel: false,
                            },
                            None,
                        )
                    }
                };

                let should_report_motion =
                    (mouse_mode == 1003 || mouse_mode == 1002) && !modifiers.shift_key();

                if should_report_motion {
                    self.set_cursor_cached(crate::window::event::CursorIcon::Default);
                    if let Some(pty_master) = pty_master {
                        let btn_code =
                            apply_mouse_modifiers(32 + self.last_mouse_button, modifiers);
                        let (pixel_x, pixel_y) =
                            pane_pixel_coords(&pane_rect, self.mouse_x, self.mouse_y);
                        send_mouse_report(
                            &pty_master,
                            mouse_proto,
                            btn_code,
                            col_idx,
                            row_idx,
                            pixel_x,
                            pixel_y,
                            false,
                        );
                    }
                } else {
                    self.set_cursor_cached(crate::window::event::CursorIcon::Text);
                    let tab = self.active_tab_mut();
                    if let Some(pane) = tab.tree.find_pane_mut(drag_pane_id) {
                        let active_grid = pane.terminal.active_grid_mut();
                        if active_grid.selection.active {
                            let abs_y = active_grid.display_row_to_abs(row_idx);
                            active_grid.selection.update_selection(col_idx, abs_y);
                            self.needs_redraw = true;
                        }
                    }
                }
            }
            return;
        }

        if self.window.uses_csd() && !self.is_mouse_down {
            let win_size = self.window.inner_size();
            let border_thickness = 5.0f32;
            if let Some(edge) = crate::window::csd::detect_resize_edge(
                self.mouse_x as f32,
                self.mouse_y as f32,
                win_size.width,
                win_size.height,
                border_thickness,
            ) {
                let cursor = match edge {
                    crate::window::WindowResizeEdge::Left
                    | crate::window::WindowResizeEdge::Right => {
                        crate::window::event::CursorIcon::ColResize
                    }
                    crate::window::WindowResizeEdge::Top
                    | crate::window::WindowResizeEdge::Bottom => {
                        crate::window::event::CursorIcon::RowResize
                    }
                    crate::window::WindowResizeEdge::TopLeft
                    | crate::window::WindowResizeEdge::BottomRight
                    | crate::window::WindowResizeEdge::TopRight
                    | crate::window::WindowResizeEdge::BottomLeft => {
                        crate::window::event::CursorIcon::Default
                    }
                };
                self.set_cursor_cached(cursor);
                return;
            }
        }

        let csd_h = self.window.csd_title_bar_height() as f64;
        if csd_h > 0.0 && self.mouse_y < csd_h {
            let win_w = self.window.width();
            let hit = self.csd_button_layout.hit_test(
                self.mouse_x as f32,
                self.mouse_y as f32,
                win_w,
                csd_h as u32,
            );
            if hit != self.hovered_csd_button {
                self.hovered_csd_button = hit;
                self.needs_redraw = true;
            }

            if self.tab_bar.clear_hover() {
                self.tab_bar_dirty = true;
                self.needs_redraw = true;
            }
            self.set_cursor_cached(crate::window::event::CursorIcon::Default);
            if self.hovered_separator.is_some() {
                self.hovered_separator = None;
                self.needs_redraw = true;
            }
            return;
        } else if self.hovered_csd_button.is_some() {
            self.hovered_csd_button = None;
            self.needs_redraw = true;
        }

        let tab_bar_h = self.tab_bar_height() as f64;

        // ── 3. Tab Bar Hover & Hit-Testing (When not dragging) ───────────────
        if tab_bar_h > 0.0 && self.mouse_y >= csd_h && self.mouse_y < csd_h + tab_bar_h {
            let win_w = self.window.width() as f32;
            let hit = self.tab_bar.hit_test(
                self.mouse_x as f32,
                (self.mouse_y - csd_h) as f32,
                win_w,
                self.base_cell_height,
                self.tabs.len(),
            );

            let (hover_tab, hover_close, hover_new, cursor) = match hit {
                TabBarHitResult::Tab(idx) => (
                    Some(idx),
                    None,
                    false,
                    crate::window::event::CursorIcon::Pointer,
                ),
                TabBarHitResult::CloseTab(idx) => (
                    Some(idx),
                    Some(idx),
                    false,
                    crate::window::event::CursorIcon::Pointer,
                ),
                TabBarHitResult::NewTab => {
                    (None, None, true, crate::window::event::CursorIcon::Pointer)
                }
                _ => (None, None, false, crate::window::event::CursorIcon::Default),
            };

            if self.tab_bar.set_hover(hover_tab, hover_close, hover_new) {
                self.tab_bar_dirty = true;
                self.needs_redraw = true;
            }
            self.set_cursor_cached(cursor);
            return;
        } else if self.tab_bar.clear_hover() {
            self.tab_bar_dirty = true;
            self.needs_redraw = true;
        }

        // ── 4. Separator Hover Hit-testing (When not dragging) ───────────────
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
                    self.set_cursor_cached(crate::window::event::CursorIcon::RowResize)
                }
                SplitDirection::Vertical => {
                    self.set_cursor_cached(crate::window::event::CursorIcon::ColResize)
                }
            }
            return;
        } else if self.hovered_separator.is_some() {
            self.hovered_separator = None;
            self.needs_redraw = true;
        }

        // ── 5. Pane Hover (When not dragging) ────────────────────────────────
        let hit_pane_rect = pane_rects
            .iter()
            .find(|r| r.contains(self.mouse_x as f32, self.mouse_y as f32))
            .copied();

        if let Some(pane_rect) = hit_pane_rect {
            let Some((col_idx, row_idx)) =
                pane_coords_if_in_text(&pane_rect, self.mouse_x, self.mouse_y)
            else {
                self.set_cursor_cached(crate::window::event::CursorIcon::Default);
                return;
            };

            if self.last_mouse_pane_id != Some(pane_rect.pane_id)
                || (col_idx, row_idx) != self.last_mouse_cell
            {
                self.last_mouse_pane_id = Some(pane_rect.pane_id);
                self.last_mouse_cell = (col_idx, row_idx);

                let (mouse_mode, mouse_proto, is_link, pty_master) = {
                    let tab = self.active_tab();
                    if let Some(pane) = tab.tree.find_pane(pane_rect.pane_id) {
                        let mouse_mode = pane.terminal.mouse_mode;
                        let mouse_proto = MouseProtocol {
                            sgr: pane.terminal.mouse_sgr,
                            utf8: pane.terminal.mouse_utf8,
                            pixel: pane.terminal.mouse_pixel,
                        };

                        let is_link = if mouse_mode == 0
                            || modifiers.shift_key()
                            || modifiers.control_key()
                        {
                            let active_grid = pane.terminal.active_grid();
                            let y_offset = active_grid.display_row_to_abs(row_idx);
                            active_grid.has_url_at(col_idx, y_offset)
                        } else {
                            false
                        };
                        (
                            Some(mouse_mode),
                            Some(mouse_proto),
                            is_link,
                            Some(pane.pty_master.clone()),
                        )
                    } else {
                        (None, None, false, None)
                    }
                };

                if let (Some(mouse_mode), Some(mouse_proto), Some(pty_master)) =
                    (mouse_mode, mouse_proto, pty_master)
                {
                    if mouse_mode == 0 || modifiers.shift_key() || modifiers.control_key() {
                        if is_link {
                            self.set_cursor_cached(crate::window::event::CursorIcon::Pointer);
                        } else {
                            self.set_cursor_cached(crate::window::event::CursorIcon::Text);
                        }
                    } else {
                        self.set_cursor_cached(crate::window::event::CursorIcon::Default);
                    }

                    let should_report_motion = mouse_mode == 1003 && !modifiers.shift_key();

                    if should_report_motion {
                        let (pixel_x, pixel_y) =
                            pane_pixel_coords(&pane_rect, self.mouse_x, self.mouse_y);
                        send_mouse_report(
                            &pty_master,
                            mouse_proto,
                            35,
                            col_idx,
                            row_idx,
                            pixel_x,
                            pixel_y,
                            false,
                        );
                    }
                }
            }
        } else {
            self.set_cursor_cached(crate::window::event::CursorIcon::Default);
        }
    }

    pub fn handle_mouse_wheel(&mut self, delta_y: f32, _modifiers: ModifiersState) -> bool {
        self.mark_interaction();
        let lines_f = delta_y as f64;
        let lines = (lines_f * self.scroll_multiplier).round() as i32;
        if lines != 0 {
            let csd_h = self.window.csd_title_bar_height() as f64;
            if csd_h > 0.0 && self.mouse_y < csd_h {
                return false;
            }

            let tab_bar_h = self.tab_bar_height() as f64;

            // Scroll over tab bar switches tabs
            if tab_bar_h > 0.0 && self.mouse_y >= csd_h && self.mouse_y < csd_h + tab_bar_h {
                if lines > 0 {
                    self.prev_tab();
                } else {
                    self.next_tab();
                }
                return false;
            }

            if self.tabs.is_empty() {
                return false;
            }

            let (pane_rects, _) = self.recalculate_panes_layout(self.active_tab_index);
            let hit_pane_rect = pane_rects
                .iter()
                .find(|r| r.contains(self.mouse_x as f32, self.mouse_y as f32))
                .copied()
                .or_else(|| pane_rects.first().copied());

            if let Some(pane_rect) = hit_pane_rect {
                let (col_idx, row_idx) = pane_cell_coords(&pane_rect, self.mouse_x, self.mouse_y);

                let tab = self.active_tab();
                if let Some(pane) = tab.tree.find_pane(pane_rect.pane_id) {
                    if pane.terminal.mouse_mode > 0 {
                        let btn = if lines > 0 { 64 } else { 65 };
                        let proto = MouseProtocol {
                            sgr: pane.terminal.mouse_sgr,
                            utf8: pane.terminal.mouse_utf8,
                            pixel: pane.terminal.mouse_pixel,
                        };
                        let (pixel_x, pixel_y) =
                            pane_pixel_coords(&pane_rect, self.mouse_x, self.mouse_y);
                        for _ in 0..lines.abs() {
                            send_mouse_report(
                                &pane.pty_master,
                                proto,
                                btn,
                                col_idx,
                                row_idx,
                                pixel_x,
                                pixel_y,
                                false,
                            );
                        }
                        return true;
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
                        return true;
                    } else {
                        let tab_mut = self.active_tab_mut();
                        if let Some(pane_mut) = tab_mut.tree.find_pane_mut(pane_rect.pane_id) {
                            let active_grid = pane_mut.terminal.active_grid_mut();
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
                        return false;
                    }
                }
            }
        }
        false
    }

    pub fn handle_mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        modifiers: ModifiersState,
    ) {
        self.mark_interaction();

        if state == ElementState::Pressed {
            let csd_h = self.window.csd_title_bar_height() as f64;
            if csd_h > 0.0 && self.mouse_y < csd_h {
                if button == MouseButton::Right {
                    self.window
                        .show_window_menu(self.mouse_x as i32, self.mouse_y as i32);
                    return;
                }
                if button == MouseButton::Left {
                    let win_w = self.window.width();
                    let hit = self.csd_button_layout.hit_test(
                        self.mouse_x as f32,
                        self.mouse_y as f32,
                        win_w,
                        csd_h as u32,
                    );
                    if let Some(btn_type) = hit {
                        self.pressed_csd_button = Some(btn_type);
                        self.needs_redraw = true;
                    } else {
                        let now = std::time::Instant::now();
                        let is_double_click = if let Some(last_time) = self.last_click_instant {
                            self.last_click_pos == (usize::MAX, usize::MAX)
                                && self.last_click_pane_id.is_none()
                                && last_time.elapsed().as_millis() < 400
                        } else {
                            false
                        };
                        self.last_click_instant = Some(now);
                        self.last_click_pos = (usize::MAX, usize::MAX);
                        self.last_click_pane_id = None;

                        if is_double_click {
                            self.window.toggle_maximize();
                            self.needs_redraw = true;
                        } else {
                            self.window.start_interactive_move();
                        }
                    }
                }
                return;
            }

            if self.window.uses_csd() && button == MouseButton::Left {
                let win_size = self.window.inner_size();
                let border_thickness = 5.0f32;
                if let Some(edge) = crate::window::csd::detect_resize_edge(
                    self.mouse_x as f32,
                    self.mouse_y as f32,
                    win_size.width,
                    win_size.height,
                    border_thickness,
                ) {
                    self.window.start_interactive_resize(edge);
                    return;
                }
            }

            let tab_bar_h = self.tab_bar_height() as f64;

            // ── 1. Tab Bar Mouse Clicks ──────────────────────────────────────
            if tab_bar_h > 0.0 && self.mouse_y >= csd_h && self.mouse_y < csd_h + tab_bar_h {
                let win_w = self.window.width() as f32;
                let hit = self.tab_bar.hit_test(
                    self.mouse_x as f32,
                    (self.mouse_y - csd_h) as f32,
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
                                    && self.last_click_pane_id.is_none()
                                    && last_time.elapsed().as_millis() < 400
                            } else {
                                false
                            };
                            self.last_click_instant = Some(now);
                            self.last_click_pos = (0, 0);
                            self.last_click_pane_id = None;
                            if is_double_click {
                                self.create_tab(None, None, None, None);
                            }
                        }
                    }
                    TabBarHitResult::None => {}
                }
                return;
            }

            if self.tabs.is_empty() {
                return;
            }

            // ── 2. Separator Clicks & Drag Initiation ────────────────────────
            let (pane_rects, sep_rects) = self.recalculate_panes_layout(self.active_tab_index);

            if button == MouseButton::Left {
                for sep in &sep_rects {
                    if sep.contains(self.mouse_x as f32, self.mouse_y as f32) {
                        self.is_mouse_down = true;
                        self.mouse_down_pane_id = None;
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
            }

            // ── 3. Pane Clicks & Focus ───────────────────────────────────────
            let hit_pane_rect = pane_rects
                .iter()
                .find(|r| r.contains(self.mouse_x as f32, self.mouse_y as f32))
                .copied()
                .or_else(|| pane_rects.first().copied());

            let pane_rect = match hit_pane_rect {
                Some(r) => r,
                None => return,
            };

            let clicked_pane_id = pane_rect.pane_id;
            self.mouse_down_pane_id = Some(clicked_pane_id);

            // Switch active pane to clicked pane if different
            let tab = self.active_tab_mut();
            if tab.active_pane_id != clicked_pane_id {
                tab.set_active_pane(clicked_pane_id);
                self.sync_active_pane_font_size();
                self.needs_redraw = true;
                self.content_dirty = true;
            }
            self.active_tab_mut().clear_unfocused_selections();

            let (col_idx, row_idx) = pane_cell_coords(&pane_rect, self.mouse_x, self.mouse_y);
            let grid_width = pane_rect.cols;
            let grid_height = pane_rect.rows;

            // Ctrl + Click opens link immediately without needing Shift or app mouse reporting
            if modifiers.control_key()
                && button == MouseButton::Left
                && col_idx < grid_width
                && row_idx < grid_height
            {
                let tab = self.active_tab_mut();
                if let Some(pane) = tab.tree.find_pane_mut(clicked_pane_id) {
                    let active_grid = pane.terminal.active_grid_mut();
                    let y_offset = active_grid.display_row_to_abs(row_idx);

                    if let Some(url) = active_grid.hyperlink_url_at(col_idx, y_offset) {
                        let _ = crate::hyperlink::detector::open(&url);
                        return;
                    }
                }
            }

            let (mouse_mode, mouse_proto, pty_master) = {
                let pane = match self.active_tab().tree.find_pane(clicked_pane_id) {
                    Some(p) => p,
                    None => return,
                };
                (
                    pane.terminal.mouse_mode,
                    MouseProtocol {
                        sgr: pane.terminal.mouse_sgr,
                        utf8: pane.terminal.mouse_utf8,
                        pixel: pane.terminal.mouse_pixel,
                    },
                    pane.pty_master.clone(),
                )
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
                    self.is_mouse_down = true;
                    self.last_mouse_button = btn;

                    let report_btn = apply_mouse_modifiers(btn, modifiers);
                    let (pixel_x, pixel_y) =
                        pane_pixel_coords(&pane_rect, self.mouse_x, self.mouse_y);
                    send_mouse_report(
                        &pty_master,
                        mouse_proto,
                        report_btn,
                        col_idx,
                        row_idx,
                        pixel_x,
                        pixel_y,
                        false,
                    );
                }
                return;
            }

            // Terminal Local Mouse Behavior (Selection / URL clicking / Middle-paste)
            match button {
                MouseButton::Left => {
                    self.is_mouse_down = true;
                    if col_idx < grid_width && row_idx < grid_height {
                        let now = std::time::Instant::now();
                        let is_double_click = if let Some(last_time) = self.last_click_instant {
                            self.last_click_pane_id == Some(clicked_pane_id)
                                && self.last_click_pos == (col_idx, row_idx)
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
                        self.last_click_pane_id = Some(clicked_pane_id);
                        let click_count = self.click_count;

                        let tab = self.active_tab_mut();
                        if let Some(pane) = tab.tree.find_pane_mut(clicked_pane_id) {
                            let active_grid = pane.terminal.active_grid_mut();
                            let y_offset = active_grid.display_row_to_abs(row_idx);

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

                            self.needs_redraw = true;
                        }
                    }
                }
                MouseButton::Middle => {
                    self.is_mouse_down = true;
                    self.last_mouse_button = 1;
                    let mut text = crate::clipboard::clipboard::primary_selection();
                    if text.is_empty() {
                        text = crate::clipboard::clipboard::paste();
                    }
                    if !text.is_empty() {
                        let formatted = {
                            let tab = self.active_tab_mut();
                            if let Some(pane) = tab.tree.find_pane_mut(clicked_pane_id) {
                                let scroll_on_keystroke = pane.terminal.scroll_on_keystroke;
                                let f = pane.terminal.format_paste(&text);
                                if scroll_on_keystroke {
                                    pane.terminal.active_grid_mut().scroll_offset = 0;
                                }
                                Some(f)
                            } else {
                                None
                            }
                        };
                        if let Some(formatted) = formatted {
                            self.write_to_pane(clicked_pane_id, formatted.into_bytes());
                            self.needs_redraw = true;
                        }
                    }
                }
                MouseButton::Right => {
                    self.is_mouse_down = true;
                    self.last_mouse_button = 2;
                    let tab = self.active_tab_mut();
                    if let Some(pane) = tab.tree.find_pane_mut(clicked_pane_id) {
                        let active_grid = pane.terminal.active_grid_mut();
                        if active_grid.selection.active {
                            active_grid.selection.active = false;
                            self.needs_redraw = true;
                        }
                    }
                }
                _ => {}
            }
        } else {
            // ── Mouse Button Release ─────────────────────────────────────────
            self.is_mouse_down = false;

            if let Some(pressed_btn) = self.pressed_csd_button.take() {
                self.needs_redraw = true;
                let csd_h = self.window.csd_title_bar_height() as f64;
                if csd_h > 0.0 && self.mouse_y < csd_h {
                    let win_w = self.window.width();
                    let hit = self.csd_button_layout.hit_test(
                        self.mouse_x as f32,
                        self.mouse_y as f32,
                        win_w,
                        csd_h as u32,
                    );
                    if hit == Some(pressed_btn) {
                        match pressed_btn {
                            crate::window::csd::CsdButtonType::Close => {
                                self.window.post_close_requested();
                            }
                            crate::window::csd::CsdButtonType::Maximize => {
                                self.window.toggle_maximize();
                            }
                            crate::window::csd::CsdButtonType::Minimize => {
                                self.window.minimize();
                            }
                        }
                    }
                }
                return;
            }

            if self.dragging_separator.take().is_some() {
                self.last_separator_drag_sync = None;
                self.sync_active_tab_layout();
                self.needs_redraw = true;
                self.content_dirty = true;
                return;
            }

            if self.tabs.is_empty() {
                return;
            }

            let (pane_rects, _) = self.recalculate_panes_layout(self.active_tab_index);
            let target_pane_id = self.mouse_down_pane_id.take().or_else(|| {
                pane_rects
                    .iter()
                    .find(|r| r.contains(self.mouse_x as f32, self.mouse_y as f32))
                    .map(|r| r.pane_id)
            });

            let target_pane_id = match target_pane_id {
                Some(id) => id,
                None => return,
            };

            let pane_rect = match pane_rects
                .iter()
                .find(|r| r.pane_id == target_pane_id)
                .copied()
            {
                Some(r) => r,
                None => return,
            };

            let (col_idx, row_idx) = pane_cell_coords(&pane_rect, self.mouse_x, self.mouse_y);

            let (mouse_mode, mouse_proto) = {
                let pane = match self.active_tab().tree.find_pane(target_pane_id) {
                    Some(p) => p,
                    None => return,
                };
                (
                    pane.terminal.mouse_mode,
                    MouseProtocol {
                        sgr: pane.terminal.mouse_sgr,
                        utf8: pane.terminal.mouse_utf8,
                        pixel: pane.terminal.mouse_pixel,
                    },
                )
            };

            let btn_code = match button {
                MouseButton::Left => Some(0),
                MouseButton::Middle => Some(1),
                MouseButton::Right => Some(2),
                _ => None,
            };

            // Application Mouse Reporting on Release
            if mouse_mode > 0 && !modifiers.shift_key() {
                if let Some(btn) = btn_code {
                    let report_btn = apply_mouse_modifiers(btn, modifiers);
                    let pty_master = self
                        .active_tab()
                        .tree
                        .find_pane(target_pane_id)
                        .map(|p| p.pty_master.clone());
                    if let Some(pty) = pty_master {
                        let (pixel_x, pixel_y) =
                            pane_pixel_coords(&pane_rect, self.mouse_x, self.mouse_y);
                        send_mouse_report(
                            &pty,
                            mouse_proto,
                            report_btn,
                            col_idx,
                            row_idx,
                            pixel_x,
                            pixel_y,
                            true,
                        );
                    }
                }
                return;
            }

            // Terminal Local Mouse Behavior on Release
            if button == MouseButton::Left {
                let tab = self.active_tab_mut();
                if let Some(pane) = tab.tree.find_pane_mut(target_pane_id) {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::app::App;
    use crate::cli::CliOptions;
    use crate::window::event::CursorIcon;

    #[test]
    fn test_mouse_hover_over_detected_url() {
        if let Ok(mut app) = App::new(CliOptions::default()) {
            if let Ok(wid) = app.create_window(None, None, None, None) {
                let ws = app.windows.get_mut(&wid).unwrap();
                let pane = ws.active_pane_mut();
                pane.terminal
                    .feed(b"Visit https://github.com/rust-lang/rust here\r\n");

                let (pane_rects, _) = ws.recalculate_panes_layout(ws.active_tab_index);
                let pane_rect = &pane_rects[0];
                let px = (pane_rect.x + pane_rect.padding_x) as f64;
                let py = (pane_rect.y + pane_rect.padding_y) as f64;
                let cw = pane_rect.cell_width as f64;
                let ch = pane_rect.cell_height as f64;

                // Move over non-URL text first (e.g. col 1 "isit")
                let text_x = px + cw * 1.0 + cw * 0.5;
                let text_y = py + ch * 0.0 + ch * 0.5;
                ws.handle_cursor_moved((text_x, text_y), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Text);

                // Move over URL (e.g. col 10 "https://gi...")
                let url_x = px + cw * 10.0 + cw * 0.5;
                let url_y = py + ch * 0.0 + ch * 0.5;
                ws.handle_cursor_moved((url_x, url_y), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Pointer);

                // Move over padding / window margin -> should be Default
                ws.handle_cursor_moved((px - 5.0, py + 10.0), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Default);

                // Test with Unicode prompt line containing multi-byte characters
                let pane_mut = ws.active_pane_mut();
                pane_mut
                    .terminal
                    .feed("Velox on  main ❯ echo https://velox.dev/test\r\n".as_bytes());

                let prompt_line_y = py + ch * 1.0 + ch * 0.5;
                // Move over 'V' of "Velox" (col 0) -> Text
                ws.handle_cursor_moved((px + cw * 0.5, prompt_line_y), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Text);

                // Move over 'h' of "https://velox.dev/test"
                // "Velox on  main ❯ echo " has 24 chars (cols 0..23)
                // "https://velox.dev/test" starts at col 24
                let link_x = px + cw * 25.0 + cw * 0.5;
                ws.handle_cursor_moved((link_x, prompt_line_y), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Pointer);

                // Move past the link into empty space on that row -> Text
                let past_link_x = px + cw * 50.0 + cw * 0.5;
                ws.handle_cursor_moved((past_link_x, prompt_line_y), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Text);

                // Test with a soft-wrapped URL across rows
                // Narrow the active pane or feed a long URL that wraps across lines
                let pane_mut = ws.active_pane_mut();
                let width = pane_mut.terminal.grid.width;
                // Pad so the URL starts near the end of line 2 and wraps onto line 3
                let padding_spaces = " ".repeat(width - 10);
                let wrapped_link = "https://github.com/rust-lang/rust/issues/1234567890";
                pane_mut
                    .terminal
                    .feed(format!("{}{}\r\n", padding_spaces, wrapped_link).as_bytes());

                let wrap_row_1_y = py + ch * 2.0 + ch * 0.5;
                let wrap_row_2_y = py + ch * 3.0 + ch * 0.5;

                // On wrap_row_1, col (width - 5) is within the first part of the link
                let part1_x = px + cw * (width as f64 - 5.0) + cw * 0.5;
                ws.handle_cursor_moved((part1_x, wrap_row_1_y), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Pointer);

                // On wrap_row_2, col 5 is within the continuation of the link
                let part2_x = px + cw * 5.0 + cw * 0.5;
                ws.handle_cursor_moved((part2_x, wrap_row_2_y), ModifiersState::empty());
                assert_eq!(ws.current_cursor_icon, CursorIcon::Pointer);
            }
        }
    }

    #[test]
    fn test_ctrl_click_opens_url_without_shift() {
        if let Ok(mut app) = App::new(CliOptions::default()) {
            if let Ok(wid) = app.create_window(None, None, None, None) {
                let ws = app.windows.get_mut(&wid).unwrap();
                let pane = ws.active_pane_mut();
                pane.terminal
                    .feed(b"Check https://example.com/velox here\r\n");

                let (pane_rects, _) = ws.recalculate_panes_layout(ws.active_tab_index);
                let pane_rect = &pane_rects[0];
                let px = (pane_rect.x + pane_rect.padding_x) as f64;
                let py = (pane_rect.y + pane_rect.padding_y) as f64;
                let cw = pane_rect.cell_width as f64;
                let ch = pane_rect.cell_height as f64;

                // Position over URL: "Check " is 6 chars, URL starts at col 6
                let url_x = px + cw * 10.0 + cw * 0.5;
                let url_y = py + ch * 0.0 + ch * 0.5;

                // 1. Plain left click should NOT open URL; selection becomes active
                ws.handle_cursor_moved((url_x, url_y), ModifiersState::empty());
                ws.handle_mouse_input(
                    ElementState::Pressed,
                    MouseButton::Left,
                    ModifiersState::empty(),
                );
                let pane = ws.active_pane();
                assert!(
                    pane.terminal.grid.selection.active,
                    "Plain click should start text selection, not open link"
                );

                // Clear selection
                let pane_mut = ws.active_pane_mut();
                pane_mut.terminal.grid.selection.clear();

                // 2. Ctrl + click (with NO Shift) should trigger URL open and NOT start text selection
                let ctrl_only = ModifiersState::CONTROL;
                ws.handle_mouse_input(ElementState::Pressed, MouseButton::Left, ctrl_only);
                let pane = ws.active_pane();
                assert!(
                    !pane.terminal.grid.selection.active,
                    "Ctrl+Click should open URL and not start text selection"
                );

                // 3. Test when terminal is in application mouse mode (e.g. mouse_mode = 1000)
                let pane_mut = ws.active_pane_mut();
                pane_mut.terminal.mouse_mode = 1000;
                // With Ctrl only (no Shift), Ctrl+Click should still open URL without needing Shift
                ws.handle_mouse_input(ElementState::Pressed, MouseButton::Left, ctrl_only);
                let pane = ws.active_pane();
                assert!(!pane.terminal.grid.selection.active);
            }
        }
    }

    #[test]
    fn test_mouse_reporting_protocols() {
        let mut buf = [0u8; 64];

        // 1. Legacy X10 mode
        let proto_x10 = MouseProtocol {
            sgr: false,
            utf8: false,
            pixel: false,
        };
        let len = format_mouse_report(proto_x10, 0, 5, 10, 50, 100, false, &mut buf);
        assert_eq!(&buf[..len], &[0x1b, b'[', b'M', 32, 32 + 6, 32 + 11]);

        let len_rel = format_mouse_report(proto_x10, 0, 5, 10, 50, 100, true, &mut buf);
        assert_eq!(
            &buf[..len_rel],
            &[0x1b, b'[', b'M', 32 + 3, 32 + 6, 32 + 11]
        );

        // 2. SGR 1006 mode
        let proto_sgr = MouseProtocol {
            sgr: true,
            utf8: false,
            pixel: false,
        };
        let len_sgr = format_mouse_report(proto_sgr, 0, 5, 10, 50, 100, false, &mut buf);
        assert_eq!(
            std::str::from_utf8(&buf[..len_sgr]).unwrap(),
            "\x1b[<0;6;11M"
        );

        let len_sgr_rel = format_mouse_report(proto_sgr, 0, 5, 10, 50, 100, true, &mut buf);
        assert_eq!(
            std::str::from_utf8(&buf[..len_sgr_rel]).unwrap(),
            "\x1b[<0;6;11m"
        );

        // 3. UTF-8 1005 mode (column > 223 encoding past single ASCII byte)
        let proto_utf8 = MouseProtocol {
            sgr: false,
            utf8: true,
            pixel: false,
        };
        let len_utf8 = format_mouse_report(proto_utf8, 0, 250, 10, 2500, 100, false, &mut buf);
        let s = std::str::from_utf8(&buf[..len_utf8]).unwrap();
        assert!(s.starts_with("\x1b[M"));
        let chars: Vec<char> = s[3..].chars().collect();
        assert_eq!(chars.len(), 3);
        assert_eq!(chars[0] as u32, 32);
        assert_eq!(chars[1] as u32, 32 + 250 + 1);
        assert_eq!(chars[2] as u32, 32 + 10 + 1);

        // 4. SGR pixel 1016 mode
        let proto_pixel = MouseProtocol {
            sgr: false,
            utf8: false,
            pixel: true,
        };
        let len_pixel = format_mouse_report(proto_pixel, 0, 5, 10, 120, 340, false, &mut buf);
        assert_eq!(
            std::str::from_utf8(&buf[..len_pixel]).unwrap(),
            "\x1b[<0;121;341M"
        );

        let len_pixel_rel = format_mouse_report(proto_pixel, 0, 5, 10, 120, 340, true, &mut buf);
        assert_eq!(
            std::str::from_utf8(&buf[..len_pixel_rel]).unwrap(),
            "\x1b[<0;121;341m"
        );
    }
}
