use super::framebuffer::Framebuffer;

/// Direct CPU rasterization for Box Drawing and Block Element characters.
/// Returns `true` if the character was handled as a geometric primitive.
pub fn try_render_primitive(
    c: char,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    fg: u32,
    fb: &mut Framebuffer,
) -> bool {
    match c {
        // Full block
        '█' => {
            fb.fill_span(px, py, cell_w, cell_h, fg);
            true
        }

        // Lower eighths
        '\u{2581}'..='\u{2587}' => {
            let frac = (c as u32 - 0x2580) as f32 / 8.0;
            let h = ((cell_h as f32 * frac).round() as u32).max(1);
            let y = py + cell_h.saturating_sub(h);
            fb.fill_span(px, y, cell_w, h, fg);
            true
        }

        // Upper half block
        '▀' => {
            let h = cell_h / 2;
            fb.fill_span(px, py, cell_w, h, fg);
            true
        }

        // Upper one eighth
        '▔' => {
            let h = (cell_h / 8).max(1);
            fb.fill_span(px, py, cell_w, h, fg);
            true
        }

        // Left eighths: ▏ ▎ ▍ ▌ ▋ ▊ ▉ (0x258F ..= 0x2589)
        '\u{2589}'..='\u{258F}' => {
            let eighths = 8 - (c as u32 - 0x2588);
            let w = ((cell_w as f32 * (eighths as f32 / 8.0)).round() as u32).max(1);
            fb.fill_span(px, py, w, cell_h, fg);
            true
        }

        // Right half block
        '▐' => {
            let w = cell_w / 2;
            let x = px + cell_w.saturating_sub(w);
            fb.fill_span(x, py, w, cell_h, fg);
            true
        }

        // Right one eighth
        '▕' => {
            let w = (cell_w / 8).max(1);
            let x = px + cell_w.saturating_sub(w);
            fb.fill_span(x, py, w, cell_h, fg);
            true
        }

        // Quadrant blocks
        '▖' => {
            // Lower left
            fb.fill_span(px, py + cell_h / 2, cell_w / 2, cell_h - cell_h / 2, fg);
            true
        }
        '▗' => {
            // Lower right
            fb.fill_span(
                px + cell_w / 2,
                py + cell_h / 2,
                cell_w - cell_w / 2,
                cell_h - cell_h / 2,
                fg,
            );
            true
        }
        '▘' => {
            // Upper left
            fb.fill_span(px, py, cell_w / 2, cell_h / 2, fg);
            true
        }
        '▝' => {
            // Upper right
            fb.fill_span(px + cell_w / 2, py, cell_w - cell_w / 2, cell_h / 2, fg);
            true
        }
        '▙' => {
            // Upper left, lower left, lower right
            fb.fill_span(px, py, cell_w / 2, cell_h, fg);
            fb.fill_span(
                px + cell_w / 2,
                py + cell_h / 2,
                cell_w - cell_w / 2,
                cell_h - cell_h / 2,
                fg,
            );
            true
        }
        '▛' => {
            // Upper left, upper right, lower left
            fb.fill_span(px, py, cell_w, cell_h / 2, fg);
            fb.fill_span(px, py + cell_h / 2, cell_w / 2, cell_h - cell_h / 2, fg);
            true
        }
        '▜' => {
            // Upper left, upper right, lower right
            fb.fill_span(px, py, cell_w, cell_h / 2, fg);
            fb.fill_span(
                px + cell_w / 2,
                py + cell_h / 2,
                cell_w - cell_w / 2,
                cell_h - cell_h / 2,
                fg,
            );
            true
        }
        '▟' => {
            // Lower left, lower right, upper right
            fb.fill_span(px, py + cell_h / 2, cell_w, cell_h - cell_h / 2, fg);
            fb.fill_span(px + cell_w / 2, py, cell_w - cell_w / 2, cell_h / 2, fg);
            true
        }

        // Shades: ░ (25%), ▒ (50%), ▓ (75%)
        '░' | '▒' | '▓' => {
            for y in 0..cell_h {
                for x in 0..cell_w {
                    let set = if c == '▒' {
                        ((px + x) + (py + y)).is_multiple_of(2)
                    } else if c == '░' {
                        (px + x).is_multiple_of(2) && (py + y).is_multiple_of(2)
                    } else {
                        !((px + x).is_multiple_of(2) && (py + y).is_multiple_of(2))
                    };
                    if set {
                        fb.fill_span(px + x, py + y, 1, 1, fg);
                    }
                }
            }
            true
        }

        // Box Drawing Characters
        '\u{2500}'..='\u{257f}' | '|' => render_box_drawing(c, px, py, cell_w, cell_h, fg, fb),

        _ => false,
    }
}

