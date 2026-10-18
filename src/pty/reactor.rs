use crate::app::app::{CustomEvent, EventLoopProxy, WindowId};
use crate::pty::buffer_pool::{PTY_BUFFER_SIZE, acquire_pty_buffer, recycle_pty_buffer};
use crate::pty::master::PtyMaster;
use std::collections::HashMap;
use std::os::fd::RawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneKey {
    pub window_id: WindowId,
    pub tab_id: u64,
    pub pane_id: u64,
}

pub enum ReactorCommand {
    Register {
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
        pty: Arc<PtyMaster>,
    },
    Unregister {
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct PtyReactorHandle {
    command_tx: Sender<ReactorCommand>,
    wake_fd: RawFd,
    wake_pending: Arc<AtomicBool>,
    is_stopped: Arc<AtomicBool>,
}

impl PtyReactorHandle {
    pub fn dummy() -> Self {
        let (command_tx, _) = channel();
        Self {
            command_tx,
            wake_fd: -1,
            wake_pending: Arc::new(AtomicBool::new(false)),
            is_stopped: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn register(
        &self,
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
        pty: Arc<PtyMaster>,
    ) -> Result<(), String> {
        if self.is_stopped.load(Ordering::Acquire) {
            return Err("Reactor is stopped".to_string());
        }
        self.send_command(ReactorCommand::Register {
            window_id,
            tab_id,
            pane_id,
            pty,
        })
    }

    pub fn unregister(&self, window_id: WindowId, tab_id: u64, pane_id: u64) -> Result<(), String> {
        if self.is_stopped.load(Ordering::Acquire) {
            return Ok(());
        }
        self.send_command(ReactorCommand::Unregister {
            window_id,
            tab_id,
            pane_id,
        })
    }

    fn send_command(&self, cmd: ReactorCommand) -> Result<(), String> {
        self.command_tx
            .send(cmd)
            .map_err(|e| format!("Failed to send reactor command: {}", e))?;
        if self.wake_fd >= 0 && !self.wake_pending.swap(true, Ordering::AcqRel) {
            let val = 1u64;
            unsafe {
                libc::write(
                    self.wake_fd,
                    &val as *const _ as *const libc::c_void,
                    std::mem::size_of::<u64>(),
                );
            }
        }
        Ok(())
    }
}

pub struct PtyReactor {
    handle: PtyReactorHandle,
    wake_fd: RawFd,
    is_stopped: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

struct Registration {
    key: PaneKey,
    pty: Arc<PtyMaster>,
}

impl PtyReactor {
    pub fn new(proxy: EventLoopProxy) -> Result<Self, String> {
        let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epoll_fd < 0 {
            return Err(format!(
                "epoll_create1 failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let wake_fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if wake_fd < 0 {
            unsafe {
                libc::close(epoll_fd);
            }
            return Err(format!(
                "eventfd failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let mut ev = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: wake_fd as u64,
        };
        let res = unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, wake_fd, &mut ev) };
        if res < 0 {
            unsafe {
                libc::close(wake_fd);
                libc::close(epoll_fd);
            }
            return Err(format!(
                "Failed to register wake_fd with epoll: {}",
                std::io::Error::last_os_error()
            ));
        }

        let (command_tx, command_rx) = channel();
        let is_stopped = Arc::new(AtomicBool::new(false));
        let is_stopped_clone = is_stopped.clone();
        let wake_pending = Arc::new(AtomicBool::new(false));
        let wake_pending_clone = wake_pending.clone();

        let join_handle = thread::Builder::new()
            .name("velox-pty-reactor".to_string())
            .spawn(move || {
                run_reactor_loop(
                    epoll_fd,
                    wake_fd,
                    command_rx,
                    proxy,
                    wake_pending_clone,
                    is_stopped_clone,
                );
            })
            .map_err(|e| {
                unsafe {
                    libc::close(wake_fd);
                    libc::close(epoll_fd);
                }
                format!("Failed to spawn PTY reactor thread: {}", e)
            })?;

        let handle = PtyReactorHandle {
            command_tx,
            wake_fd,
            wake_pending,
            is_stopped: is_stopped.clone(),
        };

        Ok(Self {
            handle,
            wake_fd,
            is_stopped,
            join_handle: Some(join_handle),
        })
    }

    pub fn handle(&self) -> PtyReactorHandle {
        self.handle.clone()
    }

    pub fn register(
        &self,
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
        pty: Arc<PtyMaster>,
    ) -> Result<(), String> {
        self.handle.register(window_id, tab_id, pane_id, pty)
    }

    pub fn unregister(&self, window_id: WindowId, tab_id: u64, pane_id: u64) -> Result<(), String> {
        self.handle.unregister(window_id, tab_id, pane_id)
    }

    pub fn shutdown(&mut self) {
        if self.is_stopped.swap(true, Ordering::AcqRel) {
            return;
        }

        let _ = self.handle.command_tx.send(ReactorCommand::Shutdown);
        if self.wake_fd >= 0 {
            let val = 1u64;
            unsafe {
                libc::write(
                    self.wake_fd,
                    &val as *const _ as *const libc::c_void,
                    std::mem::size_of::<u64>(),
                );
            }
        }

        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }

        if self.wake_fd >= 0 {
            unsafe {
                libc::close(self.wake_fd);
            }
            self.wake_fd = -1;
        }
    }
}

impl Drop for PtyReactor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_reactor_loop(
    epoll_fd: RawFd,
    wake_fd: RawFd,
    command_rx: Receiver<ReactorCommand>,
    proxy: EventLoopProxy,
    wake_pending: Arc<AtomicBool>,
    is_stopped: Arc<AtomicBool>,
) {
    let mut fds: HashMap<RawFd, Registration> = HashMap::new();
    let mut key_to_fd: HashMap<PaneKey, RawFd> = HashMap::new();
    let mut events = [libc::epoll_event { events: 0, u64: 0 }; 64];

    'reactor: loop {
        let nfds = unsafe {
            libc::epoll_wait(
                epoll_fd,
                events.as_mut_ptr(),
                events.len() as i32,
                -1, // Block until an event occurs
            )
        };

        if nfds < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            log::error!("PtyReactor epoll_wait error: {}", err);
            break;
        }

        for i in 0..nfds as usize {
            let ev = events[i];
            let fd = ev.u64 as RawFd;

            if fd == wake_fd {
                // Drain wake_fd
                let mut buf = [0u8; 8];
                unsafe {
                    libc::read(wake_fd, buf.as_mut_ptr() as *mut libc::c_void, 8);
                }
                wake_pending.store(false, Ordering::Release);

                // Process pending commands
                while let Ok(cmd) = command_rx.try_recv() {
                    match cmd {
                        ReactorCommand::Register {
                            window_id,
                            tab_id,
                            pane_id,
                            pty,
                        } => {
                            let key = PaneKey {
                                window_id,
                                tab_id,
                                pane_id,
                            };
                            let pty_fd = pty.fd;
                            if pty_fd < 0 {
                                continue;
                            }

                            // If key is already registered with a different fd, deregister the old fd
                            if let Some(&old_fd) = key_to_fd.get(&key) {
                                if old_fd != pty_fd {
                                    unsafe {
                                        libc::epoll_ctl(
                                            epoll_fd,
                                            libc::EPOLL_CTL_DEL,
                                            old_fd,
                                            std::ptr::null_mut(),
                                        );
                                    }
                                    fds.remove(&old_fd);
                                }
                            }

                            // Set non-blocking on pty_fd
                            let _ = pty.set_nonblocking();

                            // Register in epoll
                            let mut ep_event = libc::epoll_event {
                                events: (libc::EPOLLIN
                                    | libc::EPOLLRDHUP
                                    | libc::EPOLLHUP
                                    | libc::EPOLLERR)
                                    as u32,
                                u64: pty_fd as u64,
                            };
                            let ret = unsafe {
                                libc::epoll_ctl(
                                    epoll_fd,
                                    libc::EPOLL_CTL_ADD,
                                    pty_fd,
                                    &mut ep_event,
                                )
                            };
                            if ret < 0 {
                                let err = std::io::Error::last_os_error();
                                if err.raw_os_error() == Some(libc::EEXIST) {
                                    unsafe {
                                        libc::epoll_ctl(
                                            epoll_fd,
                                            libc::EPOLL_CTL_MOD,
                                            pty_fd,
                                            &mut ep_event,
                                        );
                                    }
                                } else {
                                    log::warn!(
                                        "epoll_ctl ADD failed for PTY fd {}: {}",
                                        pty_fd,
                                        err
                                    );
                                    continue;
                                }
                            }

                            fds.insert(pty_fd, Registration { key, pty });
                            key_to_fd.insert(key, pty_fd);
                        }
                        ReactorCommand::Unregister {
                            window_id,
                            tab_id,
                            pane_id,
                        } => {
                            let key = PaneKey {
                                window_id,
                                tab_id,
                                pane_id,
                            };
                            if let Some(pty_fd) = key_to_fd.remove(&key) {
                                unsafe {
                                    libc::epoll_ctl(
                                        epoll_fd,
                                        libc::EPOLL_CTL_DEL,
                                        pty_fd,
                                        std::ptr::null_mut(),
                                    );
                                }
                                fds.remove(&pty_fd);
                            }
                        }
                        ReactorCommand::Shutdown => {
                            break 'reactor;
                        }
                    }
                }
            } else if fds.contains_key(&fd) {
                let (key, exit_detected) = {
                    let reg = fds.get(&fd).unwrap();
                    let key = reg.key;
                    let pty = &reg.pty;

                    // Batch read repeatedly until EAGAIN / EWOULDBLOCK.
                    // Reads directly into the pooled 64KB buffer slice, eliminating intermediate scratch buffer copies.
                    let mut batch = acquire_pty_buffer();
                    let mut offset = 0;
                    let mut exit_detected = false;

                    loop {
                        match pty.read(&mut batch[offset..]) {
                            Ok(0) => {
                                // EOF
                                exit_detected = true;
                                break;
                            }
                            Ok(n) => {
                                offset += n;
                                if offset >= PTY_BUFFER_SIZE {
                                    let send_buf = batch;
                                    let _ = proxy.send(CustomEvent::PtyData {
                                        window_id: key.window_id,
                                        tab_id: key.tab_id,
                                        pane_id: key.pane_id,
                                        data: send_buf,
                                    });
                                    batch = acquire_pty_buffer();
                                    offset = 0;
                                }
                            }
                            Err(e)
                                if e.raw_os_error() == Some(libc::EAGAIN)
                                    || e.raw_os_error() == Some(libc::EWOULDBLOCK) =>
                            {
                                break;
                            }
                            Err(e) if e.raw_os_error() == Some(libc::EINTR) => {
                                continue;
                            }
                            Err(e) if e.raw_os_error() == Some(libc::EIO) => {
                                // Standard Linux PTY slave close produces EIO on master read
                                exit_detected = true;
                                break;
                            }
                            Err(e) => {
                                log::warn!("PTY read error on fd {}: {}", fd, e);
                                exit_detected = true;
                                break;
                            }
                        }
                    }

                    if offset > 0 {
                        batch.truncate(offset);
                        let _ = proxy.send(CustomEvent::PtyData {
                            window_id: key.window_id,
                            tab_id: key.tab_id,
                            pane_id: key.pane_id,
                            data: batch,
                        });
                    } else {
                        recycle_pty_buffer(batch);
                    }

                    if !exit_detected
                        && (ev.events & (libc::EPOLLHUP | libc::EPOLLRDHUP | libc::EPOLLERR) as u32)
                            != 0
                    {
                        // If epoll reported hangup/error, check if fd is dead
                        let mut check_buf = [0u8; 1];
                        if pty.read(&mut check_buf).is_err() {
                            exit_detected = true;
                        }
                    }

                    (key, exit_detected)
                };

                if exit_detected {
                    // Remove from epoll and registration table before sending PtyExit
                    unsafe {
                        libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut());
                    }
                    fds.remove(&fd);
                    key_to_fd.remove(&key);

                    let _ = proxy.send(CustomEvent::PtyExit {
                        window_id: key.window_id,
                        tab_id: key.tab_id,
                        pane_id: key.pane_id,
                    });
                }
            }
        }
    }

    is_stopped.store(true, Ordering::Release);

    // Cleanup remaining descriptors from epoll
    for pty_fd in fds.keys() {
        unsafe {
            libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_DEL, *pty_fd, std::ptr::null_mut());
        }
    }
    unsafe {
        libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_DEL, wake_fd, std::ptr::null_mut());
        libc::close(epoll_fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::FromRawFd;
    use std::time::Duration;

    fn create_test_pipe() -> (Arc<PtyMaster>, File) {
        let mut fds = [0; 2];
        let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(res, 0);

        let pty = Arc::new(PtyMaster::new(fds[0], None));
        let write_end = unsafe { File::from_raw_fd(fds[1]) };
        (pty, write_end)
    }

    #[test]
    fn test_reactor_single_pty_and_batch_ordering() {
        let (sender, receiver) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");

        let (pty, mut write_end) = create_test_pipe();
        reactor.register(1, 0, 0, pty).expect("register failed");

        let msg = b"Hello from reactor pipe test!\n";
        write_end.write_all(msg).unwrap();

        let event = receiver
            .recv_timeout(Duration::from_millis(500))
            .expect("recv timed out");

        match event {
            CustomEvent::PtyData {
                window_id,
                tab_id,
                pane_id,
                data,
            } => {
                assert_eq!(window_id, 1);
                assert_eq!(tab_id, 0);
                assert_eq!(pane_id, 0);
                assert_eq!(data.as_slice(), msg);
            }
            _ => panic!("Expected PtyData event"),
        }
    }

    #[test]
    fn test_reactor_multiple_ptys_routing() {
        let (sender, receiver) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");

        let (pty1, mut write1) = create_test_pipe();
        let (pty2, mut write2) = create_test_pipe();

        reactor.register(1, 10, 100, pty1).unwrap();
        reactor.register(1, 10, 200, pty2).unwrap();

        write1.write_all(b"pane1_data").unwrap();
        write2.write_all(b"pane2_data").unwrap();

        let mut received = HashMap::new();
        for _ in 0..2 {
            let event = receiver.recv_timeout(Duration::from_millis(500)).unwrap();
            if let CustomEvent::PtyData { pane_id, data, .. } = event {
                received.insert(pane_id, data);
            }
        }

        assert_eq!(
            received.get(&100).map(|v| v.as_slice()),
            Some(&b"pane1_data"[..])
        );
        assert_eq!(
            received.get(&200).map(|v| v.as_slice()),
            Some(&b"pane2_data"[..])
        );
    }

    #[test]
    fn test_reactor_eof_and_exit() {
        let (sender, receiver) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");

        let (pty, write_end) = create_test_pipe();
        reactor.register(1, 0, 42, pty).unwrap();

        // Drop write end to trigger EOF / hangup
        drop(write_end);

        let event = receiver
            .recv_timeout(Duration::from_millis(500))
            .expect("recv timed out");

        match event {
            CustomEvent::PtyExit {
                window_id,
                tab_id,
                pane_id,
            } => {
                assert_eq!(window_id, 1);
                assert_eq!(tab_id, 0);
                assert_eq!(pane_id, 42);
            }
            _ => panic!("Expected PtyExit event on EOF"),
        }
    }

    #[test]
    fn test_reactor_unregister_prevents_events() {
        let (sender, receiver) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");

        let (pty, mut write_end) = create_test_pipe();
        let pty_keepalive = pty.clone();
        reactor.register(1, 0, 5, pty).unwrap();
        // Give reactor time to register
        thread::sleep(Duration::from_millis(20));

        reactor.unregister(1, 0, 5).unwrap();
        // Give reactor time to unregister
        thread::sleep(Duration::from_millis(20));

        write_end.write_all(b"should not arrive").unwrap();

        let event = receiver.recv_timeout(Duration::from_millis(100));
        assert!(event.is_err(), "Expected no events after unregister");
        drop(pty_keepalive);
    }

    #[test]
    fn test_reactor_duplicate_registration_and_unknown_unregister() {
        let (sender, _) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");

        let (pty1, _) = create_test_pipe();
        let (pty2, _) = create_test_pipe();

        // Duplicate registration for same key
        assert!(reactor.register(1, 0, 1, pty1).is_ok());
        assert!(reactor.register(1, 0, 1, pty2).is_ok());

        // Unregister unknown key (should gracefully no-op)
        assert!(reactor.unregister(99, 99, 99).is_ok());
    }

    #[test]
    fn test_reactor_clean_shutdown() {
        let (sender, _) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let mut reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");

        let (pty, _) = create_test_pipe();
        reactor.register(1, 0, 0, pty).unwrap();

        reactor.shutdown();
        assert!(reactor.handle().is_stopped.load(Ordering::Acquire));
        // Calling shutdown again should be safe and idempotent
        reactor.shutdown();
    }

    #[test]
    fn test_reactor_large_burst_and_bounded_buffers() {
        let (sender, receiver) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let reactor = PtyReactor::new(proxy).expect("PtyReactor::new failed");

        let (pty, mut write_end) = create_test_pipe();
        reactor.register(1, 0, 10, pty).unwrap();

        // Write 150KB of deterministic data in chunks in a background thread to prevent pipe buffer stall
        let total_bytes = 150 * 1024;
        let test_data: Vec<u8> = (0..total_bytes).map(|i| (i % 251) as u8).collect();
        let data_to_write = test_data.clone();

        let writer_thread = thread::spawn(move || {
            let mut offset = 0;
            while offset < data_to_write.len() {
                let chunk_size = (data_to_write.len() - offset).min(8192);
                write_end
                    .write_all(&data_to_write[offset..offset + chunk_size])
                    .unwrap();
                offset += chunk_size;
            }
        });

        let mut received_bytes = Vec::with_capacity(total_bytes);
        while received_bytes.len() < total_bytes {
            let event = receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("recv timed out during burst");
            match event {
                CustomEvent::PtyData { data, .. } => {
                    assert!(
                        data.len() <= PTY_BUFFER_SIZE,
                        "Data chunk must be bounded by PTY_BUFFER_SIZE"
                    );
                    received_bytes.extend_from_slice(&data);
                    recycle_pty_buffer(data);
                }
                _ => panic!("Expected PtyData during burst test"),
            }
        }

        writer_thread.join().unwrap();
        assert_eq!(received_bytes.len(), test_data.len());
        assert_eq!(
            received_bytes, test_data,
            "Burst data must be completely lossless and retain ordering"
        );
    }
}
