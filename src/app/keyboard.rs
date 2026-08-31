use crate::app::app::WindowState;
use crate::app::split::FocusDirection;
use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

impl WindowState {
    pub fn handle_keyboard_input(&mut self, event: KeyEvent, modifiers: ModifiersState) {
        if event.state.is_pressed() {
            self.mark_interaction();
            if self.hide_mouse_on_typing {
                self.window.set_cursor_visible(false);
            }

            // ── Split Pane Divider Resizing (Ctrl+Alt+Arrows) ────────────────
            if modifiers.control_key() && modifiers.alt_key() {
                match event.logical_key {
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowUp) => {
                        if self.adjust_active_split_ratio(-0.05) {
                            return;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowDown)
                        if self.adjust_active_split_ratio(0.05) =>
                    {
                        return;
                    }
                    _ => {}
                }
            }

            // ── Split Pane Navigation (Alt+Arrows) ────────────────────────────
            if modifiers.alt_key() && !modifiers.control_key() && !modifiers.shift_key() {
                match event.logical_key {
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.focus_direction(FocusDirection::Left) {
                            return;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.focus_direction(FocusDirection::Right) {
                            return;
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.focus_direction(FocusDirection::Up) {
                            return;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown)
                        if self.focus_direction(FocusDirection::Down) =>
                    {
                        return;
                    }
                    _ => {}
                }
            }

            // ── Tab Management & Split Shortcuts (Ctrl+Shift) ─────────────────
            if modifiers.control_key() && modifiers.shift_key() {
                if let Key::Character(s) = &event.logical_key {
                    let ch = s.to_lowercase();
                    match ch.as_str() {
                        "t" => {
                            self.create_tab(None, None, None, None);
                            return;
                        }
                        "w" => {
                            self.close_pane();
                            return;
                        }
                        "o" | "h" => {
                            self.split_horizontal();
                            return;
                        }
                        "e" | "d" => {
                            self.split_vertical();
                            return;
                        }
                        "n" => {
                            self.focus_next_pane();
                            return;
                        }
                        "p" => {
                            self.focus_previous_pane();
                            return;
                        }
                        "c" => {
                            let active_pane = self.active_pane();
                            let active_grid = active_pane.terminal.active_grid();
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
                                let formatted = self.active_pane().terminal.format_paste(&text);
                                if self.active_pane().terminal.scroll_on_keystroke {
                                    self.active_pane_mut()
                                        .terminal
                                        .active_grid_mut()
                                        .scroll_offset = 0;
                                    self.needs_redraw = true;
                                }
                                let _ = self.active_pane().pty_master.write(formatted.as_bytes());
                            }
                            return;
                        }
                        "+" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = active_size + 1.0;
                            if (new_size - active_size).abs() > 0.01 {
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
                        if !self.focus_direction(FocusDirection::Left) && self.active_tab_index > 0
                        {
                            self.move_tab(self.active_tab_index, self.active_tab_index - 1);
                        }
                        return;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if !self.focus_direction(FocusDirection::Right)
                            && self.active_tab_index + 1 < self.tabs.len()
                        {
                            self.move_tab(self.active_tab_index, self.active_tab_index + 1);
                        }
                        return;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.focus_direction(FocusDirection::Up) {
                            return;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown)
                        if self.focus_direction(FocusDirection::Down) =>
                    {
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
                        "+" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = active_size + 1.0;
                            if (new_size - active_size).abs() > 0.01 {
                                self.set_font_size(new_size);
                            }
                            return;
                        }
                        "-" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = (active_size - 1.0).max(1.0);
                            if (new_size - active_size).abs() > 0.01 {
                                self.set_font_size(new_size);
                            }
                            return;
                        }
                        "0" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = self.default_font_size;
                            if (new_size - active_size).abs() > 0.01 {
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
                    let active_pane = self.active_pane_mut();
                    let active_grid = if active_pane.terminal.is_alt_screen {
                        &mut active_pane.terminal.alt_grid
                    } else {
                        &mut active_pane.terminal.grid
                    };
                    let history_len = active_grid.scrollback.len();
                    active_grid.scroll_offset =
                        (active_grid.scroll_offset + active_grid.height / 2).min(history_len);
                    self.needs_redraw = true;
                    return;
                } else if let Key::Named(NamedKey::PageDown) = event.logical_key {
                    let active_pane = self.active_pane_mut();
                    let active_grid = if active_pane.terminal.is_alt_screen {
                        &mut active_pane.terminal.alt_grid
                    } else {
                        &mut active_pane.terminal.grid
                    };
                    active_grid.scroll_offset = active_grid
                        .scroll_offset
                        .saturating_sub(active_grid.height / 2);
                    self.needs_redraw = true;
                    return;
                }
            }

            // ── 3. Normal Typing / PTY Input to Active Pane ───────────────────
            let cursor_keys_mode = self.active_pane().terminal.cursor_keys_mode;
            let kitty_flags = self.active_pane().terminal.kitty_keyboard_flags;
            if let Some(bytes) = crate::input::keyboard::translate_key(
                &event.logical_key,
                event.text.as_deref(),
                modifiers,
                cursor_keys_mode,
                kitty_flags,
            ) {
                let scroll_on_keystroke = self.active_pane().terminal.scroll_on_keystroke;
                let active_pane = self.active_pane_mut();
                let active_grid = active_pane.terminal.active_grid_mut();
                let mut redraw = false;
                if active_grid.selection.active {
                    active_grid.selection.clear();
                    redraw = true;
                }
                if scroll_on_keystroke && active_grid.scroll_offset > 0 {
                    active_grid.scroll_offset = 0;
                    redraw = true;
                }
                let _ = active_pane.pty_master.write(&bytes);
                if redraw {
                    self.needs_redraw = true;
                }
            }
        }
    }
}
