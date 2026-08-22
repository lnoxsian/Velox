use crate::terminal::terminal::Terminal;

pub fn handle_escape(byte: u8, terminal: &mut Terminal) {
    match byte {
        b'7' => {
            terminal.save_cursor();
        }
        b'8' => {
            terminal.restore_cursor();
        }
        b'c' => {
            // RIS - Full Reset
            terminal.reset_attrs();
            terminal.is_alt_screen = false;
            terminal.bracketed_paste_mode = false;
            terminal.set_synchronized_output(false);
            terminal.focus_tracking = false;
            terminal.semantic_zone = crate::terminal::terminal::SemanticZone::Output;
            terminal.prompt_marks.clear();
            terminal.last_command_exit_code = None;
            terminal.theme.cursor_color = terminal.theme.initial_cursor_color;
            terminal.theme.cursor_text_color = terminal.theme.initial_cursor_text_color;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.grid.erase_display(3, fg, bg);
            terminal.grid.cursor.x = 0;
            terminal.grid.cursor.y = 0;
            terminal.alt_grid.erase_display(3, fg, bg);
            terminal.alt_grid.cursor.x = 0;
            terminal.alt_grid.cursor.y = 0;
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
