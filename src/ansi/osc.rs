use crate::screen::cell::Color;
use crate::terminal::terminal::Terminal;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static NOTIFY_RATE_LIMIT: Mutex<Option<(Instant, u32)>> = Mutex::new(None);

pub fn handle_osc(params: &[&[u8]], terminal: &mut Terminal) {
    if params.is_empty() {
        return;
    }

    let cmd = match std::str::from_utf8(params[0]) {
        Ok(c) => c.trim(),
        Err(_) => return,
    };

    match cmd {
        // OSC 0 / OSC 2: Set window title
        "0" | "2" => {
            if params.len() >= 2
                && let Ok(title) = std::str::from_utf8(params[1])
            {
                terminal.osc_title = Some(title.to_string());
            }
        }

        // OSC 4: Query or set ANSI/256 palette colors
        // Format: OSC 4 ; index ; color [; index ; color ...]
        "4" => {
            let mut i = 1;
            let mut modified = false;
            while i + 1 < params.len() {
                let idx_str = std::str::from_utf8(params[i]).unwrap_or("").trim();
                let color_str = std::str::from_utf8(params[i + 1]).unwrap_or("").trim();

                if let Ok(idx) = idx_str.parse::<usize>() {
                    if idx < 256 {
                        if color_str == "?" {
                            let c = terminal.theme.get_256_color(idx as u8);
                            let mut buf = [0u8; 64];
                            let len = format_color_query_response(b"4", Some(idx), c, &mut buf);
                            terminal.send_to_shell(&buf[..len]);
                        } else if let Some(c) = parse_color_spec(color_str) {
                            std::sync::Arc::make_mut(&mut terminal.theme.palette_256)[idx] = c;
                            if idx < 16 {
                                terminal.theme.ansi_colors[idx] = c;
                            }
                            modified = true;
                        }
                    }
                }
                i += 2;
            }
            if modified {
                terminal.grid.mark_all_dirty();
                terminal.alt_grid.mark_all_dirty();
            }
        }

        // OSC 10: Query or set default foreground color
        "10" => {
            if params.len() >= 2 {
                let color_str = std::str::from_utf8(params[1]).unwrap_or("").trim();
                if color_str == "?" {
                    let c = terminal.theme.default_fg;
                    let mut buf = [0u8; 64];
                    let len = format_color_query_response(b"10", None, c, &mut buf);
                    terminal.send_to_shell(&buf[..len]);
                } else if let Some(c) = parse_color_spec(color_str) {
                    terminal.theme.default_fg = c;
                    terminal.current_fg = c;
                    terminal.grid.mark_all_dirty();
                    terminal.alt_grid.mark_all_dirty();
                }
            }
        }

        // OSC 11: Query or set default background color
        "11" => {
            if params.len() >= 2 {
                let color_str = std::str::from_utf8(params[1]).unwrap_or("").trim();
                if color_str == "?" {
                    let c = terminal.theme.default_bg;
                    let mut buf = [0u8; 64];
                    let len = format_color_query_response(b"11", None, c, &mut buf);
                    terminal.send_to_shell(&buf[..len]);
                } else if let Some(c) = parse_color_spec(color_str) {
                    terminal.theme.default_bg = c;
                    terminal.grid.default_bg = c;
                    terminal.alt_grid.default_bg = c;
                    terminal.current_bg = c;
                    terminal.grid.mark_all_dirty();
                    terminal.alt_grid.mark_all_dirty();
                }
            }
        }

        // OSC 12: Query or set dynamic cursor color
        "12" => {
            if params.len() >= 2 {
                let color_str = std::str::from_utf8(params[1]).unwrap_or("").trim();
                if color_str == "?" {
                    let c = terminal
                        .theme
                        .cursor_color
                        .unwrap_or(terminal.theme.default_fg);
                    let mut buf = [0u8; 64];
                    let len = format_color_query_response(b"12", None, c, &mut buf);
                    terminal.send_to_shell(&buf[..len]);
                } else if let Some(c) = parse_color_spec(color_str) {
                    terminal.theme.cursor_color = Some(c);
                    terminal.grid.mark_all_dirty();
                    terminal.alt_grid.mark_all_dirty();
                }
            }
        }

        // OSC 104: Reset ANSI/256 palette color(s)
        "104" => {
            if params.len() <= 1 || params[1].is_empty() {
                terminal.theme.ansi_colors = terminal.theme.initial_ansi_colors;
                terminal.theme.palette_256 =
                    std::sync::Arc::clone(&terminal.theme.initial_palette_256);
            } else {
                for param in &params[1..] {
                    if let Ok(idx_str) = std::str::from_utf8(param)
                        && let Ok(idx) = idx_str.trim().parse::<usize>()
                        && idx < 256
                    {
                        std::sync::Arc::make_mut(&mut terminal.theme.palette_256)[idx] =
                            terminal.theme.initial_palette_256[idx];
                        if idx < 16 {
                            terminal.theme.ansi_colors[idx] =
                                terminal.theme.initial_ansi_colors[idx];
                        }
                    }
                }
            }
            terminal.grid.mark_all_dirty();
            terminal.alt_grid.mark_all_dirty();
        }

        // OSC 110: Reset default foreground color
        "110" => {
            let c = terminal.theme.initial_fg;
            terminal.theme.default_fg = c;
            terminal.current_fg = c;
            terminal.grid.mark_all_dirty();
            terminal.alt_grid.mark_all_dirty();
        }

        // OSC 111: Reset default background color
        "111" => {
            let c = terminal.theme.initial_bg;
            terminal.theme.default_bg = c;
            terminal.grid.default_bg = c;
            terminal.alt_grid.default_bg = c;
            terminal.current_bg = c;
            terminal.grid.mark_all_dirty();
            terminal.alt_grid.mark_all_dirty();
        }

        // OSC 112: Reset cursor color
        "112" => {
            terminal.theme.cursor_color = terminal.theme.initial_cursor_color;
            terminal.theme.cursor_text_color = terminal.theme.initial_cursor_text_color;
            terminal.grid.mark_all_dirty();
            terminal.alt_grid.mark_all_dirty();
        }

        // OSC 52: Clipboard read / write
        // Format: OSC 52 ; target ; data
        "52" if params.len() >= 2 => {
            let (target_str, data_bytes) = if params.len() >= 3 {
                (std::str::from_utf8(params[1]).unwrap_or("c"), params[2])
            } else {
                ("c", params[1])
            };

            // Limit data_bytes length to 10 MB to prevent DOS / unbounded allocation
            if data_bytes.len() > 10 * 1024 * 1024 {
                return;
            }

            let data_str = std::str::from_utf8(data_bytes).unwrap_or("").trim();

            if data_str == "?" {
                // Clipboard query request from shell/app
                let text = if target_str.contains('p') {
                    crate::clipboard::clipboard::primary_selection()
                } else {
                    crate::clipboard::clipboard::paste()
                };
                let b64 = crate::clipboard::clipboard::base64_encode(text.as_bytes());
                let resp = format!("\x1b]52;{};{}\x07", target_str, b64);
                terminal.send_to_shell(resp.as_bytes());
            } else {
                // Write decoded data to system clipboard
                if let Some(decoded) = crate::clipboard::clipboard::base64_decode(data_str)
                    && let Ok(text) = String::from_utf8(decoded)
                {
                    crate::clipboard::clipboard::copy(text);
                }
            }
        }

        // OSC 7: Current Working Directory notification
        "7" => {
            if params.len() >= 2 {
                let uri_bytes = params[1..].join(&b';');
                if let Ok(uri) = std::str::from_utf8(&uri_bytes)
                    && let Some(path) = parse_osc7_cwd(uri.trim())
                {
                    terminal.current_dir = Some(path);
                }
            }
        }

        // OSC 8: Hyperlinks
        "8" => {
            if let Some((id, url)) = crate::hyperlink::osc8::parse(params) {
                let link = if url.is_empty() {
                    None
                } else {
                    Some(std::sync::Arc::new(crate::hyperlink::osc8::Hyperlink {
                        id,
                        url,
                    }))
                };
                terminal.grid.current_hyperlink = link.clone();
                terminal.alt_grid.current_hyperlink = link;
            }
        }

        // OSC 133: Shell Integration / Semantic Prompt Marking (FinalTerm / FTCS)
        "133" if params.len() >= 2 => {
            let sub_cmd = std::str::from_utf8(params[1]).unwrap_or("").trim();
            match sub_cmd {
                "A" => {
                    terminal
                        .mark_semantic_zone(crate::terminal::terminal::SemanticZone::Prompt, None);
                }
                "B" => {
                    terminal
                        .mark_semantic_zone(crate::terminal::terminal::SemanticZone::Input, None);
                }
                "C" => {
                    terminal
                        .mark_semantic_zone(crate::terminal::terminal::SemanticZone::Output, None);
                }
                "D" => {
                    let exit_code = if params.len() >= 3 {
                        std::str::from_utf8(params[2])
                            .unwrap_or("")
                            .trim()
                            .parse::<i32>()
                            .ok()
                    } else {
                        None
                    };
                    terminal.last_command_exit_code = exit_code;
                    if let Some(mark) = terminal.prompt_marks.back_mut() {
                        mark.exit_code = exit_code;
                    }
                    terminal.semantic_zone = crate::terminal::terminal::SemanticZone::Prompt;
                }
                _ => {}
            }
        }

        // OSC 777: Desktop Notification (urxvt / foot / kitty standard)
        // Format: OSC 777 ; notify ; <title> ; <message>
        "777" if params.len() >= 4 => {
            let action = std::str::from_utf8(params[1]).unwrap_or("").trim();
            if action == "notify" {
                let allow = if let Ok(mut guard) = NOTIFY_RATE_LIMIT.lock() {
                    let now = Instant::now();
                    match *guard {
                        Some((start, ref mut count)) => {
                            if now.duration_since(start) < Duration::from_secs(2) {
                                if *count >= 5 {
                                    false
                                } else {
                                    *count += 1;
                                    true
                                }
                            } else {
                                *guard = Some((now, 1));
                                true
                            }
                        }
                        None => {
                            *guard = Some((now, 1));
                            true
                        }
                    }
                } else {
                    false
                };

                if allow {
                    let raw_title = std::str::from_utf8(params[2]).unwrap_or("").trim();
                    let raw_body = std::str::from_utf8(params[3]).unwrap_or("").trim();
                    let title: String = raw_title
                        .chars()
                        .filter(|c| !c.is_control() || *c == ' ')
                        .take(256)
                        .collect();
                    let body: String = raw_body
                        .chars()
                        .filter(|c| !c.is_control() || *c == ' ')
                        .take(256)
                        .collect();

                    if !title.is_empty() || !body.is_empty() {
                        std::thread::spawn(move || {
                            let _ = std::process::Command::new("notify-send")
                                .arg(&title)
                                .arg(&body)
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                        });
                    }
                }
            }
        }

        _ => {}
    }
}

