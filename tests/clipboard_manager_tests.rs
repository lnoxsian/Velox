use std::os::fd::FromRawFd;
use std::sync::{Arc, Mutex};
use velox::app::app::WindowState;
use velox::clipboard::clipboard::{
    clear_history, copy, copy_primary, get_history, get_internal_clipboard, get_internal_primary,
    set_history_limit,
};
use velox::clipboard::manager::ClipboardManager;
use velox::pty::master::PtyMaster;
use velox::window::event::{ElementState, Key, ModifiersState, MouseButton, NamedKey};

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn create_test_window_state() -> (WindowState, std::fs::File) {
    let mut fds = [0; 2];
    let res = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(res, 0);
    let pty = Arc::new(PtyMaster::new(fds[1], None));
    let read_end = unsafe { std::fs::File::from_raw_fd(fds[0]) };
    let ws = WindowState::create_test_window_state(pty, 80, 24);
    (ws, read_end)
}

fn read_pty_output(file: &std::fs::File) -> Vec<u8> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    let mut buf = [0u8; 4096];
    let mut bytes = Vec::new();
    loop {
        let n = unsafe {
            libc::recv(
                fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                libc::MSG_DONTWAIT,
            )
        };
        if n > 0 {
            bytes.extend_from_slice(&buf[..n as usize]);
        } else {
            break;
        }
    }
    bytes
}

#[test]
fn test_clipboard_manager_history_ring_capacity_and_eviction() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let manager = ClipboardManager::new(5);
    manager.set_history_limit(3);

    manager.copy("First entry".to_string());
    manager.copy("Second entry".to_string());
    manager.copy("Third entry".to_string());
    manager.copy("Fourth entry".to_string());

    let history = manager.get_history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0], "Fourth entry");
    assert_eq!(history[1], "Third entry");
    assert_eq!(history[2], "Second entry");

    // Dynamic shrink
    manager.set_history_limit(2);
    let history_shrunk = manager.get_history();
    assert_eq!(history_shrunk.len(), 2);
    assert_eq!(history_shrunk[0], "Fourth entry");
    assert_eq!(history_shrunk[1], "Third entry");

    manager.clear_history();
    assert!(manager.get_history().is_empty());
}

#[test]
fn test_clipboard_manager_deduplication() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let manager = ClipboardManager::new(10);
    manager.copy("Duplicate text".to_string());
    manager.copy("Duplicate text".to_string());
    manager.copy("Duplicate text".to_string());

    let history = manager.get_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], "Duplicate text");

    manager.copy("Different text".to_string());
    let history2 = manager.get_history();
    assert_eq!(history2.len(), 2);
    assert_eq!(history2[0], "Different text");
    assert_eq!(history2[1], "Duplicate text");
}

#[test]
fn test_clipboard_primary_buffer_isolation() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let manager = ClipboardManager::new(10);

    // User copied something in Chrome/Browser to CLIPBOARD
    manager.copy("Browser Ctrl+C Content".to_string());
    assert_eq!(manager.get_internal_clipboard(), "Browser Ctrl+C Content");

    // Terminal mouse highlight updates PRIMARY only
    manager.copy_primary("Terminal Selected Command".to_string());
    assert_eq!(manager.get_internal_primary(), "Terminal Selected Command");

    // Verify browser clipboard content was NOT destroyed
    assert_eq!(manager.get_internal_clipboard(), "Browser Ctrl+C Content");
}

#[test]
fn test_middle_click_paste_behavior() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let manager = ClipboardManager::new(10);

    // Case 1: Primary has text -> middle click pastes primary
    manager.copy("Clipboard text".to_string());
    manager.copy_primary("Primary selection text".to_string());
    assert_eq!(manager.middle_click_paste(), "Primary selection text");

    // Case 2: Primary is empty -> middle click falls back to clipboard
    let manager_empty_prim = ClipboardManager::new(10);
    manager_empty_prim.copy("Fallback clipboard text".to_string());
    if manager_empty_prim.get_internal_primary().is_empty() {
        assert_eq!(
            manager_empty_prim.middle_click_paste(),
            "Fallback clipboard text"
        );
    }
}

#[test]
fn test_global_clipboard_facade_smoke() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    set_history_limit(20);
    clear_history();

    copy("Velox Terminal Unit Test".to_string());
    assert_eq!(get_internal_clipboard(), "Velox Terminal Unit Test");

    copy_primary("Velox Primary Selection".to_string());
    assert_eq!(get_internal_primary(), "Velox Primary Selection");
    assert_eq!(get_internal_clipboard(), "Velox Terminal Unit Test");

    let history = get_history();
    assert!(!history.is_empty());
    assert_eq!(history[0], "Velox Terminal Unit Test");
}

