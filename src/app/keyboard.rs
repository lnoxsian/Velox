use crate::app::app::WindowState;
use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

impl WindowState {
    pub fn handle_keyboard_input(&mut self, event: KeyEvent, modifiers: ModifiersState) {
        if event.state.is_pressed() {
            // ── Tab Management & Common Shortcuts (Ctrl+Shift) ───────────────
            if modifiers.control_key() && modifiers.shift_key() {
                if let Key::Character(s) = &event.logical_key {
                    let ch = s.to_lowercase();
                    match ch.as_str() {
                        "t" => {
                            self.create_tab(None, None, None, None);
                            return;
                        }
                        "w" => {
                            self.close_tab(self.active_tab_index);
                            return;
                        }
                        "c" => {
                            let active_tab = self.active_tab();
                            let active_grid = active_tab.terminal.active_grid();
                            let text = if active_grid.selection.active {
                                active_grid.extract_selection_text()
                            } else {
                                (0..active_grid.height)
                                    .map(|y| {
                                        (0..active_grid.width)
                                            .map(|x| {
                                                active_grid.cells[y * active_grid.width + x]
                                                    .character
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
                        }
                        "v" => {
                            let text = crate::clipboard::clipboard::paste();
                            if !text.is_empty() {
                                let formatted = self.active_tab().terminal.format_paste(&text);
                                if self.active_tab().terminal.scroll_on_keystroke {
                                    self.active_tab_mut()
                                        .terminal
                                        .active_grid_mut()
                                        .scroll_offset = 0;
                                    self.needs_redraw = true;
                                }
                                let _ = self.active_tab().pty_master.write(formatted.as_bytes());
                            }
                            return;
                        }
                        "+" | "=" => {
                            let new_size = (self.current_font_size + 1.0).min(72.0);
                            if (new_size - self.current_font_size).abs() > 0.01 {
                                self.current_font_size = new_size;
                                self.set_font_size(new_size);
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                match event.logical_key {
                    Key::Named(NamedKey::Tab) => {
                        self.prev_tab();
                        return;
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.active_tab_index > 0 {
                            self.move_tab(self.active_tab_index, self.active_tab_index - 1);
                        }
                        return;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.active_tab_index + 1 < self.tabs.len() {
                            self.move_tab(self.active_tab_index, self.active_tab_index + 1);
                        }
                        return;
                    }
                    _ => {}
                }
            }

            if modifiers.control_key() && !modifiers.shift_key() && !modifiers.alt_key() {
                if let Key::Named(NamedKey::Tab) = event.logical_key {
                    self.next_tab();
                    return;
                } else if let Key::Named(NamedKey::PageDown) = event.logical_key {
                    self.next_tab();
                    return;
                } else if let Key::Named(NamedKey::PageUp) = event.logical_key {
                    self.prev_tab();
                    return;
                }

                if let Key::Character(ref s) = event.logical_key {
                    match s.as_str() {
                        "1" => {
                            self.switch_tab(0);
                            return;
                        }
                        "2" => {
                            self.switch_tab(1);
                            return;
                        }
                        "3" => {
                            self.switch_tab(2);
                            return;
                        }
                        "4" => {
                            self.switch_tab(3);
                            return;
                        }
                        "5" => {
                            self.switch_tab(4);
                            return;
                        }
                        "6" => {
                            self.switch_tab(5);
                            return;
                        }
                        "7" => {
                            self.switch_tab(6);
                            return;
                        }
                        "8" => {
                            self.switch_tab(7);
                            return;
                        }
                        "9" => {
                            if !self.tabs.is_empty() {
                                self.switch_tab(self.tabs.len() - 1);
                            }
                            return;
                        }
                        "-" => {
                            let new_size = (self.current_font_size - 1.0).max(4.0);
                            if (new_size - self.current_font_size).abs() > 0.01 {
                                self.current_font_size = new_size;
                                self.set_font_size(new_size);
                            }
                            return;
                        }
                        "0" => {
                            let new_size = self.default_font_size;
                            if (new_size - self.current_font_size).abs() > 0.01 {
                                self.current_font_size = new_size;
                                self.set_font_size(new_size);
                            }
                            return;
                        }
                        _ => {}
                    }
                }
            }

            if modifiers.shift_key() {
                if let Key::Named(NamedKey::PageUp) = event.logical_key {
                    let active_tab = self.active_tab_mut();
                    let active_grid = if active_tab.terminal.is_alt_screen {
                        &mut active_tab.terminal.alt_grid
                    } else {
                        &mut active_tab.terminal.grid
                    };
                    let history_len = active_grid.scrollback.len();
                    active_grid.scroll_offset =
                        (active_grid.scroll_offset + active_grid.height / 2).min(history_len);
                    self.needs_redraw = true;
                    return;
                } else if let Key::Named(NamedKey::PageDown) = event.logical_key {
                    let active_tab = self.active_tab_mut();
                    let active_grid = if active_tab.terminal.is_alt_screen {
                        &mut active_tab.terminal.alt_grid
                    } else {
                        &mut active_tab.terminal.grid
                    };
                    active_grid.scroll_offset = active_grid
                        .scroll_offset
                        .saturating_sub(active_grid.height / 2);
                    self.needs_redraw = true;
                    return;
                }
            }

            // ── 3. Normal Typing / PTY Input ─────────────────────────────────
            let cursor_keys_mode = self.active_tab().terminal.cursor_keys_mode;
            if let Some(bytes) = crate::input::keyboard::translate_key(
                &event.logical_key,
                modifiers,
                cursor_keys_mode,
            ) {
                let scroll_on_keystroke = self.active_tab().terminal.scroll_on_keystroke;
                let active_tab = self.active_tab_mut();
                let active_grid = active_tab.terminal.active_grid_mut();
                let mut redraw = false;
                if active_grid.selection.active {
                    active_grid.selection.clear();
                    redraw = true;
                }
                if scroll_on_keystroke && active_grid.scroll_offset > 0 {
                    active_grid.scroll_offset = 0;
                    redraw = true;
                }
                let _ = active_tab.pty_master.write(&bytes);
                if redraw {
                    self.needs_redraw = true;
                }
            }
        }
    }
}