fn parse_osc7_cwd(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let path = if let Some(slash_pos) = rest.find('/') {
        &rest[slash_pos..]
    } else {
        rest
    };
    if path.is_empty() {
        None
    } else {
        percent_decode_str(path)
    }
}

fn percent_decode_str(s: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes().peekable();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next()?;
            let h2 = chars.next()?;
            let hex_buf = [h1, h2];
            let hex_str = std::str::from_utf8(&hex_buf).ok()?;
            let val = u8::from_str_radix(hex_str, 16).ok()?;
            bytes.push(val);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).ok()
}

const HEX_TABLE: &[u8; 16] = b"0123456789abcdef";

#[inline]
fn write_hex_byte(byte: u8, out: &mut [u8]) {
    out[0] = HEX_TABLE[(byte >> 4) as usize];
    out[1] = HEX_TABLE[(byte & 0x0f) as usize];
}

#[inline]
fn format_color_query_response(
    osc_prefix: &[u8],
    idx: Option<usize>,
    c: Color,
    out: &mut [u8; 64],
) -> usize {
    out[0] = 0x1b;
    out[1] = b']';
    let mut len = 2;
    out[len..len + osc_prefix.len()].copy_from_slice(osc_prefix);
    len += osc_prefix.len();

    if let Some(i) = idx {
        out[len] = b';';
        len += 1;
        len += crate::ansi::csi::format_u32_into(i as u32, &mut out[len..]);
    }

    out[len..len + 5].copy_from_slice(b";rgb:");
    len += 5;

    write_hex_byte(c.r, &mut out[len..len + 2]);
    write_hex_byte(c.r, &mut out[len + 2..len + 4]);
    out[len + 4] = b'/';
    len += 5;

    write_hex_byte(c.g, &mut out[len..len + 2]);
    write_hex_byte(c.g, &mut out[len + 2..len + 4]);
    out[len + 4] = b'/';
    len += 5;

    write_hex_byte(c.b, &mut out[len..len + 2]);
    write_hex_byte(c.b, &mut out[len + 2..len + 4]);
    len += 4;

    out[len] = 0x1b;
    out[len + 1] = b'\\';
    len += 2;

    len
}

fn parse_color_spec(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("rgb:") {
        let mut parts = rest.split('/');
        let r_str = parts.next()?;
        let g_str = parts.next()?;
        let b_str = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let parse_channel = |c_str: &str| -> Option<u8> {
            if c_str.len() >= 4 {
                u8::from_str_radix(&c_str[0..2], 16).ok()
            } else if c_str.len() == 2 {
                u8::from_str_radix(c_str, 16).ok()
            } else if c_str.len() == 1 {
                u8::from_str_radix(c_str, 16).ok().map(|v| v * 17)
            } else {
                None
            }
        };
        let r = parse_channel(r_str)?;
        let g = parse_channel(g_str)?;
        let b = parse_channel(b_str)?;
        Some(Color { r, g, b })
    } else {
        crate::config::config::parse_hex_color(s)
    }
}