#[test]
fn test_copy_on_select_false_vs_true() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let (mut ws, _read_end) = create_test_window_state();
    ws.active_pane_mut().terminal.feed(b"rustacean\r\n");

    // Case 1: copy_on_select = false (default)
    ws.copy_on_select = false;
    copy("initial browser text".to_string());

    // Double-click on row 0, col 2 ('s' of "rustacean")
    let pane_rects = ws.recalculate_panes_layout(ws.active_tab_index).0;
    let r = &pane_rects[0];
    let click_x = r.x as f64 + r.padding_x as f64 + (r.cell_width as f64) * 2.5;
    let click_y = r.y as f64 + r.padding_y as f64 + (r.cell_height as f64) * 0.5;

    ws.handle_cursor_moved((click_x, click_y), ModifiersState::empty());
    // First click
    ws.handle_mouse_input(
        ElementState::Pressed,
        MouseButton::Left,
        ModifiersState::empty(),
    );
    ws.handle_mouse_input(
        ElementState::Released,
        MouseButton::Left,
        ModifiersState::empty(),
    );
    // Second click (double click)
    ws.handle_mouse_input(
        ElementState::Pressed,
        MouseButton::Left,
        ModifiersState::empty(),
    );
    ws.handle_mouse_input(
        ElementState::Released,
        MouseButton::Left,
        ModifiersState::empty(),
    );

    // With copy_on_select = false: PRIMARY has "rustacean", CLIPBOARD retains "initial browser text"
    assert_eq!(get_internal_primary(), "rustacean");
    assert_eq!(get_internal_clipboard(), "initial browser text");

    // Case 2: copy_on_select = true
    ws.copy_on_select = true;
    ws.click_count = 1; // trigger double-click on next press
    ws.last_click_pos = (2, 0);
    ws.last_click_pane_id = Some(0);
    ws.last_click_instant = Some(std::time::Instant::now());

    ws.handle_mouse_input(
        ElementState::Pressed,
        MouseButton::Left,
        ModifiersState::empty(),
    );
    ws.handle_mouse_input(
        ElementState::Released,
        MouseButton::Left,
        ModifiersState::empty(),
    );

    // With copy_on_select = true: BOTH PRIMARY and CLIPBOARD are updated to "rustacean"
    assert_eq!(get_internal_primary(), "rustacean");
    assert_eq!(get_internal_clipboard(), "rustacean");
}

#[test]
fn test_putty_paste_style_right_click() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let (mut ws, read_end) = create_test_window_state();

    copy_primary("pwd\n".to_string());

    // When putty_paste_style = false (default)
    ws.putty_paste_style = false;
    let pane_rects = ws.recalculate_panes_layout(ws.active_tab_index).0;
    let r = &pane_rects[0];
    let click_x = r.x as f64 + r.padding_x as f64 + 10.0;
    let click_y = r.y as f64 + r.padding_y as f64 + 10.0;

    ws.handle_cursor_moved((click_x, click_y), ModifiersState::empty());
    ws.handle_mouse_input(
        ElementState::Pressed,
        MouseButton::Right,
        ModifiersState::empty(),
    );
    ws.handle_mouse_input(
        ElementState::Released,
        MouseButton::Right,
        ModifiersState::empty(),
    );

    // No paste occurs
    assert_eq!(read_pty_output(&read_end), b"");

    // When putty_paste_style = true
    ws.putty_paste_style = true;
    ws.handle_mouse_input(
        ElementState::Pressed,
        MouseButton::Right,
        ModifiersState::empty(),
    );
    ws.handle_mouse_input(
        ElementState::Released,
        MouseButton::Right,
        ModifiersState::empty(),
    );

    // Instant paste triggered on right click (\n translates to \r in terminal format_paste)
    assert_eq!(read_pty_output(&read_end), b"pwd\r");
}

#[test]
fn test_keyboard_paste_shortcuts() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let (mut ws, read_end) = create_test_window_state();

    copy("cargo build\n".to_string());

    // 1. Shift+Insert
    let handled =
        ws.handle_keyboard_input(&Key::Named(NamedKey::Insert), None, ModifiersState::SHIFT);
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), b"cargo build\r");

    // 2. Ctrl+Shift+Insert
    let handled = ws.handle_keyboard_input(
        &Key::Named(NamedKey::Insert),
        None,
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    );
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), b"cargo build\r");

    // 3. Ctrl+Shift+V
    let handled = ws.handle_keyboard_input(
        &Key::Character("v".to_string()),
        Some("v"),
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    );
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), b"cargo build\r");
}

