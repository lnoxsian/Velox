use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use velox::app::app::{CustomEvent, EventLoopProxy};
use velox::ipc::{self, IpcMessage, IpcResponse};
use velox::pty::PtyReactor;
use velox::pty::master::PtyMaster;
use velox::pty::process::spawn_process;
use velox::window::x11::shm::ShmSegmentGuard;

#[test]
fn test_pty_master_lifecycle_and_double_close_prevention() {
    let mut fds = [0; 2];
    let res = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(res, 0);

    let master_fd = fds[0];
    let slave_fd = fds[1];

    {
        let master = PtyMaster::new(master_fd, Some(99999));
        assert_eq!(master.as_raw_fd(), master_fd);
        assert_eq!(master.child_pid, Some(99999));

        // Write through master
        master.write(b"lifecycle test\n").expect("Master write");

        let mut read_buf = [0u8; 32];
        let n = unsafe {
            libc::read(
                slave_fd,
                read_buf.as_mut_ptr() as *mut libc::c_void,
                read_buf.len(),
            )
        };
        assert_eq!(&read_buf[..n as usize], b"lifecycle test\n");

        // Write through slave, read through master
        let s_msg = b"from slave\n";
        let written =
            unsafe { libc::write(slave_fd, s_msg.as_ptr() as *const libc::c_void, s_msg.len()) };
        assert_eq!(written as usize, s_msg.len());

        let mut m_buf = [0u8; 32];
        let m_read = master.read(&mut m_buf).expect("Master read");
        assert_eq!(&m_buf[..m_read], b"from slave\n");
    } // master drops here, should close master_fd

    // Verify master_fd is closed (reading slave should now see EOF or EBADF on master_fd)
    let mut eof_buf = [0u8; 8];
    let eof_res = unsafe {
        libc::read(
            slave_fd,
            eof_buf.as_mut_ptr() as *mut libc::c_void,
            eof_buf.len(),
        )
    };
    assert_eq!(
        eof_res, 0,
        "Slave must see EOF after master drops and closes its fd"
    );

    unsafe {
        libc::close(slave_fd);
    }

    // Dummy master lifecycle
    let dummy = PtyMaster::dummy();
    assert_eq!(dummy.as_raw_fd(), -1);
    assert_eq!(dummy.child_pid, None);
    assert!(dummy.get_foreground_process_name().is_none());
    assert!(dummy.get_foreground_process_cwd().is_none());
    assert_eq!(dummy.read(&mut [0u8; 8]).unwrap(), 0);
    assert_eq!(dummy.write(b"data").unwrap(), 4);
    assert!(dummy.resize(80, 24).is_ok());
    assert!(dummy.set_nonblocking().is_ok());
    drop(dummy); // dummy drop must be a no-op
}

#[test]
fn test_pty_reactor_clean_shutdown_and_join() {
    let (tx, rx) = channel();
    let proxy = EventLoopProxy::new(tx, -1);
    let mut reactor = PtyReactor::new(proxy).expect("Create PtyReactor");

    assert!(!reactor.is_stopped());
    let handle = reactor.handle();
    assert!(!handle.is_stopped());

    // Register a test pipe
    let mut fds = [0; 2];
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
        0
    );
    let master = Arc::new(PtyMaster::new(fds[0], None));
    reactor
        .register(1, 0, 10, master)
        .expect("Register pty in reactor");

    // Write through slave
    let slave = unsafe { std::fs::File::from_raw_fd(fds[1]) };
    let mut s_write = slave;
    s_write.write_all(b"reactor active\n").unwrap();

    let event = rx
        .recv_timeout(Duration::from_millis(500))
        .expect("Receive pty data");
    match event {
        CustomEvent::PtyData { pane_id, data, .. } => {
            assert_eq!(pane_id, 10);
            assert_eq!(data.as_slice(), b"reactor active\n");
        }
        _ => panic!("Unexpected event"),
    }

    // Perform shutdown
    let start = Instant::now();
    reactor.shutdown();
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "Reactor shutdown must complete promptly"
    );

    assert!(reactor.is_stopped());
    assert!(handle.is_stopped());

    // Subsequent commands on handle must fail gracefully without panicking or use-after-close
    let reg_res = handle.register(1, 0, 20, Arc::new(PtyMaster::dummy()));
    assert!(reg_res.is_err());
    assert!(reg_res.unwrap_err().contains("Reactor is stopped"));

    // Idempotent shutdown
    reactor.shutdown();
}

