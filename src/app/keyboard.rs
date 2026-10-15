use crate::app::app::WindowState;
use crate::app::split::FocusDirection;
use crate::window::event::{Key, ModifiersState, NamedKey};

impl WindowState {
    pub fn handle_keyboard_input(
        &mut self,
        logical_key: &Key,
        text: Option<&str>,
        modifiers: ModifiersState,
    ) -> bool {
        self.mark_interaction();

            // ── Split Pane Divider Resizing (Ctrl+Alt+Arrows) ────────────────
            if modifiers.control_key() && modifiers.alt_key() {
                match logical_key {
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowUp) => {
                        if self.adjust_active_split_ratio(-0.05) {
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowDown)
                        if self.adjust_active_split_ratio(0.05) =>
                    {
                        return false;
                    }
                    _ => {}
                }
            }

            // ── Split Pane Navigation (Alt+Arrows) ────────────────────────────
            if modifiers.alt_key() && !modifiers.control_key() && !modifiers.shift_key() {
                match logical_key {
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.focus_direction(FocusDirection::Left) {
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.focus_direction(FocusDirection::Right) {
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.focus_direction(FocusDirection::Up) {
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown)
                        if self.focus_direction(FocusDirection::Down) =>
                    {
                        return false;
                    }
                    _ => {}
                }
            }

            // ── Tab Management & Split Shortcuts (Ctrl+Shift) ─────────────────
            if modifiers.control_key() && modifiers.shift_key() {
                if let Key::Character(s) = &logical_key {
                    let ch = s.to_lowercase();
                    match ch.as_str() {
                        "t" => {
                            self.create_tab(None, None, None, None);
                            return false;
                        }
                        "w" => {
                            self.close_pane();
                            return false;
                        }
                        "o" | "h" => {
                            self.split_horizontal();
                            return false;
                        }
                        "e" | "d" => {
                            self.split_vertical();
                            return false;
                        }
                        "n" => {
                            self.focus_next_pane();
                            return false;
                        }
                        "p" => {
                            self.focus_previous_pane();
                            return false;
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
                            return false;
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
                                self.last_key_instant = std::time::Instant::now();
                                self.mark_interaction();
                                return true;
                            }
                            return false;
                        }
                        "+" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = active_size + 1.0;
                            self.set_font_size(new_size);
                            return false;
                        }
                        _ => {}
                    }
                }

                match logical_key {
                    Key::Named(NamedKey::Tab) => {
                        self.prev_tab();
                        return false;
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if !self.focus_direction(FocusDirection::Left) && self.active_tab_index > 0
                        {
                            self.move_tab(self.active_tab_index, self.active_tab_index - 1);
                        }
                        return false;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if !self.focus_direction(FocusDirection::Right)
                            && self.active_tab_index + 1 < self.tabs.len()
                        {
                            self.move_tab(self.active_tab_index, self.active_tab_index + 1);
                        }
                        return false;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.focus_direction(FocusDirection::Up) {
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown)
                        if self.focus_direction(FocusDirection::Down) =>
                    {
                        return false;
                    }
                    _ => {}
                }
            }

            if modifiers.control_key() && !modifiers.shift_key() && !modifiers.alt_key() {
                if let Key::Named(NamedKey::Tab) = logical_key {
                    self.next_tab();
                    return false;
                } else if let Key::Named(NamedKey::PageDown) = logical_key {
                    self.next_tab();
                    return false;
                } else if let Key::Named(NamedKey::PageUp) = logical_key {
                    self.prev_tab();
                    return false;
                }

                if let Key::Character(s) = logical_key {
                    match s.as_str() {
                        "+" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = active_size + 1.0;
                            self.set_font_size(new_size);
                            return false;
                        }
                        "-" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = active_size - 1.0;
                            self.set_font_size(new_size);
                            return false;
                        }
                        "0" => {
                            let new_size = self.default_font_size;
                            self.set_font_size(new_size);
                            return false;
                        }
                        _ => {}
                    }
                }
            }

            if modifiers.shift_key() {
                if let Key::Named(NamedKey::PageUp) = logical_key {
                    let active_pane = self.active_pane_mut();
                    let active_grid = if active_pane.terminal.is_alt_screen {
                        &mut active_pane.terminal.alt_grid
                    } else {
                        &mut active_pane.terminal.grid
                    };
                    let history_len = active_grid.scrollback.len();
                    active_grid.scroll_offset =
                        (active_grid.scroll_offset + active_grid.height / 2).min(history_len);
                    active_grid.damage.mark_all();
                    self.needs_redraw = true;
                    return false;
                } else if let Key::Named(NamedKey::PageDown) = logical_key {
                    let active_pane = self.active_pane_mut();
                    let active_grid = if active_pane.terminal.is_alt_screen {
                        &mut active_pane.terminal.alt_grid
                    } else {
                        &mut active_pane.terminal.grid
                    };
                    active_grid.scroll_offset = active_grid
                        .scroll_offset
                        .saturating_sub(active_grid.height / 2);
                    active_grid.damage.mark_all();
                    self.needs_redraw = true;
                    return false;
                }
            }

            // ── 3. Normal Typing / PTY Input to Active Pane ───────────────────
            let cursor_keys_mode = self.active_pane().terminal.cursor_keys_mode;
            let kitty_flags = self.active_pane().terminal.kitty_keyboard_flags;
            if let Some(bytes) = crate::input::keyboard::translate_key(
                logical_key,
                text,
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
                    active_grid.damage.mark_all();
                    redraw = true;
                }
                let _ = active_pane.pty_master.write(&bytes);
                self.last_key_instant = std::time::Instant::now();
                self.mark_interaction();
                if redraw {
                    self.needs_redraw = true;
                }
                return true;
            }
            false
    }
}

