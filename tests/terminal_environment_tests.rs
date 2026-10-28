use std::collections::HashMap;
use std::ffi::OsStr;
use std::process::Command;
use velox::diagnostics::format_terminal_diagnostics;
use velox::pty::{
    VELOX_COLORTERM, VELOX_TERM, configure_terminal_environment, spawn_process, spawn_terminal_pty,
};

#[test]
fn test_terminal_environment_constants() {
    assert_eq!(VELOX_TERM, "xterm-256color");
    assert_eq!(VELOX_COLORTERM, "truecolor");
}

#[test]
fn test_configure_terminal_environment_sets_expected_profile() {
    let mut cmd = Command::new("/bin/sh");
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
}

#[test]
fn test_configure_terminal_environment_preserves_custom_inherited_variable() {
    let mut cmd = Command::new("/bin/sh");
    cmd.env("VELOX_TEST_INHERITED", "value-from-parent");

    configure_terminal_environment(&mut cmd);

    let envs: HashMap<_, _> = cmd.get_envs().collect();
    assert_eq!(
        envs.get(OsStr::new("VELOX_TEST_INHERITED")),
        Some(&Some(OsStr::new("value-from-parent")))
    );
}

#[test]
fn test_configure_terminal_environment_does_not_set_vte_or_terminator_variables() {
    let mut cmd = Command::new("/bin/sh");
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
fn test_configure_terminal_environment_does_not_overwrite_system_variables() {
    let mut cmd = Command::new("/bin/sh");
    cmd.env("PATH", "/usr/local/bin:/usr/bin");
    cmd.env("HOME", "/home/velox_tester");
    cmd.env("LANG", "en_US.UTF-8");
    cmd.env("LC_CTYPE", "UTF-8");

    configure_terminal_environment(&mut cmd);

    let envs: HashMap<_, _> = cmd.get_envs().collect();
    assert_eq!(
        envs.get(OsStr::new("PATH")),
        Some(&Some(OsStr::new("/usr/local/bin:/usr/bin")))
    );
    assert_eq!(
        envs.get(OsStr::new("HOME")),
        Some(&Some(OsStr::new("/home/velox_tester")))
    );
    assert_eq!(
        envs.get(OsStr::new("LANG")),
        Some(&Some(OsStr::new("en_US.UTF-8")))
    );
    assert_eq!(
        envs.get(OsStr::new("LC_CTYPE")),
        Some(&Some(OsStr::new("UTF-8")))
    );
}

#[test]
fn test_spawn_terminal_pty_integration_preserves_environment_and_sets_term() {
    unsafe {
        std::env::set_var("VELOX_TEST_INHERITED", "value-from-parent");
    }

    let cmd_args = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf \"%s\\n\" \"$TERM\" \"$COLORTERM\" \"$VELOX_TEST_INHERITED\"; test -t 0 && echo tty".to_string(),
    ];

    let pty = spawn_terminal_pty("/bin/sh", Some(&cmd_args), None)
        .expect("spawn_terminal_pty failed for integration test");

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
        "Output should contain xterm-256color, got: {:?}",
        output
    );
    assert!(
        output.contains("truecolor"),
        "Output should contain truecolor, got: {:?}",
        output
    );
    assert!(
        output.contains("value-from-parent"),
        "Output should contain value-from-parent, got: {:?}",
        output
    );
    assert!(
        output.contains("tty"),
        "Output should contain tty, got: {:?}",
        output
    );
}

#[test]
fn test_spawn_process_interactive_shell_preserves_environment() {
    unsafe {
        std::env::set_var("VELOX_TEST_INHERITED", "value-from-parent");
    }

    let pty = spawn_process("/bin/sh", None, None).expect("spawn_process failed");
    std::thread::sleep(std::time::Duration::from_millis(50));
    let cmd =
        b"printf 'RESULT:%s:%s:%s\\n' \"$TERM\" \"$COLORTERM\" \"$VELOX_TEST_INHERITED\"; exit\n";
    let _ = pty.write(cmd);

    let mut output = String::new();
    let mut buf = [0u8; 1024];
    let start = std::time::Instant::now();

    while start.elapsed() < std::time::Duration::from_secs(3) {
        match pty.read(&mut buf) {
            Ok(n) if n > 0 => {
                output.push_str(&String::from_utf8_lossy(&buf[..n]));
                if output.contains("RESULT:xterm-256color:truecolor:value-from-parent") {
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

    assert!(output.contains("RESULT:xterm-256color:truecolor:value-from-parent"));
}

#[test]
fn test_diagnostics_terminal_environment_output() {
    let diag = format_terminal_diagnostics();
    assert!(diag.contains("[Terminal Environment & TTY]"));
    assert!(diag.contains("TERM:"));
    assert!(diag.contains("COLORTERM:"));
    assert!(diag.contains("SHELL:"));
    assert!(diag.contains("PATH:"));
    assert!(diag.contains("HOME:"));
    assert!(diag.contains("LANG:"));
    assert!(diag.contains("LC_ALL:"));
    assert!(diag.contains("LC_CTYPE:"));
    assert!(diag.contains("tty status:"));
    assert!(diag.contains("stty size:"));
}