fn render_box_drawing(
    c: char,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    fg: u32,
    fb: &mut Framebuffer,
) -> bool {
    let mid_x = px + cell_w / 2;
    let mid_y = py + cell_h / 2;
    let t_light = (cell_w / 10).max(1);
    let t_heavy = (t_light * 2).min(cell_w);

    let half_t_light = t_light / 2;
    let half_t_heavy = t_heavy / 2;

    match c {
        // ASCII pipe
        '|' => {
            fb.fill_span(mid_x.saturating_sub(half_t_light), py, t_light, cell_h, fg);
            true
        }

        // Light horizontal: ─
        '─' => {
            fb.fill_span(px, mid_y.saturating_sub(half_t_light), cell_w, t_light, fg);
            true
        }
        // Heavy horizontal: ━
        '━' => {
            fb.fill_span(px, mid_y.saturating_sub(half_t_heavy), cell_w, t_heavy, fg);
            true
        }
        // Light vertical: │
        '│' => {
            fb.fill_span(mid_x.saturating_sub(half_t_light), py, t_light, cell_h, fg);
            true
        }
        // Heavy vertical: ┃
        '┃' => {
            fb.fill_span(mid_x.saturating_sub(half_t_heavy), py, t_heavy, cell_h, fg);
            true
        }

        // Corners
        // Light ┌
        '┌' | '╭' => {
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                mid_y.saturating_sub(half_t_light),
                cell_w - (cell_w / 2),
                t_light,
                fg,
            );
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                mid_y.saturating_sub(half_t_light),
                t_light,
                cell_h - (cell_h / 2),
                fg,
            );
            true
        }
        // Light ┐
        '┐' | '╮' => {
            fb.fill_span(
                px,
                mid_y.saturating_sub(half_t_light),
                cell_w / 2 + half_t_light + 1,
                t_light,
                fg,
            );
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                mid_y.saturating_sub(half_t_light),
                t_light,
                cell_h - (cell_h / 2),
                fg,
            );
            true
        }
        // Light └
        '└' | '╰' => {
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                mid_y.saturating_sub(half_t_light),
                cell_w - (cell_w / 2),
                t_light,
                fg,
            );
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                py,
                t_light,
                cell_h / 2 + half_t_light + 1,
                fg,
            );
            true
        }
        // Light ┘
        '┘' | '╯' => {
            fb.fill_span(
                px,
                mid_y.saturating_sub(half_t_light),
                cell_w / 2 + half_t_light + 1,
                t_light,
                fg,
            );
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                py,
                t_light,
                cell_h / 2 + half_t_light + 1,
                fg,
            );
            true
        }

        // Tees
        // Light ├
        '├' => {
            fb.fill_span(mid_x.saturating_sub(half_t_light), py, t_light, cell_h, fg);
            fb.fill_span(
                mid_x,
                mid_y.saturating_sub(half_t_light),
                cell_w - (cell_w / 2),
                t_light,
                fg,
            );
            true
        }
        // Light ┤
        '┤' => {
            fb.fill_span(mid_x.saturating_sub(half_t_light), py, t_light, cell_h, fg);
            fb.fill_span(
                px,
                mid_y.saturating_sub(half_t_light),
                cell_w / 2,
                t_light,
                fg,
            );
            true
        }
        // Light ┬
        '┬' => {
            fb.fill_span(px, mid_y.saturating_sub(half_t_light), cell_w, t_light, fg);
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                mid_y,
                t_light,
                cell_h - (cell_h / 2),
                fg,
            );
            true
        }
        // Light ┴
        '┴' => {
            fb.fill_span(px, mid_y.saturating_sub(half_t_light), cell_w, t_light, fg);
            fb.fill_span(
                mid_x.saturating_sub(half_t_light),
                py,
                t_light,
                cell_h / 2,
                fg,
            );
            true
        }
        // Light ┼
        '┼' => {
            fb.fill_span(px, mid_y.saturating_sub(half_t_light), cell_w, t_light, fg);
            fb.fill_span(mid_x.saturating_sub(half_t_light), py, t_light, cell_h, fg);
            true
        }

        // Double lines
        '═' => {
            let offset = (cell_h / 6).max(2);
            fb.fill_span(px, mid_y.saturating_sub(offset), cell_w, t_light, fg);
            fb.fill_span(px, mid_y + offset, cell_w, t_light, fg);
            true
        }
        '║' => {
            let offset = (cell_w / 6).max(2);
            fb.fill_span(mid_x.saturating_sub(offset), py, t_light, cell_h, fg);
            fb.fill_span(mid_x + offset, py, t_light, cell_h, fg);
            true
        }

        _ => false,
    }
}
