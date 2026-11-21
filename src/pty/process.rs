use crate::pty::master::PtyMaster;
use nix::pty::{Winsize, openpty};
use std::ffi::CString;
use std::os::fd::IntoRawFd;

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

/// Terminal identification string indicating 256-color xterm emulation.
pub const VELOX_TERM: &str = "xterm-256color";

/// Color terminal capability string indicating 24-bit direct color (truecolor) support.
pub const VELOX_COLORTERM: &str = "truecolor";

/// Configures terminal-specific environment variables for a child process.
///
/// # Environment Policy
/// - **Preserve Inherited Environment**: Velox inherits the parent process environment
///   by default. It never calls `env_clear()`, ensuring that user and system configuration
///   remain available to child processes.
/// - **Terminal-Specific Variables**: Velox explicitly sets only `TERM` (`xterm-256color`) and
///   `COLORTERM` (`truecolor`) in one central location to describe its terminal capability profile.
/// - **Inherited System Variables**: Essential system environment variables such as `PATH`,
///   `HOME`, `USER`, `LOGNAME`, `SHELL`, locale variables (`LANG`, `LANGUAGE`, `LC_ALL`, `LC_CTYPE`),
///   proxy configurations (`HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`), session/desktop variables
///   (`XDG_CONFIG_DIRS`, `XDG_DATA_DIRS`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`, `DISPLAY`,
///   `WAYLAND_DISPLAY`), and authentication sockets (`SSH_AUTH_SOCK`) remain inherited and are
///   never cleared, overwritten, or substituted with empty fallbacks.
/// - **No VTE/Terminator Specifics**: Terminator and VTE-specific environment variables
///   (`VTE_VERSION`, `TERMINATOR_DBUS_NAME`, `TERMINATOR_DBUS_PATH`, `TERMINATOR_UUID`)
///   are not copied, set, or required.
/// - **Terminfo Preserved**: `TERMINFO` and `TERMINFO_DIRS` are intentionally preserved as inherited
///   and not overridden because Velox targets standard `xterm-256color`.
/// - **Package Manager Neutrality**: `DEBIAN_FRONTEND` is not set by Velox; package-manager
///   behavior is not modified.
pub fn configure_terminal_environment(command: &mut std::process::Command) {
    command.env("TERM", VELOX_TERM);
    command.env("COLORTERM", VELOX_COLORTERM);
}

pub fn build_terminal_env_cstrings() -> (Vec<CString>, Vec<*const libc::c_char>) {
    use std::os::unix::ffi::OsStrExt;
    let mut env_map: std::collections::HashMap<std::ffi::OsString, std::ffi::OsString> =
        std::env::vars_os().collect();
    env_map.insert("TERM".into(), VELOX_TERM.into());
    env_map.insert("COLORTERM".into(), VELOX_COLORTERM.into());

    let mut env_cstrings = Vec::with_capacity(env_map.len());
    for (k, v) in env_map {
        let mut bytes = k.as_bytes().to_vec();
        bytes.push(b'=');
        bytes.extend_from_slice(v.as_bytes());
        if let Ok(cs) = CString::new(bytes) {
            env_cstrings.push(cs);
        }
    }
    let mut envp_ptrs: Vec<*const libc::c_char> = env_cstrings.iter().map(|s| s.as_ptr()).collect();
    envp_ptrs.push(std::ptr::null());
    (env_cstrings, envp_ptrs)
}

pub fn resolve_executable_in_path(program: &str) -> Option<String> {
    let p = std::path::Path::new(program);
    if p.is_absolute() {
        if p.exists() {
            return Some(program.to_string());
        }
        return None;
    }
    if program.contains('/') {
        if p.exists() {
            return Some(program.to_string());
        }
        return None;
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(program);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

struct ForkGuard {
    master_fd: std::os::fd::RawFd,
    slave_fd: std::os::fd::RawFd,
    err_read_fd: std::os::fd::RawFd,
    err_write_fd: std::os::fd::RawFd,
    disarmed: bool,
}

impl Drop for ForkGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            unsafe {
                if self.master_fd >= 0 {
                    libc::close(self.master_fd);
                }
                if self.slave_fd >= 0 {
                    libc::close(self.slave_fd);
                }
                if self.err_read_fd >= 0 {
                    libc::close(self.err_read_fd);
                }
                if self.err_write_fd >= 0 {
                    libc::close(self.err_write_fd);
                }
            }
        }
    }
}

