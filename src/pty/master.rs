use std::os::fd::RawFd;
use std::sync::Mutex;
use std::time::Instant;

pub struct PtyMaster {
    pub fd: RawFd,
    pub child_pid: Option<u32>,
    cached_comm: Mutex<Option<(i32, Instant, String)>>,
    cached_cwd: Mutex<Option<(i32, Instant, String)>>,
}

impl PtyMaster {
    pub fn dummy() -> Self {
        Self {
            fd: -1,
            child_pid: None,
            cached_comm: Mutex::new(None),
            cached_cwd: Mutex::new(None),
        }
    }

    pub fn new(fd: RawFd, child_pid: Option<u32>) -> Self {
        Self {
            fd,
            child_pid,
            cached_comm: Mutex::new(None),
            cached_cwd: Mutex::new(None),
        }
    }

    pub fn invalidate_proc_cache(&self) {
        if let Ok(mut lock) = self.cached_comm.lock() {
            *lock = None;
        }
        if let Ok(mut lock) = self.cached_cwd.lock() {
            *lock = None;
        }
    }

    pub fn get_foreground_process_name(&self) -> Option<String> {
        if self.fd < 0 {
            return None;
        }
        let pgid = unsafe { libc::tcgetpgrp(self.fd) };
        if pgid < 0 {
            return None;
        }
        let now = Instant::now();
        if let Ok(mut lock) = self.cached_comm.lock() {
            if let Some((cached_pgid, last_time, ref name)) = *lock {
                if cached_pgid == pgid && now.duration_since(last_time).as_millis() < 250 {
                    return Some(name.clone());
                }
            }
            let comm_path = format!("/proc/{}/comm", pgid);
            if let Ok(content) = std::fs::read_to_string(comm_path) {
                let name = content.trim().to_string();
                *lock = Some((pgid, now, name.clone()));
                return Some(name);
            }
        }
        None
    }

    pub fn get_foreground_process_cwd(&self) -> Option<String> {
        if self.fd < 0 {
            return None;
        }
        let pgid = unsafe { libc::tcgetpgrp(self.fd) };
        let now = Instant::now();
        if let Ok(mut lock) = self.cached_cwd.lock() {
            if let Some((cached_pgid, last_time, ref cwd)) = *lock {
                if cached_pgid == pgid && now.duration_since(last_time).as_millis() < 250 {
                    return Some(cwd.clone());
                }
            }
            let mut resolved = None;
            if pgid > 0 {
                let cwd_link = format!("/proc/{}/cwd", pgid);
                if let Ok(path) = std::fs::read_link(cwd_link) {
                    if let Some(s) = path.to_str() {
                        if path.is_dir() {
                            resolved = Some(s.to_string());
                        }
                    }
                }
            }
            if resolved.is_none()
                && let Some(child_pid) = self.child_pid
            {
                let cwd_link = format!("/proc/{}/cwd", child_pid);
                if let Ok(path) = std::fs::read_link(cwd_link) {
                    if let Some(s) = path.to_str() {
                        if path.is_dir() {
                            resolved = Some(s.to_string());
                        }
                    }
                }
            }
            if let Some(ref cwd) = resolved {
                *lock = Some((pgid, now, cwd.clone()));
            }
            return resolved;
        }
        None
    }

    pub fn read_into_vec(&self, buf: &mut Vec<u8>, max_to_read: usize) -> std::io::Result<usize> {
        if self.fd < 0 || max_to_read == 0 {
            return Ok(0);
        }
        let current_len = buf.len();
        if buf.capacity() < current_len + max_to_read {
            buf.reserve(max_to_read);
        }
        let spare = buf.spare_capacity_mut();
        let read_len = spare.len().min(max_to_read);
        let res = unsafe { libc::read(self.fd, spare.as_mut_ptr() as *mut libc::c_void, read_len) };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            let n = res as usize;
            unsafe {
                buf.set_len(current_len + n);
            }
            Ok(n)
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.fd < 0 {
            return Ok(0);
        }
        let res = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }

    pub fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
        if self.fd < 0 {
            return Ok(buf.len());
        }
        let res = unsafe { libc::write(self.fd, buf.as_ptr() as *const libc::c_void, buf.len()) };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.invalidate_proc_cache();
        if self.fd < 0 {
            return Ok(());
        }
        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let res = unsafe { libc::ioctl(self.fd, libc::TIOCSWINSZ, &ws) };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn set_nonblocking(&self) -> std::io::Result<()> {
        if self.fd < 0 {
            return Ok(());
        }
        let flags = unsafe { libc::fcntl(self.fd, libc::F_GETFL, 0) };
        if flags < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let res = unsafe { libc::fcntl(self.fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
        if res < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for PtyMaster {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}
