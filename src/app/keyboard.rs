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
        let has_ctrl = modifiers.control_key();
        let has_alt = modifiers.alt_key();
        let has_shift = modifiers.shift_key();

        // Fast path: if neither Ctrl nor Alt is held, check Shift-only Velox shortcuts
        if !has_ctrl && !has_alt {
            if has_shift {
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
                    Key::Named(NamedKey::Home) => {
                        let active_pane = self.active_pane_mut();
                        let active_grid = if active_pane.terminal.is_alt_screen {
                            &mut active_pane.terminal.alt_grid
                        } else {
                            &mut active_pane.terminal.grid
                        };
                        let history_len = active_grid.scrollback.len();
                        active_grid.scroll_offset = history_len;
                        active_grid.damage.mark_all();
                        self.needs_redraw = true;
                        log::debug!("Velox shortcut: scroll to top of history");
                        return false;
                    }
                    Key::Named(NamedKey::End) => {
                        let active_pane = self.active_pane_mut();
                        let active_grid = if active_pane.terminal.is_alt_screen {
                            &mut active_pane.terminal.alt_grid
                        } else {
                            &mut active_pane.terminal.grid
                        };
                        active_grid.scroll_offset = 0;
                        active_grid.damage.mark_all();
                        self.needs_redraw = true;
                        log::debug!("Velox shortcut: scroll to bottom of history");
                        return false;
                    }
                    Key::Named(NamedKey::Insert) => {
                        let text = crate::clipboard::clipboard::paste();
                        if !text.is_empty() {
                            let formatted = self.active_pane().terminal.format_paste(&text);
                            if self.active_pane().terminal.scroll_on_keystroke {
                                self.active_pane_mut()
                                    .terminal
                                    .active_grid_mut()
                                    .scroll_offset = 0;
                            }
                            self.write_to_active_pane(formatted.into_bytes());
                            self.last_key_instant = std::time::Instant::now();
                            self.mark_interaction();
                            self.needs_redraw = true;
                            log::debug!("Velox shortcut: paste (Shift+Insert)");
                            return true;
                        }
                        return false;
                    }
                    _ => {}
                }
            }
            // All other unmodified or Shift-only keystrokes bypass remaining shortcut matchers!
        } else {
            // Split Pane Divider Resizing (Ctrl+Alt+Arrows without Shift)
            if has_ctrl && has_alt && !has_shift {
                match logical_key {
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowUp) => {
                        if self.adjust_active_split_ratio(-0.05) {
                            log::debug!("Velox shortcut: split resize -0.05");
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowDown) => {
                        if self.adjust_active_split_ratio(0.05) {
                            log::debug!("Velox shortcut: split resize +0.05");
                            return false;
                        }
                    }
                    _ => {}
                }
            }

            // Split Pane Navigation (Alt+Arrows without Ctrl/Shift)
            if has_alt && !has_ctrl && !has_shift {
                match logical_key {
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.focus_direction(FocusDirection::Left) {
                            log::debug!("Velox shortcut: focus left");
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.focus_direction(FocusDirection::Right) {
                            log::debug!("Velox shortcut: focus right");
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.focus_direction(FocusDirection::Up) {
                            log::debug!("Velox shortcut: focus up");
                            return false;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if self.focus_direction(FocusDirection::Down) {
                            log::debug!("Velox shortcut: focus down");
                            return false;
                        }
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
            if has_ctrl && has_shift && !has_alt {
                if let Key::Character(s) = &logical_key {
                    if s.len() == 1 {
                        let ch = s.as_bytes()[0].to_ascii_lowercase();
                        match ch {
                            b't' => {
                                self.create_tab(None, None, None, None);
                                log::debug!("Velox shortcut: create tab");
                                return false;
                            }
                            b'w' => {
                                self.close_pane();
                                log::debug!("Velox shortcut: close pane");
                                return false;
                            }
                            b'o' | b'h' => {
                                self.split_horizontal();
                                log::debug!("Velox shortcut: split horizontal");
                                return false;
                            }
                            b'e' | b'd' => {
                                self.split_vertical();
                                log::debug!("Velox shortcut: split vertical");
                                return false;
                            }
                            b'n' => {
                                self.focus_next_pane();
                                log::debug!("Velox shortcut: focus next pane");
                                return false;
                            }
                            b'p' => {
                                self.focus_previous_pane();
                                log::debug!("Velox shortcut: focus previous pane");
                                return false;
                            }
                            b'c' => {
                                let active_pane = self.active_pane();
                                let active_grid = active_pane.terminal.active_grid();
                                let text = if active_grid.selection.active
                                    && !active_grid.selection.is_empty()
                                {
                                    active_grid.extract_selection_text()
                                } else {
                                    let lines: Vec<String> = (0..active_grid.height)
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
                                        .collect();
                                    if let Some(idx) = lines.iter().rposition(|l| !l.is_empty()) {
                                        lines[..=idx].join("\n")
                                    } else {
                                        String::new()
                                    }
                                };
                                if !text.is_empty() {
                                    crate::clipboard::clipboard::copy(text);
                                }
                                log::debug!("Velox shortcut: copy");
                                return false;
                            }
                            b'v' => {
                                let text = crate::clipboard::clipboard::paste();
                                if !text.is_empty() {
                                    let formatted = self.active_pane().terminal.format_paste(&text);
                                    if self.active_pane().terminal.scroll_on_keystroke {
                                        self.active_pane_mut()
                                            .terminal
                                            .active_grid_mut()
                                            .scroll_offset = 0;
                                    }
                                    self.write_to_active_pane(formatted.into_bytes());
                                    self.last_key_instant = std::time::Instant::now();
                                    self.mark_interaction();
                                    self.needs_redraw = true;
                                    log::debug!("Velox shortcut: paste");
                                    return true;
                                }
                                return false;
                            }
                            b'+' | b'=' => {
                                let active_size = self.active_pane().font_size;
                                let new_size = active_size + 1.0;
                                self.set_font_size(new_size);
                                log::debug!("Velox shortcut: zoom in");
                                return false;
                            }
                            b'_' | b'-' => {
                                let active_size = self.active_pane().font_size;
                                let new_size = (active_size - 1.0).max(4.0);
                                self.set_font_size(new_size);
                                log::debug!("Velox shortcut: zoom out");
                                return false;
                            }
                            b']' | b'}' => {
                                self.next_tab();
                                log::debug!("Velox shortcut: next tab");
                                return false;
                            }
                            b'[' | b'{' => {
                                self.prev_tab();
                                log::debug!("Velox shortcut: prev tab");
                                return false;
                            }
                            _ => {}
                        }
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
            if has_ctrl && !has_shift && !has_alt {
                if matches!(logical_key, Key::Named(NamedKey::Insert)) {
                    let active_pane = self.active_pane();
                    let active_grid = active_pane.terminal.active_grid();
                    let text = if active_grid.selection.active && !active_grid.selection.is_empty()
                    {
                        active_grid.extract_selection_text()
                    } else {
                        let lines: Vec<String> = (0..active_grid.height)
                            .map(|y| {
                                (0..active_grid.width)
                                    .map(|x| active_grid.cells[y * active_grid.width + x].character)
                                    .collect::<String>()
                                    .trim_end()
                                    .to_string()
                            })
                            .collect();
                        if let Some(idx) = lines.iter().rposition(|l| !l.is_empty()) {
                            lines[..=idx].join("\n")
                        } else {
                            String::new()
                        }
                    };
                    if !text.is_empty() {
                        crate::clipboard::clipboard::copy(text);
                    }
                    log::debug!("Velox shortcut: copy (Ctrl+Insert)");
                    return false;
                }

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
                    if s.eq_ignore_ascii_case("c") {
                        let active_pane = self.active_pane();
                        let active_grid = active_pane.terminal.active_grid();
                        if active_grid.selection.active && !active_grid.selection.is_empty() {
                            let text = active_grid.extract_selection_text();
                            if !text.is_empty() {
                                crate::clipboard::clipboard::copy(text);
                            }
                            self.active_pane_mut()
                                .terminal
                                .active_grid_mut()
                                .selection
                                .clear();
                            self.needs_redraw = true;
                            log::debug!("Velox shortcut: copy selection via Ctrl+C");
                            return false;
                        }
                    }

                    match s.as_str() {
                        "+" | "=" => {
                            let active_size = self.active_pane().font_size;
                            let new_size = active_size + 1.0;
                            self.set_font_size(new_size);
                            log::debug!("Velox shortcut: zoom in");
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
        }

        // ── 2. PTY Input Translation and Dispatch ─────────────────────────────
        let (cursor_keys_mode, keypad_application_mode, kitty_flags, scroll_on_keystroke) = {
            let active_term = &self.active_pane().terminal;
            (
                active_term.cursor_keys_mode,
                active_term.keypad_application_mode,
                active_term.kitty_keyboard_flags,
                active_term.scroll_on_keystroke,
            )
        };
        if let Some(bytes) = crate::input::keyboard::translate_key(
            logical_key,
            text,
            modifiers,
            cursor_keys_mode,
            keypad_application_mode,
            kitty_flags,
        ) {
            log::debug!("PTY write: {} bytes: {:?}", bytes.len(), bytes.as_slice());
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
    fn test_alt_arrow_falls_through_on_single_pane() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Alt+ArrowLeft forwards to PTY when there is no neighboring pane
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::ArrowLeft), None, ModifiersState::ALT);
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), b"\x1b[1;3D");

        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::ArrowUp), None, ModifiersState::ALT);
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), b"\x1b[1;3A");
    }

    #[test]
    fn test_ctrl_alt_arrow_falls_through_on_single_pane() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Ctrl+Alt+ArrowLeft forwards to PTY when there are no split panes
        let handled = ws.handle_keyboard_input(
            &Key::Named(NamedKey::ArrowLeft),
            None,
            ModifiersState::CONTROL | ModifiersState::ALT,
        );
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), b"\x1b[1;7D");
    }

    #[test]
    fn test_ctrl_c_selection_copy_vs_sigint() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // When NO selection is active, Ctrl+C sends SIGINT (0x03)
        let handled = ws.handle_keyboard_input(
            &Key::Character("c".to_string()),
            Some("c"),
            ModifiersState::CONTROL,
        );
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), &[3]);

        // When selection IS active and non-empty, Ctrl+C copies selection without sending 0x03
        {
            let grid = ws.active_pane_mut().terminal.active_grid_mut();
            grid.selection.start_selection(0, 0);
            grid.selection.update_selection(5, 0);
        }
        let handled = ws.handle_keyboard_input(
            &Key::Character("c".to_string()),
            Some("c"),
            ModifiersState::CONTROL,
        );
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");
        assert!(!ws.active_pane().terminal.active_grid().selection.active);
    }

    #[test]
    fn test_insert_shortcuts() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Ctrl+Insert is consumed for copy
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::Insert), None, ModifiersState::CONTROL);
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");
    }

    #[test]
    fn test_shift_home_and_end() {
        let (pty, read_end) = create_test_pty_pipe();
        let mut ws = WindowState::create_test_window_state(pty, 80, 24);

        // Shift+Home consumed for scroll to top
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::Home), None, ModifiersState::SHIFT);
        assert!(!handled);
        assert_eq!(read_pty_output(&read_end), b"");

        // Shift+End consumed for scroll to bottom
        let handled =
            ws.handle_keyboard_input(&Key::Named(NamedKey::End), None, ModifiersState::SHIFT);
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

        // Ctrl+- passes through to PTY as 0x1f (readline Undo)
        let handled = ws.handle_keyboard_input(
            &Key::Character("-".to_string()),
            Some("-"),
            ModifiersState::CONTROL,
        );
        assert!(handled);
        assert_eq!(read_pty_output(&read_end), &[31]);
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
