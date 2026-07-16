#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Wayland,
    X11,
}

pub fn detect_backend() -> BackendType {
    BackendType::Wayland
}

pub fn initialize_backend() -> Result<(), String> {
    Ok(())
}

pub fn poll_events() {
    // stub
}
