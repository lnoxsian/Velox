#[derive(Debug)]
pub enum WindowError {
    Creation(String),
}

pub struct Window {
    // window handle state
}

impl Window {
    pub fn create_window() -> Result<Self, WindowError> {
        Ok(Self {})
    }

    pub fn resize(&mut self, _width: u32, _height: u32) {
        // stub
    }

    pub fn set_title(&mut self, _title: &str) {
        // stub
    }

    pub fn set_icon(&mut self) {
        // stub
    }

    pub fn toggle_fullscreen(&mut self) {
        // stub
    }

    pub fn request_redraw(&self) {
        // stub
    }
}