#[test]
fn test_event_loop_proxy_clone_and_drop_ordering() {
    let (tx, rx) = channel();
    let wake_raw = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
    assert!(wake_raw >= 0);

    let proxy = EventLoopProxy::new(tx, wake_raw);
    let mut clones = Vec::new();
    for _ in 0..5 {
        clones.push(proxy.clone());
    }

    // Drop original proxy
    drop(proxy);

    // Clones can still send and wake
    for (i, p) in clones.iter().enumerate() {
        p.send(CustomEvent::PtyExit {
            window_id: 1,
            tab_id: 0,
            pane_id: i as u64,
        })
        .expect("Send from clone");
    }

    // Verify events received
    for i in 0..5 {
        let ev = rx.recv_timeout(Duration::from_millis(200)).unwrap();
        match ev {
            CustomEvent::PtyExit { pane_id, .. } => assert_eq!(pane_id, i as u64),
            _ => panic!("Unexpected event"),
        }
    }

    // Drop rx; sending from clone should gracefully fail with SendError
    drop(rx);
    let send_res = clones[0].send(CustomEvent::PtyExit {
        window_id: 1,
        tab_id: 0,
        pane_id: 99,
    });
    assert!(send_res.is_err());

    // Drop all clones; the owned eventfd is cleanly dropped
    drop(clones);
}

#[test]
fn test_ipc_listener_deterministic_shutdown_and_thread_join() {
    let (tx, _rx) = channel();
    let proxy = EventLoopProxy::new(tx, -1);

    let socket_path =
        std::env::temp_dir().join(format!("velox-test-ipc-{}.sock", std::process::id()));
    unsafe {
        std::env::set_var("VELOX_IPC_SOCKET", socket_path.to_str().unwrap());
    }

    let mut handle = ipc::start_ipc_server(proxy).expect("Start IPC server");
    assert!(
        socket_path.exists(),
        "Socket path must exist while listening"
    );

    // Send a Ping to verify it's responsive
    let ping_resp = ipc::send_ipc_message(&IpcMessage::Ping).expect("Send IPC Ping");
    match ping_resp {
        IpcResponse::Ok => {}
        _ => panic!("Expected Ok response to Ping"),
    }

    // Deterministic shutdown
    let start = Instant::now();
    handle.shutdown();
    let duration = start.elapsed();

    assert!(
        duration < Duration::from_millis(500),
        "IPC shutdown and thread join must complete in < 500ms (took {:?})",
        duration
    );
    assert!(
        !socket_path.exists(),
        "Socket path must be unlinked after shutdown"
    );

    // Clean up env
    unsafe {
        std::env::remove_var("VELOX_IPC_SOCKET");
    }
}

#[test]
fn test_shm_guard_cleanup_on_mmap_or_attach_failure() {
    // 1. Test ShmSegmentGuard
    let shm_id = unsafe { libc::shmget(libc::IPC_PRIVATE, 4096, libc::IPC_CREAT | 0o600) };
    assert!(shm_id >= 0, "shmget failed");
    let addr = unsafe { libc::shmat(shm_id, std::ptr::null(), 0) };
    assert_ne!(addr, -1isize as *mut libc::c_void, "shmat failed");

    {
        let _guard = ShmSegmentGuard { shm_id, addr };
        // Guard drops here
    }

    // Verify segment is detached/marked for deletion
    let mut ds: libc::shmid_ds = unsafe { std::mem::zeroed() };
    let ctl_res = unsafe { libc::shmctl(shm_id, libc::IPC_STAT, &mut ds) };
    if ctl_res == 0 {
        assert_eq!(
            ds.shm_nattch, 0,
            "Attached count must be 0 after guard drop"
        );
    }
}

#[test]
fn test_spawn_process_terminal_execution() {
    let pty = spawn_process(
        "/bin/sh",
        Some(&[
            "sh".to_string(),
            "-c".to_string(),
            "printf 'process_spawn_ok\n'".to_string(),
        ]),
        None,
    )
    .expect("Spawn process");

    assert!(pty.as_raw_fd() >= 0);
    assert!(pty.child_pid.is_some());

    let mut buf = [0u8; 64];
    let mut accumulated = Vec::new();
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(n) = pty.read(&mut buf)
            && n > 0
        {
            accumulated.extend_from_slice(&buf[..n]);
            if accumulated.windows(16).any(|w| w == b"process_spawn_ok") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        accumulated.windows(16).any(|w| w == b"process_spawn_ok"),
        "Spanned process must produce expected output; got {:?}",
        String::from_utf8_lossy(&accumulated)
    );
}
