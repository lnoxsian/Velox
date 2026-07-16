use crate::ansi::state::ParserState;
use smallvec::SmallVec;

pub struct AnsiParser {
    pub state: ParserState,
    pub params: SmallVec<[u16; 8]>,
    pub param_buf: SmallVec<[u8; 16]>,
    pub osc_buf: SmallVec<[u8; 64]>,
    pub utf8_buf: SmallVec<[u8; 4]>,
    pub is_private: bool,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: SmallVec::new(),
            param_buf: SmallVec::new(),
            osc_buf: SmallVec::new(),
            utf8_buf: SmallVec::new(),
            is_private: false,
        }
    }

    pub fn feed(&mut self, byte: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        match self.state {
            ParserState::Ground => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                } else if byte == 0x0a || byte == 0x0d || byte == 0x08 || byte == 0x09 {
                    self.execute(byte, terminal);
                } else if byte >= 0x20 {
                    self.handle_char_byte(byte, terminal);
                }
            }
            ParserState::Escape => {
                if byte == b'[' {
                    self.state = ParserState::CSI;
                    self.params.clear();
                    self.param_buf.clear();
                    self.is_private = false;
                } else if byte == b']' {
                    self.state = ParserState::OSC;
                    self.osc_buf.clear();
                } else if byte == b'\\' {
                    self.dispatch_osc(terminal);
                    self.state = ParserState::Ground;
                } else {
                    crate::ansi::esc::handle_escape(byte, terminal);
                    self.state = ParserState::Ground;
                }
            }
            ParserState::CSI => {
                if byte == b'?' {
                    self.is_private = true;
                } else if byte >= 0x30 && byte <= 0x3f {
                    self.param_buf.push(byte);
                } else if byte >= 0x40 && byte <= 0x7e {
                    self.parse_params();
                    crate::ansi::csi::handle_csi(byte, &self.params, self.is_private, terminal);
                    self.state = ParserState::Ground;
                } else if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                } else {
                    self.state = ParserState::Ground;
                }
            }
            ParserState::OSC => {
                if byte == 0x07 {
                    self.dispatch_osc(terminal);
                    self.state = ParserState::Ground;
                } else if byte == 0x1b {
                    self.state = ParserState::Escape;
                } else {
                    self.osc_buf.push(byte);
                }
            }
            _ => { self.state = ParserState::Ground; }
        }
    }

    pub fn execute(&mut self, byte: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        match byte {
            0x0a => {
                let bg = terminal.current_bg;
                terminal.active_grid_mut().scroll_or_move_down(bg);
            }
            0x0d => {
                terminal.active_grid_mut().cursor.x = 0;
            }
            0x08 => {
                let active = terminal.active_grid_mut();
                if active.cursor.x > 0 {
                    active.cursor.x -= 1;
                }
            }
            0x09 => {
                let active = terminal.active_grid_mut();
                let next_tab = (active.cursor.x + 8) & !7;
                active.cursor.x = next_tab.min(active.width - 1);
            }
            _ => {}
        }
    }

    fn handle_char_byte(&mut self, byte: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        self.utf8_buf.push(byte);
        if let Ok(s) = std::str::from_utf8(&self.utf8_buf) {
            if let Some(c) = s.chars().next() {
                let fg = terminal.current_fg;
                let bg = terminal.current_bg;
                let flags = terminal.current_flags;
                terminal.active_grid_mut().put_char(c, fg, bg, flags);
                self.utf8_buf.clear();
            }
        } else if self.utf8_buf.len() >= 4 {
            self.utf8_buf.clear();
        }
    }

    fn parse_params(&mut self) {
        self.params.clear();
        if self.param_buf.is_empty() {
            return;
        }

        for part in self.param_buf.split(|&b| b == b';') {
            if part.is_empty() {
                self.params.push(0);
                continue;
            }

            let mut sub_parts = part.split(|&b| b == b':');
            if let Some(first) = sub_parts.next() {
                let primary = std::str::from_utf8(first).ok()
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(0);
                self.params.push(primary);

                for sub in sub_parts {
                    let sub_val = if sub.is_empty() {
                        0
                    } else {
                        std::str::from_utf8(sub).ok()
                            .and_then(|s| s.parse::<u16>().ok())
                            .unwrap_or(0)
                    };
                    self.params.push(sub_val | 0x8000);
                }
            }
        }
        self.param_buf.clear();
    }

    fn dispatch_osc(&mut self, terminal: &mut crate::terminal::terminal::Terminal) {
        if self.osc_buf.is_empty() {
            return;
        }
        let params: Vec<&[u8]> = self.osc_buf.split(|&b| b == b';').collect();
        crate::ansi::osc::handle_osc(&params, terminal);
        self.osc_buf.clear();
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}