pub fn spawn_process(
    shell_path: &str,
    command: Option<&[String]>,
    working_directory: Option<&str>,
) -> Result<PtyMaster, PtyError> {
    spawn_process_with_size(shell_path, command, working_directory, None)
}

pub fn spawn_process_with_size(
    shell_path: &str,
    command: Option<&[String]>,
    working_directory: Option<&str>,
    initial_size: Option<(u16, u16)>,
) -> Result<PtyMaster, PtyError> {
    // 1. Pre-build argument CStrings in parent before fork
    let (prog_cstring, argv_cstrings) = if let Some(args) = command
        && !args.is_empty()
    {
        let prog = resolve_executable_in_path(&args[0]).unwrap_or_else(|| args[0].clone());
        let prog_cs = CString::new(prog.as_bytes()).map_err(|e| PtyError::Fork(e.to_string()))?;
        let mut argv = Vec::with_capacity(args.len());
        for arg in args {
            argv.push(CString::new(arg.as_bytes()).map_err(|e| PtyError::Fork(e.to_string()))?);
        }
        (prog_cs, argv)
    } else {
        let prog = resolve_executable_in_path(shell_path).unwrap_or_else(|| shell_path.to_string());
        let prog_cs = CString::new(prog.as_bytes()).map_err(|e| PtyError::Fork(e.to_string()))?;
        let argv = vec![prog_cs.clone()];
        (prog_cs, argv)
    };
    let mut argv_ptrs: Vec<*const libc::c_char> =
        argv_cstrings.iter().map(|s| s.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());

    // 2. Pre-build environment CStrings in parent before fork
    let (_env_cstrings, envp_ptrs) = build_terminal_env_cstrings();

    // 3. Pre-resolve working directory in parent
    let c_dir = working_directory
        .filter(|d| std::path::Path::new(d).is_dir())
        .and_then(|d| CString::new(d.as_bytes()).ok());

    // 4. Create PTY with initial geometry if requested
    let pty = if let Some((cols, rows)) = initial_size {
        let ws = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        openpty(Some(&ws), None).map_err(|e| PtyError::Fork(e.to_string()))?
    } else {
        openpty(None, None).map_err(|e| PtyError::Fork(e.to_string()))?
    };
    let master_fd = pty.master.into_raw_fd();
    let slave_fd = pty.slave.into_raw_fd();

    // 5. Create O_CLOEXEC sync pipe for synchronous error reporting
    let mut err_pipe = [-1, -1];
    let pipe_res = unsafe { libc::pipe2(err_pipe.as_mut_ptr(), libc::O_CLOEXEC) };
    if pipe_res != 0 {
        unsafe {
            libc::close(master_fd);
            libc::close(slave_fd);
        }
        return Err(PtyError::Fork(std::io::Error::last_os_error().to_string()));
    }
    let err_read_fd = err_pipe[0];
    let err_write_fd = err_pipe[1];

    let mut guard = ForkGuard {
        master_fd,
        slave_fd,
        err_read_fd,
        err_write_fd,
        disarmed: false,
    };

    let fork_result = unsafe { nix::unistd::fork() };
    match fork_result {
        Ok(nix::unistd::ForkResult::Child) => {
            guard.disarmed = true;
            unsafe {
                libc::close(err_read_fd);

                if libc::setsid() < 0 {
                    let err = libc::__errno_location().read();
                    let _ = libc::write(err_write_fd, &err as *const _ as *const libc::c_void, 4);
                    libc::_exit(127);
                }
                #[cfg(target_os = "linux")]
                libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);

                let pid = libc::getpid();
                libc::tcsetpgrp(slave_fd, pid);

                if libc::dup2(slave_fd, libc::STDIN_FILENO) < 0
                    || libc::dup2(slave_fd, libc::STDOUT_FILENO) < 0
                    || libc::dup2(slave_fd, libc::STDERR_FILENO) < 0
                {
                    let err = libc::__errno_location().read();
                    let _ = libc::write(err_write_fd, &err as *const _ as *const libc::c_void, 4);
                    libc::_exit(127);
                }

                libc::close(master_fd);
                if slave_fd > 2 {
                    libc::close(slave_fd);
                }

                // Reset standard Unix signal dispositions to default
                for &sig in &[
                    libc::SIGPIPE,
                    libc::SIGCHLD,
                    libc::SIGINT,
                    libc::SIGQUIT,
                    libc::SIGHUP,
                    libc::SIGTSTP,
                ] {
                    libc::signal(sig, libc::SIG_DFL);
                }
                let mut empty_mask: libc::sigset_t = std::mem::zeroed();
                libc::sigemptyset(&mut empty_mask);
                libc::sigprocmask(libc::SIG_SETMASK, &empty_mask, std::ptr::null_mut());

                if let Some(ref dir) = c_dir {
                    let _ = libc::chdir(dir.as_ptr());
                }

                // Preserve err_write_fd on descriptor 3 with FD_CLOEXEC
                if err_write_fd != 3 {
                    libc::dup2(err_write_fd, 3);
                    libc::close(err_write_fd);
                }
                let f_flags = libc::fcntl(3, libc::F_GETFD, 0);
                if f_flags >= 0 {
                    libc::fcntl(3, libc::F_SETFD, f_flags | libc::FD_CLOEXEC);
                }

                // Close all inherited file descriptors >= 4
                #[cfg(target_os = "linux")]
                {
                    if libc::syscall(libc::SYS_close_range, 4, libc::c_uint::MAX, 0) != 0 {
                        let max_fd =
                            libc::sysconf(libc::_SC_OPEN_MAX).clamp(256, 4096) as libc::c_int;
                        for fd in 4..max_fd {
                            libc::close(fd);
                        }
                    }
                }

                libc::execve(
                    prog_cstring.as_ptr(),
                    argv_ptrs.as_ptr(),
                    envp_ptrs.as_ptr(),
                );

                // If execve returned, write exact errno to sync pipe and exit
                let err = libc::__errno_location().read();
                let _ = libc::write(3, &err as *const _ as *const libc::c_void, 4);
                libc::_exit(127);
            }
        }
        Ok(nix::unistd::ForkResult::Parent { child }) => {
            unsafe {
                libc::close(err_write_fd);
            }
            guard.err_write_fd = -1;

            let ret = unsafe { libc::close(slave_fd) };
            guard.slave_fd = -1;
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::EBADF) {
                    log::warn!("Failed to close slave_fd {} in parent: {}", slave_fd, err);
                }
            }

            // Check if child reported an exec error synchronously
            let mut err_code = 0i32;
            let n = unsafe {
                libc::read(
                    err_read_fd,
                    &mut err_code as *mut _ as *mut libc::c_void,
                    std::mem::size_of::<i32>(),
                )
            };
            unsafe {
                libc::close(err_read_fd);
            }
            guard.err_read_fd = -1;

            if n > 0 {
                // Child failed to execute: reap child process immediately to prevent zombies
                let mut status = 0;
                unsafe {
                    libc::waitpid(child.as_raw(), &mut status, 0);
                    libc::close(master_fd);
                }
                guard.master_fd = -1;
                return Err(PtyError::Fork(format!(
                    "exec failed with errno {} ({})",
                    err_code,
                    std::io::Error::from_raw_os_error(err_code)
                )));
            }

            // Disarm guard: child process successfully executed
            guard.disarmed = true;

            // Set O_NONBLOCK and FD_CLOEXEC on master_fd immediately
            unsafe {
                let fl = libc::fcntl(master_fd, libc::F_GETFL, 0);
                if fl >= 0 {
                    libc::fcntl(master_fd, libc::F_SETFL, fl | libc::O_NONBLOCK);
                }
                let fd_fl = libc::fcntl(master_fd, libc::F_GETFD, 0);
                if fd_fl >= 0 {
                    libc::fcntl(master_fd, libc::F_SETFD, fd_fl | libc::FD_CLOEXEC);
                }
            }

            let child_pid = if child.as_raw() > 0 {
                Some(child.as_raw() as u32)
            } else {
                None
            };
            Ok(PtyMaster {
                fd: master_fd,
                child_pid,
            })
        }
        Err(e) => {
            // guard will drop and close master_fd, slave_fd, err_read_fd, err_write_fd
            Err(PtyError::Fork(e.to_string()))
        }
    }
}

