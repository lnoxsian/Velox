use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_EMBEDDED_ICONS: &[&str] = &[
    "assets/generated_icons/icon_16x16.png",
    "assets/generated_icons/icon_32x32.png",
    "assets/generated_icons/icon_48x48.png",
    "assets/generated_icons/icon_128x128.png",
];

const REQUIRED_DISTRIBUTION_ICONS: &[&str] = &[
    "assets/generated_icons/icon_64x64.png",
    "assets/generated_icons/icon_256x256.png",
    "assets/generated_icons/icon_512x512.png",
    "assets/generated_icons/icon_1024x1024.png",
    "assets/generated_icons/velox_icon.ico",
];

const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n";

#[test]
fn test_distributed_icon_assets_exist_and_are_valid() {
    for icon_path_str in REQUIRED_EMBEDDED_ICONS
        .iter()
        .chain(REQUIRED_DISTRIBUTION_ICONS)
    {
        let path = Path::new(icon_path_str);
        assert!(
            path.exists(),
            "Required icon asset '{}' must exist and be committed/distributed in the repository",
            icon_path_str
        );

        let data = fs::read(path).unwrap_or_else(|e| {
            panic!(
                "Failed to read required icon file '{}': {}",
                icon_path_str, e
            )
        });
        assert!(
            !data.is_empty(),
            "Icon asset '{}' must not be an empty file",
            icon_path_str
        );

        if icon_path_str.ends_with(".png") {
            assert!(
                data.starts_with(PNG_HEADER),
                "Icon asset '{}' must be a valid PNG file starting with standard PNG header",
                icon_path_str
            );
        } else if icon_path_str.ends_with(".ico") {
            // ICO magic header is [0x00, 0x00, 0x01, 0x00]
            assert!(
                data.starts_with(&[0x00, 0x00, 0x01, 0x00]),
                "Icon asset '{}' must be a valid ICO file",
                icon_path_str
            );
        }
    }
}

fn compile_build_script() -> PathBuf {
    let out_dir =
        std::env::temp_dir().join(format!("velox_test_build_runner_{}", std::process::id()));
    let _ = fs::create_dir_all(&out_dir);
    let bin_path = out_dir.join("build_script_bin");

    let status = Command::new("rustc")
        .arg("build.rs")
        .arg("-o")
        .arg(&bin_path)
        .status()
        .expect("Failed to compile build.rs for test verification");

    assert!(status.success(), "build.rs must compile without errors");
    bin_path
}

struct TempMockScript {
    path: PathBuf,
}

impl TempMockScript {
    fn create(name: &str, content: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!("velox_test_{}_{}", std::process::id(), name));
        fs::write(&path, content).expect("Failed to write mock script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
        Self { path }
    }
}

impl Drop for TempMockScript {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[test]
fn test_build_script_succeeds_when_assets_present() {
    let bin_path = compile_build_script();

    let output = Command::new(&bin_path)
        .output()
        .expect("Failed to execute build.rs");

    assert!(
        output.status.success(),
        "build.rs must exit with code 0 when required assets are present"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo:rerun-if-changed=assets/icons/velox_terminal_icon_final.svg"));
    assert!(stdout.contains("cargo:rerun-if-changed=scripts/generate_icons.py"));
    assert!(stdout.contains("cargo:rerun-if-changed=assets/generated_icons/icon_128x128.png"));
}

#[test]
fn test_build_script_actionable_error_on_missing_python() {
    let bin_path = compile_build_script();

    let output = Command::new(&bin_path)
        .env("VELOX_BUILD_TEST_FORCE_MISSING_ICONS", "1")
        .env("VELOX_PYTHON_BIN", "non_existent_python_binary_xyz_123")
        .output()
        .expect("Failed to execute build.rs");

    assert!(
        !output.status.success(),
        "build.rs must fail with non-zero exit code when required icons are missing and python cannot run"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "build.rs must exit cleanly with code 1 rather than panicking with code 101"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("VELOX BUILD ERROR: Missing Required Icon Assets"),
        "stderr must contain actionable error banner: {}",
        stderr
    );
    assert!(
        stderr.contains("non_existent_python_binary_xyz_123"),
        "stderr must report the failed python binary command"
    );
    assert!(
        stderr.contains("pip install Pillow"),
        "stderr must include actionable Pillow installation instructions"
    );
    assert!(
        !stderr.contains("panicked at"),
        "stderr must not be an unformatted panic backtrace"
    );
}

#[test]
fn test_build_script_actionable_error_when_pillow_missing() {
    let bin_path = compile_build_script();

    let mock_script = TempMockScript::create(
        "mock_failing_generator.sh",
        b"#!/bin/sh\necho \"ModuleNotFoundError: No module named 'PIL'\" >&2\nexit 1\n",
    );

    let output = Command::new(&bin_path)
        .env("VELOX_BUILD_TEST_FORCE_MISSING_ICONS", "1")
        .env("VELOX_PYTHON_BIN", &mock_script.path)
        .output()
        .expect("Failed to execute build.rs");

    assert!(
        !output.status.success(),
        "build.rs must fail when generator returns non-zero"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "build.rs must exit cleanly with code 1"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("VELOX BUILD ERROR: Icon Generation Failed (exit code 1)"),
        "stderr must state the exit code: {}",
        stderr
    );
    assert!(
        stderr.contains("No module named 'PIL'"),
        "stderr must include the generator's stderr output: {}",
        stderr
    );
    assert!(
        stderr.contains("Python 3 is installed, but the 'Pillow' image library is missing"),
        "stderr must identify Pillow as the missing dependency"
    );
    assert!(
        stderr.contains("python3 -m pip install Pillow"),
        "stderr must recommend pip install Pillow"
    );
    assert!(
        stderr.contains("apt-get install python3-pil")
            || stderr.contains("dnf install python3-pillow"),
        "stderr must recommend package manager alternatives"
    );
    assert!(
        !stderr.contains("panicked at"),
        "stderr must not be an unformatted panic backtrace"
    );
}

#[test]
fn test_build_script_never_silently_succeeds_with_missing_assets() {
    let bin_path = compile_build_script();

    let mock_noop_script = TempMockScript::create(
        "mock_noop_generator.sh",
        b"#!/bin/sh\necho \"Fake success without creating files\"\nexit 0\n",
    );

    let output = Command::new(&bin_path)
        .env("VELOX_BUILD_TEST_FORCE_MISSING_ICONS", "1")
        .env("VELOX_PYTHON_BIN", &mock_noop_script.path)
        .output()
        .expect("Failed to execute build.rs");

    assert!(
        !output.status.success(),
        "build.rs must NOT silently succeed when required icon assets are missing even if generator script returned 0"
    );
    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("VELOX BUILD ERROR"),
        "Must output clear error banner: {}",
        stderr
    );
    assert!(
        stderr.contains("Fake success without creating files"),
        "Must display stdout to assist debugging: {}",
        stderr
    );
}
