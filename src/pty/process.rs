use crate::pty::master::PtyMaster;
use nix::pty::openpty;
use std::ffi::CString;
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

pub fn spawn_process(
    shell_path: &str,
    command: Option<&[String]>,
    working_directory: Option<&str>,
) -> Result<PtyMaster, PtyError> {
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

                if let Some(dir) = working_directory {
                    if let Ok(c_dir) = CString::new(dir) {
                        if libc::chdir(c_dir.as_ptr()) != 0 {
                            let home = libc::getenv(c"HOME".as_ptr());
                            if !home.is_null() {
                                libc::chdir(home);
                            }
                        }
                    }
                } else {
                    let home = libc::getenv(c"HOME".as_ptr());
                    if !home.is_null() {
                        libc::chdir(home);
                    }
                }
            }

            let program: &str = if let Some(args) = command {
                if !args.is_empty() {
                    &args[0]
                } else {
                    shell_path
                }
            } else {
                shell_path
            };

            let mut cmd = if let Some(args) = command {
                if !args.is_empty() {
                    let mut c = std::process::Command::new(&args[0]);
                    if args.len() > 1 {
                        c.args(&args[1..]);
                    }
                    c
                } else {
                    std::process::Command::new(shell_path)
                }
            } else {
                std::process::Command::new(shell_path)
            };

            configure_terminal_environment(&mut cmd);

            let err = cmd.exec();
            eprintln!("exec failed for '{}': {}", program, err);
            unsafe {
                libc::_exit(127);
            }
        }
        Ok(nix::unistd::ForkResult::Parent { child }) => {
            unsafe {
                libc::close(slave_fd);
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
        Err(e) => Err(PtyError::Fork(e.to_string())),
    }
}

/// Spawns a terminal shell process with layered fallbacks:
/// 1. Requested shell + requested working directory
/// 2. Requested shell + $HOME directory
/// 3. Standard /bin/sh + $HOME directory
pub fn spawn_terminal_pty(
    shell_path: &str,
    command: Option<&[String]>,
    working_directory: Option<&str>,
) -> Result<PtyMaster, PtyError> {
    let home = std::env::var("HOME").ok();
    let effective_dir = working_directory.or(home.as_deref());

    spawn_process(shell_path, command, effective_dir)
        .or_else(|_| spawn_process(shell_path, command, home.as_deref()))
        .or_else(|_| spawn_process("/bin/sh", command, home.as_deref()))
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
