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

        log::debug!(
            "handle_keyboard_input: logical_key={:?}, text={:?}, modifiers={:?}",
            logical_key,
            text,
            modifiers
        );

        // ── 1. Velox-Owned Shortcuts ──────────────────────────────────────────

        // Split Pane Divider Resizing (Ctrl+Alt+Arrows without Shift)
        if modifiers.control_key() && modifiers.alt_key() && !modifiers.shift_key() {
            match logical_key {
                Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowUp) => {
                    self.adjust_active_split_ratio(-0.05);
                    log::debug!("Velox shortcut: split resize -0.05");
                    return false;
                }
                Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowDown) => {
                    self.adjust_active_split_ratio(0.05);
                    log::debug!("Velox shortcut: split resize +0.05");
                    return false;
                }
                _ => {}
            }
        }

        // Split Pane Navigation (Alt+Arrows without Ctrl/Shift)
        if modifiers.alt_key() && !modifiers.control_key() && !modifiers.shift_key() {
            match logical_key {
                Key::Named(NamedKey::ArrowLeft) => {
                    self.focus_direction(FocusDirection::Left);
                    log::debug!("Velox shortcut: focus left");
                    return false;
                }
                Key::Named(NamedKey::ArrowRight) => {
                    self.focus_direction(FocusDirection::Right);
                    log::debug!("Velox shortcut: focus right");
                    return false;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.focus_direction(FocusDirection::Up);
                    log::debug!("Velox shortcut: focus up");
                    return false;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.focus_direction(FocusDirection::Down);
                    log::debug!("Velox shortcut: focus down");
                    return false;
                }
                _ => {}
            }

            // Alt+1 .. Alt+9: Switch to Tab N
            if let Key::Character(s) = logical_key {
                if s.len() == 1 {
                    let ch = s.chars().next().unwrap();
                    if ('1'..='9').contains(&ch) {
                        let idx = (ch as usize) - ('1' as usize);
                        self.switch_tab(idx);
                        log::debug!("Velox shortcut: switch to tab {}", idx);
                        return false;
                    }
                }
            }
        }

        // Tab Management & Split Shortcuts (Ctrl+Shift without Alt)
        if modifiers.control_key() && modifiers.shift_key() && !modifiers.alt_key() {
            if let Key::Character(s) = &logical_key {
                let ch = s.to_lowercase();
                match ch.as_str() {
                    "t" => {
                        self.create_tab(None, None, None, None);
                        log::debug!("Velox shortcut: create tab");
                        return false;
                    }
                    "w" => {
                        self.close_pane();
                        log::debug!("Velox shortcut: close pane");
                        return false;
                    }
                    "o" | "h" => {
                        self.split_horizontal();
                        log::debug!("Velox shortcut: split horizontal");
                        return false;
                    }
                    "e" | "d" => {
                        self.split_vertical();
                        log::debug!("Velox shortcut: split vertical");
                        return false;
                    }
                    "n" => {
                        self.focus_next_pane();
                        log::debug!("Velox shortcut: focus next pane");
                        return false;
                    }
                    "p" => {
                        self.focus_previous_pane();
                        log::debug!("Velox shortcut: focus previous pane");
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
                        log::debug!("Velox shortcut: copy");
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
                            log::debug!("Velox shortcut: paste");
                            return true;
                        }
                        return false;
                    }
                    "+" | "=" => {
                        let active_size = self.active_pane().font_size;
                        let new_size = active_size + 1.0;
                        self.set_font_size(new_size);
                        log::debug!("Velox shortcut: zoom in");
                        return false;
                    }
                    "_" | "-" => {
                        let active_size = self.active_pane().font_size;
                        let new_size = (active_size - 1.0).max(4.0);
                        self.set_font_size(new_size);
                        log::debug!("Velox shortcut: zoom out");
                        return false;
                    }
                    "]" | "}" => {
                        self.next_tab();
                        log::debug!("Velox shortcut: next tab");
                        return false;
                    }
                    "[" | "{" => {
                        self.prev_tab();
                        log::debug!("Velox shortcut: prev tab");
                        return false;
                    }
                    _ => {}
                }
            }

            match logical_key {
                Key::Named(NamedKey::Tab) => {
                    self.prev_tab();
                    log::debug!("Velox shortcut: prev tab (Ctrl+Shift+Tab)");
                    return false;
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    if self.active_tab_index > 0 {
                        self.move_tab(self.active_tab_index, self.active_tab_index - 1);
                    }
                    log::debug!("Velox shortcut: move tab left");
                    return false;
                }
                Key::Named(NamedKey::ArrowRight) => {
                    if self.active_tab_index + 1 < self.tabs.len() {
                        self.move_tab(self.active_tab_index, self.active_tab_index + 1);
                    }
                    log::debug!("Velox shortcut: move tab right");
                    return false;
                }
                _ => {}
            }
        }

        // Single Ctrl shortcuts (Ctrl without Shift or Alt)
        if modifiers.control_key() && !modifiers.shift_key() && !modifiers.alt_key() {
            match logical_key {
                Key::Named(NamedKey::Tab) | Key::Named(NamedKey::PageDown) => {
                    self.next_tab();
                    log::debug!("Velox shortcut: next tab (Ctrl+Tab/PageDown)");
                    return false;
                }
                Key::Named(NamedKey::PageUp) => {
                    self.prev_tab();
                    log::debug!("Velox shortcut: prev tab (Ctrl+PageUp)");
                    return false;
                }
                _ => {}
            }

            if let Key::Character(s) = logical_key {
                match s.as_str() {
                    "+" | "=" => {
                        let active_size = self.active_pane().font_size;
                        let new_size = active_size + 1.0;
                        self.set_font_size(new_size);
                        log::debug!("Velox shortcut: zoom in");
                        return false;
                    }
                    "-" => {
                        let active_size = self.active_pane().font_size;
                        let new_size = (active_size - 1.0).max(4.0);
                        self.set_font_size(new_size);
                        log::debug!("Velox shortcut: zoom out");
                        return false;
                    }
                    "0" => {
                        let new_size = self.default_font_size;
                        self.set_font_size(new_size);
                        log::debug!("Velox shortcut: reset zoom");
                        return false;
                    }
                    _ => {}
                }
            }
        }

        // Single Shift shortcuts (Shift without Ctrl or Alt)
        if modifiers.shift_key() && !modifiers.control_key() && !modifiers.alt_key() {
            match logical_key {
                Key::Named(NamedKey::PageUp) => {
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
                    log::debug!("Velox shortcut: scroll half page up");
                    return false;
                }
                Key::Named(NamedKey::PageDown) => {
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
                    log::debug!("Velox shortcut: scroll half page down");
                    return false;
                }
                _ => {}
            }
        }

        // ── 2. PTY Input Translation and Dispatch ─────────────────────────────
        let cursor_keys_mode = self.active_pane().terminal.cursor_keys_mode;
        let kitty_flags = self.active_pane().terminal.kitty_keyboard_flags;
        if let Some(bytes) = crate::input::keyboard::translate_key(
            logical_key,
            text,
            modifiers,
            cursor_keys_mode,
            kitty_flags,
        ) {
            log::debug!("PTY write: {} bytes: {:?}", bytes.len(), bytes.as_slice());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::master::PtyMaster;
    use std::sync::Arc;

    fn create_test_pty_pipe() -> (Arc<PtyMaster>, std::os::fd::OwnedFd) {
        let mut fds = [0; 2];
        unsafe {
            assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
            let flags = libc::fcntl(fds[0], libc::F_GETFL);
            libc::fcntl(fds[0], libc::F_SETFL, flags | libc::O_NONBLOCK);
            use std::os::fd::FromRawFd;
            let read_end = std::os::fd::OwnedFd::from_raw_fd(fds[0]);
            let pty = Arc::new(PtyMaster::new(fds[1], None));
            (pty, read_end)
        }
    }

    fn read_pty_output(fd: &std::os::fd::OwnedFd) -> Vec<u8> {
        use std::os::fd::AsRawFd;
        let mut buf = [0u8; 1024];
        let n = unsafe {
            libc::read(
                fd.as_raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n > 0 {
            buf[..n as usize].to_vec()
        } else {
            Vec::new()
        }
    }

    #[test]
    fn test_ctrl_tab_and_ctrl_shift_tab_consumed() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Ctrl+Tab consumed for next tab
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::Tab), None, ModifiersState::CONTROL);
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");

        // Ctrl+Shift+Tab consumed for prev tab
        let handled = ws.handle_keyboard_input(
            &Key::Named(NamedKey::Tab),
            None,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        );
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");
    }

    #[test]
    fn test_ctrl_shift_c_and_v_shortcuts() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Ctrl+Shift+C consumed for copy
        let handled = ws.handle_keyboard_input(
            &Key::Character("c".to_string()),
            Some("c"),
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        );
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");
    }

    #[test]
    fn test_alt_arrow_consumed_for_navigation() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Alt+ArrowLeft consumed for pane navigation (not forwarded to PTY even if single pane)
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::ArrowLeft), None, ModifiersState::ALT);
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");

        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::ArrowUp), None, ModifiersState::ALT);
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");
    }

    #[test]
    fn test_ctrl_alt_arrow_consumed_for_split_resizing() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Ctrl+Alt+ArrowLeft consumed for split resizing
        let handled = ws.handle_keyboard_input(
            &Key::Named(NamedKey::ArrowLeft),
            None,
            ModifiersState::CONTROL | ModifiersState::ALT,
        );
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");
    }

    #[test]
    fn test_unbound_combinations_forwarded_to_pty() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Ctrl+Shift+R passes through to PTY as 0x12 (^R)
        let handled = ws.handle_keyboard_input(
            &Key::Character("r".to_string()),
            Some("r"),
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        );
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), &[18]);

        // Shift+Tab passes through to PTY as ESC [ Z
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::Tab), None, ModifiersState::SHIFT);
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), b"\x1b[Z");

        // Ctrl+Shift+Up passes through to PTY as ESC [ 1;6 A
        let handled = ws.handle_keyboard_input(
            &Key::Named(NamedKey::ArrowUp),
            None,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        );
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), b"\x1b[1;6A");

        // Alt+x passes through to PTY as ESC x
        let handled = ws.handle_keyboard_input(
            &Key::Character("x".to_string()),
            Some("x"),
            ModifiersState::ALT,
        );
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), b"\x1bx");
    }

    #[test]
    fn test_modifier_only_key_does_not_write_bytes() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Shift pressed alone
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::Shift), None, ModifiersState::SHIFT);
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");

        // Control pressed alone
        let handled = ws.handle_keyboard_input(
            &Key::Named(NamedKey::Control),
            None,
            ModifiersState::CONTROL,
        );
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");
    }
}
