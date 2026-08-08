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
                } else if matches!(byte, 0x08 | 0x09 | 0x0a | 0x0d | 0x0e | 0x0f) {
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
                } else if byte == b'(' {
                    self.state = ParserState::EscapeDesignateG0;
                } else if byte == b')' || byte == b'-' {
                    self.state = ParserState::EscapeDesignateG1;
                } else if byte == b'*' || byte == b'.' {
                    self.state = ParserState::EscapeDesignateG2;
                } else if byte == b'+' || byte == b'/' {
                    self.state = ParserState::EscapeDesignateG3;
                } else {
                    crate::ansi::esc::handle_escape(byte, terminal);
                    self.state = ParserState::Ground;
                }
            }
            ParserState::EscapeDesignateG0 => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                } else {
                    terminal.g0_charset = match byte {
                        b'0' => 1, // DEC Line Drawing
                        _ => 0,    // USASCII or other
                    };
                    self.state = ParserState::Ground;
                }
            }
            ParserState::EscapeDesignateG1 => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                } else {
                    terminal.g1_charset = match byte {
                        b'0' => 1, // DEC Line Drawing
                        _ => 0,    // USASCII or other
                    };
                    self.state = ParserState::Ground;
                }
            }
            ParserState::EscapeDesignateG2 => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                } else {
                    self.state = ParserState::Ground;
                }
            }
            ParserState::EscapeDesignateG3 => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                } else {
                    self.state = ParserState::Ground;
                }
            }
            ParserState::CSI => {
                if byte == b'?' || byte == b'>' || byte == b'<' || byte == b'=' {
                    self.is_private = true;
                    self.param_buf.push(byte);
                } else if byte >= 0x20 && byte <= 0x3f {
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
            0x0e => {
                terminal.active_charset = 1;
            }
            0x0f => {
                terminal.active_charset = 0;
            }
            _ => {}
        }
    }

    fn handle_char_byte(&mut self, byte: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        if self.utf8_buf.is_empty() && byte < 0x80 {
            let mut c = byte as char;
            let active_charset = terminal.active_charset;
            let charset = if active_charset == 0 { terminal.g0_charset } else { terminal.g1_charset };
            if charset == 1 {
                c = Self::translate_dec_line_drawing(c);
            }

            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            let flags = terminal.current_flags;
            terminal.active_grid_mut().put_char(c, fg, bg, flags);
            return;
        }

        self.utf8_buf.push(byte);
        if let Ok(s) = std::str::from_utf8(&self.utf8_buf) {
            if let Some(mut c) = s.chars().next() {
                let active_charset = terminal.active_charset;
                let charset = if active_charset == 0 { terminal.g0_charset } else { terminal.g1_charset };
                if charset == 1 {
                    c = Self::translate_dec_line_drawing(c);
                }

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

fn translate_dec_line_drawing(c: char) -> char {
    match c {
        '_' => ' ',
        'q' => '─',
        'x' => '│',
        'm' => '└',
        'j' => '┘',
        'l' => '┌',
        'k' => '┐',
        't' => '├',
        'u' => '┤',
        'v' => '┴',
        'w' => '┬',
        'n' => '┼',
        'o' => '⎺',
        'p' => '⎻',
        'r' => '⎼',
        's' => '⎽',
        '`' => '◆',
        'a' => '▒',
        'b' => '␉',
        'c' => '␌',
        'd' => '␍',
        'e' => '␊',
        'f' => '°',
        'g' => '±',
        'h' => '␤',
        'i' => '␋',
        '~' => '·',
        'y' => '≤',
        'z' => '≥',
        '{' => 'π',
        '|' => '≠',
        '}' => '£',
        _ => c,
    }
}


    fn parse_params(&mut self) {
        self.params.clear();
        if self.param_buf.is_empty() {
            return;
        }

        let mut current_val: Option<u16> = None;
        let mut is_sub = false;

        for &b in &self.param_buf {
            match b {
                b';' => {
                    let val = current_val.unwrap_or(0);
                    if is_sub {
                        self.params.push(val | 0x8000);
                    } else {
                        self.params.push(val);
                    }
                    current_val = None;
                    is_sub = false;
                }
                b':' => {
                    let val = current_val.unwrap_or(0);
                    if is_sub {
                        self.params.push(val | 0x8000);
                    } else {
                        self.params.push(val);
                    }
                    current_val = None;
                    is_sub = true;
                }
                b'0'..=b'9' => {
                    let digit = (b - b'0') as u16;
                    current_val = Some(current_val.unwrap_or(0).saturating_mul(10).saturating_add(digit));
                }
                _ => {}
            }
        }

        // Push the final parameter
        let val = current_val.unwrap_or(0);
        if is_sub {
            self.params.push(val | 0x8000);
        } else {
            self.params.push(val);
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
