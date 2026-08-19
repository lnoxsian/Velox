use crate::app::app::WindowState;
use winit::event::KeyEvent;
use winit::keyboard::ModifiersState;

impl WindowState {
    pub fn handle_keyboard_input(&mut self, event: KeyEvent, modifiers: ModifiersState) {
        if event.state.is_pressed() {
            // 1. Copy (Ctrl+Shift+C) and Paste (Ctrl+Shift+V)
            if modifiers.control_key() && modifiers.shift_key() {
                let key_ch = match &event.logical_key {
                    winit::keyboard::Key::Character(s) => Some(s.to_lowercase()),
                    _ => None,
                };
                if let Some(ch) = key_ch {
                    if ch == "c" {
                        let active_grid = self.terminal.active_grid();
                        let text = if active_grid.selection.active {
                            active_grid.extract_selection_text()
                        } else {
                            (0..active_grid.height)
                                .map(|y| {
                                    (0..active_grid.width)
                                        .map(|x| {
                                            active_grid.cells[y * active_grid.width + x].character
                                        })
                                        .collect::<String>()
                                        .trim_end()
                                        .to_string()
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        };
                        if !text.is_empty() {
                            crate::clipboard::clipboard::copy(text);
                        }
                        return;
                    } else if ch == "v" {
                        let text = crate::clipboard::clipboard::paste();
                        if !text.is_empty() {
                            let formatted = self.terminal.format_paste(&text);
                            if self.terminal.scroll_on_keystroke {
                                self.terminal.active_grid_mut().scroll_offset = 0;
                                self.needs_redraw = true;
                            }
                            let _ = self.pty_master.write(formatted.as_bytes());
                        }
                        return;
                    }
                }
            }

            // 2. Font Zoom In (Ctrl+Shift++ / Ctrl+Shift+=), Zoom Out (Ctrl+-), Reset (Ctrl+0)
            let is_zoom_in = modifiers.control_key()
                && modifiers.shift_key()
                && matches!(&event.logical_key, winit::keyboard::Key::Character(s) if s == "+" || s == "=");

            let is_zoom_out = modifiers.control_key()
                && !modifiers.shift_key()
                && !modifiers.alt_key()
                && matches!(&event.logical_key, winit::keyboard::Key::Character(s) if s == "-");

            let is_zoom_reset = modifiers.control_key()
                && !modifiers.shift_key()
                && !modifiers.alt_key()
                && matches!(&event.logical_key, winit::keyboard::Key::Character(s) if s == "0");

            if is_zoom_in || is_zoom_out || is_zoom_reset {
                let new_size = if is_zoom_in {
                    (self.current_font_size + 1.0).min(72.0)
                } else if is_zoom_out {
                    (self.current_font_size - 1.0).max(4.0)
                } else {
                    self.default_font_size
                };

                if (new_size - self.current_font_size).abs() > 0.01 {
                    self.current_font_size = new_size;
                    self.set_font_size(new_size);
                    let size = self.window.inner_size();
                    let avail_w = (size.width as f32 - self.padding_x * 2.0).max(10.0);
                    let avail_h = (size.height as f32 - self.padding_y * 2.0).max(10.0);
                    let cols = ((avail_w as u32) / self.cell_width()).max(20);
                    let rows = ((avail_h as u32) / self.cell_height()).max(10);
                    self.terminal.resize(cols, rows);
                    let _ = self.pty_master.resize(cols as u16, rows as u16);
                    self.needs_redraw = true;
                    self.content_dirty = true;
                }
                return;
            }

            if modifiers.shift_key() {
                if let winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageUp) =
                    event.logical_key
                {
                    let active_grid = if self.terminal.is_alt_screen {
                        &mut self.terminal.alt_grid
                    } else {
                        &mut self.terminal.grid
                    };
                    let history_len = active_grid.scrollback.len();
                    active_grid.scroll_offset =
                        (active_grid.scroll_offset + active_grid.height / 2).min(history_len);
                    self.needs_redraw = true;
                    return;
                } else if let winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageDown) =
                    event.logical_key
                {
                    let active_grid = if self.terminal.is_alt_screen {
                        &mut self.terminal.alt_grid
                    } else {
                        &mut self.terminal.grid
                    };
                    active_grid.scroll_offset = active_grid
                        .scroll_offset
                        .saturating_sub(active_grid.height / 2);
                    self.needs_redraw = true;
                    return;
                }
            }

            // Clear active text selection when typing input into the PTY
            let cursor_keys_mode = self.terminal.cursor_keys_mode;
            if let Some(bytes) = crate::input::keyboard::translate_key(
                &event.logical_key,
                modifiers,
                cursor_keys_mode,
            ) {
                let scroll_on_keystroke = self.terminal.scroll_on_keystroke;
                let active_grid = self.terminal.active_grid_mut();
                if active_grid.selection.active {
                    active_grid.selection.clear();
                    self.needs_redraw = true;
                }
                if scroll_on_keystroke && active_grid.scroll_offset > 0 {
                    active_grid.scroll_offset = 0;
                    self.needs_redraw = true;
                }
                let _ = self.pty_master.write(&bytes);
            }
        }
    }
}
