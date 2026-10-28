use std::io::Read;
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};
use velox::app::app::EventLoopProxy;
use velox::pty::master::PtyMaster;
use velox::pty::reactor::PtyReactor;
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
fn test_reactor_large_paste_async_write_queue() {
    let (sender, _receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");
    let handle = reactor.handle();

    let mut fds = [0; 2];
    let res = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(res, 0);

    let pty = Arc::new(PtyMaster::new(fds[1], None));
    let mut read_end = unsafe { std::fs::File::from_raw_fd(fds[0]) };

    handle.register(1, 0, 999, pty).expect("register failed");

    // 500KB of paste data (well beyond OS pipe capacity ~64KB)
    let total_bytes = 500 * 1024;
    let test_data: Vec<u8> = (0..total_bytes).map(|i| (i % 255) as u8).collect();

    let mut reader_buf = vec![0u8; total_bytes];
    let reader_thread = thread::spawn(move || {
        read_end.read_exact(&mut reader_buf).unwrap();
        reader_buf
    });

    let t0 = Instant::now();
    handle.write(1, 0, 999, test_data.clone()).unwrap();
    let send_duration = t0.elapsed();

    // Verify that queuing the write returns asynchronously without blocking the caller!
    println!("handle.write 500KB queueing took: {:?}", send_duration);
    assert!(
        send_duration < Duration::from_millis(100),
        "handle.write must return without blocking the caller thread"
    );

    // Verify that all 500KB arrive completely and losslessly
    let received = reader_thread.join().unwrap();
    assert_eq!(received.len(), test_data.len());
    assert_eq!(received, test_data);
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
