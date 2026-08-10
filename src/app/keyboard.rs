use winit::event::KeyEvent;
use crate::app::app::App;

impl App {
    pub fn handle_keyboard_input(&mut self, event: KeyEvent) {
        if event.state.is_pressed() {
            // 1. Copy (Ctrl+Shift+C) and Paste (Ctrl+Shift+V)
            if self.modifiers.control_key() && self.modifiers.shift_key() {
                let key_ch = match &event.logical_key {
                    winit::keyboard::Key::Character(s) => Some(s.to_lowercase()),
                    _ => None,
                };
                if let Some(ch) = key_ch {
                    if ch == "c" {
                        if let Some(terminal) = &self.terminal {
                            let active_grid = terminal.active_grid();
                            let text = if active_grid.selection.active {
                                active_grid.selection.extract_text(active_grid.width, active_grid.height, &active_grid.cells)
                            } else {
                                (0..active_grid.height)
                                    .map(|y| {
                                        (0..active_grid.width)
                                            .map(|x| active_grid.cells[y * active_grid.width + x].character)
                                            .collect::<String>()
                                            .trim_end()
                                            .to_string()
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            };
                            if !text.is_empty() {
                                crate::clipboard::clipboard::copy(&text);
                            }
                        }
                        return;
                    } else if ch == "v" {
                        let text = crate::clipboard::clipboard::paste();
                        if !text.is_empty()
                            && let Some(pty_master) = &self.pty_master {
                                let formatted = if let Some(term) = &self.terminal {
                                    term.format_paste(&text)
                                } else {
                                    text
                                };
                                let _ = pty_master.write(formatted.as_bytes());
                            }
                        return;
                    }
                }
            }

            // 2. Font Zoom In (Ctrl+Shift++ / Ctrl+Shift+=), Zoom Out (Ctrl+-), Reset (Ctrl+0)
            let is_zoom_in = self.modifiers.control_key() && self.modifiers.shift_key() &&
                matches!(&event.logical_key, winit::keyboard::Key::Character(s) if s == "+" || s == "=");

            let is_zoom_out = self.modifiers.control_key() && !self.modifiers.shift_key() && !self.modifiers.alt_key() &&
                matches!(&event.logical_key, winit::keyboard::Key::Character(s) if s == "-");

            let is_zoom_reset = self.modifiers.control_key() && !self.modifiers.shift_key() && !self.modifiers.alt_key() &&
                matches!(&event.logical_key, winit::keyboard::Key::Character(s) if s == "0");

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
                    if let (Some(window), Some(renderer), Some(terminal), Some(pty_master)) = 
                       (&self.window, &mut self.renderer, &mut self.terminal, &self.pty_master) 
                    {
                        renderer.set_font_size(new_size);
                        let size = window.inner_size();
                        let avail_w = (size.width as f32 - self.padding_x * 2.0).max(10.0);
                        let avail_h = (size.height as f32 - self.padding_y * 2.0).max(10.0);
                        let cols = ((avail_w as u32) / renderer.font_loader.cell_width).max(20);
                        let rows = ((avail_h as u32) / renderer.font_loader.cell_height).max(10);
                        terminal.resize(cols, rows);
                        let _ = pty_master.resize(cols as u16, rows as u16);
                        self.needs_redraw = true;
                        self.content_dirty = true;
                    }
                }
                return;
            }

            if self.modifiers.shift_key() {
                if let winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageUp) = event.logical_key {
                    if let Some(terminal) = &mut self.terminal {
                        let active_grid = if terminal.is_alt_screen { &mut terminal.alt_grid } else { &mut terminal.grid };
                        let history_len = active_grid.scrollback.len();
                        active_grid.scroll_offset = (active_grid.scroll_offset + active_grid.height / 2).min(history_len);
                        self.needs_redraw = true;
                        return;
                    }
                } else if let winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageDown) = event.logical_key
                    && let Some(terminal) = &mut self.terminal {
                        let active_grid = if terminal.is_alt_screen { &mut terminal.alt_grid } else { &mut terminal.grid };
                        active_grid.scroll_offset = active_grid.scroll_offset.saturating_sub(active_grid.height / 2);
                        self.needs_redraw = true;
                        return;
                    }
            }

            // Clear active text selection when typing input into the PTY
            if let Some(pty_master) = &self.pty_master {
                let cursor_keys_mode = self.terminal.as_ref().map(|t| t.cursor_keys_mode).unwrap_or(false);
                if let Some(bytes) = crate::input::keyboard::translate_key(&event.logical_key, self.modifiers, cursor_keys_mode) {
                    if let Some(terminal) = &mut self.terminal {
                        let active_grid = terminal.active_grid_mut();
                        if active_grid.selection.active {
                            active_grid.selection.clear();
                            self.needs_redraw = true;
                        }
                    }
                    let _ = pty_master.write(&bytes);
                }
            }
        }
    }
}
