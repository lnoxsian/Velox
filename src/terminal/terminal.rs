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
        if let Some(colors) = &config.ansi_colors {
            for (i, hex) in colors.iter().enumerate().take(16) {
                if let Some(c) = crate::config::config::parse_hex_color(hex) {
                    theme.ansi_colors[i] = c;
                }
            }
        }

        let default_fg = theme.default_fg;
        let default_bg = theme.default_bg;

        Self {
            grid: Grid::new(width, height, default_fg, default_bg),
            alt_grid: Grid::new(width, height, default_fg, default_bg),
            is_alt_screen: false,
            parser: AnsiParser::new(),
            theme,
            current_fg: default_fg,
            current_bg: default_bg,
            current_flags: CellFlags::empty(),
            outgoing: Vec::new(),
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        // Feed bytes one by one. Note: We temporarily take ownership or borrow
        // to avoid duplicate mutable borrows on Self.
        let mut parser = std::mem::replace(&mut self.parser, AnsiParser::new());
        for &byte in data {
            parser.feed(byte, self);
        }
        self.parser = parser;
    }

    pub fn execute(&mut self) {
        // stub
    }

    pub fn handle_input(&mut self) {
        // stub
    }

    pub fn handle_mouse(&mut self) {
        // stub
    }

    pub fn send_to_shell(&mut self, data: &[u8]) {
        self.outgoing.extend_from_slice(data);
    }

    pub fn update_cursor(&mut self) {
        // stub
    }

    pub fn paste(&mut self) {
        // stub
    }

    pub fn copy(&mut self) {
        // stub
    }

    pub fn select(&mut self) {
        // stub
    }

    pub fn reset_attrs(&mut self) {
        self.current_fg = self.theme.default_fg;
        self.current_bg = self.theme.default_bg;
        self.current_flags = CellFlags::empty();
    }

    pub fn save_cursor(&mut self) {
        let active_grid = if self.is_alt_screen { &mut self.alt_grid } else { &mut self.grid };
        active_grid.saved_cursor = active_grid.cursor;
    }

    pub fn restore_cursor(&mut self) {
        let active_grid = if self.is_alt_screen { &mut self.alt_grid } else { &mut self.grid };
        active_grid.cursor = active_grid.saved_cursor;
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
}
