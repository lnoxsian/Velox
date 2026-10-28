use crate::terminal::terminal::Terminal;

pub fn handle_escape(byte: u8, terminal: &mut Terminal) {
    match byte {
        b'7' => {
            terminal.save_cursor();
        }
        b'8' => {
            terminal.restore_cursor();
        }
        b'=' => {
            // DECKPAM - Application Keypad Mode
            terminal.keypad_application_mode = true;
        }
        b'>' => {
            // DECKPNM - Normal Keypad Mode
            terminal.keypad_application_mode = false;
        }
        b'H' => {
            // HTS - Horizontal Tab Set
            let col = terminal.active_grid().cursor.x;
            terminal.active_grid_mut().set_tab_stop(col);
        }
        b'c' => {
            // RIS - Full Reset
            terminal.reset_to_defaults(false);
        }
        b'D' => {
            // Index (IND)
            let bg = terminal.current_bg;
            terminal.active_grid_mut().scroll_or_move_down(bg);
        }
        b'M' => {
            // Reverse Index (RI)
            let bg = terminal.current_bg;
            terminal.active_grid_mut().scroll_or_move_up(bg);
        }
        b'E' => {
            // Next Line (NEL)
            let bg = terminal.current_bg;
            let grid = terminal.active_grid_mut();
            grid.cursor.x = 0;
            grid.scroll_or_move_down(bg);
        }
        _ => {}
    }
}
