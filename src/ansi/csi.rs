use crate::screen::cell::{CellFlags, Color};
use crate::terminal::terminal::Terminal;

#[inline]
pub(crate) fn format_u32_into(mut n: u32, buf: &mut [u8]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut temp = [0u8; 10];
    let mut i = 0;
    while n > 0 {
        temp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    for j in 0..i {
        buf[j] = temp[i - 1 - j];
    }
    i
}

pub fn handle_csi(
    action: u8,
    params: &[u16],
    prefix: Option<u8>,
    intermediate: Option<u8>,
    terminal: &mut Terminal,
) {
    match action {
        b'm' => {
            // Select Graphic Rendition
            if prefix.is_some() {
                return;
            } // Private sequences (e.g. XTMODKEYS) are not SGR
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
                            terminal.current_flags.remove(CellFlags::ALL_UNDERLINES);
                            if i + 1 < params.len() && (params[i + 1] & 0x8000) != 0 {
                                let sub = params[i + 1] & 0x7fff;
                                match sub {
                                    0 => {}
                                    2 => terminal.current_flags.insert(CellFlags::DOUBLE_UNDERLINE),
                                    3..=5 => terminal.current_flags.insert(CellFlags::CURLY_UNDERLINE),
                                    _ => terminal.current_flags.insert(CellFlags::UNDERLINE),
                                }
                                i += 1;
                            } else {
                                terminal.current_flags.insert(CellFlags::UNDERLINE);
                            }
                        }
                        5 => terminal.current_flags.insert(CellFlags::BLINK),
                        7 => terminal.current_flags.insert(CellFlags::REVERSE),
                        8 => terminal.current_flags.insert(CellFlags::HIDDEN),
                        9 => terminal.current_flags.insert(CellFlags::STRIKE),
                        21 => {
                            terminal.current_flags.remove(CellFlags::ALL_UNDERLINES);
                            terminal.current_flags.insert(CellFlags::DOUBLE_UNDERLINE);
                        }
                        22 => {
                            terminal.current_flags.remove(CellFlags::BOLD | CellFlags::DIM);
                        }
                        23 => terminal.current_flags.remove(CellFlags::ITALIC),
                        24 => {
                            terminal.current_flags.remove(CellFlags::ALL_UNDERLINES);
                        }
                        25 => terminal.current_flags.remove(CellFlags::BLINK),
                        27 => terminal.current_flags.remove(CellFlags::REVERSE),
                        28 => terminal.current_flags.remove(CellFlags::HIDDEN),
                        29 => terminal.current_flags.remove(CellFlags::STRIKE),
                        30..=37 => {
                            terminal.current_fg = terminal.theme.get_ansi_color(code - 30, false)
                        }
                        38 => {
                            if let Some(color) = parse_sgr_color(params, &mut i, &terminal.theme) {
                                terminal.current_fg = color;
                            }
                        }
                        39 => terminal.current_fg = terminal.theme.default_fg,
                        40..=47 => {
                            terminal.current_bg = terminal.theme.get_ansi_color(code - 40, true)
                        }
                        48 => {
                            if let Some(color) = parse_sgr_color(params, &mut i, &terminal.theme) {
                                terminal.current_bg = color;
                            }
                        }
                        49 => terminal.current_bg = terminal.theme.default_bg,
                        58 => {
                            if let Some(color) = parse_sgr_color(params, &mut i, &terminal.theme) {
                                terminal.current_underline_color = Some(color);
                            }
                        }
                        59 => {
                            terminal.current_underline_color = None;
                        }
                        90..=97 => {
                            terminal.current_fg =
                                terminal.theme.get_ansi_color(code - 90 + 8, false)
                        }
                        100..=107 => {
                            terminal.current_bg =
                                terminal.theme.get_ansi_color(code - 100 + 8, true)
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
        }
        b'b' => {
            // Repeat Preceding Character (ECMA-48 REP)
            let n = params.first().copied().unwrap_or(1).max(1) as usize;
            if let Some(c) = terminal.last_char {
                let fg = terminal.current_fg;
                let bg = terminal.current_bg;
                let uc = terminal.current_underline_color;
                let flags = terminal.current_flags;
                terminal
                    .active_grid_mut()
                    .repeat_char(n, c, fg, bg, uc, flags);
            }
        }
        b'Z' => {
            // Cursor Backward Tabulation (CBT)
            let n = params.first().copied().unwrap_or(1).max(1) as usize;
            let active = terminal.active_grid_mut();
            for _ in 0..n {
                active.cursor.x = active.prev_tab_stop(active.cursor.x);
            }
        }
        b't' => {
            // Window manipulation & metrics (XTWINOPS)
            let op = params.first().copied().unwrap_or(0);
            match op {
                14 => {
                    // Report text area size in pixels: \x1b[4;<h>;<w>t
                    let active = terminal.active_grid();
                    let w = (active.width as u32) * terminal.cell_width_px;
                    let h = (active.height as u32) * terminal.cell_height_px;
                    let mut buf = [0u8; 32];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(cur, "\x1b[4;{};{}t", h, w);
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                16 => {
                    // Report character cell size in pixels: \x1b[6;<h>;<w>t
                    let mut buf = [0u8; 32];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(
                        cur,
                        "\x1b[6;{};{}t",
                        terminal.cell_height_px, terminal.cell_width_px
                    );
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                18 => {
                    // Report text area size in characters: \x1b[8;<rows>;<cols>t
                    let active = terminal.active_grid();
                    let mut buf = [0u8; 32];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(cur, "\x1b[8;{};{}t", active.height, active.width);
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                19 => {
                    // Report screen size in characters: \x1b[9;<rows>;<cols>t
                    let active = terminal.active_grid();
                    let mut buf = [0u8; 32];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(cur, "\x1b[9;{};{}t", active.height, active.width);
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                22 => {
                    // Push title to stack
                    let title = terminal
                        .osc_title
                        .clone()
                        .unwrap_or_else(|| terminal.app_title.clone().unwrap_or_default());
                    terminal.title_stack.push(title);
                }
                23 => {
                    // Pop title from stack
                    if let Some(title) = terminal.title_stack.pop() {
                        terminal.osc_title = Some(title);
                    }
                }
                _ => {}
            }
        }
        b'A' => {
            // Cursor Up
            let n = params.first().copied().unwrap_or(1) as usize;
            let min_y = if terminal.origin_mode {
                terminal.active_grid().scroll_region_top
            } else {
                0
            };
            let active = terminal.active_grid_mut();
            active.cursor.y = active.cursor.y.saturating_sub(n).max(min_y);
        }
        b'B' => {
            // Cursor Down
            let n = params.first().copied().unwrap_or(1) as usize;
            let max_y = if terminal.origin_mode {
                terminal.active_grid().scroll_region_bottom
            } else {
                terminal.active_grid().height.saturating_sub(1)
            };
            let active = terminal.active_grid_mut();
            active.cursor.y = (active.cursor.y + n).min(max_y);
        }
        b'C' => {
            // Cursor Forward
            let mut n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            while n > 0 {
                if active.cursor.x + 1 < active.width {
                    let available = active.width.saturating_sub(1).saturating_sub(active.cursor.x);
                    let step = n.min(available);
                    active.cursor.x += step;
                    n -= step;
                } else if active.cursor.y + 1 < active.height {
                    let phys_y = active.physical_row(active.cursor.y);
                    if active.row_wrapped.get(phys_y).copied().unwrap_or(false) {
                        active.cursor.y += 1;
                        active.cursor.x = 0;
                        n -= 1;
                    } else {
                        active.cursor.x = (active.cursor.x + n).min(active.width.saturating_sub(1));
                        break;
                    }
                } else {
                    active.cursor.x = (active.cursor.x + n).min(active.width.saturating_sub(1));
                    break;
                }
            }
        }
        b'D' => {
            // Cursor Backward
            let mut n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            while n > 0 {
                if active.cursor.x > 0 {
                    let step = n.min(active.cursor.x);
                    active.cursor.x -= step;
                    n -= step;
                } else if active.cursor.y > 0 {
                    let prev_phys_y = active.physical_row(active.cursor.y - 1);
                    if active.row_wrapped.get(prev_phys_y).copied().unwrap_or(false) {
                        active.cursor.y -= 1;
                        active.cursor.x = active.width.saturating_sub(1);
                        n -= 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        b'H' | b'f' => {
            // Cursor Position
            let r = params.first().copied().unwrap_or(1).saturating_sub(1) as usize;
            let c = params.get(1).copied().unwrap_or(1).saturating_sub(1) as usize;
            let origin_mode = terminal.origin_mode;
            let active = terminal.active_grid_mut();
            if origin_mode {
                active.cursor.y = (active.scroll_region_top + r).min(active.scroll_region_bottom);
            } else {
                active.cursor.y = r.min(active.height.saturating_sub(1));
            }
            active.cursor.x = c.min(active.width.saturating_sub(1));
        }
        b'J' => {
            // Erase Display
            let mode = params.first().copied().unwrap_or(0) as u8;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().erase_display(mode, fg, bg);
        }
        b'K' => {
            // Erase Line
            let mode = params.first().copied().unwrap_or(0) as u8;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().erase_line(mode, fg, bg);
        }
        b'g' => {
            // Tab Clear (TBC)
            let op = params.first().copied().unwrap_or(0);
            if op == 0 {
                let col = terminal.active_grid().cursor.x;
                terminal.active_grid_mut().clear_tab_stop(col);
            } else if op == 3 {
                terminal.active_grid_mut().clear_all_tab_stops();
            }
        }
        b'h' | b'l' => {
            // Modes
            let active = action == b'h';
            if prefix == Some(b'?') {
                for &mode in params {
                    match mode {
                        1 => {
                            terminal.cursor_keys_mode = active;
                        }
                        5 => {
                            terminal.reverse_video = active;
                            terminal.active_grid_mut().mark_all_dirty();
                        }
                        6 => {
                            terminal.set_origin_mode(active);
                        }
                        7 => {
                            terminal.set_auto_wrap(active);
                        }
                        25 => {
                            terminal.active_grid_mut().cursor.visible = active;
                        }
                        66 => {
                            terminal.keypad_application_mode = active;
                        }
                        1000 => {
                            terminal.mouse_mode = if active { 1000 } else { 0 };
                        }
                        1002 => {
                            terminal.mouse_mode = if active { 1002 } else { 0 };
                        }
                        1003 => {
                            terminal.mouse_mode = if active { 1003 } else { 0 };
                        }
                        1004 => {
                            terminal.focus_tracking = active;
                        }
                        1005 => {
                            terminal.mouse_utf8 = active;
                        }
                        1006 => {
                            terminal.mouse_sgr = active;
                        }
                        1016 => {
                            terminal.mouse_pixel = active;
                        }
                        2004 => {
                            terminal.bracketed_paste_mode = active;
                        }
                        2026 => {
                            terminal.set_synchronized_output(active);
                        }
                        47 | 1047 => {
                            terminal.set_alt_screen(active);
                        }
                        1048 => {
                            if active {
                                terminal.save_cursor();
                            } else {
                                terminal.restore_cursor();
                            }
                        }
                        1049 => {
                            if active {
                                terminal.save_cursor();
                                terminal.set_alt_screen(true);
                                let fg = terminal.current_fg;
                                let bg = terminal.current_bg;
                                let active_grid = terminal.active_grid_mut();
                                active_grid.erase_display(2, fg, bg);
                                active_grid.cursor.x = 0;
                                active_grid.cursor.y = 0;
                            } else {
                                terminal.set_alt_screen(false);
                                terminal.restore_cursor();
                            }
                        }
                        _ => {}
                    }
                }
            } else if prefix.is_none() {
                for &mode in params {
                    match mode {
                        4 => {
                            terminal.set_insert_mode(active);
                        }
                        _ => {}
                    }
                }
            }
        }
        b'n' => {
            // Device Status Report (DSR)
            if params.first() == Some(&6) {
                let active = terminal.active_grid();
                let (r, c) = if terminal.origin_mode {
                    (
                        active.cursor.y.saturating_sub(active.scroll_region_top) + 1,
                        active.cursor.x + 1,
                    )
                } else {
                    (active.cursor.y + 1, active.cursor.x + 1)
                };
                let mut buf = [0u8; 32];
                buf[0] = 0x1b;
                buf[1] = b'[';
                let mut len = 2;
                len += format_u32_into(r as u32, &mut buf[len..]);
                buf[len] = b';';
                len += 1;
                len += format_u32_into(c as u32, &mut buf[len..]);
                buf[len] = b'R';
                len += 1;
                terminal.send_to_shell(&buf[..len]);
            } else if params.first() == Some(&5) {
                terminal.send_to_shell(b"\x1b[0n");
            }
        }
        b'r' => {
            if prefix == Some(b'?') {
                // Restore DEC Private Mode (CSI ? <mode> r)
                for &mode in params {
                    if let Some(val) = terminal.saved_dec_modes.get(&mode) {
                        match mode {
                            1 => terminal.cursor_keys_mode = val,
                            5 => {
                                terminal.reverse_video = val;
                                terminal.active_grid_mut().mark_all_dirty();
                            }
                            6 => terminal.set_origin_mode(val),
                            7 => terminal.set_auto_wrap(val),
                            25 => terminal.active_grid_mut().cursor.visible = val,
                            66 => terminal.keypad_application_mode = val,
                            1000 => terminal.mouse_mode = if val { 1000 } else { 0 },
                            1002 => terminal.mouse_mode = if val { 1002 } else { 0 },
                            1003 => terminal.mouse_mode = if val { 1003 } else { 0 },
                            1004 => terminal.focus_tracking = val,
                            1005 => terminal.mouse_utf8 = val,
                            1006 => terminal.mouse_sgr = val,
                            1016 => terminal.mouse_pixel = val,
                            2004 => terminal.bracketed_paste_mode = val,
                            2026 => terminal.set_synchronized_output(val),
                            _ => {}
                        }
                    }
                }
            } else {
                // Set Scroll Margins (DECSTBM)
                let top = params.first().copied().unwrap_or(0) as usize;
                let bottom = params.get(1).copied().unwrap_or(0) as usize;
                terminal.active_grid_mut().set_scroll_region(top, bottom);
                let origin_mode = terminal.origin_mode;
                let active = terminal.active_grid_mut();
                if origin_mode {
                    active.cursor.y = active.scroll_region_top;
                    active.cursor.x = 0;
                } else {
                    active.cursor.y = 0;
                    active.cursor.x = 0;
                }
            }
        }
        b'S' => {
            // Scroll Up (Pan Up)
            let delta = params.first().copied().unwrap_or(1).max(1);
            let bg = terminal.current_bg;
            terminal.active_grid_mut().scroll(delta as i32, bg);
        }
        b'T' => {
            // Scroll Down (Pan Down)
            let delta = params.first().copied().unwrap_or(1).max(1);
            let bg = terminal.current_bg;
            terminal.active_grid_mut().scroll_down(delta as usize, bg);
        }
        b'c' => {
            // Device Attributes (DA)
            match prefix {
                Some(b'>') => {
                    // Secondary Device Attributes (DA2): `CSI > c` or `CSI > 0 c`
                    // Tmux and other tools probe DA2 on session attach/connect.
                    terminal.send_to_shell(b"\x1b[>0;10;0c");
                }
                Some(b'=') => {
                    // Tertiary Device Attributes (DA3): `CSI = c`
                    terminal.send_to_shell(b"\x1bP!|00000000\x1b\\");
                }
                _ => {
                    // Primary Device Attributes (DA1): `CSI c` or `CSI 0 c` (VT220 conformance)
                    terminal.send_to_shell(b"\x1b[?62;c");
                }
            }
        }
        b's' => {
            if prefix == Some(b'?') {
                // Save DEC Private Mode (CSI ? <mode> s)
                for &mode in params {
                    let val = match mode {
                        1 => terminal.cursor_keys_mode,
                        5 => terminal.reverse_video,
                        6 => terminal.origin_mode,
                        7 => terminal.auto_wrap,
                        25 => terminal.active_grid().cursor.visible,
                        66 => terminal.keypad_application_mode,
                        1000 => terminal.mouse_mode == 1000,
                        1002 => terminal.mouse_mode == 1002,
                        1003 => terminal.mouse_mode == 1003,
                        1004 => terminal.focus_tracking,
                        1005 => terminal.mouse_utf8,
                        1006 => terminal.mouse_sgr,
                        1016 => terminal.mouse_pixel,
                        2004 => terminal.bracketed_paste_mode,
                        2026 => terminal.synchronized_output,
                        _ => false,
                    };
                    terminal.saved_dec_modes.insert(mode, val);
                }
            } else {
                // Save Cursor
                terminal.save_cursor();
            }
        }
        b'u' => {
            match prefix {
                Some(b'?') => {
                    // Kitty Keyboard Protocol: Query flags (`CSI ? u`)
                    let mut buf = [0u8; 32];
                    use std::io::Write;
                    let mut cur = std::io::Cursor::new(&mut buf[..]);
                    let _ = write!(cur, "\x1b[?{}u", terminal.kitty_keyboard_flags);
                    let written = cur.position() as usize;
                    terminal.send_to_shell(&buf[..written]);
                }
                Some(b'>') => {
                    // Kitty Keyboard Protocol: Push flags (`CSI > <flags> u`)
                    let flags = params.first().copied().unwrap_or(0);
                    terminal.push_kitty_keyboard_flags(flags);
                }
                Some(b'<') => {
                    // Kitty Keyboard Protocol: Pop flags (`CSI < <count> u`)
                    let count = params.first().copied().unwrap_or(1) as usize;
                    terminal.pop_kitty_keyboard_flags(count);
                }
                Some(b'=') => {
                    // Kitty Keyboard Protocol: Set / modify flags (`CSI = <flags> [; <mode>] u`)
                    let flags = params.first().copied().unwrap_or(0);
                    let mode = params.get(1).copied().unwrap_or(1);
                    terminal.set_kitty_keyboard_flags(flags, mode);
                }
                _ => {
                    // Restore Cursor (ANSI / VT)
                    terminal.restore_cursor();
                }
            }
        }
        b'G' => {
            // Cursor Horizontal Absolute (CHA)
            let col = params.first().copied().unwrap_or(1).saturating_sub(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.x = col.min(active.width.saturating_sub(1));
        }
        b'd' => {
            // Vertical Line Position Absolute (VPA)
            let row = params.first().copied().unwrap_or(1).saturating_sub(1) as usize;
            let origin_mode = terminal.origin_mode;
            let active = terminal.active_grid_mut();
            if origin_mode {
                active.cursor.y = (active.scroll_region_top + row).min(active.scroll_region_bottom);
            } else {
                active.cursor.y = row.min(active.height.saturating_sub(1));
            }
        }
        b'E' => {
            // Cursor Next Line (CNL)
            let n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.y = (active.cursor.y + n).min(active.height.saturating_sub(1));
            active.cursor.x = 0;
        }
        b'F' => {
            // Cursor Previous Line (CPL)
            let n = params.first().copied().unwrap_or(1) as usize;
            let active = terminal.active_grid_mut();
            active.cursor.y = active.cursor.y.saturating_sub(n);
            active.cursor.x = 0;
        }
        b'X' => {
            // Erase Character (ECH)
            let n = params.first().copied().unwrap_or(1) as usize;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().erase_characters(n, fg, bg);
        }
        b'P' => {
            // Delete Character (DCH)
            let n = params.first().copied().unwrap_or(1) as usize;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().delete_characters(n, fg, bg);
        }
        b'@' => {
            // Insert Character (ICH)
            let n = params.first().copied().unwrap_or(1) as usize;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().insert_characters(n, fg, bg);
        }
        b'L' => {
            // Insert Line (IL)
            let n = params.first().copied().unwrap_or(1) as usize;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().insert_lines(n, fg, bg);
        }
        b'M' => {
            // Delete Line (DL)
            let n = params.first().copied().unwrap_or(1) as usize;
            let fg = terminal.current_fg;
            let bg = terminal.current_bg;
            terminal.active_grid_mut().delete_lines(n, fg, bg);
        }
        b'q' => {
            if prefix == Some(b'>') {
                // XTVERSION - Query Terminal Name and Version (`\x1b[>q` or `\x1b[>0q`)
                terminal.send_to_shell(b"\x1bP>|Velox(0.1.9)\x1b\\");
            } else if prefix.is_none() || prefix == Some(b' ') {
                // Set Cursor Style (DECSCUSR)
                let param = params.first().copied().unwrap_or(0);
                let default_shape = terminal.configured_cursor_shape;
                let active = terminal.active_grid_mut();
                match param {
                    0 => {
                        active.cursor.shape = default_shape;
                        active.cursor.visible = true;
                    }
                    1 | 2 => {
                        active.cursor.shape = crate::screen::cursor::CursorShape::Block;
                        active.cursor.visible = true;
                    }
                    3 | 4 => {
                        active.cursor.shape = crate::screen::cursor::CursorShape::Underline;
                        active.cursor.visible = true;
                    }
                    5 | 6 => {
                        active.cursor.shape = crate::screen::cursor::CursorShape::Beam;
                        active.cursor.visible = true;
                    }
                    _ => {}
                }
            }
        }
        b'p' => {
            if prefix == Some(b'!') || intermediate == Some(b'!') {
                // DECSTR - Soft Terminal Reset
                terminal.reset_to_defaults(true);
            } else if intermediate == Some(b'$') {
                // DECRQM / DECRPM - Mode Query Report
                let mode = params.first().copied().unwrap_or(0);
                if prefix == Some(b'?') {
                    let status = match mode {
                        1 => {
                            if terminal.cursor_keys_mode {
                                1
                            } else {
                                2
                            }
                        }
                        5 => {
                            if terminal.reverse_video {
                                1
                            } else {
                                2
                            }
                        }
                        6 => {
                            if terminal.origin_mode {
                                1
                            } else {
                                2
                            }
                        }
                        7 => {
                            if terminal.auto_wrap {
                                1
                            } else {
                                2
                            }
                        }
                        25 => {
                            if terminal.active_grid().cursor.visible {
                                1
                            } else {
                                2
                            }
                        }
                        66 => {
                            if terminal.keypad_application_mode {
                                1
                            } else {
                                2
                            }
                        }
                        1000 => {
                            if terminal.mouse_mode == 1000 {
                                1
                            } else {
                                2
                            }
                        }
                        1002 => {
                            if terminal.mouse_mode == 1002 {
                                1
                            } else {
                                2
                            }
                        }
                        1003 => {
                            if terminal.mouse_mode == 1003 {
                                1
                            } else {
                                2
                            }
                        }
                        1004 => {
                            if terminal.focus_tracking {
                                1
                            } else {
                                2
                            }
                        }
                        1005 => {
                            if terminal.mouse_utf8 {
                                1
                            } else {
                                2
                            }
                        }
                        1006 => {
                            if terminal.mouse_sgr {
                                1
                            } else {
                                2
                            }
                        }
                        1016 => {
                            if terminal.mouse_pixel {
                                1
                            } else {
                                2
                            }
                        }
                        2004 => {
                            if terminal.bracketed_paste_mode {
                                1
                            } else {
                                2
                            }
                        }
                        2026 => {
                            if terminal.synchronized_output {
                                1
                            } else {
                                2
                            }
                        }
                        _ => 0,
                    };
                    let mut buf = [0u8; 32];
                    buf[0] = 0x1b;
                    buf[1] = b'[';
                    buf[2] = b'?';
                    let mut len = 3;
                    len += format_u32_into(mode as u32, &mut buf[len..]);
                    buf[len] = b';';
                    len += 1;
                    len += format_u32_into(status as u32, &mut buf[len..]);
                    buf[len] = b'$';
                    buf[len + 1] = b'y';
                    len += 2;
                    terminal.send_to_shell(&buf[..len]);
                } else if prefix.is_none() {
                    let status = match mode {
                        4 => {
                            if terminal.insert_mode {
                                1
                            } else {
                                2
                            }
                        }
                        _ => 0,
                    };
                    let mut buf = [0u8; 32];
                    buf[0] = 0x1b;
                    buf[1] = b'[';
                    let mut len = 2;
                    len += format_u32_into(mode as u32, &mut buf[len..]);
                    buf[len] = b';';
                    len += 1;
                    len += format_u32_into(status as u32, &mut buf[len..]);
                    buf[len] = b'$';
                    buf[len + 1] = b'y';
                    len += 2;
                    terminal.send_to_shell(&buf[..len]);
                }
            }
        }
        _ => {}
    }
}

fn parse_sgr_color(
    params: &[u16],
    i: &mut usize,
    theme: &crate::theme::theme::Theme,
) -> Option<Color> {
    let mut j = *i + 1;
    while j < params.len() && (params[j] & 0x8000) != 0 {
        j += 1;
    }
    let num_subs = j - (*i + 1);

    if num_subs > 0 {
        let type_param = params[*i + 1] & 0x7fff;
        let color = if type_param == 5 && num_subs >= 2 {
            Some(theme.get_256_color((params[*i + 2] & 0x7fff) as u8))
        } else if type_param == 2 {
            if num_subs == 4 {
                Some(Color {
                    r: (params[*i + 2] & 0x7fff) as u8,
                    g: (params[*i + 3] & 0x7fff) as u8,
                    b: (params[*i + 4] & 0x7fff) as u8,
                })
            } else if num_subs >= 5 {
                Some(Color {
                    r: (params[*i + 3] & 0x7fff) as u8,
                    g: (params[*i + 4] & 0x7fff) as u8,
                    b: (params[*i + 5] & 0x7fff) as u8,
                })
            } else {
                None
            }
        } else {
            None
        };
        *i += num_subs;
        color
    } else if *i + 1 < params.len() {
        let mode = params[*i + 1];
        if mode == 5 {
            if *i + 2 < params.len() {
                let color = theme.get_256_color(params[*i + 2] as u8);
                *i += 2;
                Some(color)
            } else {
                *i += 1;
                None
            }
        } else if mode == 2 {
            if *i + 4 < params.len() {
                let color = Color {
                    r: params[*i + 2] as u8,
                    g: params[*i + 3] as u8,
                    b: params[*i + 4] as u8,
                };
                *i += 4;
                Some(color)
            } else {
                let rem = params.len() - 1 - *i;
                *i += rem;
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csi_zero_dimensions_safe() {
        let mut term = Terminal::new(80, 24);
        term.grid.width = 0;
        term.grid.height = 0;

        // Cursor forward: \x1b[5C
        handle_csi(b'C', &[5], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 0);

        // Cursor position: \x1b[10;10H
        handle_csi(b'H', &[10, 10], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 0);
        assert_eq!(term.grid.cursor.y, 0);

        // CHA: \x1b[5G
        handle_csi(b'G', &[5], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 0);

        // VPA: \x1b[5d
        handle_csi(b'd', &[5], None, None, &mut term);
        assert_eq!(term.grid.cursor.y, 0);

        // CNL: \x1b[5E
        handle_csi(b'E', &[5], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 0);
        assert_eq!(term.grid.cursor.y, 0);
    }

    #[test]
    fn test_csi_cursor_forward_backward_with_wrapped_rows_and_offset() {
        let mut term = Terminal::new(10, 4);
        term.grid.cursor.x = 9;
        term.grid.cursor.y = 0;
        let phys_0 = term.grid.physical_row(0);
        term.grid.row_wrapped[phys_0] = true;

        // Forward 1 should wrap to row 1, col 0
        handle_csi(b'C', &[1], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 0);
        assert_eq!(term.grid.cursor.y, 1);

        // Backward 1 should wrap back to row 0, col 9
        handle_csi(b'D', &[1], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 9);
        assert_eq!(term.grid.cursor.y, 0);

        // Now test with non-zero row_offset (circular buffer)
        term.grid.row_offset = 2;
        let phys_0 = term.grid.physical_row(0);
        term.grid.row_wrapped.fill(false);
        term.grid.row_wrapped[phys_0] = true;
        term.grid.cursor.x = 9;
        term.grid.cursor.y = 0;

        handle_csi(b'C', &[1], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 0);
        assert_eq!(term.grid.cursor.y, 1);

        handle_csi(b'D', &[1], None, None, &mut term);
        assert_eq!(term.grid.cursor.x, 9);
        assert_eq!(term.grid.cursor.y, 0);
    }
}
