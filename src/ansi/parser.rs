use crate::ansi::state::ParserState;
use crate::screen::cell::{CellFlags, Color};

pub struct AnsiParser {
    pub state: ParserState,
    pub params: Vec<u16>,
    pub param_buf: String,
    pub osc_buf: Vec<u8>,
    pub utf8_buf: Vec<u8>,
    pub is_private: bool,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            param_buf: String::new(),
            osc_buf: Vec::new(),
            utf8_buf: Vec::new(),
            is_private: false,
        }
    }

    pub fn feed(&mut self, byte: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        match self.state {
            ParserState::Ground => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                } else if byte == 0x0a {
                    terminal.active_grid_mut().scroll_or_move_down();
                } else if byte == 0x0d {
                    terminal.active_grid_mut().cursor.x = 0;
                } else if byte == 0x08 {
                    let active = terminal.active_grid_mut();
                    if active.cursor.x > 0 {
                        active.cursor.x -= 1;
                    }
                } else if byte == 0x09 {
                    let active = terminal.active_grid_mut();
                    let next_tab = (active.cursor.x + 8) & !7;
                    active.cursor.x = next_tab.min(active.width - 1);
                } else if byte >= 0x20 {
                    self.handle_char_byte(byte, terminal);
                }
            }
            ParserState::Escape => {
                if byte == b'[' {
                    self.state = ParserState::CSI;
                    self.params.clear();
                    self.param_buf.clear();
                } else if byte == b']' {
                    self.state = ParserState::OSC;
                    self.osc_buf.clear();
                } else {
                    match byte {
                        b'7' => { terminal.save_cursor(); }
                        b'8' => { terminal.restore_cursor(); }
                        _ => {}
                    }
                    self.state = ParserState::Ground;
                }
            }
            ParserState::CSI => {
                if byte == b'?' {
                    self.is_private = true;
                } else if byte >= 0x30 && byte <= 0x3f {
                    self.param_buf.push(byte as char);
                } else if byte >= 0x40 && byte <= 0x7e {
                    self.parse_params();
                    self.dispatch_csi(byte, terminal);
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

    pub fn parse_byte(&mut self, _byte: u8) -> ParserState {
        self.state
    }

    pub fn execute(&mut self) {
        // stub
    }

    pub fn dispatch(&mut self) {
        // stub
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
        if self.param_buf.is_empty() { return; }
        for s in self.param_buf.split(';') {
            if let Ok(val) = s.parse::<u16>() {
                self.params.push(val);
            } else {
                self.params.push(0);
            }
        }
    }

    fn dispatch_csi(&mut self, cmd: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        match cmd {
            b'm' => { // Select Graphic Rendition
                if self.params.is_empty() {
                    terminal.reset_attrs();
                } else {
                    let mut i = 0;
                    while i < self.params.len() {
                        let code = self.params[i];
                        match code {
                            0 => terminal.reset_attrs(),
                            1 => terminal.current_flags.insert(CellFlags::BOLD),
                            3 => terminal.current_flags.insert(CellFlags::ITALIC),
                            4 => terminal.current_flags.insert(CellFlags::UNDERLINE),
                            5 => terminal.current_flags.insert(CellFlags::BLINK),
                            7 => terminal.current_flags.insert(CellFlags::REVERSE),
                            8 => terminal.current_flags.insert(CellFlags::HIDDEN),
                            9 => terminal.current_flags.insert(CellFlags::STRIKE),
                            22 => terminal.current_flags.remove(CellFlags::BOLD),
                            23 => terminal.current_flags.remove(CellFlags::ITALIC),
                            24 => terminal.current_flags.remove(CellFlags::UNDERLINE),
                            25 => terminal.current_flags.remove(CellFlags::BLINK),
                            27 => terminal.current_flags.remove(CellFlags::REVERSE),
                            28 => terminal.current_flags.remove(CellFlags::HIDDEN),
                            29 => terminal.current_flags.remove(CellFlags::STRIKE),
                            30..=37 => terminal.current_fg = terminal.theme.get_ansi_color(code - 30, false),
                            38 => {
                                if i + 2 < self.params.len() && self.params[i+1] == 5 {
                                    terminal.current_fg = terminal.theme.get_256_color(self.params[i+2] as u8);
                                    i += 2;
                                } else if i + 4 < self.params.len() && self.params[i+1] == 2 {
                                    terminal.current_fg = Color { r: self.params[i+2] as u8, g: self.params[i+3] as u8, b: self.params[i+4] as u8, a: 255 };
                                    i += 4;
                                }
                            }
                            39 => terminal.current_fg = terminal.theme.default_fg,
                            40..=47 => terminal.current_bg = terminal.theme.get_ansi_color(code - 40, true),
                            48 => {
                                if i + 2 < self.params.len() && self.params[i+1] == 5 {
                                    terminal.current_bg = terminal.theme.get_256_color(self.params[i+2] as u8);
                                    i += 2;
                                } else if i + 4 < self.params.len() && self.params[i+1] == 2 {
                                    terminal.current_bg = Color { r: self.params[i+2] as u8, g: self.params[i+3] as u8, b: self.params[i+4] as u8, a: 255 };
                                    i += 4;
                                }
                            }
                            49 => terminal.current_bg = terminal.theme.default_bg,
                            90..=97 => terminal.current_fg = terminal.theme.get_ansi_color(code - 90 + 8, false),
                            100..=107 => terminal.current_bg = terminal.theme.get_ansi_color(code - 100 + 8, true),
                            _ => {}
                        }
                        i += 1;
                    }
                }
            }
            b'A' => { // Cursor Up
                let n = self.params.first().copied().unwrap_or(1) as usize;
                let active = terminal.active_grid_mut();
                active.cursor.y = active.cursor.y.saturating_sub(n);
            }
            b'B' => { // Cursor Down
                let n = self.params.first().copied().unwrap_or(1) as usize;
                let active = terminal.active_grid_mut();
                active.cursor.y = (active.cursor.y + n).min(active.height - 1);
            }
            b'C' => { // Cursor Forward
                let n = self.params.first().copied().unwrap_or(1) as usize;
                let active = terminal.active_grid_mut();
                active.cursor.x = (active.cursor.x + n).min(active.width - 1);
            }
            b'D' => { // Cursor Backward
                let n = self.params.first().copied().unwrap_or(1) as usize;
                let active = terminal.active_grid_mut();
                active.cursor.x = active.cursor.x.saturating_sub(n);
            }
            b'H' | b'f' => { // Cursor Position
                let r = self.params.first().copied().unwrap_or(1).saturating_sub(1) as usize;
                let c = self.params.get(1).copied().unwrap_or(1).saturating_sub(1) as usize;
                let active = terminal.active_grid_mut();
                active.cursor.y = r.min(active.height - 1);
                active.cursor.x = c.min(active.width - 1);
            }
            b'J' => { // Erase Display
                let mode = self.params.first().copied().unwrap_or(0) as u8;
                terminal.active_grid_mut().erase_display(mode);
            }
            b'K' => { // Erase Line
                let mode = self.params.first().copied().unwrap_or(0) as u8;
                terminal.active_grid_mut().erase_line(mode);
            }
            b'h' | b'l' => { // Modes
                let active = cmd == b'h';
                if self.is_private {
                    for &mode in &self.params {
                        match mode {
                            1049 => { terminal.set_alt_screen(active); }
                            25 => { terminal.active_grid_mut().cursor.visible = active; }
                            _ => {}
                        }
                    }
                }
            }
            b'n' => { // Device Status Report (DSR)
                if self.params.first() == Some(&6) {
                    let active = terminal.active_grid();
                    let response = format!("\x1b[{};{}R", active.cursor.y + 1, active.cursor.x + 1);
                    terminal.send_to_shell(response.as_bytes());
                } else if self.params.first() == Some(&5) {
                    terminal.send_to_shell(b"\x1b[0n");
                }
            }
            b'c' => { // Device Attributes (DA)
                terminal.send_to_shell(b"\x1b[?6c");
            }
            _ => {}
        }
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}
