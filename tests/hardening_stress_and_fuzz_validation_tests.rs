use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use velox::app::app::{CustomEvent, EventLoopProxy};
use velox::ipc::{IpcMessage, send_ipc_message_to, start_ipc_server_at};
use velox::pty::buffer_pool::recycle_pty_buffer;
use velox::pty::master::PtyMaster;
use velox::pty::reactor::PtyReactor;
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::scrollback::Scrollback;
use velox::terminal::terminal::Terminal;

fn create_socketpair() -> (Arc<PtyMaster>, File) {
    let mut fds = [0; 2];
    let res = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(res, 0, "socketpair creation must succeed");

    let pty = Arc::new(PtyMaster::new(fds[0], None));
    let peer = unsafe { File::from_raw_fd(fds[1]) };
    (pty, peer)
}

fn set_socket_buffer_size(fd: i32, size: i32) {
    let res = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &size as *const _ as *const libc::c_void,
            std::mem::size_of::<i32>() as libc::socklen_t,
        )
    };
    assert_eq!(res, 0, "setsockopt SO_SNDBUF failed");
}

#[test]
fn test_high_rate_pty_output_burst_and_streaming_throughput() {
    let (sender, receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let mut reactor = PtyReactor::new(proxy).expect("PtyReactor creation failed");

    let (pty, mut peer) = create_socketpair();
    reactor.register(1, 0, 10, pty).unwrap();

    let total_bursts = 100;
    let burst_size = 100 * 1024; // 100 KB
    let total_expected_bytes = total_bursts * burst_size; // 10 MB

    let writer_thread = thread::spawn(move || {
        let chunk = vec![b'X'; burst_size];
        for _ in 0..total_bursts {
            let mut written = 0;
            while written < chunk.len() {
                match peer.write(&chunk[written..]) {
                    Ok(n) if n > 0 => written += n,
                    Ok(_) => thread::sleep(Duration::from_millis(1)),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => panic!("writer error: {}", e),
                }
            }
        }
        let _ = peer.flush();
        drop(peer);
    });

    let mut total_received = 0;
    let mut term = Terminal::new(120, 40);
    let start = Instant::now();

    while total_received < total_expected_bytes {
        if start.elapsed() > Duration::from_secs(15) {
            panic!(
                "Timed out waiting for high-rate PTY data; received {}/{}",
                total_received, total_expected_bytes
            );
        }
        match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(CustomEvent::PtyData { data, .. }) => {
                total_received += data.len();
                term.feed(&data);
                recycle_pty_buffer(data);
            }
            Ok(CustomEvent::PtyExit { .. }) => break,
            Ok(_) => {}
            Err(_) => {}
        }
    }

    writer_thread.join().expect("Writer thread panic");
    reactor.shutdown();

    assert_eq!(
        total_received, total_expected_bytes,
        "All 10MB of streamed PTY bursts must be received intact without byte loss"
    );
    assert!(term.grid.cursor.x <= 120);
    assert!(term.grid.cursor.y < 40);
}

