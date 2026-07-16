use crate::terminal::terminal::Terminal;

pub fn handle_osc(params: &[&[u8]], terminal: &mut Terminal) {
    if params.len() >= 2 {
        if params[0] == b"11" {
            let color_bytes = params[1];
            if let Ok(color_str) = std::str::from_utf8(color_bytes) {
                let color_str = color_str.trim();
                if color_str != "?" {
                    let clean_color = if color_str.starts_with("rgb:") {
                        let rgb_parts: Vec<&str> = color_str[4..].split('/').collect();
                        if rgb_parts.len() == 3 {
                            let r = rgb_parts[0].get(0..2).unwrap_or("00");
                            let g = rgb_parts[1].get(0..2).unwrap_or("00");
                            let b = rgb_parts[2].get(0..2).unwrap_or("00");
                            format!("#{}{}{}", r, g, b)
                        } else {
                            color_str.to_string()
                        }
                    } else {
                        color_str.to_string()
                    };

                    if let Some(c) = crate::config::config::parse_hex_color(&clean_color) {
                        terminal.theme.default_bg = c;
                        terminal.grid.default_bg = c;
                        terminal.alt_grid.default_bg = c;
                        terminal.current_bg = c;
                    }
                }
            }
        }
    }
}

