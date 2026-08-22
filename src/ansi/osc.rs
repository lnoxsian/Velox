use crate::screen::cell::Color;
use crate::terminal::terminal::Terminal;

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
                        && idx < 16
                    {
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

        // OSC 12: Query or set dynamic cursor color
        "12" => {
            if params.len() >= 2 {
                let color_str = std::str::from_utf8(params[1]).unwrap_or("").trim();
                if color_str == "?" {
                    let c = terminal
                        .theme
                        .cursor_color
                        .unwrap_or(terminal.theme.default_fg);
                    let resp = format!(
                        "\x1b]12;rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}\x07",
                        c.r, c.r, c.g, c.g, c.b, c.b
                    );
                    terminal.send_to_shell(resp.as_bytes());
                } else if let Some(c) = parse_color_spec(color_str) {
                    terminal.theme.cursor_color = Some(c);
                }
            }
        }

        // OSC 104: Reset ANSI palette color(s)
        "104" => {
            if params.len() <= 1 || params[1].is_empty() {
                terminal.theme.ansi_colors = terminal.theme.initial_ansi_colors;
            } else {
                for param in &params[1..] {
                    if let Ok(idx_str) = std::str::from_utf8(param)
                        && let Ok(idx) = idx_str.trim().parse::<usize>()
                        && idx < 16
                    {
                        terminal.theme.ansi_colors[idx] = terminal.theme.initial_ansi_colors[idx];
                    }
                }
            }
        }

        // OSC 110: Reset default foreground color
        "110" => {
            let c = terminal.theme.initial_fg;
            terminal.theme.default_fg = c;
            terminal.current_fg = c;
        }

        // OSC 111: Reset default background color
        "111" => {
            let c = terminal.theme.initial_bg;
            terminal.theme.default_bg = c;
            terminal.grid.default_bg = c;
            terminal.alt_grid.default_bg = c;
            terminal.current_bg = c;
        }

        // OSC 112: Reset cursor color
        "112" => {
            terminal.theme.cursor_color = terminal.theme.initial_cursor_color;
            terminal.theme.cursor_text_color = terminal.theme.initial_cursor_text_color;
        }

        // OSC 52: Clipboard read / write
        // Format: OSC 52 ; target ; data
        "52" if params.len() >= 2 => {
            let (target_str, data_bytes) = if params.len() >= 3 {
                (std::str::from_utf8(params[1]).unwrap_or("c"), params[2])
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
            let _ = crate::hyperlink::osc8::parse(params);
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
            Some(Color { r, g, b })
        } else {
            None
        }
    } else {
        crate::config::config::parse_hex_color(s)
    }
}