/// Resolves a shell path, falling back through $SHELL, /bin/bash, and /bin/sh if not found.
pub fn resolve_executable_shell(shell_path: &str) -> String {
    let p = std::path::Path::new(shell_path);
    if p.is_absolute() && p.exists() {
        return shell_path.to_string();
    }
    if !p.is_absolute() {
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let candidate = dir.join(shell_path);
                if candidate.exists() {
                    return candidate.to_string_lossy().to_string();
                }
            }
        }
    }

    if let Ok(sh) = std::env::var("SHELL") {
        if std::path::Path::new(&sh).exists() {
            log::warn!(
                "Configured shell '{}' not found. Falling back to $SHELL '{}'",
                shell_path,
                sh
            );
            return sh;
        }
    }

    for candidate in &["/bin/bash", "/usr/bin/bash", "/bin/sh", "/usr/bin/sh"] {
        if std::path::Path::new(candidate).exists() {
            log::warn!(
                "Configured shell '{}' not found. Falling back to '{}'",
                shell_path,
                candidate
            );
            return candidate.to_string();
        }
    }

    shell_path.to_string()
}

/// Spawns a terminal shell process with layered fallbacks:
/// 1. Resolved executable shell + requested working directory
/// 2. Resolved executable shell + inherited working directory
/// 3. Standard /bin/sh + inherited working directory
pub fn spawn_terminal_pty(
    shell_path: &str,
    command: Option<&[String]>,
    working_directory: Option<&str>,
) -> Result<PtyMaster, PtyError> {
    spawn_terminal_pty_with_size(shell_path, command, working_directory, None)
}

