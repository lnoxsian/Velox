use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;
use velox::app::app::{CustomEvent, EventLoopProxy, WindowState};
use velox::app::pane::Pane;
use velox::app::split::SplitDirection;
use velox::pty::master::PtyMaster;
use velox::pty::reactor::PtyReactor;
use velox::terminal::terminal::Terminal;

fn create_test_pipe() -> (Arc<PtyMaster>, File) {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(res, 0);

    let pty = Arc::new(PtyMaster::new(fds[0], None));
    let write_end = unsafe { File::from_raw_fd(fds[1]) };
    (pty, write_end)
}

fn get_process_thread_count() -> usize {
    std::fs::read_dir("/proc/self/task")
        .map(|entries| entries.count())
        .unwrap_or(1)
}

#[test]
fn test_reactor_lifecycle_and_output_routing() {
    let (sender, receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let reactor = PtyReactor::new(proxy).expect("Failed to create PtyReactor");

    // 1. Initial pane registration
    let (pty1, mut write1) = create_test_pipe();
    reactor.register(1, 0, 0, pty1).unwrap();

    // 2. Split pane registration
    let (pty2, mut write2) = create_test_pipe();
    let pty2_keepalive = pty2.clone();
    reactor.register(1, 0, 1, pty2).unwrap();

    // 3. Tab creation registration
    let (pty3, mut write3) = create_test_pipe();
    let pty3_keepalive = pty3.clone();
    reactor.register(1, 1, 2, pty3).unwrap();

    // Send data to each
    write1.write_all(b"hello pane 0\n").unwrap();
    write2.write_all(b"hello pane 1\n").unwrap();
    write3.write_all(b"hello tab 1 pane 2\n").unwrap();

    let mut events = HashMap::new();
    for _ in 0..3 {
        let ev = receiver
            .recv_timeout(Duration::from_millis(500))
            .expect("Timed out waiting for PtyData");
        if let CustomEvent::PtyData {
            window_id,
            tab_id,
            pane_id,
            data,
        } = ev
        {
            events.insert((window_id, tab_id, pane_id), data);
        }
    }

    assert_eq!(
        events.get(&(1, 0, 0)).map(|v| v.as_slice()),
        Some(&b"hello pane 0\n"[..])
    );
    assert_eq!(
        events.get(&(1, 0, 1)).map(|v| v.as_slice()),
        Some(&b"hello pane 1\n"[..])
    );
    assert_eq!(
        events.get(&(1, 1, 2)).map(|v| v.as_slice()),
        Some(&b"hello tab 1 pane 2\n"[..])
    );

    // 4. Close pane unregistration
    reactor.unregister(1, 0, 1).unwrap();
    std::thread::sleep(Duration::from_millis(20));

    // Writing to pane 1 must not produce events
    write2.write_all(b"ignored data").unwrap();
    let res = receiver.recv_timeout(Duration::from_millis(50));
    assert!(res.is_err(), "Expected no events for unregistered pane");
    drop(pty2_keepalive);

    // 5. Close tab unregistration
    reactor.unregister(1, 1, 2).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    write3.write_all(b"tab closed data").unwrap();
    let res = receiver.recv_timeout(Duration::from_millis(50));
    assert!(res.is_err(), "Expected no events for closed tab pane");
    drop(pty3_keepalive);
}

#[test]
fn test_persistent_thread_count_invariance() {
    let (sender, _receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let mut reactor = PtyReactor::new(proxy).expect("Failed to create PtyReactor");

    // Give reactor thread time to initialize
    std::thread::sleep(Duration::from_millis(20));
    let threads_before_panes = get_process_thread_count();

    // Register 10 panes across multiple tabs and splits
    let mut pipes = Vec::new();
    for i in 0..10 {
        let (pty, write_end) = create_test_pipe();
        reactor.register(1, i / 2, i, pty).unwrap();
        pipes.push(write_end);
    }

    // Thread count MUST NOT increase when opening 10 panes!
    let threads_with_10_panes = get_process_thread_count();
    assert_eq!(
        threads_with_10_panes, threads_before_panes,
        "Opening 10 panes must NOT increase persistent thread count (before: {}, with 10 panes: {})",
        threads_before_panes, threads_with_10_panes
    );

    // Unregister all panes
    for i in 0..10 {
        reactor.unregister(1, i / 2, i).unwrap();
    }

    let threads_after_unregister = get_process_thread_count();
    assert_eq!(
        threads_after_unregister, threads_before_panes,
        "Thread count must remain steady after unregistering panes"
    );

    // Shutdown reactor
    reactor.shutdown();
}

#[test]
fn test_window_state_split_and_tab_reactor_lifecycle() {
    let (sender, receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let reactor = PtyReactor::new(proxy).expect("Failed to create PtyReactor");

    let (initial_pty, mut initial_write) = create_test_pipe();
    let initial_keepalive = initial_pty.clone();
    let mut ws = WindowState::create_test_window_state(initial_pty.clone(), 80, 24);
    // Wire real reactor handle into WindowState
    ws.pty_reactor = reactor.handle();

    // Register initial pane
    reactor.register(ws.window_id, 0, 0, initial_pty).unwrap();

    initial_write.write_all(b"from initial pane\n").unwrap();
    let ev = receiver.recv_timeout(Duration::from_millis(500)).unwrap();
    if let CustomEvent::PtyData { pane_id, data, .. } = ev {
        assert_eq!(pane_id, 0);
        assert_eq!(data.as_slice(), b"from initial pane\n");
    } else {
        panic!("Expected PtyData");
    }

    // Add a second pane to tab 0
    let (pane2_pty, mut pane2_write) = create_test_pipe();
    let pane2_keepalive = pane2_pty.clone();
    let pane2_id = ws.next_pane_id;
    ws.next_pane_id += 1;
    let new_pane = Pane::new(
        pane2_id,
        pane2_pty.clone(),
        Terminal::new(40, 24),
        14.0,
        false,
    );
    ws.active_tab_mut()
        .tree
        .split_pane(0, new_pane, SplitDirection::Vertical, 0.5, 1);
    ws.pty_reactor
        .register(ws.window_id, 0, pane2_id, pane2_pty)
        .unwrap();

    pane2_write.write_all(b"from split pane\n").unwrap();
    let ev = receiver.recv_timeout(Duration::from_millis(500)).unwrap();
    if let CustomEvent::PtyData { pane_id, data, .. } = ev {
        assert_eq!(pane_id, pane2_id);
        assert_eq!(data.as_slice(), b"from split pane\n");
    } else {
        panic!("Expected PtyData for split pane");
    }

    // Close split pane
    assert!(!ws.close_pane_in_tab(0, pane2_id));
    std::thread::sleep(Duration::from_millis(20));

    // Write to pane 2 should not produce events
    pane2_write.write_all(b"after close\n").unwrap();
    let res = receiver.recv_timeout(Duration::from_millis(50));
    assert!(res.is_err(), "No events expected after closing split pane");
    drop(pane2_keepalive);

    // Close remaining tab / pane
    assert!(ws.close_tab(0));
    std::thread::sleep(Duration::from_millis(20));
    initial_write.write_all(b"after tab close\n").unwrap();
    let res = receiver.recv_timeout(Duration::from_millis(50));
    assert!(res.is_err(), "No events expected after closing tab");
    drop(initial_keepalive);
}
