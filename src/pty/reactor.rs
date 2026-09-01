use crate::app::app::CustomEvent;
use crate::pty::buffer_pool::{acquire_pty_buffer, recycle_pty_buffer};
use crate::pty::master::PtyMaster;
use std::collections::HashMap;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

const WAKE_EVENT_TOKEN: u64 = u64::MAX;

struct PtyEntry {
    window_id: WindowId,
    tab_id: u64,
    pane_id: u64,
    pty_master: Arc<PtyMaster>,
}

enum ReactorCommand {
    Register {
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
        pty_master: Arc<PtyMaster>,
    },
    Unregister {
        pane_id: u64,
    },
    Shutdown,
}

pub struct PtyReactor {
    commands: Arc<Mutex<Vec<ReactorCommand>>>,
    wake_fd: RawFd,
    thread_handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl PtyReactor {
    pub fn new(proxy: EventLoopProxy<CustomEvent>) -> Option<Self> {
        let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epoll_fd < 0 {
            return None;
        }

        let wake_fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if wake_fd < 0 {
            unsafe { libc::close(epoll_fd) };
            return None;
        }

        let mut wake_event = libc::epoll_event {
            events: (libc::EPOLLIN | libc::EPOLLET) as u32,
            u64: WAKE_EVENT_TOKEN,
        };

        let res = unsafe {
            libc::epoll_ctl(
                epoll_fd,
                libc::EPOLL_CTL_ADD,
                wake_fd,
                &mut wake_event as *mut _,
            )
        };
        if res < 0 {
            unsafe {
                libc::close(wake_fd);
                libc::close(epoll_fd);
            }
            return None;
        }

        let commands = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(true));

        let thread_commands = commands.clone();
        let thread_running = running.clone();

        let thread_handle = std::thread::spawn(move || {
            let mut entries: HashMap<u64, PtyEntry> = HashMap::new();
            let mut fd_to_pane: HashMap<RawFd, u64> = HashMap::new();
            let mut events = [libc::epoll_event { events: 0, u64: 0 }; 64];

            while thread_running.load(Ordering::Relaxed) {
                let nfds = unsafe {
                    libc::epoll_wait(
                        epoll_fd,
                        events.as_mut_ptr(),
                        events.len() as i32,
                        100, // 100ms timeout for safety
                    )
                };

                if nfds < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::EINTR) {
                        continue;
                    }
                    break;
                }

                for ev in events.iter().take(nfds as usize) {
                    if ev.u64 == WAKE_EVENT_TOKEN {
                        // Drain wake_fd
                        let mut buf = [0u8; 8];
                        unsafe {
                            libc::read(wake_fd, buf.as_mut_ptr() as *mut libc::c_void, 8);
                        }

                        // Process pending commands
                        let mut cmds = Vec::new();
                        if let Ok(mut lock) = thread_commands.lock() {
                            std::mem::swap(&mut *lock, &mut cmds);
                        }

                        for cmd in cmds {
                            match cmd {
                                ReactorCommand::Register {
                                    window_id,
                                    tab_id,
                                    pane_id,
                                    pty_master,
                                } => {
                                    let fd = pty_master.fd;
                                    if fd >= 0 {
                                        let mut ev = libc::epoll_event {
                                            events: (libc::EPOLLIN
                                                | libc::EPOLLHUP
                                                | libc::EPOLLERR)
                                                as u32,
                                            u64: pane_id,
                                        };
                                        let _ = unsafe {
                                            libc::epoll_ctl(
                                                epoll_fd,
                                                libc::EPOLL_CTL_ADD,
                                                fd,
                                                &mut ev as *mut _,
                                            )
                                        };
                                        fd_to_pane.insert(fd, pane_id);
                                        entries.insert(
                                            pane_id,
                                            PtyEntry {
                                                window_id,
                                                tab_id,
                                                pane_id,
                                                pty_master,
                                            },
                                        );
                                    }
                                }
                                ReactorCommand::Unregister { pane_id } => {
                                    if let Some(entry) = entries.remove(&pane_id) {
                                        let fd = entry.pty_master.fd;
                                        fd_to_pane.remove(&fd);
                                        let _ = unsafe {
                                            libc::epoll_ctl(
                                                epoll_fd,
                                                libc::EPOLL_CTL_DEL,
                                                fd,
                                                std::ptr::null_mut(),
                                            )
                                        };
                                    }
                                }
                                ReactorCommand::Shutdown => {
                                    thread_running.store(false, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                    } else {
                        let pane_id = ev.u64;
                        if let Some(entry) = entries.get(&pane_id) {
                            let mut buf = acquire_pty_buffer();
                            match entry.pty_master.read(&mut buf) {
                                Ok(0) => {
                                    recycle_pty_buffer(buf);
                                    let _ = proxy.send_event(CustomEvent::PtyExit {
                                        window_id: entry.window_id,
                                        tab_id: entry.tab_id,
                                        pane_id: entry.pane_id,
                                    });
                                    let fd = entry.pty_master.fd;
                                    let _ = unsafe {
                                        libc::epoll_ctl(
                                            epoll_fd,
                                            libc::EPOLL_CTL_DEL,
                                            fd,
                                            std::ptr::null_mut(),
                                        )
                                    };
                                    entries.remove(&pane_id);
                                    fd_to_pane.remove(&fd);
                                }
                                Ok(n) => {
                                    buf.truncate(n);
                                    let _ = proxy.send_event(CustomEvent::PtyData {
                                        window_id: entry.window_id,
                                        tab_id: entry.tab_id,
                                        pane_id: entry.pane_id,
                                        data: buf,
                                    });
                                }
                                Err(e) => {
                                    recycle_pty_buffer(buf);
                                    if e.raw_os_error() != Some(libc::EAGAIN)
                                        && e.raw_os_error() != Some(libc::EWOULDBLOCK)
                                    {
                                        let _ = proxy.send_event(CustomEvent::PtyExit {
                                            window_id: entry.window_id,
                                            tab_id: entry.tab_id,
                                            pane_id: entry.pane_id,
                                        });
                                        let fd = entry.pty_master.fd;
                                        let _ = unsafe {
                                            libc::epoll_ctl(
                                                epoll_fd,
                                                libc::EPOLL_CTL_DEL,
                                                fd,
                                                std::ptr::null_mut(),
                                            )
                                        };
                                        entries.remove(&pane_id);
                                        fd_to_pane.remove(&fd);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            unsafe {
                libc::close(wake_fd);
                libc::close(epoll_fd);
            }
        });

        Some(Self {
            commands,
            wake_fd,
            thread_handle: Some(thread_handle),
            running,
        })
    }

    pub fn register(
        &self,
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
        pty_master: Arc<PtyMaster>,
    ) {
        if let Ok(mut lock) = self.commands.lock() {
            lock.push(ReactorCommand::Register {
                window_id,
                tab_id,
                pane_id,
                pty_master,
            });
            self.wake();
        }
    }

    pub fn unregister(&self, pane_id: u64) {
        if let Ok(mut lock) = self.commands.lock() {
            lock.push(ReactorCommand::Unregister { pane_id });
            self.wake();
        }
    }

    fn wake(&self) {
        let val = 1u64;
        unsafe {
            libc::write(
                self.wake_fd,
                &val as *const _ as *const libc::c_void,
                std::mem::size_of::<u64>(),
            );
        }
    }
}

impl Drop for PtyReactor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Ok(mut lock) = self.commands.lock() {
            lock.push(ReactorCommand::Shutdown);
        }
        self.wake();
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}
