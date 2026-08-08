use crate::terminal::terminal::Terminal;
use crate::screen::cell::Color;

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
                && let Ok(title) = std::str::from_utf8(params[1]) {
                    terminal.app_title = Some(title.to_string());
                }
        }

        // OSC 4: Query or set ANSI palette colors
        // Format: OSC 4 ; index ; color [; index ; color ...]
        "4" => {
            let mut i = 1;
            while i + 1 < params.len() {
                let idx_str = std::str::from_utf8(params[i]).unwrap_or("").trim();
                let color_str = std::str::from_utf8(params[i + 1]).unwrap_or("").trim();

                if let Ok(idx) = idx_str.parse::<usize>() {
                    if color_str == "?" {
                        let c = if idx < 16 {
                            terminal.theme.ansi_colors[idx]
                        } else {
                            terminal.theme.get_256_color(idx as u8)
                        };
                        let resp = format!(
                            "\x1b]4;{};rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}\x07",
                            idx, c.r, c.r, c.g, c.g, c.b, c.b
                        );
                        terminal.send_to_shell(resp.as_bytes());
                    } else if let Some(c) = parse_color_spec(color_str)
                        && idx < 16 {
                            terminal.theme.ansi_colors[idx] = c;
                        }
                }
                i += 2;
            }
        }

        // OSC 10: Query or set default foreground color
        "10" => {
            if params.len() >= 2 {
                let color_str = std::str::from_utf8(params[1]).unwrap_or("").trim();
                if color_str == "?" {
                    let c = terminal.theme.default_fg;
                    let resp = format!(
                        "\x1b]10;rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}\x07",
                        c.r, c.r, c.g, c.g, c.b, c.b
                    );
                    terminal.send_to_shell(resp.as_bytes());
                } else if let Some(c) = parse_color_spec(color_str) {
                    terminal.theme.default_fg = c;
                    terminal.current_fg = c;
                }
            }
        }

        // OSC 11: Query or set default background color
        "11" => {
            if params.len() >= 2 {
                let color_str = std::str::from_utf8(params[1]).unwrap_or("").trim();
                if color_str == "?" {
                    let c = terminal.theme.default_bg;
                    let resp = format!(
                        "\x1b]11;rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}\x07",
                        c.r, c.r, c.g, c.g, c.b, c.b
                    );
                    terminal.send_to_shell(resp.as_bytes());
                } else if let Some(c) = parse_color_spec(color_str) {
                    terminal.theme.default_bg = c;
                    terminal.grid.default_bg = c;
                    terminal.alt_grid.default_bg = c;
                    terminal.current_bg = c;
                }
            }
        }

        // OSC 52: Clipboard read / write
        // Format: OSC 52 ; target ; data
        "52"
            if params.len() >= 2 => {
                let (target_str, data_bytes) = if params.len() >= 3 {
                    (
                        std::str::from_utf8(params[1]).unwrap_or("c"),
                        params[2],
                    )
                } else {
                    ("c", params[1])
                };

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
                        && let Ok(text) = String::from_utf8(decoded) {
                            crate::clipboard::clipboard::copy(&text);
                        }
                }
            }

        _ => {}
    }
}

fn parse_color_spec(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("rgb:") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 3 {
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
            let r = parse_channel(parts[0])?;
            let g = parse_channel(parts[1])?;
            let b = parse_channel(parts[2])?;
            Some(Color { r, g, b, a: 255 })
        } else {
            None
        }
    } else {
        crate::config::config::parse_hex_color(s)
    }
}
