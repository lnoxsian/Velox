use crate::ansi::state::ParserState;
use crate::screen::cell::CellFlags;
use smallvec::SmallVec;

pub struct AnsiParser {
    pub state: ParserState,
    pub params: SmallVec<[u16; 8]>,
    pub param_buf: SmallVec<[u8; 16]>,
    pub osc_buf: SmallVec<[u8; 64]>,
    pub dcs_buf: SmallVec<[u8; 64]>,
    pub utf8_buf: SmallVec<[u8; 4]>,
    pub is_private: bool,
    pub prefix: Option<u8>,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: SmallVec::new(),
            param_buf: SmallVec::new(),
            osc_buf: SmallVec::new(),
            dcs_buf: SmallVec::new(),
            utf8_buf: SmallVec::new(),
            is_private: false,
            prefix: None,
        }
    }

    pub fn feed(&mut self, byte: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        match self.state {
            ParserState::Ground => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                    self.prefix = None;
                } else if matches!(byte, 0x08 | 0x09 | 0x0a | 0x0d | 0x0e | 0x0f) {
                    Self::execute(byte, terminal);
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
                    self.prefix = None;
                } else if byte == b']' {
                    self.state = ParserState::OSC;
                    self.osc_buf.clear();
                } else if byte == b'P' {
                    self.state = ParserState::DCS;
                    self.dcs_buf.clear();
                } else if byte == b'\\' {
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
                if byte == b'?' || byte == b'>' || byte == b'<' || byte == b'=' || byte == b'!' {
                    self.is_private = true;
                    if self.prefix.is_none() && self.param_buf.is_empty() {
                        self.prefix = Some(byte);
                    }
                    self.param_buf.push(byte);
                } else if (0x20..=0x3f).contains(&byte) {
                    self.param_buf.push(byte);
                } else if (0x40..=0x7e).contains(&byte) {
                    self.parse_params();
                    crate::ansi::csi::handle_csi(byte, &self.params, self.prefix, terminal);
                    self.state = ParserState::Ground;
                } else if byte == 0x1b {
                    self.state = ParserState::Escape;
                    self.is_private = false;
                    self.prefix = None;
                } else {
                    self.state = ParserState::Ground;
                }
            }
            ParserState::OSC => {
                if byte == 0x07 {
                    self.dispatch_osc(terminal);
                    self.state = ParserState::Ground;
                } else if byte == 0x1b {
                    self.state = ParserState::OscEscape;
                } else {
                    self.osc_buf.push(byte);
                }
            }
            ParserState::OscEscape => {
                if byte == b'\\' {
                    self.dispatch_osc(terminal);
                    self.state = ParserState::Ground;
                } else {
                    self.osc_buf.clear();
                    crate::ansi::esc::handle_escape(byte, terminal);
                    self.state = ParserState::Ground;
                }
            }
            ParserState::DCS => {
                if byte == 0x07 {
                    self.dispatch_dcs(terminal);
                    self.state = ParserState::Ground;
                } else if byte == 0x1b {
                    self.state = ParserState::DcsEscape;
                } else {
                    self.dcs_buf.push(byte);
                }
            }
            ParserState::DcsEscape => {
                if byte == b'\\' {
                    self.dispatch_dcs(terminal);
                    self.state = ParserState::Ground;
                } else {
                    self.dcs_buf.clear();
                    crate::ansi::esc::handle_escape(byte, terminal);
                    self.state = ParserState::Ground;
                }
            }
        }
    }

    pub fn execute(byte: u8, terminal: &mut crate::terminal::terminal::Terminal) {
        match byte {
            0x0a => {
                let bg = terminal.active_grid().default_bg;
                let active = terminal.active_grid_mut();
                if active.cursor.x == active.width {
                    active.cursor.x = 0;
                    active.scroll_or_move_down(bg);
                } else if active.cursor.x == 0
                    && active.cursor.y > 0
                    && active.row_wrapped[active.physical_row(active.cursor.y - 1)]
                {
                    let physical_y = active.physical_row(active.cursor.y);
                    let row_start = physical_y * active.width;
                    let is_empty = active
                        .cells
                        .get(row_start..row_start + active.width)
                        .is_some_and(|slice| {
                            slice.iter().all(|c| {
                                c.character == ' ' && c.flags.is_empty() && c.background == bg
                            })
                        });
                    if is_empty {
                        let prev_physical_y = active.physical_row(active.cursor.y - 1);
                        active.row_wrapped[prev_physical_y] = false;
                    } else {
                        active.scroll_or_move_down(bg);
                    }
                } else {
                    active.scroll_or_move_down(bg);
                }
            }
            0x0d => {
                let active = terminal.active_grid_mut();
                active.cursor.x = 0;
            }
            0x08 => {
                let active = terminal.active_grid_mut();
                if active.cursor.x > 0 {
                    active.cursor.x -= 1;
                    let physical_y = active.physical_row(active.cursor.y);
                    let idx = physical_y * active.width + active.cursor.x;
                    if idx < active.cells.len()
                        && active.cells[idx]
                            .flags
                            .contains(CellFlags::WIDE_CONTINUATION)
                        && active.cursor.x > 0
                    {
                        active.cursor.x -= 1;
                    }
                } else if active.cursor.y > 0
                    && active.row_wrapped[active.physical_row(active.cursor.y - 1)]
                {
                    active.cursor.y -= 1;
                    active.cursor.x = active.width.saturating_sub(1);
                    let physical_y = active.physical_row(active.cursor.y);
                    let idx = physical_y * active.width + active.cursor.x;
                    if idx < active.cells.len()
                        && active.cells[idx]
                            .flags
                            .contains(CellFlags::WIDE_CONTINUATION)
                        && active.cursor.x > 0
                    {
                        active.cursor.x -= 1;
                    }
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
            let charset = if active_charset == 0 {
                terminal.g0_charset
            } else {
                terminal.g1_charset
            };
            if charset == 1 {
                c = Self::translate_dec_line_drawing(c);
            }

            terminal.last_char = Some(c);
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            let uc = terminal.current_underline_color;
            let flags = terminal.current_flags;
            terminal.active_grid_mut().put_char(c, fg, bg, uc, flags);
            return;
        }

        self.utf8_buf.push(byte);
        if let Ok(s) = std::str::from_utf8(&self.utf8_buf) {
            if let Some(mut c) = s.chars().next() {
                let active_charset = terminal.active_charset;
                let charset = if active_charset == 0 {
                    terminal.g0_charset
                } else {
                    terminal.g1_charset
                };
                if charset == 1 {
                    c = Self::translate_dec_line_drawing(c);
                }

                terminal.last_char = Some(c);
                let fg = terminal.current_fg;
                let bg = terminal.current_bg;
                let uc = terminal.current_underline_color;
                let flags = terminal.current_flags;
                terminal.active_grid_mut().put_char(c, fg, bg, uc, flags);
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
                    current_val = Some(
                        current_val
                            .unwrap_or(0)
                            .saturating_mul(10)
                            .saturating_add(digit),
                    );
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
        if self.osc_buf.capacity() > 1024 {
            self.osc_buf.shrink_to_fit();
        }
    }

    fn dispatch_dcs(&mut self, terminal: &mut crate::terminal::terminal::Terminal) {
        if self.dcs_buf.is_empty() {
            return;
        }
        if self.dcs_buf.starts_with(b"$q") {
            // DECRQSS - Request Status String: DCS $ q <req> ST
            let req = &self.dcs_buf[2..];
            match req {
                b"\"p" | b"p" => {
                    // Conformance level (DECSCL): VT420, 8-bit controls
                    terminal.send_to_shell(b"\x1bP1$r64;1\"p\x1b\\");
                }
                b"m" => {
                    // SGR query
                    let mut buf = [0u8; 64];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(cur, "\x1bP1$r0");
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::BOLD)
                    {
                        let _ = write!(cur, ";1");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::DIM)
                    {
                        let _ = write!(cur, ";2");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::ITALIC)
                    {
                        let _ = write!(cur, ";3");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::UNDERLINE)
                    {
                        let _ = write!(cur, ";4");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::DOUBLE_UNDERLINE)
                    {
                        let _ = write!(cur, ";4:2");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::CURLY_UNDERLINE)
                    {
                        let _ = write!(cur, ";4:3");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::BLINK)
                    {
                        let _ = write!(cur, ";5");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::REVERSE)
                    {
                        let _ = write!(cur, ";7");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::HIDDEN)
                    {
                        let _ = write!(cur, ";8");
                    }
                    if terminal
                        .current_flags
                        .contains(crate::screen::cell::CellFlags::STRIKE)
                    {
                        let _ = write!(cur, ";9");
                    }
                    let _ = write!(cur, "m\x1b\\");
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                b"r" => {
                    // DECSTBM margins
                    let active = terminal.active_grid();
                    let top = active.scroll_region_top + 1;
                    let bottom = active.scroll_region_bottom + 1;
                    let mut buf = [0u8; 32];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(cur, "\x1bP1$r{};{}r\x1b\\", top, bottom);
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                b" q" => {
                    // DECSCUSR cursor shape query
                    let shape_code = match terminal.active_grid().cursor.shape {
                        crate::screen::cursor::CursorShape::Block => 2,
                        crate::screen::cursor::CursorShape::Underline => 4,
                        crate::screen::cursor::CursorShape::Beam => 6,
                        crate::screen::cursor::CursorShape::HollowBlock => 2,
                    };
                    let mut buf = [0u8; 32];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(cur, "\x1bP1$r{} q\x1b\\", shape_code);
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                _ => {
                    // Invalid / unsupported request
                    terminal.send_to_shell(b"\x1bP0$r\x1b\\");
                }
            }
        } else if self.dcs_buf.starts_with(b"+q") {
            // XTGETTCAP - Terminfo Capabilities Query: DCS + q <hex1> [; <hex2>...] ST
            let req = &self.dcs_buf[2..];
            let mut matched = false;
            let mut resp_buf = Vec::with_capacity(128);
            resp_buf.extend_from_slice(b"\x1bP1+q");

            for cap_hex in req.split(|&b| b == b';') {
                let cap_str = std::str::from_utf8(cap_hex).unwrap_or("").to_lowercase();
                match cap_str.as_str() {
                    "524742" | "5463" => {
                        // RGB / Tc: 24-bit TrueColor support
                        if matched {
                            resp_buf.push(b';');
                        }
                        resp_buf.extend_from_slice(cap_hex);
                        resp_buf.extend_from_slice(b"=31"); // hex for "1"
                        matched = true;
                    }
                    "536d756c78" => {
                        // Smulx: styled underlines (\E[4:%p1%dm)
                        if matched {
                            resp_buf.push(b';');
                        }
                        resp_buf.extend_from_slice(cap_hex);
                        resp_buf.extend_from_slice(b"=1b5b343a25703125646d");
                        matched = true;
                    }
                    "536574756c63" => {
                        // Setulc: colored underline
                        if matched {
                            resp_buf.push(b';');
                        }
                        resp_buf.extend_from_slice(cap_hex);
                        resp_buf.extend_from_slice(b"=1b5b35383a323a3a257031257b36353533367d252f25643a257031257b3235367d252f257b3235357d252625643a257031257b3235357d25262564253b6d");
                        matched = true;
                    }
                    "4d73" => {
                        // Ms: OSC 52 clipboard
                        if matched {
                            resp_buf.push(b';');
                        }
                        resp_buf.extend_from_slice(cap_hex);
                        resp_buf.extend_from_slice(b"=1b5d35323b25703125733b257032257307");
                        matched = true;
                    }
                    "53796e63" => {
                        // Sync: Synchronized updates mode 2026
                        if matched {
                            resp_buf.push(b';');
                        }
                        resp_buf.extend_from_slice(cap_hex);
                        resp_buf.extend_from_slice(b"=1b5b3f3230323668");
                        matched = true;
                    }
                    "544e" => {
                        // TN: "xterm-256color"
                        if matched {
                            resp_buf.push(b';');
                        }
                        resp_buf.extend_from_slice(cap_hex);
                        resp_buf.extend_from_slice(b"=787465726d2d323536636f6c6f72");
                        matched = true;
                    }
                    _ => {}
                }
            }

            if matched {
                resp_buf.extend_from_slice(b"\x1b\\");
                terminal.send_to_shell(&resp_buf);
            }
            // When no requested capabilities are recognized, silently ignore to prevent
            // leaking raw hex queries into interactive shells and prompts (e.g. ble.sh / starship).
        }
        self.dcs_buf.clear();
        if self.dcs_buf.capacity() > 1024 {
            self.dcs_buf.shrink_to_fit();
        }
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}
