use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use velox::app::app::{CustomEvent, EventLoopProxy};
use velox::app::pane::Pane;
use velox::pty::master::PtyMaster;
use velox::pty::reactor::PtyReactor;
use velox::terminal::terminal::Terminal;

fn create_socketpair() -> (Arc<PtyMaster>, File) {
    let mut fds = [0; 2];
    let res = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(res, 0, "socketpair creation must succeed");

    let pty = Arc::new(PtyMaster::new(fds[0], None));
    let peer = unsafe { File::from_raw_fd(fds[1]) };
    (pty, peer)
}

#[test]
fn test_pty_reactor_short_writes_and_backpressure() {
    let (sender, _receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");
    let handle = reactor.handle();

    let (pty, mut read_end) = create_socketpair();

    // Restrict send buffer to force partial writes and backpressure
    let sndbuf: libc::c_int = 4096;
    let res = unsafe {
        libc::setsockopt(
            pty.fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &sndbuf as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    assert_eq!(res, 0, "setsockopt SO_SNDBUF failed");

    handle.register(1, 0, 10, pty).expect("register failed");

    // 48 KB test data exceeds the 4KB socket buffer, forcing EPOLLOUT queueing
    let total_bytes = 48 * 1024;
    let test_data: Vec<u8> = (0..total_bytes).map(|i| (i % 251) as u8).collect();

    let reader_data = test_data.clone();
    let reader_thread = thread::spawn(move || {
        let mut received = Vec::with_capacity(reader_data.len());
        let mut buf = [0u8; 1024];
        while received.len() < reader_data.len() {
            match read_end.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    received.extend_from_slice(&buf[..n]);
                    // Small sleep to ensure backpressure is exerted on the reactor writer
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("read failed: {}", e),
            }
        }
        received
    });

    handle.write(1, 0, 10, test_data.clone()).unwrap();

    let received = reader_thread.join().unwrap();
    assert_eq!(received.len(), test_data.len());
    assert_eq!(
        received, test_data,
        "All bytes under backpressure must be delivered losslessly"
    );
}

#[test]
fn test_pty_reactor_multi_write_ordering_under_backpressure() {
    let (sender, _receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");
    let handle = reactor.handle();

    let (pty, mut read_end) = create_socketpair();

    // Restrict send buffer to 4096 bytes
    let sndbuf: libc::c_int = 4096;
    let _ = unsafe {
        libc::setsockopt(
            pty.fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &sndbuf as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };

    handle.register(1, 0, 20, pty).expect("register failed");

    // Three distinct bursts of 8KB each
    let chunk_a = vec![b'A'; 8192];
    let chunk_b = vec![b'B'; 8192];
    let chunk_c = vec![b'C'; 8192];

    let mut expected = Vec::with_capacity(8192 * 3);
    expected.extend_from_slice(&chunk_a);
    expected.extend_from_slice(&chunk_b);
    expected.extend_from_slice(&chunk_c);

    let total_len = expected.len();
    let reader_thread = thread::spawn(move || {
        let mut received = Vec::with_capacity(total_len);
        let mut buf = [0u8; 512];
        while received.len() < total_len {
            match read_end.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    received.extend_from_slice(&buf[..n]);
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("read failed: {}", e),
            }
        }
        received
    });

    // Enqueue writes in rapid succession
    handle.write(1, 0, 20, chunk_a).unwrap();
    handle.write(1, 0, 20, chunk_b).unwrap();
    handle.write(1, 0, 20, chunk_c).unwrap();

    let received = reader_thread.join().unwrap();
    assert_eq!(received.len(), expected.len());
    assert_eq!(
        received, expected,
        "Multiple writes under backpressure must strictly preserve FIFO ordering without duplicates or interleaving"
    );
}

#[test]
fn test_pty_reactor_closed_pty_exit_reporting() {
    let (sender, receiver) = channel();
    let proxy = EventLoopProxy::new(sender, -1);
    let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");
    let handle = reactor.handle();

    let (pty, peer) = create_socketpair();
    handle.register(1, 0, 99, pty).expect("register failed");

    // Give reactor a moment to register
    thread::sleep(Duration::from_millis(20));

    // Explicitly drop/close the peer socket (simulating child terminal exit)
    drop(peer);

    // Give kernel a moment to register socket close
    thread::sleep(Duration::from_millis(20));

    // Writing to a closed peer socket triggers EPIPE / EIO
    handle.write(1, 0, 99, vec![b'x'; 256]).unwrap();

    // Reactor must detect closed write and report CustomEvent::PtyExit
    let event = receiver
        .recv_timeout(Duration::from_millis(1000))
        .expect("PtyExit event expected upon writing to closed PTY");

    match event {
        CustomEvent::PtyExit {
            window_id,
            tab_id,
            pane_id,
        } => {
            assert_eq!(window_id, 1);
            assert_eq!(tab_id, 0);
            assert_eq!(pane_id, 99);
        }
        _ => panic!("Expected PtyExit event"),
    }
}

#[test]
fn test_pane_flush_outgoing_partial_and_eagain_preservation() {
    let (pty, mut read_end) = create_socketpair();
    pty.set_nonblocking().expect("set_nonblocking failed");

    // Fill the send buffer to capacity to trigger EAGAIN
    let filler = vec![0xAAu8; 16384];
    loop {
        match pty.write(&filler) {
            Ok(_) => {}
            Err(e)
                if e.raw_os_error() == Some(libc::EAGAIN)
                    || e.raw_os_error() == Some(libc::EWOULDBLOCK) =>
            {
                break;
            }
            Err(e) => panic!("unexpected error filling socket: {}", e),
        }
    }

    let terminal = Terminal::new(80, 24);
    let mut pane = Pane::new(1, pty.clone(), terminal, 14.0, false);

    // Place 50 test bytes into terminal.outgoing
    pane.terminal.outgoing = (0..50).map(|i| i as u8).collect();

    // When socket buffer is saturated, flush_outgoing must return Ok(0)
    // and PRESERVE all 50 bytes in terminal.outgoing!
    let res = pane.flush_outgoing();
    assert_eq!(res.unwrap(), 0);
    assert_eq!(
        pane.terminal.outgoing.len(),
        50,
        "Outgoing buffer must NOT be cleared when write would block"
    );

    // Make read_end nonblocking so we can drain accurately
    let _ = unsafe {
        libc::fcntl(
            std::os::fd::AsRawFd::as_raw_fd(&read_end),
            libc::F_SETFL,
            libc::O_NONBLOCK,
        )
    };

    // Drain all backpressured bytes to restore write capacity
    let mut drain_all = vec![0u8; 65536];
    while let Ok(n) = read_end.read(&mut drain_all) {
        if n == 0 {
            break;
        }
    }

    // Socket now has space. Flush outgoing again.
    let flushed = pane.flush_outgoing().unwrap();
    assert!(flushed > 0, "Must flush available capacity");
    assert_eq!(
        pane.terminal.outgoing.len(),
        50 - flushed,
        "Only flushed bytes should be drained from outgoing buffer"
    );

    // Flush the remainder
    let _ = pane.flush_outgoing();
    assert!(
        pane.terminal.outgoing.is_empty(),
        "Outgoing buffer should be empty after socket drained"
    );

    // Drop the read end to simulate closed PTY
    drop(read_end);
    pane.terminal.outgoing = vec![b'Z'; 10];
    let err = pane.flush_outgoing();
    assert!(
        err.is_err(),
        "flush_outgoing on closed PTY must return an error"
    );
}

#[test]
fn test_pty_master_write_empty_and_dummy() {
    let (pty, _read_end) = create_socketpair();

    // Writing empty slice returns Ok(0)
    assert_eq!(pty.write(&[]).unwrap(), 0);

    // Dummy PTY returns Ok(len)
    let dummy = PtyMaster::dummy();
    assert_eq!(dummy.write(b"test").unwrap(), 4);
    assert_eq!(dummy.write(&[]).unwrap(), 0);
}

#[test]
fn test_pty_master_write_eintr_handling() {
    extern "C" fn sig_handler(_: libc::c_int) {}

    // Register a signal handler for SIGUSR1 without SA_RESTART
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = sig_handler as *const () as usize;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGUSR1, &sa, std::ptr::null_mut());
    }

    let (pty, mut read_end) = create_socketpair();

    // Spawn a thread that signals this thread with SIGUSR1 during write
    let target_tid = unsafe { libc::pthread_self() };
    let signaling_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        unsafe {
            libc::pthread_kill(target_tid, libc::SIGUSR1);
        }
    });

    let msg = b"Testing EINTR handling in PtyMaster::write";
    let written = pty
        .write(msg)
        .expect("write should handle EINTR gracefully and complete");
    assert_eq!(written, msg.len());

    signaling_thread.join().unwrap();

    let mut buf = vec![0u8; msg.len()];
    read_end.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, msg);
}