#[test]
fn test_pty_reactor_bidirectional_saturation() {
    let (sender, receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let mut reactor = PtyReactor::new(proxy).expect("PtyReactor creation failed");
    let handle = reactor.handle();

    let num_panes = 4;
    let bytes_per_pane = 32 * 1024; // 32 KB
    let mut peers = Vec::new();

    for pane_id in 1..=num_panes {
        let (pty, peer) = create_socketpair();
        set_socket_buffer_size(pty.fd, 4096);
        set_socket_buffer_size(peer.as_raw_fd(), 4096);
        handle.register(1, 0, pane_id, pty).unwrap();
        peers.push((pane_id, peer));
    }

    // Write from reactor to peers (outbound)
    for &(pane_id, _) in &peers {
        let test_pattern: Vec<u8> = (0..bytes_per_pane)
            .map(|i| ((i + pane_id as usize) % 251) as u8)
            .collect();
        handle.write(1, 0, pane_id, test_pattern).unwrap();
    }

    // Spawn reader and writer threads for each peer
    let mut worker_handles = Vec::new();
    for (pane_id, mut peer) in peers {
        let handle = thread::spawn(move || {
            // Write to reactor (inbound)
            let inbound_data = vec![(pane_id as u8) + 10; bytes_per_pane];
            let mut written = 0;
            while written < inbound_data.len() {
                match peer.write(&inbound_data[written..]) {
                    Ok(n) if n > 0 => written += n,
                    Ok(_) => thread::sleep(Duration::from_millis(1)),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => panic!("peer write error: {}", e),
                }
            }

            // Read outbound data sent by reactor
            let mut read_buf = vec![0u8; bytes_per_pane];
            let mut total_read = 0;
            while total_read < bytes_per_pane {
                match peer.read(&mut read_buf[total_read..]) {
                    Ok(0) => break,
                    Ok(n) => total_read += n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => panic!("peer read error: {}", e),
                }
            }

            // Validate outbound data
            let expected_pattern: Vec<u8> = (0..bytes_per_pane)
                .map(|i| ((i + pane_id as usize) % 251) as u8)
                .collect();
            assert_eq!(read_buf, expected_pattern);
        });
        worker_handles.push(handle);
    }

    // Collect inbound CustomEvent::PtyData from reactor
    let total_expected_inbound = num_panes as usize * bytes_per_pane;
    let mut total_inbound_received = 0;
    let start = Instant::now();

    while total_inbound_received < total_expected_inbound {
        if start.elapsed() > Duration::from_secs(5) {
            panic!(
                "Timed out collecting bidirectional saturation events: {}/{}",
                total_inbound_received, total_expected_inbound
            );
        }
        if let Ok(CustomEvent::PtyData { data, .. }) =
            receiver.recv_timeout(Duration::from_millis(200))
        {
            total_inbound_received += data.len();
            recycle_pty_buffer(data);
        }
    }

    for h in worker_handles {
        h.join().expect("Worker thread panicked");
    }

    reactor.shutdown();
    assert_eq!(total_inbound_received, total_expected_inbound);
}

#[test]
fn test_ansi_parser_and_grid_fuzz_resilience() {
    let mut term = Terminal::new(80, 24);

    let pathological_corpus: Vec<&[u8]> = vec![
        // Truncated escape sequences
        b"\x1b",
        b"\x1b[",
        b"\x1b[?",
        b"\x1b]",
        b"\x1b]52;",
        b"\x1b]8;;",
        b"\x1b[1;",
        b"\x1b[38;2;",
        b"\x1b[38;2;255;",
        b"\x1b[38;2;255;255;",
        b"\x1b[48;5;",
        b"\x1bP",
        b"\x1b_",
        b"\x1b^",
        b"\x1b#",
        b"\x1b(",
        // Extreme and overflow parameters
        b"\x1b[65535;65535H",
        b"\x1b[9999999999999999999999999m",
        b"\x1b[4294967295C",
        b"\x1b[4294967295D",
        b"\x1b[4294967295A",
        b"\x1b[4294967295B",
        b"\x1b[1000000b",
        b"\x1b[1000000@",
        b"\x1b[1000000P",
        b"\x1b[1000000L",
        b"\x1b[1000000M",
        b"\x1b[0;1;2;3;4;5;7;8;9;21;22;23;24;25;27;28;29;30;31;32;33;34;35;36;37;38;39;40;41;42;43;44;45;46;47;48;49;90;91;92;93;94;95;96;97;100;101;102;103;104;105;106;107m",
        // Screen clear and rapid cursor jumps
        b"\x1b[2J\x1b[3J\x1b[H\x1b[J\x1b[1J\x1b[K\x1b[1K\x1b[2K",
        // Invalid UTF-8 bytes mixed with ANSI escapes
        b"\xff\xfe\xfd\x80\x81\x82\x1b[31m\xed\xa0\x80\x1b[0m\xc0\xaf\x80\xbf\xf0\x82\x82\xac",
        // Multi-combining marks and complex scripts on grid boundaries
        "a\u{0300}\u{0301}\u{0302}\u{0303}\u{0304}\u{0305}".as_bytes(),
        "नमस्ते दुनिया kṣa क्ष् kṣya क्ष्य".as_bytes(),
        "👨‍👩‍👦‍👦".as_bytes(),
    ];

    for chunk in pathological_corpus {
        term.feed(chunk);
        assert!(
            term.grid.cursor.x <= term.grid.width,
            "cursor.x {} exceeded grid width {}",
            term.grid.cursor.x,
            term.grid.width
        );
        assert!(
            term.grid.cursor.y < term.grid.height,
            "cursor.y {} exceeded grid height {}",
            term.grid.cursor.y,
            term.grid.height
        );
    }

    // Pseudo-random fuzz byte stream (100,000 bytes)
    let mut rng_state = 0x12345678u64;
    let mut fuzz_bytes = vec![0u8; 100_000];
    for b in fuzz_bytes.iter_mut() {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        *b = (rng_state >> 33) as u8;
    }

    term.feed(&fuzz_bytes);

    assert!(term.grid.cursor.x <= term.grid.width);
    assert!(term.grid.cursor.y < term.grid.height);
    assert_eq!(term.grid.width, 80);
    assert_eq!(term.grid.height, 24);
}

#[test]
fn test_scrollback_rapid_churn_50000_lines() {
    let hot_limit = 2000;
    let mut scrollback = Scrollback::new(hot_limit, true);

    let total_lines = 50_000usize;
    for i in 0..total_lines {
        let cell = Cell::new(
            (b'A' + (i % 26) as u8) as char,
            Color {
                r: (i % 256) as u8,
                g: ((i * 3) % 256) as u8,
                b: ((i * 7) % 256) as u8,
            },
            Color { r: 0, g: 0, b: 0 },
            if i % 2 == 0 {
                CellFlags::BOLD
            } else {
                CellFlags::UNDERLINE
            },
        );
        scrollback.push_line(&[cell], i % 4 == 0);
    }

    assert_eq!(scrollback.len(), total_lines);

    // Verify disk chunking occurred
    let expected_chunks = (total_lines - hot_limit) / 512;
    assert_eq!(scrollback.chunk_count(), expected_chunks);
    assert_eq!(scrollback.disk_rows(), (expected_chunks * 512) as u64);

    // Resident row count bounded
    let max_resident = hot_limit + 512 + 4 * 512;
    assert!(
        scrollback.resident_row_count() <= max_resident,
        "Resident row count {} exceeded limit {}",
        scrollback.resident_row_count(),
        max_resident
    );

    // Random historical access across disk chunks
    for &target in &[0, 1, 511, 512, 1024, 15000, 30000, 49999] {
        let row = scrollback.get_row(target).expect("Row lookup failed");
        let expected_char = (b'A' + (target % 26) as u8) as char;
        assert_eq!(row.cells[0].character, expected_char);
        let expected_flag = if target % 2 == 0 {
            CellFlags::BOLD
        } else {
            CellFlags::UNDERLINE
        };
        assert_eq!(row.cells[0].flags, expected_flag);
        assert_eq!(row.wrapped, target % 4 == 0);
    }
}

#[test]
fn test_pty_reactor_concurrent_hammer_and_shutdown_race() {
    let (sender, _receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let mut reactor = PtyReactor::new(proxy.clone()).expect("PtyReactor::new failed");
    let handle = reactor.handle();
    let running = Arc::new(AtomicBool::new(true));

    let mut threads = Vec::new();

    // 4 Registration/deregistration hammer threads
    for t_idx in 0..4 {
        let handle_clone = handle.clone();
        let running_clone = running.clone();
        threads.push(thread::spawn(move || {
            let mut iter = 0u64;
            while running_clone.load(Ordering::Acquire) && !handle_clone.is_stopped() {
                iter += 1;
                let pane_id = 1000 + t_idx * 100 + iter;
                let (pty, _) = create_socketpair();
                let _ = handle_clone.register(1, 0, pane_id, pty);
                thread::sleep(Duration::from_millis(1));
                let _ = handle_clone.unregister(1, 0, pane_id);
            }
        }));
    }

    // 4 Outbound write hammering threads
    for t_idx in 0..4 {
        let handle_clone = handle.clone();
        let running_clone = running.clone();
        threads.push(thread::spawn(move || {
            let data = vec![b'Z'; 256];
            while running_clone.load(Ordering::Acquire) && !handle_clone.is_stopped() {
                let _ = handle_clone.write(1, 0, 500 + t_idx, data.clone());
                thread::sleep(Duration::from_millis(1));
            }
        }));
    }

    // 4 EventLoopProxy sending threads
    for _ in 0..4 {
        let proxy_clone = proxy.clone();
        let running_clone = running.clone();
        threads.push(thread::spawn(move || {
            while running_clone.load(Ordering::Acquire) {
                let _ = proxy_clone.send(CustomEvent::PtyData {
                    window_id: 1,
                    tab_id: 0,
                    pane_id: 999,
                    data: vec![1, 2, 3],
                });
                thread::sleep(Duration::from_millis(1));
            }
        }));
    }

    // 3 Reader / consumer socket threads
    for p_id in 0..3 {
        let (pty, mut peer) = create_socketpair();
        let _ = handle.register(1, 0, 800 + p_id, pty);
        let running_clone = running.clone();
        threads.push(thread::spawn(move || {
            let mut buf = [0u8; 512];
            while running_clone.load(Ordering::Acquire) {
                match peer.read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
            }
        }));
    }

    // 1 Abrupt shutdown thread
    let shutdown_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        reactor.shutdown();
        running.store(false, Ordering::Release);
    });

    let start = Instant::now();
    shutdown_thread.join().expect("Shutdown thread failed");

    for t in threads {
        t.join().expect("Worker thread panicked during race");
    }

    assert!(
        start.elapsed() < Duration::from_secs(2),
        "Concurrent hammer and shutdown race took too long: {:?}",
        start.elapsed()
    );
}

