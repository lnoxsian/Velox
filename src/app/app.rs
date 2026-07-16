#[derive(Debug)]
pub enum AppError {
    Initialization(String),
}

pub struct App {
    // app state
}

impl App {
    pub fn new() -> Self {
        Self {}
    }

    pub fn initialize(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    pub fn render(&mut self) {
        // stub
    }

    pub fn shutdown(&mut self) {
        // stub
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
