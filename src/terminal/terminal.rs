use crate::screen::grid::Grid;
use crate::screen::cell::{Color, CellFlags};
use crate::theme::theme::Theme;
use crate::ansi::parser::AnsiParser;

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
}

impl Terminal {
    pub fn new(width: usize, height: usize) -> Self {
        let config = crate::config::loader::load().unwrap_or_else(|_| {
            crate::config::defaults::default_config()
        });
        let mut theme = Theme::new();
        if let Some(fg) = &config.default_fg {
            if let Some(c) = crate::config::config::parse_hex_color(fg) {
                theme.default_fg = c;
            }
        }
        if let Some(bg) = &config.default_bg {
            if let Some(c) = crate::config::config::parse_hex_color(bg) {
                theme.default_bg = c;
            }
        }
        if let Some(colors) = &config.colors {
            let fields = [
                (&colors.black, 0),
                (&colors.red, 1),
                (&colors.green, 2),
                (&colors.yellow, 3),
                (&colors.blue, 4),
                (&colors.magenta, 5),
                (&colors.cyan, 6),
                (&colors.white, 7),
                (&colors.bright_black, 8),
                (&colors.bright_red, 9),
                (&colors.bright_green, 10),
                (&colors.bright_yellow, 11),
                (&colors.bright_blue, 12),
                (&colors.bright_magenta, 13),
                (&colors.bright_cyan, 14),
                (&colors.bright_white, 15),
            ];
            for (opt, idx) in &fields {
                if let Some(hex) = opt {
                    if let Some(c) = crate::config::config::parse_hex_color(hex) {
                        theme.ansi_colors[*idx] = c;
                    }
                }
            }
        }

        let default_fg = theme.default_fg;
        let default_bg = theme.default_bg;

        let enable_nerdfont = config.enable_nerdfont.unwrap_or(true);
        let scrollback_limit = config.scrollback_limit.unwrap_or(1000);
        let bold_is_bright = config.bold_is_bright.unwrap_or(true);
        let app_title = config.app_title.clone();

        Self {
            grid: Grid::new(width, height, default_fg, default_bg, enable_nerdfont, scrollback_limit),
            alt_grid: Grid::new(width, height, default_fg, default_bg, enable_nerdfont, 0),
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
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.grid.scroll_offset = 0;
        // Feed bytes one by one. Note: We temporarily take ownership or borrow
        // to avoid duplicate mutable borrows on Self.
        let mut parser = std::mem::replace(&mut self.parser, AnsiParser::new());
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
        let active_grid = if self.is_alt_screen { &mut self.alt_grid } else { &mut self.grid };
        active_grid.saved_cursor = active_grid.cursor;
        active_grid.saved_fg = self.current_fg;
        active_grid.saved_bg = self.current_bg;
        active_grid.saved_flags = self.current_flags;
        active_grid.saved_g0_charset = self.g0_charset;
        active_grid.saved_g1_charset = self.g1_charset;
        active_grid.saved_active_charset = self.active_charset;
    }

    pub fn restore_cursor(&mut self) {
        let active_grid = if self.is_alt_screen { &mut self.alt_grid } else { &mut self.grid };
        active_grid.cursor = active_grid.saved_cursor;
        self.current_fg = active_grid.saved_fg;
        self.current_bg = active_grid.saved_bg;
        self.current_flags = active_grid.saved_flags;
        self.g0_charset = active_grid.saved_g0_charset;
        self.g1_charset = active_grid.saved_g1_charset;
        self.active_charset = active_grid.saved_active_charset;
    }

    pub fn set_alt_screen(&mut self, active: bool) {
        self.is_alt_screen = active;
    }

    pub fn active_grid(&self) -> &Grid {
        if self.is_alt_screen { &self.alt_grid } else { &self.grid }
    }

    pub fn active_grid_mut(&mut self) -> &mut Grid {
        if self.is_alt_screen { &mut self.alt_grid } else { &mut self.grid }
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
        term.feed(b"\x1b[c");
        assert_eq!(term.outgoing, b"\x1b[?6c");
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
        assert!(grid.scrollback.lines.len() > 0);
        
        // The first character of the oldest line in scrollback history should be 'l' from "line ..."
        assert_eq!(grid.scrollback.lines[0][0].character, 'l');
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
}
