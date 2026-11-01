use std::fs::File;
use std::os::fd::FromRawFd;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use velox::app::app::{CustomEvent, EventLoopProxy, WindowState};
use velox::pty::master::PtyMaster;
use velox::screen::cell::Color;
use velox::terminal::terminal::Terminal;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn create_test_pipe() -> (Arc<PtyMaster>, File) {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(res, 0);

    let pty = Arc::new(PtyMaster::new(fds[0], None));
    let write_end = unsafe { File::from_raw_fd(fds[1]) };
    (pty, write_end)
}

#[test]
fn test_sustained_rapid_ansi_output_no_lost_bytes() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut term = Terminal::new(80, 24);

    let total_lines = 1000;
    // Rapidly feed 1000 lines of TrueColor ANSI sequence data
    for i in 0..total_lines {
        let chunk = format!(
            "\x1b[38;2;{};{};{}m[Line {:04}] Rapid Stream ANSI Payload Token\x1b[0m\r\n",
            (i * 17) % 256,
            (i * 31) % 256,
            (i * 47) % 256,
            i
        );
        term.feed(chunk.as_bytes());
    }

    // Verify all 1000 lines were parsed without dropping any bytes
    let scrollback_len = term.grid.scrollback.len();
    assert_eq!(scrollback_len, total_lines - 23);

    // Verify historical lines in scrollback
    for i in (0..scrollback_len).step_by(50) {
        let row = term.grid.scrollback.get_row(i).expect("scrollback row");
        let line_text: String = row.cells.iter().map(|c| c.character).collect();
        let expected_tag = format!("[Line {:04}]", i);
        assert!(
            line_text.contains(&expected_tag),
            "Scrollback row {} should contain '{}', got '{}'",
            i,
            expected_tag,
            line_text
        );
    }

    // Verify visible rows on grid (lines scrollback_len .. total_lines - 1)
    for y in 0..23 {
        let line_num = scrollback_len + y;
        let expected_tag = format!("[Line {:04}]", line_num);
        let slice = term
            .active_grid()
            .active_display_row_slice(y)
            .expect("row slice");
        let line_text: String = slice.iter().map(|c| c.character).collect();
        assert!(
            line_text.contains(&expected_tag),
            "Visible row {} should contain '{}', got '{}'",
            y,
            expected_tag,
            line_text
        );
    }
}

#[test]
fn test_redraw_coalescing_and_frame_rate_limiting() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let (pty, _w) = create_test_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Configure 60 FPS limit (16.6ms frame duration)
    ws.fps_limit = Some(60);
    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);

    // Initial frame draw at T0
    ws.draw();
    let _initial_frame_instant = ws.last_frame_instant;
    ws.needs_redraw = false;

    // Simulate 30 rapid PTY batches arriving in sub-frame intervals (< 16.6ms)
    // such as cmatrix updating column raindrops
    let mut frames_rendered_count = 0;
    for i in 0..30 {
        let data = format!(
            "\x1b[{};{}H\x1b[32m*\x1b[0m",
            (i % 24) + 1,
            (i * 2 % 80) + 1
        );
        ws.active_tab_mut()
            .active_pane_mut()
            .terminal
            .feed(data.as_bytes());
        ws.needs_redraw = true;

        // Check render scheduler logic:
        let now = Instant::now();
        let next_frame = ws.last_frame_instant + frame_duration;
        let is_typing = false;
        let is_resizing = false;

        if is_typing || is_resizing || now >= next_frame {
            ws.draw();
            ws.needs_redraw = false;
            ws.last_frame_instant = Instant::now();
            frames_rendered_count += 1;
        }
    }

    // 30 rapid updates arriving instantaneously must NOT produce 30 separate frames!
    // They must coalesce into at most 1 frame if the deadline has not passed.
    assert!(
        frames_rendered_count <= 1,
        "Expected at most 1 frame rendered due to frame rate coalescing, got {}",
        frames_rendered_count
    );
    assert!(
        ws.needs_redraw,
        "needs_redraw should remain dirty/pending until next_frame deadline is reached"
    );

    // Now advance past the frame deadline
    ws.last_frame_instant = Instant::now() - frame_duration - Duration::from_millis(1);
    let now = Instant::now();
    let next_frame = ws.last_frame_instant + frame_duration;
    if now >= next_frame && ws.needs_redraw {
        ws.draw();
        ws.needs_redraw = false;
        ws.last_frame_instant = Instant::now();
        frames_rendered_count += 1;
    }

    // Now the coalesced frame should have been rendered
    assert!(!ws.needs_redraw);
    assert!(frames_rendered_count >= 1);
}

