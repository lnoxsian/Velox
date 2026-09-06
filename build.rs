fn main() {
    println!("cargo:rerun-if-changed=assets/icons/velox_terminal_icon_final.svg");
    println!("cargo:rerun-if-changed=assets/icons/velox_terminal_icon_final.png");
    println!("cargo:rerun-if-changed=assets/io.github.lnoxsian.Velox.desktop");
    println!("cargo:rerun-if-changed=scripts/generate_icons.py");

    let icon_file = std::path::Path::new("assets/generated_icons/icon_128x128.png");
    if !icon_file.exists() {
        let status = std::process::Command::new("python3")
            .arg("scripts/generate_icons.py")
            .status();

        if status.is_err() || !icon_file.exists() {
            panic!(
                "Required icon asset 'assets/generated_icons/icon_128x128.png' is missing \
                 and could not be automatically generated via 'python3 scripts/generate_icons.py'. \
                 Please run 'python3 scripts/generate_icons.py' or ensure python3 with Pillow is installed."
            );
        }
    }
}
