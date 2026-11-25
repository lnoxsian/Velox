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
        Self::read_proc_comm(pgid as u32)
    }

    /// Detects if a non-shell program or active child job is currently running in this PTY.
    /// Returns `None` for idle interactive shells or exited processes.
    pub fn running_program_name(&self, shell_path: &str) -> Option<String> {
        if self.fd < 0 {
            return None;
        }

        let pgid = unsafe { libc::tcgetpgrp(self.fd) };
        if pgid > 0 && Some(pgid as u32) != self.child_pid {
            if Self::is_proc_alive(pgid as u32)
                && let Some(comm) = Self::read_proc_comm(pgid as u32)
            {
                return Some(comm);
            }
        }

        if let Some(child_pid) = self.child_pid {
            if !Self::is_proc_alive(child_pid) {
                return None;
            }
            if let Some(comm) = Self::read_proc_comm(child_pid) {
                if !is_interactive_shell_name(&comm, shell_path) {
                    return Some(comm);
                }
                return Self::first_living_child_comm(child_pid);
            }
        }

        None
    }

    fn read_proc_comm(pid: u32) -> Option<String> {
        let mut path_buf = [0u8; 32];
        let mut cursor = std::io::Cursor::new(&mut path_buf[..]);
        write!(cursor, "/proc/{}/comm\0", pid).ok()?;
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

    fn is_proc_alive(pid: u32) -> bool {
        let mut path_buf = [0u8; 32];
        let mut cursor = std::io::Cursor::new(&mut path_buf[..]);
        if write!(cursor, "/proc/{}/stat\0", pid).is_err() {
            return false;
        }
        let fd = unsafe {
            libc::open(
                path_buf.as_ptr() as *const libc::c_char,
                libc::O_RDONLY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return false;
        }
        let mut buf = [0u8; 256];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        unsafe {
            libc::close(fd);
        }
        if n <= 0 {
            return false;
        }
        let slice = &buf[..n as usize];
        if let Some(rparen) = slice.iter().rposition(|&b| b == b')')
            && let Some(&state) = slice.get(rparen + 2)
        {
            return state != b'Z' && state != b'X';
        }
        true
    }

    fn first_living_child_in_children_file(path_cstr: &[u8]) -> Option<String> {
        let fd = unsafe {
            libc::open(
                path_cstr.as_ptr() as *const libc::c_char,
                libc::O_RDONLY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return None;
        }
        let mut buf = [0u8; 512];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        unsafe {
            libc::close(fd);
        }
        if n <= 0 {
            return None;
        }
        let text = std::str::from_utf8(&buf[..n as usize]).ok()?;
        for token in text.split_whitespace() {
            if let Ok(cpid) = token.parse::<u32>()
                && Self::is_proc_alive(cpid)
                && let Some(comm) = Self::read_proc_comm(cpid)
            {
                return Some(comm);
            }
        }
        None
    }

    fn first_living_child_comm(pid: u32) -> Option<String> {
        let mut path_buf = [0u8; 64];
        let mut cursor = std::io::Cursor::new(&mut path_buf[..]);
        if write!(cursor, "/proc/{}/task/{}/children\0", pid, pid).is_ok()
            && let Some(comm) = Self::first_living_child_in_children_file(&path_buf)
        {
            return Some(comm);
        }

        // Fallback for multi-threaded shells (e.g. nushell / pwsh) whose children may belong to another TID.
        let mut dir_buf = [0u8; 32];
        let mut dir_cur = std::io::Cursor::new(&mut dir_buf[..]);
        let Ok(()) = write!(dir_cur, "/proc/{}/task", pid) else {
            return None;
        };
        let dir_len = dir_cur.position() as usize;
        let Ok(task_dir) = std::str::from_utf8(&dir_buf[..dir_len]) else {
            return None;
        };

        let mut pid_buf = [0u8; 16];
        let mut pid_cur = std::io::Cursor::new(&mut pid_buf[..]);
        let _ = write!(pid_cur, "{}", pid);
        let pid_len = pid_cur.position() as usize;
        let pid_str = std::str::from_utf8(&pid_buf[..pid_len]).unwrap_or("");

        if let Ok(entries) = std::fs::read_dir(task_dir) {
            for entry in entries.flatten() {
                let tid_os = entry.file_name();
                let Some(tid_str) = tid_os.to_str() else {
                    continue;
                };
                if tid_str == pid_str {
                    continue;
                }
                let mut tid_path = [0u8; 64];
                let mut cur = std::io::Cursor::new(&mut tid_path[..]);
                if write!(cur, "/proc/{}/task/{}/children\0", pid, tid_str).is_ok()
                    && let Some(comm) = Self::first_living_child_in_children_file(&tid_path)
                {
                    return Some(comm);
                }
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

fn is_interactive_shell_name(comm: &str, shell_path: &str) -> bool {
    let clean = comm.trim().strip_prefix('-').unwrap_or(comm.trim());
    if let Some(shell_bin) = std::path::Path::new(shell_path)
        .file_name()
        .and_then(|s| s.to_str())
    {
        let shell_short = &shell_bin[..shell_bin.len().min(15)];
        if clean.eq_ignore_ascii_case(shell_bin) || clean.eq_ignore_ascii_case(shell_short) {
            return true;
        }
    }
    matches!(
        clean.to_ascii_lowercase().as_str(),
        "sh" | "bash"
            | "zsh"
            | "fish"
            | "dash"
            | "ash"
            | "ksh"
            | "mksh"
            | "tcsh"
            | "csh"
            | "nu"
            | "elvish"
            | "xonsh"
            | "pwsh"
            | "osh"
            | "oil"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_running_program_name_is_none() {
        let pty = PtyMaster::dummy();
        assert_eq!(pty.running_program_name("/bin/bash"), None);
    }

    #[test]
    fn test_is_interactive_shell_name() {
        assert!(is_interactive_shell_name("bash", "/bin/zsh"));
        assert!(is_interactive_shell_name("-zsh", "/bin/bash"));
        assert!(is_interactive_shell_name("customsh", "/usr/local/bin/customsh"));
        assert!(!is_interactive_shell_name("vim", "/bin/bash"));
        assert!(!is_interactive_shell_name("sleep", "/bin/zsh"));
        assert!(!is_interactive_shell_name("cargo", "/bin/fish"));
    }

    #[test]
    fn test_running_program_name_detects_live_process_and_clears_on_exit() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("5")
            .spawn()
            .expect("failed to spawn /bin/sleep");
        let devnull = std::fs::File::open("/dev/null").expect("failed to open /dev/null");
        let pty = PtyMaster::new(std::os::fd::IntoRawFd::into_raw_fd(devnull), Some(child.id()));

        let mut detected = None;
        for _ in 0..50 {
            detected = pty.running_program_name("/bin/bash");
            if detected.as_deref() == Some("sleep") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(detected.as_deref(), Some("sleep"));

        let _ = child.kill();
        let _ = child.wait();

        assert_eq!(pty.running_program_name("/bin/bash"), None);
    }
}
