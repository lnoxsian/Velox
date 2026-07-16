use crate::terminal::terminal::Terminal;

pub fn handle_escape(byte: u8, terminal: &mut Terminal) {
    match byte {
        b'7' => { terminal.save_cursor(); }
        b'8' => { terminal.restore_cursor(); }
        _ => {}
    }
}

