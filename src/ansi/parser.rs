use crate::ansi::state::ParserState;

pub struct AnsiParser {
    pub state: ParserState,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
        }
    }

    pub fn feed(&mut self, _byte: u8) {
        // stub
    }

    pub fn parse_byte(&mut self, _byte: u8) -> ParserState {
        self.state
    }

    pub fn execute(&mut self) {
        // stub
    }

    pub fn dispatch(&mut self) {
        // stub
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}
