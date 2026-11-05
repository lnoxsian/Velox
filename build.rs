use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

/// Required icon assets that are embedded via `include_bytes!` in `src/window/x11/icon.rs`
/// and strictly required to compile the Velox binary.
const REQUIRED_EMBEDDED_ICONS: &[&str] = &[
    "icon_16x16.png",
    "icon_32x32.png",
    "icon_48x48.png",
    "icon_128x128.png",
];

/// Distribution icon assets packaged in system packages (Debian .deb, Arch, Fedora .rpm, Makefile).
const REQUIRED_DISTRIBUTION_ICONS: &[&str] = &[
    "icon_64x64.png",
    "icon_256x256.png",
    "icon_512x512.png",
    "icon_1024x1024.png",
    "velox_icon.ico",
];

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/velox_terminal_icon_final.svg");
    println!("cargo:rerun-if-changed=assets/icons/velox_terminal_icon_final.png");
    println!("cargo:rerun-if-changed=assets/io.github.lnoxsian.Velox.desktop");
    println!("cargo:rerun-if-changed=scripts/generate_icons.py");

    let icons_dir = std::env::var("VELOX_GENERATED_ICONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("assets/generated_icons"));

    for icon_name in REQUIRED_EMBEDDED_ICONS
        .iter()
        .chain(REQUIRED_DISTRIBUTION_ICONS)
    {
        let icon_path = icons_dir.join(icon_name);
        println!("cargo:rerun-if-changed={}", icon_path.display());
    }

    let force_missing = std::env::var("VELOX_BUILD_TEST_FORCE_MISSING_ICONS").is_ok();

    let missing_icons = find_missing_icons(&icons_dir, force_missing);

    if missing_icons.is_empty() {
        return;
    }

    let python_bin = std::env::var("VELOX_PYTHON_BIN").unwrap_or_else(|_| "python3".to_string());
    let gen_script = std::env::var("VELOX_GENERATE_ICONS_SCRIPT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("scripts/generate_icons.py"));

    let mut cmd = Command::new(&python_bin);
    cmd.arg(&gen_script);
    if let Ok(out_dir) = std::env::var("VELOX_GENERATED_ICONS_DIR") {
        cmd.arg("--out-dir").arg(out_dir);
    }

    let cmd_result = cmd.output();

    match cmd_result {
        Err(err) => {
            print_actionable_error_missing_python(&missing_icons, &python_bin, &err);
            std::process::exit(1);
        }
        Ok(output) => {
            let still_missing = find_missing_icons(&icons_dir, force_missing);

            if !output.status.success() || !still_missing.is_empty() {
                print_actionable_error_generator_failed(
                    &still_missing,
                    &python_bin,
                    &output.status,
                    &output.stdout,
                    &output.stderr,
                );
                std::process::exit(1);
            }
        }
    }
}

fn find_missing_icons(icons_dir: &Path, force_missing: bool) -> Vec<String> {
    if force_missing {
        return REQUIRED_EMBEDDED_ICONS
            .iter()
            .map(|name| icons_dir.join(name).display().to_string())
            .collect();
    }

    REQUIRED_EMBEDDED_ICONS
        .iter()
        .chain(REQUIRED_DISTRIBUTION_ICONS)
        .map(|name| icons_dir.join(name))
        .filter(|path| !path.exists())
        .map(|path| path.display().to_string())
        .collect()
}

fn print_actionable_error_missing_python(
    missing_icons: &[String],
    python_bin: &str,
    err: &std::io::Error,
) {
    eprintln!("\n================================================================================");
    eprintln!("  VELOX BUILD ERROR: Missing Required Icon Assets");
    eprintln!("================================================================================");
    eprintln!("The build requires embedded icon assets in 'assets/generated_icons/'.");
    eprintln!("The following required icon file(s) are missing:");
    for icon in missing_icons {
        eprintln!("  - {}", icon);
    }
    eprintln!(
        "\nAutomatic icon generation failed because '{}' could not be executed:",
        python_bin
    );
    eprintln!("  Command: {} scripts/generate_icons.py", python_bin);
    eprintln!("  Reason:  {}", err);
    eprintln!("\nActionable steps to resolve this build error:");
    eprintln!("  1. Ensure generated icons are committed and distributed with the repository:");
    eprintln!("         git checkout assets/generated_icons/");
    eprintln!("  2. Or install Python 3 with the Pillow image processing library:");
    eprintln!("         python3 -m pip install Pillow");
    eprintln!("     (Debian/Ubuntu: sudo apt-get install python3-pil)");
    eprintln!("     (Fedora/RHEL:   sudo dnf install python3-pillow)");
    eprintln!("     (Arch Linux:    sudo pacman -S python-pillow)");
    eprintln!("  3. Manually run the generation script:");
    eprintln!("         python3 scripts/generate_icons.py");
    eprintln!("================================================================================\n");
}

fn print_actionable_error_generator_failed(
    missing_icons: &[String],
    python_bin: &str,
    status: &ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
) {
    let stderr_str = String::from_utf8_lossy(stderr);
    let stdout_str = String::from_utf8_lossy(stdout);
    let exit_desc = status
        .code()
        .map(|c| format!("exit code {}", c))
        .unwrap_or_else(|| "terminated by signal".to_string());

    let is_pillow_missing = stderr_str.contains("PIL")
        || stderr_str.contains("Pillow")
        || stderr_str.contains("No module named 'PIL'")
        || stdout_str.contains("No module named 'PIL'");

    eprintln!("\n================================================================================");
    eprintln!(
        "  VELOX BUILD ERROR: Icon Generation Failed ({})",
        exit_desc
    );
    eprintln!("================================================================================");
    eprintln!("The build requires embedded icon assets in 'assets/generated_icons/'.");
    eprintln!("The following required icon file(s) are missing or incomplete:");
    for icon in missing_icons {
        eprintln!("  - {}", icon);
    }
    eprintln!("\nGenerator output (stdout):");
    if stdout_str.trim().is_empty() {
        eprintln!("  (empty)");
    } else {
        for line in stdout_str.lines() {
            eprintln!("  | {}", line);
        }
    }
    eprintln!("\nGenerator error (stderr):");
    if stderr_str.trim().is_empty() {
        eprintln!("  (empty)");
    } else {
        for line in stderr_str.lines() {
            eprintln!("  | {}", line);
        }
    }
    eprintln!("\nActionable steps to resolve this build error:");
    if is_pillow_missing {
        eprintln!("  * Python 3 is installed, but the 'Pillow' image library is missing.");
        eprintln!("    Install Pillow using pip or your system package manager:");
        eprintln!("        python3 -m pip install Pillow");
        eprintln!("        (Debian/Ubuntu: sudo apt-get install python3-pil)");
        eprintln!("        (Fedora/RHEL:   sudo dnf install python3-pillow)");
        eprintln!("        (Arch Linux:    sudo pacman -S python-pillow)");
    } else {
        eprintln!(
            "  * Ensure Python 3 with Pillow or an SVG renderer (cairosvg, resvg, inkscape) is available:"
        );
        eprintln!("        python3 -m pip install Pillow cairosvg");
    }
    eprintln!("  * After installing dependencies, run the generator manually:");
    eprintln!("        {} scripts/generate_icons.py", python_bin);
    eprintln!("  * Or restore the committed icon assets:");
    eprintln!("        git checkout assets/generated_icons/");
    eprintln!("================================================================================\n");
}
