use std::collections::HashMap;
use std::ffi::OsStr;
use std::process::Command;
use velox::diagnostics::format_terminal_diagnostics;
use velox::pty::{
    VELOX_COLORTERM, VELOX_TERM, configure_terminal_environment, spawn_process, spawn_terminal_pty,
    spawn_terminal_pty_with_size,
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

#[test]
fn test_spawn_terminal_pty_with_initial_geometry() {
    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "stty size".to_string(),
    ];
    let pty = spawn_terminal_pty_with_size("/bin/sh", Some(&cmd), None, Some((135, 42)))
        .expect("spawn_terminal_pty_with_size must succeed with initial geometry");

    let mut output = String::new();
    let mut buf = [0u8; 512];
    let start = std::time::Instant::now();

    while start.elapsed() < std::time::Duration::from_secs(3) {
        match pty.read(&mut buf) {
            Ok(n) if n > 0 => {
                output.push_str(&String::from_utf8_lossy(&buf[..n]));
                if output.contains("42 135") {
                    break;
                }
            }
            Ok(_) => break,
            Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(10));
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
        output.contains("42 135"),
        "Terminal slave must be initialized with exact rows (42) and cols (135) at spawn time, got: {:?}",
        output
    );
}

#[test]
fn test_spawn_process_synchronous_exec_error() {
    let non_existent = "/nonexistent_binary_directory/velox_invalid_program_12345";
    let res = spawn_process(non_existent, None, None);
    assert!(
        res.is_err(),
        "Spawning non-existent executable must fail synchronously"
    );
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("exec failed with errno 2") || err_str.contains("No such file or directory"),
        "Error message must report exact exec failure errno, got: {}",
        err_str
    );
}

#[test]
fn test_spawn_process_invalid_working_directory_fallback() {
    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "pwd".to_string(),
    ];
    let pty = spawn_terminal_pty(
        "/bin/sh",
        Some(&cmd),
        Some("/nonexistent_directory_for_velox_test_xyz"),
    )
    .expect("spawn_terminal_pty must gracefully fall back to $HOME when requested directory is invalid");

    let mut output = String::new();
    let mut buf = [0u8; 512];
    let start = std::time::Instant::now();

    while start.elapsed() < std::time::Duration::from_secs(3) {
        match pty.read(&mut buf) {
            Ok(n) if n > 0 => {
                output.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            Ok(_) => break,
            Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(10));
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
        !output.trim().is_empty(),
        "Fallback working directory must succeed and execute pwd"
    );
}

#[test]
fn test_pty_master_foreground_info_stack_buffer() {
    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 1".to_string(),
    ];
    let pty = spawn_process("/bin/sh", Some(&cmd), Some("/tmp"))
        .expect("spawn_process failed for foreground info test");

    std::thread::sleep(std::time::Duration::from_millis(50));

    let cwd = pty.get_foreground_process_cwd();
    assert!(
        cwd.is_some(),
        "get_foreground_process_cwd must return valid directory"
    );
    assert_eq!(
        cwd.unwrap(),
        "/tmp",
        "Process cwd must reflect /tmp directory"
    );

    let name = pty.get_foreground_process_name();
    assert!(
        name.is_some(),
        "get_foreground_process_name must return foreground process comm"
    );

    if let Some(pid) = pty.child_pid {
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
            let mut status = 0;
            libc::waitpid(pid as i32, &mut status, 0);
        }
    }
}