#[test]
fn test_ipc_concurrent_connections_and_abrupt_shutdown_race() {
    let (sender, _receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let socket_path = std::path::PathBuf::from(format!(
        "/tmp/velox-test-ipc-stress-{}-{}.sock",
        std::process::id(),
        nanos
    ));

    let mut server =
        start_ipc_server_at(proxy, socket_path.clone()).expect("start_ipc_server_at must succeed");

    let running = Arc::new(AtomicBool::new(true));
    let mut client_threads = Vec::new();

    for _ in 0..8 {
        let sp = socket_path.clone();
        let run = running.clone();
        client_threads.push(thread::spawn(move || {
            while run.load(Ordering::Acquire) {
                let _ = send_ipc_message_to(&sp, &IpcMessage::Ping);
                thread::sleep(Duration::from_millis(2));
            }
        }));
    }

    thread::sleep(Duration::from_millis(60));
    let shutdown_start = Instant::now();
    server.shutdown();
    let shutdown_duration = shutdown_start.elapsed();
    running.store(false, Ordering::Release);

    for t in client_threads {
        t.join()
            .expect("Client thread panicked during abrupt shutdown");
    }

    assert!(
        shutdown_duration < Duration::from_millis(300),
        "IPC server shutdown took too long: {:?}",
        shutdown_duration
    );
    assert!(
        !socket_path.exists(),
        "Socket file must be unlinked upon server shutdown"
    );
}
