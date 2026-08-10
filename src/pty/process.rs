use crate::pty::master::PtyMaster;
use nix::pty::openpty;
use std::os::fd::IntoRawFd;
use std::os::unix::process::CommandExt;

#[derive(Debug)]
pub enum PtyError {
    Fork(String),
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtyError::Fork(msg) => write!(f, "PTY fork error: {}", msg),
        }
    }
}

impl std::error::Error for PtyError {}

pub fn spawn_shell(shell_path: &str) -> Result<PtyMaster, PtyError> {
    let pty = openpty(None, None).map_err(|e| PtyError::Fork(e.to_string()))?;
    let master_fd = pty.master.into_raw_fd();
    let slave_fd = pty.slave.into_raw_fd();

    let fork_result = unsafe { nix::unistd::fork() };
    match fork_result {
        Ok(nix::unistd::ForkResult::Child) => {
            unsafe {
                libc::setsid();
                #[cfg(target_os = "linux")]
                libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);

                let pid = libc::getpid();
                libc::tcsetpgrp(slave_fd, pid);

                libc::dup2(slave_fd, libc::STDIN_FILENO);
                libc::dup2(slave_fd, libc::STDOUT_FILENO);
                libc::dup2(slave_fd, libc::STDERR_FILENO);

                libc::close(master_fd);
                libc::close(slave_fd);
            }

            // Note: Since nix::unistd::execvp is gated, we can use std::process or libc
            let err = std::process::Command::new(shell_path)
                .env("TERM", "xterm-256color")
                .env("COLORTERM", "truecolor")
                .exec();
            panic!("exec failed: {}", err);
        }
        Ok(nix::unistd::ForkResult::Parent { child: _ }) => {
            unsafe {
                libc::close(slave_fd);
            }
            Ok(PtyMaster { fd: master_fd })
        }
        Err(e) => Err(PtyError::Fork(e.to_string())),
    }
}

