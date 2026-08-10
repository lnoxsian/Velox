fn main() {
    println!("cargo:rerun-if-changed=assets/icons/velox_terminal_icon_final.svg");
    println!("cargo:rerun-if-changed=assets/icons/velox_terminal_icon_final.png");
    println!("cargo:rerun-if-changed=scripts/generate_icons.py");

    let icon_file = std::path::Path::new("assets/generated_icons/icon_128x128.png");
    if !icon_file.exists() {
        let _ = std::process::Command::new("python3")
            .arg("scripts/generate_icons.py")
            .status();
    }
}