/// Spawns a terminal shell process with initial window size and layered fallbacks:
/// 1. Resolved executable shell + validated working directory
/// 2. Resolved executable shell + inherited working directory
/// 3. Standard /bin/sh + inherited working directory
pub fn spawn_terminal_pty_with_size(
    shell_path: &str,
    command: Option<&[String]>,
    working_directory: Option<&str>,
    initial_size: Option<(u16, u16)>,
) -> Result<PtyMaster, PtyError> {
    let resolved_shell = resolve_executable_shell(shell_path);
    let effective_dir = working_directory.filter(|dir| std::path::Path::new(dir).is_dir());

    spawn_process_with_size(&resolved_shell, command, effective_dir, initial_size)
        .or_else(|_| spawn_process_with_size(&resolved_shell, command, None, initial_size))
        .or_else(|_| spawn_process_with_size("/bin/sh", command, None, initial_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::ffi::OsStr;

    #[test]
    fn test_configure_terminal_environment_sets_expected_values() {
        let mut cmd = std::process::Command::new("/bin/sh");
        configure_terminal_environment(&mut cmd);

        let envs: HashMap<_, _> = cmd.get_envs().collect();
        assert_eq!(
            envs.get(OsStr::new("TERM")),
            Some(&Some(OsStr::new(VELOX_TERM)))
        );
        assert_eq!(
            envs.get(OsStr::new("COLORTERM")),
            Some(&Some(OsStr::new(VELOX_COLORTERM)))
        );
        assert_eq!(VELOX_TERM, "xterm-256color");
        assert_eq!(VELOX_COLORTERM, "truecolor");
    }

    #[test]
    fn test_configure_terminal_environment_preserves_inherited_variables() {
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.env("VELOX_TEST_INHERITED", "value-from-parent");

        configure_terminal_environment(&mut cmd);

        let envs: HashMap<_, _> = cmd.get_envs().collect();
        assert_eq!(
            envs.get(OsStr::new("VELOX_TEST_INHERITED")),
            Some(&Some(OsStr::new("value-from-parent")))
        );
    }

    #[test]
    fn test_configure_terminal_environment_does_not_create_unrelated_variables() {
        let mut cmd = std::process::Command::new("/bin/sh");
        configure_terminal_environment(&mut cmd);

        let envs: HashMap<_, _> = cmd.get_envs().collect();
        assert!(!envs.contains_key(OsStr::new("VTE_VERSION")));
        assert!(!envs.contains_key(OsStr::new("TERMINATOR_DBUS_NAME")));
        assert!(!envs.contains_key(OsStr::new("TERMINATOR_DBUS_PATH")));
        assert!(!envs.contains_key(OsStr::new("TERMINATOR_UUID")));
        assert!(!envs.contains_key(OsStr::new("TERMINFO")));
        assert!(!envs.contains_key(OsStr::new("TERMINFO_DIRS")));
        assert!(!envs.contains_key(OsStr::new("DEBIAN_FRONTEND")));
    }

    #[test]
    fn test_configure_terminal_environment_does_not_overwrite_unrelated_inherited_variables() {
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.env("PATH", "/deterministic/bin");
        cmd.env("HOME", "/deterministic/home");
        cmd.env("LANG", "en_US.UTF-8");
        cmd.env("LC_CTYPE", "UTF-8");
        cmd.env("USER", "testuser");
        cmd.env("LOGNAME", "testuser");
        cmd.env("SHELL", "/bin/fish");
        cmd.env("LANGUAGE", "en_US:en");
        cmd.env("LC_ALL", "C.UTF-8");
        cmd.env("XDG_CONFIG_DIRS", "/etc/xdg");
        cmd.env("XDG_DATA_DIRS", "/usr/share");
        cmd.env("XDG_RUNTIME_DIR", "/run/user/1000");
        cmd.env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus");
        cmd.env("DISPLAY", ":0");
        cmd.env("WAYLAND_DISPLAY", "wayland-0");
        cmd.env("HTTP_PROXY", "http://127.0.0.1:8080");
        cmd.env("HTTPS_PROXY", "http://127.0.0.1:8080");
        cmd.env("NO_PROXY", "localhost,127.0.0.1");
        cmd.env("SSH_AUTH_SOCK", "/run/user/1000/ssh.socket");

        configure_terminal_environment(&mut cmd);

        let envs: HashMap<_, _> = cmd.get_envs().collect();
        assert_eq!(
            envs.get(OsStr::new("PATH")),
            Some(&Some(OsStr::new("/deterministic/bin")))
        );
        assert_eq!(
            envs.get(OsStr::new("HOME")),
            Some(&Some(OsStr::new("/deterministic/home")))
        );
        assert_eq!(
            envs.get(OsStr::new("LANG")),
            Some(&Some(OsStr::new("en_US.UTF-8")))
        );
        assert_eq!(
            envs.get(OsStr::new("LC_CTYPE")),
            Some(&Some(OsStr::new("UTF-8")))
        );
        assert_eq!(
            envs.get(OsStr::new("USER")),
            Some(&Some(OsStr::new("testuser")))
        );
        assert_eq!(
            envs.get(OsStr::new("LOGNAME")),
            Some(&Some(OsStr::new("testuser")))
        );
        assert_eq!(
            envs.get(OsStr::new("SHELL")),
            Some(&Some(OsStr::new("/bin/fish")))
        );
        assert_eq!(
            envs.get(OsStr::new("LANGUAGE")),
            Some(&Some(OsStr::new("en_US:en")))
        );
        assert_eq!(
            envs.get(OsStr::new("LC_ALL")),
            Some(&Some(OsStr::new("C.UTF-8")))
        );
        assert_eq!(
            envs.get(OsStr::new("XDG_CONFIG_DIRS")),
            Some(&Some(OsStr::new("/etc/xdg")))
        );
        assert_eq!(
            envs.get(OsStr::new("XDG_DATA_DIRS")),
            Some(&Some(OsStr::new("/usr/share")))
        );
        assert_eq!(
            envs.get(OsStr::new("XDG_RUNTIME_DIR")),
            Some(&Some(OsStr::new("/run/user/1000")))
        );
        assert_eq!(
            envs.get(OsStr::new("DBUS_SESSION_BUS_ADDRESS")),
            Some(&Some(OsStr::new("unix:path=/run/user/1000/bus")))
        );
        assert_eq!(
            envs.get(OsStr::new("DISPLAY")),
            Some(&Some(OsStr::new(":0")))
        );
        assert_eq!(
            envs.get(OsStr::new("WAYLAND_DISPLAY")),
            Some(&Some(OsStr::new("wayland-0")))
        );
        assert_eq!(
            envs.get(OsStr::new("HTTP_PROXY")),
            Some(&Some(OsStr::new("http://127.0.0.1:8080")))
        );
        assert_eq!(
            envs.get(OsStr::new("HTTPS_PROXY")),
            Some(&Some(OsStr::new("http://127.0.0.1:8080")))
        );
        assert_eq!(
            envs.get(OsStr::new("NO_PROXY")),
            Some(&Some(OsStr::new("localhost,127.0.0.1")))
        );
        assert_eq!(
            envs.get(OsStr::new("SSH_AUTH_SOCK")),
            Some(&Some(OsStr::new("/run/user/1000/ssh.socket")))
        );
    }

    #[test]
    fn test_configure_terminal_environment_works_for_both_shell_and_explicit_commands() {
        // Shell command
        let mut shell_cmd = std::process::Command::new("/usr/bin/fish");
        configure_terminal_environment(&mut shell_cmd);
        let shell_envs: HashMap<_, _> = shell_cmd.get_envs().collect();
        assert_eq!(
            shell_envs.get(OsStr::new("TERM")),
            Some(&Some(OsStr::new(VELOX_TERM)))
        );
        assert_eq!(
            shell_envs.get(OsStr::new("COLORTERM")),
            Some(&Some(OsStr::new(VELOX_COLORTERM)))
        );

        // Explicit command
        let mut explicit_cmd = std::process::Command::new("git");
        explicit_cmd.args(["status", "--short"]);
        configure_terminal_environment(&mut explicit_cmd);
        let explicit_envs: HashMap<_, _> = explicit_cmd.get_envs().collect();
        assert_eq!(
            explicit_envs.get(OsStr::new("TERM")),
            Some(&Some(OsStr::new(VELOX_TERM)))
        );
        assert_eq!(
            explicit_envs.get(OsStr::new("COLORTERM")),
            Some(&Some(OsStr::new(VELOX_COLORTERM)))
        );
    }

    #[test]
    fn test_pty_integration_terminal_environment_and_tty() {
        // Set unique inherited test variable
        unsafe {
            std::env::set_var("VELOX_TEST_INHERITED", "value-from-parent");
        }

        let cmd_args = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf \"%s\\n\" \"$TERM\" \"$COLORTERM\" \"$VELOX_TEST_INHERITED\"; test -t 0 && echo tty".to_string(),
        ];

        let pty = spawn_process("/bin/sh", Some(&cmd_args), None)
            .expect("spawn_process failed for integration test");

        let mut output = String::new();
        let mut buf = [0u8; 1024];
        let start = std::time::Instant::now();

        while start.elapsed() < std::time::Duration::from_secs(3) {
            match pty.read(&mut buf) {
                Ok(n) if n > 0 => {
                    output.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if output.contains("tty") {
                        break;
                    }
                }
                Ok(_) => break,
                Err(e) if e.raw_os_error() == Some(libc::EIO) => {
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }

        if let Some(pid) = pty.child_pid {
            let mut status = 0;
            unsafe {
                libc::waitpid(pid as i32, &mut status, 0);
            }
        }

        assert!(
            output.contains("xterm-256color"),
            "Expected output to contain 'xterm-256color', got: {:?}",
            output
        );
        assert!(
            output.contains("truecolor"),
            "Expected output to contain 'truecolor', got: {:?}",
            output
        );
        assert!(
            output.contains("value-from-parent"),
            "Expected output to contain 'value-from-parent', got: {:?}",
            output
        );
        assert!(
            output.contains("tty"),
            "Expected output to contain 'tty', got: {:?}",
            output
        );
    }

    #[test]
    fn test_pty_integration_interactive_shell_environment() {
        // Set unique inherited test variable
        unsafe {
            std::env::set_var("VELOX_TEST_INHERITED", "value-from-parent");
        }

        let pty = spawn_process("/bin/sh", None, None)
            .expect("spawn_process failed for interactive shell");
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Write command into the shell's stdin. The printf template does not contain
        // the resolved values, so matching on the resolved string avoids breaking on echoed input.
        let cmd = b"printf 'INTERACTIVE_OUT:%s:%s:%s\\n' \"$TERM\" \"$COLORTERM\" \"$VELOX_TEST_INHERITED\"; exit\n";
        let _ = pty.write(cmd);

        let mut output = String::new();
        let mut buf = [0u8; 1024];
        let start = std::time::Instant::now();

        while start.elapsed() < std::time::Duration::from_secs(3) {
            match pty.read(&mut buf) {
                Ok(n) if n > 0 => {
                    output.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if output.contains("INTERACTIVE_OUT:xterm-256color:truecolor:value-from-parent")
                    {
                        break;
                    }
                }
                Ok(_) => break,
                Err(e) if e.raw_os_error() == Some(libc::EIO) => {
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }

        if let Some(pid) = pty.child_pid {
            let mut status = 0;
            unsafe {
                libc::waitpid(pid as i32, &mut status, 0);
            }
        }

        assert!(
            output.contains("INTERACTIVE_OUT:xterm-256color:truecolor:value-from-parent"),
            "Expected interactive shell output to contain formatted result, got: {:?}",
            output
        );
    }
}
