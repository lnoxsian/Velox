use std::os::fd::RawFd;
use std::sync::Arc;
use velox::app::app::WindowState;
use velox::pty::master::PtyMaster;
use velox::window::event::ModifiersState;

fn create_pipe_pty_master() -> (Arc<PtyMaster>, RawFd) {
    let mut fds = [0 as libc::c_int; 2];
    let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(res, 0, "libc::pipe failed");
    // fds[0] is read end, fds[1] is write end
    let master = Arc::new(PtyMaster::new(fds[1], None));
    (master, fds[0])
}

fn read_pipe_bytes(read_fd: RawFd) -> Vec<u8> {
    let mut buf = [0u8; 1024];
    unsafe {
        let flags = libc::fcntl(read_fd, libc::F_GETFL, 0);
        libc::fcntl(read_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let mut bytes = Vec::new();
    loop {
        let n = unsafe { libc::read(read_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n > 0 {
            bytes.extend_from_slice(&buf[..n as usize]);
        } else {
            break;
        }
    }
    bytes
}

#[test]
fn test_mouse_wheel_in_mouse_mode_sends_single_report_per_notch() {
    let (pty_master, read_fd) = create_pipe_pty_master();
    let mut ws = WindowState::create_test_window_state(pty_master, 80, 24);

    let pane = ws.active_pane_mut();
    pane.terminal.mouse_mode = 1002;
    pane.terminal.mouse_sgr = true;

    // Even with a high scroll_multiplier = 5.0, mouse mode must send exactly 1 report per notch!
    ws.scroll_multiplier = 5.0;

    // 1. Wheel Up (delta_y = 1.0)
    let handled = ws.handle_mouse_wheel(1.0, ModifiersState::empty());
    assert!(handled, "handle_mouse_wheel must return true in mouse mode");

    let output = read_pipe_bytes(read_fd);
    let output_str = String::from_utf8_lossy(&output);
    // Button 64 in SGR format: \x1b[<64;col;rowM
    assert!(
        output_str.starts_with("\x1b[<64;"),
        "Expected SGR wheel up report, got: {:?}",
        output_str
    );
    // Crucially: it must be EXACTLY ONE report, not 5 reports repeated!
    let count = output_str.matches("\x1b[<64;").count();
    assert_eq!(
        count, 1,
        "Mouse mode must emit 1 report per wheel notch, got {}",
        count
    );

    // 2. Wheel Down (delta_y = -1.0)
    let handled = ws.handle_mouse_wheel(-1.0, ModifiersState::empty());
    assert!(handled);
    let output_down = read_pipe_bytes(read_fd);
    let output_down_str = String::from_utf8_lossy(&output_down);
    // Button 65 in SGR format: \x1b[<65;col;rowM
    assert!(
        output_down_str.starts_with("\x1b[<65;"),
        "Expected SGR wheel down report, got: {:?}",
        output_down_str
    );
    let count_down = output_down_str.matches("\x1b[<65;").count();
    assert_eq!(
        count_down, 1,
        "Mouse mode must emit 1 report per wheel notch, got {}",
        count_down
    );

    unsafe { libc::close(read_fd) };
}

#[test]
fn test_mouse_wheel_in_alt_screen_sends_single_arrow_key_per_notch() {
    let (pty_master, read_fd) = create_pipe_pty_master();
    let mut ws = WindowState::create_test_window_state(pty_master, 80, 24);

    let pane = ws.active_pane_mut();
    pane.terminal.mouse_mode = 0; // Mouse mode disabled
    pane.terminal.is_alt_screen = true; // Alternate screen active (e.g. less, nano, man)
    pane.terminal.cursor_keys_mode = false; // Normal CSI mode: \x1b[A / \x1b[B

    // Even with scroll_multiplier = 5.0, alternate screen must send 1 arrow key per notch!
    ws.scroll_multiplier = 5.0;

    // 1. Wheel Up (delta_y = 1.0)
    let handled = ws.handle_mouse_wheel(1.0, ModifiersState::empty());
    assert!(handled, "handle_mouse_wheel must return true in alt screen");

    let output = read_pipe_bytes(read_fd);
    // Exactly 3 bytes for \x1b[A (1 Up arrow key)
    assert_eq!(
        output, b"\x1b[A",
        "Alt screen must emit exactly 1 Up arrow key (3 bytes), got: {:?}",
        output
    );

    // 2. Wheel Down (delta_y = -1.0)
    let handled = ws.handle_mouse_wheel(-1.0, ModifiersState::empty());
    assert!(handled);

    let output_down = read_pipe_bytes(read_fd);
    // Exactly 3 bytes for \x1b[B (1 Down arrow key)
    assert_eq!(
        output_down, b"\x1b[B",
        "Alt screen must emit exactly 1 Down arrow key (3 bytes), got: {:?}",
        output_down
    );

    unsafe { libc::close(read_fd) };
}

#[test]
fn test_mouse_wheel_in_normal_screen_scales_with_multiplier() {
    let (pty_master, read_fd) = create_pipe_pty_master();
    let mut ws = WindowState::create_test_window_state(pty_master, 80, 24);

    let pane = ws.active_pane_mut();
    pane.terminal.mouse_mode = 0;
    pane.terminal.is_alt_screen = false;

    // Feed 100 lines to populate scrollback
    for i in 0..100 {
        pane.terminal.feed(format!("Line {}\r\n", i).as_bytes());
    }

    assert!(
        pane.terminal.active_grid().scrollback.len() >= 50,
        "Scrollback must be populated"
    );
    assert_eq!(pane.terminal.active_grid().scroll_offset, 0);

    ws.scroll_multiplier = 5.0;

    // 1. One wheel up notch -> increases scroll_offset by 5 lines
    ws.handle_mouse_wheel(1.0, ModifiersState::empty());
    let offset = ws.active_pane().terminal.active_grid().scroll_offset;
    assert_eq!(offset, 5, "Scrollback offset must advance by 5 lines");

    // 2. Two wheel up notches -> increases scroll_offset by 10 lines
    ws.handle_mouse_wheel(2.0, ModifiersState::empty());
    let offset2 = ws.active_pane().terminal.active_grid().scroll_offset;
    assert_eq!(offset2, 15, "Scrollback offset must advance by 10 lines");

    // 3. One wheel down notch -> decreases scroll_offset by 5 lines
    ws.handle_mouse_wheel(-1.0, ModifiersState::empty());
    let offset3 = ws.active_pane().terminal.active_grid().scroll_offset;
    assert_eq!(offset3, 10, "Scrollback offset must decrease by 5 lines");

    unsafe { libc::close(read_fd) };
}

#[test]
fn test_mouse_wheel_accumulator_fractional_deltas() {
    let (pty_master, read_fd) = create_pipe_pty_master();
    let mut ws = WindowState::create_test_window_state(pty_master, 80, 24);

    let pane = ws.active_pane_mut();
    pane.terminal.mouse_mode = 1000; // X10 mouse mode

    // 1. Send small fractional delta: 0.4
    let h1 = ws.handle_mouse_wheel(0.4, ModifiersState::empty());
    assert!(!h1, "Sub-notch delta must not fire an event");
    assert_eq!(read_pipe_bytes(read_fd).len(), 0);
    assert!((ws.scroll_accumulator - 0.4).abs() < 1e-4);

    // 2. Send another fractional delta: 0.3 (total = 0.7 < 1.0)
    let h2 = ws.handle_mouse_wheel(0.3, ModifiersState::empty());
    assert!(!h2);
    assert_eq!(read_pipe_bytes(read_fd).len(), 0);
    assert!((ws.scroll_accumulator - 0.7).abs() < 1e-4);

    // 3. Send 0.5 (total = 1.2 >= 1.0) -> fires 1 event, remainder = 0.2
    let h3 = ws.handle_mouse_wheel(0.5, ModifiersState::empty());
    assert!(h3, "Accumulated threshold reached, event must fire");
    let bytes = read_pipe_bytes(read_fd);
    assert_eq!(bytes.len(), 6, "Legacy X10 report is 6 bytes");
    assert!((ws.scroll_accumulator - 0.2).abs() < 1e-4);

    // 4. Direction reversal: user starts scrolling down (-0.3)
    // Accumulator should reset on direction change rather than fighting previous remainder
    let h4 = ws.handle_mouse_wheel(-0.3, ModifiersState::empty());
    assert!(!h4);
    assert_eq!(read_pipe_bytes(read_fd).len(), 0);
    assert!((ws.scroll_accumulator - (-0.3)).abs() < 1e-4);

    unsafe { libc::close(read_fd) };
}

#[test]
fn test_wayland_state_scroll_accumulator_init() {
    let state = velox::window::wayland::WaylandState::new(800, 600);
    assert_eq!(state.scroll_accumulator_y, 0.0);
    assert!(state.events.is_empty());
}

#[test]
fn test_ctrl_mouse_wheel_zoom_unconstrained() {
    let (pty, read_fd) = create_pipe_pty_master();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);
    let initial_font_size = ws.active_pane().font_size;

    // Ctrl + Wheel Up (+1.0) zooms in
    ws.handle_mouse_wheel(1.0, ModifiersState::CONTROL);
    assert_eq!(ws.active_pane().font_size, initial_font_size + 1.0);

    // Zoom in repeatedly past previous 5.0x / 70.0px ceiling
    for _ in 0..100 {
        ws.handle_mouse_wheel(1.0, ModifiersState::CONTROL);
    }
    assert!(
        ws.active_pane().font_size > 100.0,
        "Font zoom in must not be artificially capped at 70px"
    );

    // Extreme zoom in clamps dynamically so number of rows is 1
    ws.set_font_size(2000.0);
    assert_eq!(
        ws.active_pane().terminal.grid.height,
        1,
        "Font zoom in must clamp dynamically so number of rows is 1"
    );

    // Ctrl + Wheel Down (-1.0) zooms out
    let current_size = ws.active_pane().font_size;
    ws.handle_mouse_wheel(-1.0, ModifiersState::CONTROL);
    assert!(ws.active_pane().font_size < current_size);

    unsafe { libc::close(read_fd) };
}
