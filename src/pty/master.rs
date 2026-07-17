use std::os::fd::RawFd;

pub struct PtyMaster {
    pub fd: RawFd,
}

impl PtyMaster {
    pub fn get_foreground_process_name(&self) -> Option<String> {
        let pgid = unsafe { libc::tcgetpgrp(self.fd) };
        if pgid < 0 {
            return None;
        }
        let comm_path = format!("/proc/{}/comm", pgid);
        std::fs::read_to_string(comm_path)
            .ok()
            .map(|s| s.trim().to_string())
    }

    pub fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        let res = unsafe {
            libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
        };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }

    pub fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
        let res = unsafe {
            libc::write(self.fd, buf.as_ptr() as *const libc::c_void, buf.len())
        };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> std::io::Result<()> {
        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let res = unsafe {
            libc::ioctl(self.fd, libc::TIOCSWINSZ, &ws)
        };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for PtyMaster {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}