#[test]
fn test_external_copy_precedence_over_previous_terminal_selection() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let manager = ClipboardManager::new(10);

    // 1. User selects text in the terminal -> updates PRIMARY
    manager.copy_primary("previous terminal text".to_string());
    assert_eq!(manager.get_internal_primary(), "previous terminal text");
    // Middle click at this point pastes the terminal text
    assert_eq!(manager.middle_click_paste(), "previous terminal text");

    // 2. User copies text from an external application (e.g. browser Ctrl+C) -> updates CLIPBOARD
    // Wait a brief moment to ensure subsequent Instant::now() has strictly progressed
    std::thread::sleep(std::time::Duration::from_millis(5));
    manager.copy("latest copy from browser".to_string());

    // 3. Middle click and paste must choose the latest copy action from the external app!
    assert_eq!(manager.paste(), "latest copy from browser");
    assert_eq!(manager.middle_click_paste(), "latest copy from browser");

    // 4. User makes a new selection in the terminal
    std::thread::sleep(std::time::Duration::from_millis(5));
    manager.copy_primary("new terminal selection".to_string());
    // Middle click now reflects the newer terminal selection
    assert_eq!(manager.middle_click_paste(), "new terminal selection");
}

#[test]
fn test_keybinds_ctrl_c_cancel_and_ctrl_v_no_paste() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let (mut ws, read_end) = create_test_window_state();

    copy("some copied text".to_string());

    // 1. Ctrl + C must send SIGINT (0x03) / cancel, NOT copy
    let handled = ws.handle_keyboard_input(
        &Key::Character("c".to_string()),
        Some("c"),
        ModifiersState::CONTROL,
    );
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), &[3]);

    // 2. Ctrl + V must NOT paste! It passes through to PTY as control code 0x16 (SYN)
    let handled = ws.handle_keyboard_input(
        &Key::Character("v".to_string()),
        Some("v"),
        ModifiersState::CONTROL,
    );
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), &[22]);

    // 3. Ctrl + Shift + V MUST paste the clipboard content
    let handled = ws.handle_keyboard_input(
        &Key::Character("v".to_string()),
        Some("v"),
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    );
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), b"some copied text");
}

#[test]
fn test_osc52_security_controls() {
    let mut term = velox::terminal::terminal::Terminal::new(80, 24);

    // 1. With osc52_allow_read = false, clipboard queries must be ignored
    term.osc52_allow_read = false;
    term.feed(b"\x1b]52;c;?\x07");
    assert!(term.outgoing.is_empty(), "OSC 52 query must be rejected when allow_read is false");

    // 2. With osc52_allow_read = true, query generates response
    term.osc52_allow_read = true;
    term.feed(b"\x1b]52;c;?\x07");
    assert!(!term.outgoing.is_empty(), "OSC 52 query must generate response when allow_read is true");
    term.outgoing.clear();

    // 3. With osc52_clipboard = false, both reads and writes must be completely ignored
    term.osc52_clipboard = false;
    term.feed(b"\x1b]52;c;SGVsbG8=\x07");
    term.feed(b"\x1b]52;c;?\x07");
    assert!(term.outgoing.is_empty(), "OSC 52 must be completely disabled when osc52_clipboard is false");
}

#[test]
fn test_block_selection_rectangular_cut() {
    let mut term = velox::terminal::terminal::Terminal::new(40, 5);
    term.feed(b"1234567890\r\nabcdefghij\r\n!@#$%^&*()\r\n");

    let grid = term.active_grid_mut();
    grid.selection.mode = velox::screen::selection::SelectionMode::Block;
    grid.selection.start_selection(2, 0); // row 0, col 2 ('3')
    grid.selection.update_selection(6, 2); // row 2, col 6 ('&')

    let text = grid.extract_selection_text();
    assert_eq!(text, "34567\ncdefg\n#$%^&");
}

#[test]
fn test_paste_nul_byte_filtering() {
    let term = velox::terminal::terminal::Terminal::new(80, 24);
    let dirty = "command\0with\0embedded\0nulls\n";
    let formatted = term.format_paste(dirty);
    assert!(!formatted.contains('\0'), "Formatted paste must strip NUL bytes");
    assert_eq!(formatted, "commandwithembeddednulls\r");
}
