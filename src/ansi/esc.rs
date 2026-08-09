use crate::terminal::terminal::Terminal;

pub fn handle_escape(byte: u8, terminal: &mut Terminal) {
    match byte {
        b'7' => { terminal.save_cursor(); }
        b'8' => { terminal.restore_cursor(); }
        b'c' => { // RIS - Full Reset
            terminal.reset_attrs();
            terminal.is_alt_screen = false;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.grid.erase_display(3, fg, bg);
            terminal.grid.cursor.x = 0;
            terminal.grid.cursor.y = 0;
            terminal.alt_grid.erase_display(3, fg, bg);
            terminal.alt_grid.cursor.x = 0;
            terminal.alt_grid.cursor.y = 0;
        }
        _ => {}
    }
}

