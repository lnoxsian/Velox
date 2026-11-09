use std::io::Write;
use std::os::fd::{AsRawFd, RawFd};

#[derive(Debug)]
pub struct PtyMaster {
    pub fd: RawFd,
    pub child_pid: Option<u32>,
}

impl AsRawFd for PtyMaster {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl PtyMaster {
    pub fn dummy() -> Self {
        Self {
            fd: -1,
            child_pid: None,
        }
    }

    pub fn new(fd: RawFd, child_pid: Option<u32>) -> Self {
        if fd >= 0 {
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFD, 0);
                if flags >= 0 {
                    libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
                }
            }
        }
        Self { fd, child_pid }
    }

    pub fn get_foreground_process_name(&self) -> Option<String> {
        if self.fd < 0 {
            return None;
        }
        let pgid = unsafe { libc::tcgetpgrp(self.fd) };
        if pgid <= 0 {
            return None;
        }
        let mut path_buf = [0u8; 32];
        let mut cursor = std::io::Cursor::new(&mut path_buf[..]);
        let _ = write!(cursor, "/proc/{}/comm\0", pgid);
        let fd = unsafe {
            libc::open(
                path_buf.as_ptr() as *const libc::c_char,
                libc::O_RDONLY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return None;
        }

        let mut buf = [0u8; 64];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        unsafe {
            libc::close(fd);
        }

        if n > 0 {
            let s = std::str::from_utf8(&buf[..n as usize]).ok()?;
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    pub fn get_foreground_process_cwd(&self) -> Option<String> {
        if self.fd < 0 {
            return None;
        }
        let pgid = unsafe { libc::tcgetpgrp(self.fd) };
        if pgid > 0 {
            if let Some(cwd) = Self::read_proc_cwd(pgid as u32) {
                return Some(cwd);
            }
        }
        if let Some(child_pid) = self.child_pid {
            if let Some(cwd) = Self::read_proc_cwd(child_pid) {
                return Some(cwd);
            }
        }
        None
    }

    fn read_proc_cwd(pid: u32) -> Option<String> {
        let mut path_buf = [0u8; 32];
        let mut cursor = std::io::Cursor::new(&mut path_buf[..]);
        let _ = write!(cursor, "/proc/{}/cwd\0", pid);

        let mut link_buf = [0u8; 4096];
        let len = unsafe {
            libc::readlink(
                path_buf.as_ptr() as *const libc::c_char,
                link_buf.as_mut_ptr() as *mut libc::c_char,
                link_buf.len() - 1,
            )
        };
        if len > 0 {
            link_buf[len as usize] = 0;
            let s = std::str::from_utf8(&link_buf[..len as usize]).ok()?;
            let p = std::path::Path::new(s);
            if p.is_dir() {
                return Some(s.to_string());
            }
        }
        None
    }

    pub fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.fd < 0 {
            return Ok(0);
        }
        loop {
            let res =
                unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if res >= 0 {
                return Ok(res as usize);
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err);
        }
    }

    pub fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
        if self.fd < 0 {
            return Ok(buf.len());
        }
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let res =
                unsafe { libc::write(self.fd, buf.as_ptr() as *const libc::c_void, buf.len()) };
            if res >= 0 {
                return Ok(res as usize);
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err);
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> std::io::Result<()> {
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
            let ret = unsafe { libc::close(self.fd) };
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::EBADF) {
                    log::warn!("PtyMaster close fd {} failed: {}", self.fd, err);
                }
            }
            self.fd = -1;
        }
    }
}
