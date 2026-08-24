use crate::ansi::parser::AnsiParser;
use crate::screen::cell::{CellFlags, Color};
use crate::screen::grid::Grid;
use crate::theme::theme::Theme;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticZone {
    Prompt,
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptMark {
    pub x: usize,
    pub y: usize,
    pub zone: SemanticZone,
    pub exit_code: Option<i32>,
}

pub struct Terminal {
    pub grid: Grid,
    pub alt_grid: Grid,
    pub is_alt_screen: bool,
    pub parser: AnsiParser,
    pub theme: Theme,
    pub current_fg: Color,
    pub current_bg: Color,
    pub current_flags: CellFlags,
    pub outgoing: Vec<u8>,
    pub cursor_keys_mode: bool,
    pub mouse_mode: u16,
    pub mouse_sgr: bool,
    pub g0_charset: u8,
    pub g1_charset: u8,
    pub active_charset: u8,
    pub bold_is_bright: bool,
    pub app_title: Option<String>,
    pub osc_title: Option<String>,
    pub configured_cursor_shape: crate::screen::cursor::CursorShape,
    pub bracketed_paste_mode: bool,
    pub synchronized_output: bool,
    pub sync_output_start: Option<std::time::Instant>,
    pub focus_tracking: bool,
    pub current_dir: Option<String>,
    pub semantic_zone: SemanticZone,
    pub prompt_marks: VecDeque<PromptMark>,
    pub last_command_exit_code: Option<i32>,
    pub scroll_on_output: bool,
    pub scroll_on_keystroke: bool,
}

impl Terminal {
    pub fn new(width: usize, height: usize) -> Self {
        let config = crate::config::loader::load()
            .unwrap_or_else(|_| crate::config::defaults::default_config());
        let theme = Theme::from_config(&config);

        let default_fg = theme.default_fg;
        let default_bg = theme.default_bg;
        let bold_is_bright = config.bold_is_bright().unwrap_or(true);
        let app_title = config.app_title.clone();

        let initial_shape = match config
            .cursor_shape()
            .unwrap_or("block")
            .to_lowercase()
            .as_str()
        {
            "hollow_block" | "hollowblock" | "hollow" => {
                crate::screen::cursor::CursorShape::HollowBlock
            }
            "beam" | "i" | "ibar" | "bar" => crate::screen::cursor::CursorShape::Beam,
            "underline" => crate::screen::cursor::CursorShape::Underline,
            _ => crate::screen::cursor::CursorShape::Block,
        };

        let scrollback_limit = config.scrollback_limit().unwrap_or(1000);
        let infinite_scrollback = config.infinite_scrollback().unwrap_or(false);

        let mut grid = Grid::new(
            width,
            height,
            default_fg,
            default_bg,
            scrollback_limit,
            infinite_scrollback,
        );
        grid.cursor.shape = initial_shape;

        let mut alt_grid = Grid::new(width, height, default_fg, default_bg, 0, false);
        alt_grid.cursor.shape = initial_shape;

        let scroll_on_output = config.scroll_on_output().unwrap_or(true);
        let scroll_on_keystroke = config.scroll_on_keystroke().unwrap_or(true);

        Self {
            grid,
            alt_grid,
            is_alt_screen: false,
            parser: AnsiParser::new(),
            theme,
            current_fg: default_fg,
            current_bg: default_bg,
            current_flags: CellFlags::empty(),
            outgoing: Vec::new(),
            cursor_keys_mode: false,
            mouse_mode: 0,
            mouse_sgr: false,
            g0_charset: 0,
            g1_charset: 0,
            active_charset: 0,
            bold_is_bright,
            app_title,
            osc_title: None,
            configured_cursor_shape: initial_shape,
            bracketed_paste_mode: false,
            synchronized_output: false,
            sync_output_start: None,
            focus_tracking: false,
            current_dir: None,
            semantic_zone: SemanticZone::Output,
            prompt_marks: VecDeque::new(),
            last_command_exit_code: None,
            scroll_on_output,
            scroll_on_keystroke,
        }
    }

    pub fn mark_semantic_zone(&mut self, zone: SemanticZone, exit_code: Option<i32>) {
        self.semantic_zone = zone;
        let cursor = self.active_grid().cursor;
        self.prompt_marks.push_back(PromptMark {
            x: cursor.x,
            y: cursor.y,
            zone,
            exit_code,
        });
        // Cap prompt marks to prevent unbounded growth in long sessions
        let max_marks = self.grid.scrollback.max_lines.max(1000) * 3;
        while self.prompt_marks.len() > max_marks {
            self.prompt_marks.pop_front();
        }
    }

    pub fn set_synchronized_output(&mut self, enabled: bool) {
        self.synchronized_output = enabled;
        if enabled {
            self.sync_output_start = Some(std::time::Instant::now());
        } else {
            self.sync_output_start = None;
        }
    }

    pub fn is_synchronized_output_active(&mut self) -> bool {
        if self.synchronized_output {
            if let Some(start) = self.sync_output_start
                && start.elapsed() > std::time::Duration::from_millis(150)
            {
                self.synchronized_output = false;
                self.sync_output_start = None;
                return false;
            }
            true
        } else {
            false
        }
    }

    pub fn format_paste(&self, text: &str) -> String {
        if self.bracketed_paste_mode {
            format!("\x1b[200~{}\x1b[201~", text)
        } else {
            text.to_string()
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        if self.scroll_on_output {
            self.grid.scroll_offset = 0;
        }
        // Feed bytes one by one. Note: We temporarily take ownership or borrow
        // to avoid duplicate mutable borrows on Self.
        let mut parser = std::mem::take(&mut self.parser);
        for &byte in data {
            parser.feed(byte, self);
        }
        self.parser = parser;
    }

    pub fn send_to_shell(&mut self, data: &[u8]) {
        self.outgoing.extend_from_slice(data);
    }

    pub fn reset_attrs(&mut self) {
        self.current_fg = self.theme.default_fg;
        self.current_bg = self.theme.default_bg;
        self.current_flags = CellFlags::empty();
    }

    pub fn save_cursor(&mut self) {
        let active_grid = if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        };
        active_grid.saved_cursor = active_grid.cursor;
        active_grid.saved_fg = self.current_fg;
        active_grid.saved_bg = self.current_bg;
        active_grid.saved_flags = self.current_flags;
        active_grid.saved_g0_charset = self.g0_charset;
        active_grid.saved_g1_charset = self.g1_charset;
        active_grid.saved_active_charset = self.active_charset;
    }

    pub fn restore_cursor(&mut self) {
        let active_grid = if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        };
        active_grid.cursor = active_grid.saved_cursor;
        active_grid.clamp_cursor();
        self.current_fg = active_grid.saved_fg;
        self.current_bg = active_grid.saved_bg;
        self.current_flags = active_grid.saved_flags;
        self.g0_charset = active_grid.saved_g0_charset;
        self.g1_charset = active_grid.saved_g1_charset;
        self.active_charset = active_grid.saved_active_charset;
    }

    pub fn set_alt_screen(&mut self, active: bool) {
        if self.is_alt_screen != active {
            self.is_alt_screen = active;
            self.active_grid_mut().mark_all_dirty();
        }
    }

    pub fn active_grid(&self) -> &Grid {
        if self.is_alt_screen {
            &self.alt_grid
        } else {
            &self.grid
        }
    }

    pub fn active_grid_mut(&mut self) -> &mut Grid {
        if self.is_alt_screen {
            &mut self.alt_grid
        } else {
            &mut self.grid
        }
    }

    pub fn resize(&mut self, cols: u32, rows: u32) {
        self.grid.resize(cols, rows);
        self.alt_grid.resize(cols, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_terminal_feed() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"Hello, Velox!");
        let grid = term.active_grid();
        assert_eq!(grid.cursor.x, 13);
        assert_eq!(grid.cursor.y, 0);
        assert_eq!(grid.cells[0].character, 'H');
        assert_eq!(grid.cells[12].character, '!');
    }

    #[test]
    fn test_ansi_cursor_position() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[5;10H");
        let grid = term.active_grid();
        assert_eq!(grid.cursor.x, 9);
        assert_eq!(grid.cursor.y, 4);
    }

    #[test]
    fn test_ansi_colors() {
        let mut term = Terminal::new(80, 24);
        // Set bold, red foreground (SGR 1 and 31)
        term.feed(b"\x1b[1;31mA");
        let grid = term.active_grid();
        let cell = grid.cells[0];
        assert_eq!(cell.character, 'A');
        assert!(cell.flags.contains(CellFlags::BOLD));
        assert_eq!(cell.foreground, term.theme.ansi_colors[1]); // Red
    }

    #[test]
    fn test_osc_st_termination() {
        let mut term = Terminal::new(80, 24);
        // OSC sequence terminated by ST (ESC \)
        term.feed(b"\x1b]7;file://localhost/path\x1b\\Hello");
        let grid = term.active_grid();
        assert_eq!(grid.cursor.x, 5);
        assert_eq!(grid.cells[0].character, 'H');
        assert_eq!(grid.cells[4].character, 'o');
    }

    #[test]
    fn test_csi_interrupted_by_esc() {
        let mut term = Terminal::new(80, 24);
        // CSI sequence interrupted by a new ESC sequence
        term.feed(b"\x1b[\x1b[1;31mA");
        let grid = term.active_grid();
        let cell = grid.cells[0];
        assert_eq!(cell.character, 'A');
        assert!(cell.flags.contains(CellFlags::BOLD));
    }

    #[test]
    fn test_device_status_report() {
        let mut term = Terminal::new(80, 24);
        // Move cursor to (10, 5) which is row 6, col 11
        term.feed(b"\x1b[6;11H");
        term.feed(b"\x1b[6n");
        assert_eq!(term.outgoing, b"\x1b[6;11R");
    }

    #[test]
    fn test_device_attributes() {
        let mut term = Terminal::new(80, 24);
        // Primary Device Attributes (DA1)
        term.feed(b"\x1b[c");
        assert_eq!(term.outgoing, b"\x1b[?6c");

        // Secondary Device Attributes (DA2) - queried by tmux, vim, etc.
        term.outgoing.clear();
        term.feed(b"\x1b[>c");
        assert_eq!(term.outgoing, b"\x1b[>0;10;0c");

        term.outgoing.clear();
        term.feed(b"\x1b[>0c");
        assert_eq!(term.outgoing, b"\x1b[>0;10;0c");

        // XTVERSION query
        term.outgoing.clear();
        term.feed(b"\x1b[>0q");
        assert_eq!(term.outgoing, b"\x1bP>|Velox(0.1.9)\x1b\\");
    }

    #[test]
    fn test_emoji_double_width() {
        let mut term = Terminal::new(80, 24);
        term.feed("😀".as_bytes());
        let grid = term.active_grid();
        assert_eq!(grid.cursor.x, 2);
        assert_eq!(grid.cells[0].character, '😀');
        assert!(grid.cells[0].flags.contains(CellFlags::WIDE));
        assert_eq!(grid.cells[1].character, ' ');
        assert!(grid.cells[1].flags.contains(CellFlags::WIDE_CONTINUATION));
    }

    #[test]
    fn test_sgr_sub_parameters() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"\x1b[4:1mA");
        let grid = term.active_grid();
        assert!(grid.cells[0].flags.contains(CellFlags::UNDERLINE));

        let mut term2 = Terminal::new(80, 24);
        term2.feed(b"\x1b[4:0mA");
        let grid2 = term2.active_grid();
        assert!(!grid2.cells[0].flags.contains(CellFlags::UNDERLINE));
    }

    #[test]
    fn test_scrollback_history() {
        let mut term = Terminal::new(80, 5); // 5 rows
        for i in 0..10 {
            term.feed(format!("line {}\r\n", i).as_bytes());
        }
        let grid = term.active_grid();
        // Since height is 5, we have scrolled multiple lines off the screen
        assert!(!grid.scrollback.is_empty());

        // The first character of the oldest line in scrollback history should be 'l' from "line ..."
        assert_eq!(grid.scrollback.get_row(0).unwrap()[0].character, 'l');
    }

    #[test]
    fn test_charset_designation_and_translation() {
        let mut term = Terminal::new(80, 24);

        // Designate G1 as DEC line drawing (\x1b)0)
        term.feed(b"\x1b)0");
        // Shift Out (\x0e) to activate G1
        term.feed(b"\x0e");
        // Feed 'q' and 'x' which should translate to '─' and '│'
        term.feed(b"qx");

        let grid = term.active_grid();
        assert_eq!(grid.cells[0].character, '─');
        assert_eq!(grid.cells[1].character, '│');

        // Shift In (\x0f) to activate G0 (which defaults to USASCII)
        term.feed(b"\x0f");
        // Feed 'qx' again, which should not translate
        term.feed(b"qx");

        let grid = term.active_grid();
        assert_eq!(grid.cells[2].character, 'q');
        assert_eq!(grid.cells[3].character, 'x');
    }

    #[test]
    fn test_interrupted_charset_designation() {
        let mut term = Terminal::new(80, 24);
        // Start a G0 designation sequence: ESC (
        // But then interrupt it with another ESC sequence: ESC [ H (moves cursor to 1,1)
        term.feed(b"\x1b(\x1b[1;1HA");
        let grid = term.active_grid();
        // The character 'A' should be printed at (0, 0)
        assert_eq!(grid.cells[0].character, 'A');
        assert_eq!(grid.cursor.x, 1);
        assert_eq!(grid.cursor.y, 0);
    }

    #[test]
    fn test_csi_save_restore_cursor() {
        let mut term = Terminal::new(80, 24);
        term.feed(b"Hello");
        assert_eq!(term.active_grid().cursor.x, 5);
        assert_eq!(term.active_grid().cursor.y, 0);

        // Save cursor position via CSI s
        term.feed(b"\x1b[s");

        term.feed(b" World");
        assert_eq!(term.active_grid().cursor.x, 11);
        assert_eq!(term.active_grid().cursor.y, 0);

        // Restore cursor position via CSI u
        term.feed(b"\x1b[u");
        assert_eq!(term.active_grid().cursor.x, 5);
        assert_eq!(term.active_grid().cursor.y, 0);
    }

    #[test]
    fn test_cursor_attributes_preservation() {
        let mut term = Terminal::new(80, 24);
        use crate::screen::cell::CellFlags;

        // 1. Set bold and designate/activate G1 line-drawing
        term.feed(b"\x1b[1m\x1b)0\x0e");
        assert!(term.current_flags.contains(CellFlags::BOLD));
        assert_eq!(term.active_charset, 1);

        // 2. Save cursor and attributes (via ESC 7)
        term.feed(b"\x1b7");

        // 3. Clear bold, reset to G0
        term.feed(b"\x1b[0m\x0f");
        assert!(!term.current_flags.contains(CellFlags::BOLD));
        assert_eq!(term.active_charset, 0);

        // 4. Restore cursor and attributes (via ESC 8)
        term.feed(b"\x1b8");
        assert!(term.current_flags.contains(CellFlags::BOLD));
        assert_eq!(term.active_charset, 1);
    }

    #[test]
    fn test_csi_line_char_editing() {
        let mut term = Terminal::new(80, 24);

        // 1. Test CHA (CSI G) and VPA (CSI d)
        term.feed(b"\x1b[5G\x1b[3d");
        assert_eq!(term.active_grid().cursor.x, 4);
        assert_eq!(term.active_grid().cursor.y, 2);

        // 2. Test ECH (CSI X)
        term.feed(b"\x1b[1;1Hhello");
        term.feed(b"\x1b[1;2H\x1b[3X");
        let grid = term.active_grid();
        assert_eq!(grid.cells[0].character, 'h');
        assert_eq!(grid.cells[1].character, ' ');
        assert_eq!(grid.cells[2].character, ' ');
        assert_eq!(grid.cells[3].character, ' ');
        assert_eq!(grid.cells[4].character, 'o');

        // 3. Test DCH (CSI P)
        let mut term2 = Terminal::new(80, 24);
        term2.feed(b"hello");
        term2.feed(b"\x1b[1;2H\x1b[2P");
        let grid2 = term2.active_grid();
        assert_eq!(grid2.cells[0].character, 'h');
        assert_eq!(grid2.cells[1].character, 'l');
        assert_eq!(grid2.cells[2].character, 'o');
        assert_eq!(grid2.cells[3].character, ' ');

        // 4. Test ICH (CSI @)
        let mut term3 = Terminal::new(80, 24);
        term3.feed(b"hello");
        term3.feed(b"\x1b[1;2H\x1b[2@");
        let grid3 = term3.active_grid();
        assert_eq!(grid3.cells[0].character, 'h');
        assert_eq!(grid3.cells[1].character, ' ');
        assert_eq!(grid3.cells[2].character, ' ');
        assert_eq!(grid3.cells[3].character, 'e');
        assert_eq!(grid3.cells[4].character, 'l');
    }

    #[test]
    fn test_intermediate_bytes_and_dec_line_drawing() {
        let mut term = Terminal::new(80, 24);
        // Test 1: CSI sequences with intermediate bytes like \x1b[+q and \x1b[6q should NOT leak '+' or 'q' into grid
        term.feed(b"\x1b[+q\x1b[6q");
        let grid = term.active_grid();
        assert_eq!(grid.cells[0].character, ' ');
        assert_eq!(grid.cells[1].character, ' ');
        assert_eq!(grid.cursor.x, 0);

        // Test 2: DEC line drawing designation \x1b(0 followed by 'q' should draw horizontal line '─'
        term.feed(b"\x1b(0q");
        assert_eq!(term.active_grid().cells[0].character, '─');
    }

    #[test]
    fn test_decscusr_cursor_shapes() {
        let mut term = Terminal::new(80, 24);
        use crate::screen::cursor::CursorShape;

        term.feed(b"\x1b[5q"); // Beam
        assert_eq!(term.active_grid().cursor.shape, CursorShape::Beam);

        term.feed(b"\x1b[3q"); // Underline
        assert_eq!(term.active_grid().cursor.shape, CursorShape::Underline);

        term.feed(b"\x1b[2q"); // Block
        assert_eq!(term.active_grid().cursor.shape, CursorShape::Block);
    }

    #[test]
    fn test_osc_color_queries_and_sets() {
        let mut term = Terminal::new(80, 24);
        // OSC 10 query
        term.feed(b"\x1b]10;?\x07");
        assert!(!term.outgoing.is_empty());
        let resp = String::from_utf8(term.outgoing.clone()).unwrap();
        assert!(resp.starts_with("\x1b]10;rgb:"));
        term.outgoing.clear();

        // OSC 11 set background
        term.feed(b"\x1b]11;#123456\x07");
        assert_eq!(term.theme.default_bg.r, 0x12);
        assert_eq!(term.theme.default_bg.g, 0x34);
        assert_eq!(term.theme.default_bg.b, 0x56);
    }

    #[test]
    fn test_osc52_clipboard() {
        let mut term = Terminal::new(80, 24);
        // OSC 52 write "SGVsbG8=" ("Hello")
        term.feed(b"\x1b]52;c;SGVsbG8=\x07");
        // OSC 52 query
        term.feed(b"\x1b]52;c;?\x07");
        assert!(!term.outgoing.is_empty());
        let resp = String::from_utf8(term.outgoing.clone()).unwrap();
        assert!(resp.starts_with("\x1b]52;c;"));
    }

    #[test]
    fn test_sgr_underline_color_fallthrough() {
        let mut term = Terminal::new(80, 24);
        // SGR 58;5;4 (Set Underline color) should not trigger CellFlags::UNDERLINE
        term.feed(b"\x1b[58;5;4mA");
        let cell = term.active_grid().cells[0];
        assert_eq!(cell.character, 'A');
        assert!(!cell.flags.contains(CellFlags::UNDERLINE));
    }

    #[test]
    fn test_dcs_xtgettcap_not_leaked() {
        let mut term = Terminal::new(80, 24);
        // Feeding DCS sequence \x1bP+q4D73\x1b\ (Neovim termcap query) should NOT leak "+q4D73" onto grid
        term.feed(b"\x1bP+q4D73\x1b\\");
        assert_eq!(term.active_grid().cells[0].character, ' ');
        assert_eq!(term.active_grid().cells[1].character, ' ');
        assert_eq!(term.active_grid().cursor.x, 0);
    }

    #[test]
    fn test_xtmodkeys_not_treated_as_sgr() {
        let mut term = Terminal::new(80, 24);
        // Fish shell sends CSI > 4;1 m (XTMODKEYS) which must not be treated as SGR
        term.feed(b"\x1b[>4;1m");
        assert!(!term.current_flags.contains(CellFlags::UNDERLINE));
        assert!(!term.current_flags.contains(CellFlags::BOLD));

        term.feed(b"Hello");
        for x in 0..5 {
            assert!(
                !term.active_grid().cells[x]
                    .flags
                    .contains(CellFlags::UNDERLINE)
            );
        }
    }

    #[test]
    fn test_bracketed_paste_mode() {
        let mut term = Terminal::new(80, 24);
        assert!(!term.bracketed_paste_mode);
        assert_eq!(term.format_paste("echo hello"), "echo hello");

        // Enable bracketed paste mode via CSI ? 2004 h
        term.feed(b"\x1b[?2004h");
        assert!(term.bracketed_paste_mode);
        assert_eq!(
            term.format_paste("echo hello"),
            "\x1b[200~echo hello\x1b[201~"
        );

        // Disable bracketed paste mode via CSI ? 2004 l
        term.feed(b"\x1b[?2004l");
        assert!(!term.bracketed_paste_mode);
        assert_eq!(term.format_paste("echo hello"), "echo hello");
    }

    #[test]
    fn test_synchronized_output_mode() {
        let mut term = Terminal::new(80, 24);
        assert!(!term.synchronized_output);
        assert!(!term.is_synchronized_output_active());

        // Enable synchronized output via CSI ? 2026 h
        term.feed(b"\x1b[?2026h");
        assert!(term.synchronized_output);
        assert!(term.is_synchronized_output_active());

        // Query mode status via DECRPM: CSI ? 2026 $ p
        term.outgoing.clear();
        term.feed(b"\x1b[?2026$p");
        assert_eq!(term.outgoing, b"\x1b[?2026;1$y");
        term.outgoing.clear();

        // Disable synchronized output via CSI ? 2026 l
        term.feed(b"\x1b[?2026l");
        assert!(!term.synchronized_output);
        assert!(!term.is_synchronized_output_active());

        // Query mode status via DECRPM after disabling
        term.feed(b"\x1b[?2026$p");
        assert_eq!(term.outgoing, b"\x1b[?2026;2$y");
    }

    #[test]
    fn test_focus_tracking_mode() {
        let mut term = Terminal::new(80, 24);
        assert!(!term.focus_tracking);

        // Enable focus tracking via CSI ? 1004 h
        term.feed(b"\x1b[?1004h");
        assert!(term.focus_tracking);

        // Disable focus tracking via CSI ? 1004 l
        term.feed(b"\x1b[?1004l");
        assert!(!term.focus_tracking);
    }

    #[test]
    fn test_any_event_mouse_tracking_mode() {
        let mut term = Terminal::new(80, 24);
        assert_eq!(term.mouse_mode, 0);

        // Enable any-event mouse tracking via CSI ? 1003 h
        term.feed(b"\x1b[?1003h");
        assert_eq!(term.mouse_mode, 1003);

        // Disable mouse tracking via CSI ? 1003 l
        term.feed(b"\x1b[?1003l");
        assert_eq!(term.mouse_mode, 0);
    }

    #[test]
    fn test_osc7_current_working_directory() {
        let mut term = Terminal::new(80, 24);
        assert_eq!(term.current_dir, None);

        // Feed OSC 7 sequence with hostname: OSC 7 ; file://localhost/home/user/project ST
        term.feed(b"\x1b]7;file://localhost/home/user/project\x1b\\");
        assert_eq!(term.current_dir.as_deref(), Some("/home/user/project"));

        // Feed OSC 7 sequence with percent encoding and BEL termination
        term.feed(b"\x1b]7;file:///home/user/my%20folder\x07");
        assert_eq!(term.current_dir.as_deref(), Some("/home/user/my folder"));
    }

    #[test]
    fn test_osc133_shell_integration() {
        let mut term = Terminal::new(80, 24);
        assert_eq!(term.semantic_zone, SemanticZone::Output);
        assert!(term.prompt_marks.is_empty());

        // Prompt Start: OSC 133 ; A
        term.feed(b"\x1b]133;A\x07");
        assert_eq!(term.semantic_zone, SemanticZone::Prompt);
        assert_eq!(term.prompt_marks.len(), 1);
        assert_eq!(term.prompt_marks[0].zone, SemanticZone::Prompt);

        // Command Start: OSC 133 ; B
        term.feed(b"\x1b]133;B\x07");
        assert_eq!(term.semantic_zone, SemanticZone::Input);
        assert_eq!(term.prompt_marks.len(), 2);
        assert_eq!(term.prompt_marks[1].zone, SemanticZone::Input);

        // Output Start: OSC 133 ; C
        term.feed(b"\x1b]133;C\x07");
        assert_eq!(term.semantic_zone, SemanticZone::Output);
        assert_eq!(term.prompt_marks.len(), 3);
        assert_eq!(term.prompt_marks[2].zone, SemanticZone::Output);

        // Command Finished with Exit Code 0: OSC 133 ; D ; 0
        term.feed(b"\x1b]133;D;0\x07");
        assert_eq!(term.last_command_exit_code, Some(0));
        assert_eq!(term.prompt_marks.back().unwrap().exit_code, Some(0));
    }

    #[test]
    fn test_osc_color_resets_and_cursor_color() {
        let mut term = Terminal::new(80, 24);

        // 1. OSC 12: Set cursor color to red (#FF0000)
        term.feed(b"\x1b]12;#FF0000\x07");
        assert_eq!(term.theme.cursor_color.unwrap().r, 255);
        assert_eq!(term.theme.cursor_color.unwrap().g, 0);

        // OSC 12: Query cursor color
        term.outgoing.clear();
        term.feed(b"\x1b]12;?\x07");
        assert!(
            String::from_utf8(term.outgoing.clone())
                .unwrap()
                .starts_with("\x1b]12;rgb:ffff/0000/0000")
        );

        // OSC 112: Reset cursor color
        term.feed(b"\x1b]112\x07");
        assert_eq!(term.theme.cursor_color, None);

        // 2. OSC 4: Set palette color 1 (Red) to blue
        term.feed(b"\x1b]4;1;#0000FF\x07");
        assert_eq!(term.theme.ansi_colors[1].b, 255);

        // OSC 104: Reset specific palette color index 1
        term.feed(b"\x1b]104;1\x07");
        assert_eq!(term.theme.ansi_colors[1], term.theme.initial_ansi_colors[1]);

        // 3. OSC 10: Modify FG, OSC 110: Reset FG
        term.feed(b"\x1b]10;#123456\x07");
        assert_eq!(term.theme.default_fg.r, 0x12);
        term.feed(b"\x1b]110\x07");
        assert_eq!(term.theme.default_fg, term.theme.initial_fg);

        // 4. OSC 11: Modify BG, OSC 111: Reset BG
        term.feed(b"\x1b]11;#654321\x07");
        assert_eq!(term.theme.default_bg.r, 0x65);
        term.feed(b"\x1b]111\x07");
        assert_eq!(term.theme.default_bg, term.theme.initial_bg);
    }

    #[test]
    fn test_long_command_line_wrap() {
        let mut term = Terminal::new(10, 5);
        term.feed(b"1234567890abcdefghij12345");
        let grid = term.active_grid();
        assert!(grid.row_wrapped[0]);
        assert!(grid.row_wrapped[1]);
        assert_eq!(grid.cells[0].character, '1');
        assert_eq!(grid.cells[9].character, '0');
        assert_eq!(grid.cells[10].character, 'a');
        assert_eq!(grid.cells[19].character, 'j');
        assert_eq!(grid.cells[20].character, '1');
        assert_eq!(grid.cells[24].character, '5');
        assert_eq!(grid.cursor.x, 5);
        assert_eq!(grid.cursor.y, 2);
    }

    #[test]
    fn test_wrapped_line_backspace_navigation() {
        let mut term = Terminal::new(10, 5);
        term.feed(b"1234567890a");
        assert_eq!(term.active_grid().cursor.x, 1);
        assert_eq!(term.active_grid().cursor.y, 1);

        term.feed(b"\x08\x08");
        assert_eq!(term.active_grid().cursor.x, 9);
        assert_eq!(term.active_grid().cursor.y, 0);
    }

    #[test]
    fn test_scroll_on_output_behavior() {
        let mut term = Terminal::new(80, 5);
        term.scroll_on_output = true;

        // Produce 10 lines of output to populate scrollback
        for i in 0..10 {
            term.feed(format!("Line {}\n", i).as_bytes());
        }

        assert_eq!(term.grid.scrollback.len(), 6);
        assert_eq!(term.grid.scroll_offset, 0);

        // Manually scroll up 3 lines
        term.grid.scroll_offset = 3;

        // When scroll_on_output = true, new feed resets scroll_offset to 0
        term.feed(b"New output\n");
        assert_eq!(term.grid.scroll_offset, 0);

        // Now set scroll_on_output = false
        term.scroll_on_output = false;
        term.grid.scroll_offset = 3;

        // Feed new output without resetting scroll_offset
        term.feed(b"Another line\n");
        // scroll_offset should not be reset to 0; it should increase to track new scrollback lines
        assert!(term.grid.scroll_offset >= 3);
    }

    #[test]
    fn test_alt_screen_damage_tracking() {
        let mut term = Terminal::new(80, 24);

        // Clear initial damage on primary grid
        term.grid.clear_damage();
        assert!(!term.grid.damage.full_redraw);
        assert!(!term.grid.damage.dirty_rows.iter().any(|&d| d));

        // Enter alternate screen (e.g. Neovim startup via CSI ? 1049 h)
        term.feed(b"\x1b[?1049h");
        assert!(term.is_alt_screen);
        assert!(term.alt_grid.damage.full_redraw);
        assert!(term.alt_grid.damage.dirty_rows.iter().all(|&d| d));

        // Clear damage on alt grid while inside app
        term.alt_grid.clear_damage();
        assert!(!term.alt_grid.damage.full_redraw);

        // Exit alternate screen (e.g. Neovim exit via CSI ? 1049 l)
        term.feed(b"\x1b[?1049l");
        assert!(!term.is_alt_screen);
        assert!(term.grid.damage.full_redraw);
        assert!(term.grid.damage.dirty_rows.iter().all(|&d| d));
    }
}
