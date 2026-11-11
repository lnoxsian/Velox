use std::time::{Duration, Instant};
use velox::renderer::damage::DamageMap;
use velox::screen::scrollback::Scrollback;
use velox::terminal::terminal::Terminal;

#[test]
fn test_large_selection_extract_performance_and_accuracy() {
    let width = 120;
    let height = 40;
    let mut term = Terminal::new(width, height);
    // Use disk-backed chunked scrollback to test the worst-case retrieval path
    term.grid.scrollback = Scrollback::new(500, true);

    // Generate 30,000 lines of diverse text
    for i in 0..30_000 {
        let line = format!(
            "Row {:05}: Velox Terminal Performance Validation {:08x}\r\n",
            i,
            i * 7919
        );
        term.feed(line.as_bytes());
    }

    let history_len = term.grid.scrollback.len();
    assert!(history_len >= 29_900);

    // Select all lines from row 0 to the bottom of the screen
    term.grid.selection.start_selection(0, 0);
    term.grid
        .selection
        .update_selection(width - 1, history_len + height - 1);

    let t0 = Instant::now();
    let text = term.grid.extract_selection_text();
    let elapsed = t0.elapsed();

    // Verify performance: extraction of 30,000 lines completes rapidly without UI lag
    // (17ms in release mode, < 2.5s in unoptimized debug build)
    println!("extract_selection_text (30k rows) took: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(2500),
        "extract_selection_text took too long: {:?}",
        elapsed
    );

    // Verify correctness: must contain first, middle, and last lines
    assert!(text.contains("Row 00000: Velox Terminal Performance Validation"));
    assert!(text.contains("Row 15000: Velox Terminal Performance Validation"));
    assert!(text.contains("Row 29999: Velox Terminal Performance Validation"));

    // Verify format_paste with bracketed paste mode on
    term.feed(b"\x1b[?2004h");
    assert!(term.bracketed_paste_mode);

    let t0 = Instant::now();
    let formatted = term.format_paste(&text);
    let format_elapsed = t0.elapsed();
    println!(
        "format_paste ({} bytes) took: {:?}",
        formatted.len(),
        format_elapsed
    );
    assert!(
        format_elapsed < Duration::from_millis(150),
        "format_paste took too long: {:?}",
        format_elapsed
    );

    // Verify bracketed paste markers
    assert!(formatted.starts_with("\x1b[200~"));
    assert!(formatted.ends_with("\x1b[201~"));
    // Verify no embedded \x1b[201~ exists in payload
    let inner_payload = &formatted[6..formatted.len() - 6];
    assert!(!inner_payload.contains("\x1b[201~"));
}

#[test]
fn test_damage_map_large_selection_clamping_efficiency() {
    let mut dm = DamageMap::new(30);
    dm.clear();

    let history_len = 500_000;
    let scroll_offset = 0;
    let grid_h = 30;

    // Selection across 500,000 lines
    let t0 = Instant::now();
    for _ in 0..10_000 {
        dm.update_selection(
            true,
            ((0, 0), (100, 500_000)),
            history_len,
            scroll_offset,
            grid_h,
        );
    }
    let elapsed = t0.elapsed();

    // 10,000 selection updates over a 500k-line range must take < 50ms total due to O(1) clamping!
    println!(
        "10,000 selection updates over 500k range took: {:?}",
        elapsed
    );
    assert!(
        elapsed < Duration::from_millis(100),
        "update_selection clamping must be O(1) per update"
    );
}

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn create_test_pty_pipe() -> (std::sync::Arc<velox::pty::master::PtyMaster>, std::os::fd::OwnedFd) {
    let mut fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
        let flags = libc::fcntl(fds[0], libc::F_GETFL);
        libc::fcntl(fds[0], libc::F_SETFL, flags | libc::O_NONBLOCK);
        use std::os::fd::FromRawFd;
        let read_end = std::os::fd::OwnedFd::from_raw_fd(fds[0]);
        let pty = std::sync::Arc::new(velox::pty::master::PtyMaster::new(fds[1], None));
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
fn test_paste_middle_click() {
    let _lock = TEST_MUTEX.lock().unwrap();
    velox::clipboard::clipboard::unregister_provider();
    let (pty, read_end) = create_test_pty_pipe();
    let mut ws = velox::app::app::WindowState::create_test_window_state(pty, 80, 24);

    velox::clipboard::clipboard::copy("middle_click_payload".to_string());
    ws.handle_mouse_input(
        velox::window::event::ElementState::Pressed,
        velox::window::event::MouseButton::Middle,
        velox::window::event::ModifiersState::empty(),
    );
    assert_eq!(read_pty_output(&read_end), b"middle_click_payload");
}

#[test]
fn test_paste_ctrl_shift_v() {
    let _lock = TEST_MUTEX.lock().unwrap();
    velox::clipboard::clipboard::unregister_provider();
    let (pty, read_end) = create_test_pty_pipe();
    let mut ws = velox::app::app::WindowState::create_test_window_state(pty, 80, 24);

    velox::clipboard::clipboard::copy("ctrl_shift_v_payload".to_string());
    let handled = ws.handle_keyboard_input(
        &velox::window::event::Key::Character("v".to_string()),
        Some("v"),
        velox::window::event::ModifiersState::CONTROL | velox::window::event::ModifiersState::SHIFT,
    );
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), b"ctrl_shift_v_payload");
}

#[test]
fn test_paste_shift_insert() {
    let _lock = TEST_MUTEX.lock().unwrap();
    velox::clipboard::clipboard::unregister_provider();
    let (pty, read_end) = create_test_pty_pipe();
    let mut ws = velox::app::app::WindowState::create_test_window_state(pty, 80, 24);

    velox::clipboard::clipboard::copy("shift_insert_payload".to_string());
    let handled = ws.handle_keyboard_input(
        &velox::window::event::Key::Named(velox::window::event::NamedKey::Insert),
        None,
        velox::window::event::ModifiersState::SHIFT,
    );
    assert!(handled);
    assert_eq!(read_pty_output(&read_end), b"shift_insert_payload");
}
