use crate::terminal::terminal::Terminal;
use crate::screen::cell::{Color, CellFlags};

pub fn handle_csi(action: u8, params: &[u16], is_private: bool, terminal: &mut Terminal) {
    match action {
        b'm' => { // Select Graphic Rendition
            if params.is_empty() {
                terminal.reset_attrs();
            } else {
                let mut i = 0;
                while i < params.len() {
                    let param = params[i];
                    if (param & 0x8000) != 0 {
                        i += 1;
                        continue;
                    }
                    let code = param;
                    match code {
                        0 => terminal.reset_attrs(),
                        1 => terminal.current_flags.insert(CellFlags::BOLD),
                        2 => terminal.current_flags.insert(CellFlags::DIM),
                        3 => terminal.current_flags.insert(CellFlags::ITALIC),
                        4 => {
                            if i + 1 < params.len() && (params[i+1] & 0x8000) != 0 {
                                let sub = params[i+1] & 0x7fff;
                                match sub {
                                    0 => {
                                        terminal.current_flags.remove(CellFlags::UNDERLINE);
                                        terminal.current_flags.remove(CellFlags::DOUBLE_UNDERLINE);
                                        terminal.current_flags.remove(CellFlags::CURLY_UNDERLINE);
                                    }
                                    1 => {
                                        terminal.current_flags.insert(CellFlags::UNDERLINE);
                                        terminal.current_flags.remove(CellFlags::DOUBLE_UNDERLINE);
                                        terminal.current_flags.remove(CellFlags::CURLY_UNDERLINE);
                                    }
                                    2 => {
                                        terminal.current_flags.remove(CellFlags::UNDERLINE);
                                        terminal.current_flags.insert(CellFlags::DOUBLE_UNDERLINE);
                                        terminal.current_flags.remove(CellFlags::CURLY_UNDERLINE);
                                    }
                                    3..=5 => {
                                        terminal.current_flags.remove(CellFlags::UNDERLINE);
                                        terminal.current_flags.remove(CellFlags::DOUBLE_UNDERLINE);
                                        terminal.current_flags.insert(CellFlags::CURLY_UNDERLINE);
                                    }
                                    _ => {
                                        terminal.current_flags.insert(CellFlags::UNDERLINE);
                                    }
                                }
                                i += 1;
                            } else {
                                terminal.current_flags.insert(CellFlags::UNDERLINE);
                                terminal.current_flags.remove(CellFlags::DOUBLE_UNDERLINE);
                                terminal.current_flags.remove(CellFlags::CURLY_UNDERLINE);
                            }
                        }
                        5 => terminal.current_flags.insert(CellFlags::BLINK),
                        7 => terminal.current_flags.insert(CellFlags::REVERSE),
                        8 => terminal.current_flags.insert(CellFlags::HIDDEN),
                        9 => terminal.current_flags.insert(CellFlags::STRIKE),
                        22 => {
                            terminal.current_flags.remove(CellFlags::BOLD);
                            terminal.current_flags.remove(CellFlags::DIM);
                        }
                        23 => terminal.current_flags.remove(CellFlags::ITALIC),
                        24 => {
                            terminal.current_flags.remove(CellFlags::UNDERLINE);
                            terminal.current_flags.remove(CellFlags::DOUBLE_UNDERLINE);
                            terminal.current_flags.remove(CellFlags::CURLY_UNDERLINE);
                        }
                        25 => terminal.current_flags.remove(CellFlags::BLINK),
                        27 => terminal.current_flags.remove(CellFlags::REVERSE),
                        28 => terminal.current_flags.remove(CellFlags::HIDDEN),
                        29 => terminal.current_flags.remove(CellFlags::STRIKE),
                        30..=37 => terminal.current_fg = terminal.theme.get_ansi_color(code - 30, false),
                        38 => {
                            let mut j = i + 1;
                            while j < params.len() && (params[j] & 0x8000) != 0 {
                                j += 1;
                            }
                            let num_subs = j - (i + 1);

                            if num_subs > 0 {
                                let type_param = params[i+1] & 0x7fff;
                                if type_param == 5 && num_subs >= 2 {
                                    terminal.current_fg = terminal.theme.get_256_color((params[i+2] & 0x7fff) as u8);
                                } else if type_param == 2 {
                                    if num_subs == 4 {
                                        terminal.current_fg = Color {
                                            r: (params[i+2] & 0x7fff) as u8,
                                            g: (params[i+3] & 0x7fff) as u8,
                                            b: (params[i+4] & 0x7fff) as u8,
                                            a: 255,
                                        };
                                    } else if num_subs >= 5 {
                                        terminal.current_fg = Color {
                                            r: (params[i+3] & 0x7fff) as u8,
                                            g: (params[i+4] & 0x7fff) as u8,
                                            b: (params[i+5] & 0x7fff) as u8,
                                            a: 255,
                                        };
                                    }
                                }
                                i += num_subs;
                            } else {
                                if i + 2 < params.len() && params[i+1] == 5 {
                                    terminal.current_fg = terminal.theme.get_256_color(params[i+2] as u8);
                                    i += 2;
                                } else if i + 4 < params.len() && params[i+1] == 2 {
                                    terminal.current_fg = Color {
                                        r: params[i+2] as u8,
                                        g: params[i+3] as u8,
                                        b: params[i+4] as u8,
                                        a: 255,
                                    };
                                    i += 4;
                                }
                            }
                        }
                        39 => terminal.current_fg = terminal.theme.default_fg,
                        40..=47 => terminal.current_bg = terminal.theme.get_ansi_color(code - 40, true),
                        48 => {
                            let mut j = i + 1;
                            while j < params.len() && (params[j] & 0x8000) != 0 {
                                j += 1;
                            }
                            let num_subs = j - (i + 1);

                            if num_subs > 0 {
                                let type_param = params[i+1] & 0x7fff;
                                if type_param == 5 && num_subs >= 2 {
                                    terminal.current_bg = terminal.theme.get_256_color((params[i+2] & 0x7fff) as u8);
                                } else if type_param == 2 {
                                    if num_subs == 4 {
                                        terminal.current_bg = Color {
                                            r: (params[i+2] & 0x7fff) as u8,
                                            g: (params[i+3] & 0x7fff) as u8,
                                            b: (params[i+4] & 0x7fff) as u8,
                                            a: 255,
                                        };
                                    } else if num_subs >= 5 {
                                        terminal.current_bg = Color {
                                            r: (params[i+3] & 0x7fff) as u8,
                                            g: (params[i+4] & 0x7fff) as u8,
                                            b: (params[i+5] & 0x7fff) as u8,
                                            a: 255,
                                        };
                                    }
                                }
                                i += num_subs;
                            } else {
                                if i + 2 < params.len() && params[i+1] == 5 {
                                    terminal.current_bg = terminal.theme.get_256_color(params[i+2] as u8);
                                    i += 2;
                                } else if i + 4 < params.len() && params[i+1] == 2 {
                                    terminal.current_bg = Color {
                                        r: params[i+2] as u8,
                                        g: params[i+3] as u8,
                                        b: params[i+4] as u8,
                                        a: 255,
                                    };
                                    i += 4;
                                }
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
            let n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.y = active.cursor.y.saturating_sub(n);
        }
        b'B' => { // Cursor Down
            let n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.y = (active.cursor.y + n).min(active.height - 1);
        }
        b'C' => { // Cursor Forward
            let n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.x = (active.cursor.x + n).min(active.width - 1);
        }
        b'D' => { // Cursor Backward
            let n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.x = active.cursor.x.saturating_sub(n);
        }
        b'H' | b'f' => { // Cursor Position
            let r = params.first().copied().unwrap_or(1).saturating_sub(1) as usize;
            let c = params.get(1).copied().unwrap_or(1).saturating_sub(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.y = r.min(active.height - 1);
            active.cursor.x = c.min(active.width - 1);
        }
        b'J' => { // Erase Display
            let mode = params.first().copied().unwrap_or(0) as u8;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().erase_display(mode, fg, bg);
        }
        b'K' => { // Erase Line
            let mode = params.first().copied().unwrap_or(0) as u8;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().erase_line(mode, fg, bg);
        }
        b'h' | b'l' => { // Modes
            let active = action == b'h';
            if is_private {
                for &mode in params {
                    match mode {
                        1 => { terminal.cursor_keys_mode = active; }
                        25 => { terminal.active_grid_mut().cursor.visible = active; }
                        1000 => { terminal.mouse_mode = if active { 1000 } else { 0 }; }
                        1002 => { terminal.mouse_mode = if active { 1002 } else { 0 }; }
                        1006 => { terminal.mouse_sgr = active; }
                        1049 => { terminal.set_alt_screen(active); }
                        _ => {}
                    }
                }
            }
        }
        b'n' => { // Device Status Report (DSR)
            if params.first() == Some(&6) {
                let active = terminal.active_grid();
                let response = format!("\x1b[{};{}R", active.cursor.y + 1, active.cursor.x + 1);
                terminal.send_to_shell(response.as_bytes());
            } else if params.first() == Some(&5) {
                terminal.send_to_shell(b"\x1b[0n");
            }
        }
        b'r' => { // Set Scroll Margins (DECSTBM)
            let top = params.get(0).copied().unwrap_or(0) as usize;
            let bottom = params.get(1).copied().unwrap_or(0) as usize;
            terminal.active_grid_mut().set_scroll_region(top, bottom);
        }
        b'S' => { // Scroll Up (Pan Up)
            let delta = params.get(0).copied().unwrap_or(1).max(1);
            let bg = terminal.current_bg;
            terminal.active_grid_mut().scroll(delta as i32, bg);
        }
        b'T' => { // Scroll Down (Pan Down)
            let delta = params.get(0).copied().unwrap_or(1).max(1);
            let bg = terminal.current_bg;
            terminal.active_grid_mut().scroll_down(delta as usize, bg);
        }
        b'c' => { // Device Attributes (DA)
            terminal.send_to_shell(b"\x1b[?6c");
        }
        _ => {}
    }
}