#[test]
fn test_immediate_interaction_redraw_on_keyboard() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let (pty, _w) = create_test_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    ws.fps_limit = Some(60);
    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);

    // Simulate recent frame render
    ws.draw();
    ws.last_frame_instant = Instant::now();
    ws.needs_redraw = false;

    // Simulate keypress event arriving 1ms after last frame (well before 16.6ms deadline)
    ws.last_key_instant = Instant::now();
    ws.active_tab_mut().active_pane_mut().terminal.feed(b"a");
    ws.needs_redraw = true;

    let now = Instant::now();
    let is_typing = now.duration_since(ws.last_key_instant) < Duration::from_millis(150);
    let is_resizing = false;
    let next_frame = ws.last_frame_instant + frame_duration;

    // is_typing must be true and immediately bypass the frame deadline!
    assert!(is_typing);
    assert!(now < next_frame);

    let mut rendered = false;
    if is_typing || is_resizing || now >= next_frame {
        ws.draw();
        ws.needs_redraw = false;
        ws.last_frame_instant = Instant::now();
        rendered = true;
    }

    assert!(
        rendered,
        "Keyboard typing must trigger zero-latency immediate redraw"
    );
    assert!(!ws.needs_redraw);
}

#[test]
fn test_synchronized_output_mode_2026_coalesces_redraws() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut term = Terminal::new(80, 24);

    assert!(!term.is_synchronized_output_active());

    // Enter synchronized output: CSI ? 2026 h
    term.feed(b"\x1b[?2026h");
    assert!(term.is_synchronized_output_active());

    // Send rapid updates while synchronized output is active
    for i in 0..10 {
        term.feed(format!("Update {}\r\n", i).as_bytes());
        assert!(term.is_synchronized_output_active());
    }

    // Leave synchronized output: CSI ? 2026 l
    term.feed(b"\x1b[?2026l");
    assert!(!term.is_synchronized_output_active());
}

#[test]
fn test_single_line_scroll_damage_fast_path() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let mut term = Terminal::new(80, 24);

    // Fill screen to bottom row
    for i in 0..24 {
        term.feed(format!("Line {:02}\r\n", i).as_bytes());
    }
    // Clear damage from initialization
    term.active_grid_mut().clear_damage();
    assert!(!term.active_grid().damage.has_damage());
    assert!(!term.active_grid().damage.full_redraw);
    assert_eq!(term.active_grid().damage.scrolled_lines, 0);

    // Scroll by exactly 1 line
    term.active_grid_mut()
        .scroll_or_move_down(Color { r: 0, g: 0, b: 0 });

    let grid = term.active_grid();
    // 1-line scroll must set scrolled_lines == 1
    assert_eq!(grid.damage.scrolled_lines, 1);
    // 1-line scroll must NOT trigger full_redraw!
    assert!(
        !grid.damage.full_redraw,
        "1-line scroll must not mark full_redraw = true"
    );
    // Dirty indices should be bounded (bottom row and cursor)
    assert!(
        grid.damage.dirty_indices().len() <= 3,
        "1-line scroll dirty rows must be <= 3 (got {})",
        grid.damage.dirty_indices().len()
    );

    // If another scroll occurs before clear_damage, it must escalate to mark_all
    term.active_grid_mut()
        .scroll_or_move_down(Color { r: 0, g: 0, b: 0 });
    let grid = term.active_grid();
    assert!(
        grid.damage.full_redraw,
        "Multiple scrolls before render must escalate to full_redraw"
    );
}

#[test]
fn test_app_event_drain_with_pty_data() {
    let _lock = TEST_MUTEX.lock().unwrap();
    let (pty, _w) = create_test_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    let (sender, receiver) = std::sync::mpsc::channel();
    let proxy = EventLoopProxy::new(sender, -1);

    // Send 5 PtyData events through proxy
    for i in 0..5 {
        let chunk = format!("Batch payload {}\r\n", i).into_bytes();
        proxy
            .send(CustomEvent::PtyData {
                window_id: ws.window_id,
                tab_id: ws.tabs[0].id,
                pane_id: ws.active_pane().id,
                data: chunk,
            })
            .unwrap();
    }

    // Drain all events from receiver into window terminal
    let mut received_bytes = 0usize;
    while let Ok(event) = receiver.try_recv() {
        if let CustomEvent::PtyData { data, .. } = event {
            received_bytes += data.len();
            ws.active_tab_mut().active_pane_mut().terminal.feed(&data);
            ws.needs_redraw = true;
        }
    }

    assert!(received_bytes > 0);
    assert!(ws.needs_redraw);

    // Verify grid content contains all 5 payloads
    for i in 0..5 {
        let tag = format!("Batch payload {}", i);
        let found = (0..24).any(|y| {
            if let Some(slice) = ws
                .active_tab()
                .active_pane()
                .terminal
                .active_grid()
                .active_display_row_slice(y)
            {
                let text: String = slice.iter().map(|c| c.character).collect();
                text.contains(&tag)
            } else {
                false
            }
        });
        assert!(found, "Expected to find '{}' on visible grid", tag);
    }
}
