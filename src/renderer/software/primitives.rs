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

        // Lower eighths:   ▂ ▃ ▄ ▅ ▆ ▇
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
        '▚' => {
            // Upper left and lower right
            fb.fill_span(px, py, cell_w / 2, cell_h / 2, fg);
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
        '▞' => {
            // Upper right and lower left
            fb.fill_span(px + cell_w / 2, py, cell_w - cell_w / 2, cell_h / 2, fg);
            fb.fill_span(px, py + cell_h / 2, cell_w / 2, cell_h - cell_h / 2, fg);
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

        // Box Drawing Characters (U+2500 ..= U+257F) + ASCII vertical pipe
        '\u{2500}'..='\u{257f}' | '|' => render_box_drawing(c, px, py, cell_w, cell_h, fg, fb),

        _ => false,
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum LineStyle {
    None,
    Light,
    Heavy,
    Double,
}

#[inline(always)]
fn get_box_drawing_arms(c: char) -> Option<(LineStyle, LineStyle, LineStyle, LineStyle)> {
    use LineStyle::{Double as D, Heavy as H, Light as L, None as N};
    let arms = match c {
        // Straight lines
        '─' => (N, N, L, L),
        '━' => (N, N, H, H),
        '│' | '|' => (L, L, N, N),
        '┃' => (H, H, N, N),

        // Corners: Light / Heavy
        '┌' => (N, L, N, L),
        '┍' => (N, L, N, H),
        '┎' => (N, H, N, L),
        '┏' => (N, H, N, H),
        '┐' => (N, L, L, N),
        '┑' => (N, L, H, N),
        '┒' => (N, H, L, N),
        '┓' => (N, H, H, N),
        '└' => (L, N, N, L),
        '┕' => (L, N, N, H),
        '┖' => (H, N, N, L),
        '┗' => (H, N, N, H),
        '┘' => (L, N, L, N),
        '┙' => (L, N, H, N),
        '┚' => (H, N, L, N),
        '┛' => (H, N, H, N),

        // Tees: Right
        '├' => (L, L, N, L),
        '┝' => (L, L, N, H),
        '┞' => (H, L, N, L),
        '┟' => (L, H, N, L),
        '┠' => (H, H, N, L),
        '┡' => (L, H, N, H),
        '┢' => (H, L, N, H),
        '┣' => (H, H, N, H),

        // Tees: Left
        '┤' => (L, L, L, N),
        '┥' => (L, L, H, N),
        '┦' => (H, L, L, N),
        '┧' => (L, H, L, N),
        '┨' => (H, H, L, N),
        '┩' => (L, H, H, N),
        '┪' => (H, L, H, N),
        '┫' => (H, H, H, N),

        // Tees: Down
        '┬' => (N, L, L, L),
        '┭' => (N, L, H, L),
        '┮' => (N, L, L, H),
        '┯' => (N, L, H, H),
        '┰' => (N, H, L, L),
        '┱' => (N, H, H, L),
        '┲' => (N, H, L, H),
        '┳' => (N, H, H, H),

        // Tees: Up
        '┴' => (L, N, L, L),
        '┵' => (L, N, H, L),
        '┶' => (L, N, L, H),
        '┷' => (L, N, H, H),
        '┸' => (H, N, L, L),
        '┹' => (H, N, H, L),
        '┺' => (H, N, L, H),
        '┻' => (H, N, H, H),

        // Crosses
        '┼' => (L, L, L, L),
        '┽' => (L, L, H, L),
        '┾' => (L, L, L, H),
        '┿' => (L, L, H, H),
        '╀' => (H, L, L, L),
        '╁' => (L, H, L, L),
        '╂' => (H, H, L, L),
        '╃' => (H, L, H, L),
        '╄' => (H, L, L, H),
        '╅' => (L, H, H, L),
        '╆' => (L, H, L, H),
        '╇' => (L, H, H, H),
        '╈' => (H, L, H, H),
        '╉' => (H, H, H, L),
        '╊' => (H, H, L, H),
        '╋' => (H, H, H, H),

        // Double & Mixed lines
        '═' => (N, N, D, D),
        '║' => (D, D, N, N),
        '╒' => (N, L, N, D),
        '╓' => (N, D, N, L),
        '╔' => (N, D, N, D),
        '╕' => (N, L, D, N),
        '╖' => (N, D, L, N),
        '╗' => (N, D, D, N),
        '╘' => (L, N, N, D),
        '╙' => (D, N, N, L),
        '╚' => (D, N, N, D),
        '╛' => (L, N, D, N),
        '╜' => (D, N, L, N),
        '╝' => (D, N, D, N),
        '╞' => (L, L, N, D),
        '╟' => (D, D, N, L),
        '╠' => (D, D, N, D),
        '╡' => (L, L, D, N),
        '╢' => (D, D, L, N),
        '╣' => (D, D, D, N),
        '╤' => (N, L, D, D),
        '╥' => (N, D, L, L),
        '╦' => (N, D, D, D),
        '╧' => (L, N, D, D),
        '╨' => (D, N, L, L),
        '╩' => (D, N, D, D),
        '╪' => (L, L, D, D),
        '╫' => (D, D, L, L),
        '╬' => (D, D, D, D),

        // Half lines
        '╴' => (N, N, L, N),
        '╵' => (L, N, N, N),
        '╶' => (N, N, N, L),
        '╷' => (N, L, N, N),
        '╸' => (N, N, H, N),
        '╹' => (H, N, N, N),
        '╺' => (N, N, N, H),
        '╻' => (N, H, N, N),
        '╼' => (N, N, L, H),
        '╽' => (L, H, N, N),
        '╾' => (N, N, H, L),
        '╿' => (H, L, N, N),

        _ => return None,
    };
    Some(arms)
}

#[allow(clippy::too_many_arguments)]
fn draw_box_arms(
    up: LineStyle,
    down: LineStyle,
    left: LineStyle,
    right: LineStyle,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    fg: u32,
    fb: &mut Framebuffer,
) {
    let mid_x = px + cell_w / 2;
    let mid_y = py + cell_h / 2;

    let t_light = (cell_w / 10).max(1);
    let t_heavy = (t_light * 2).clamp(t_light + 1, cell_w);

    let half_tl = t_light / 2;
    let half_th = t_heavy / 2;

    let d_off_x = (cell_w / 5).max(t_light + 1);
    let d_off_y = (cell_h / 5).max(t_light + 1);

    let y_d_top = (mid_y.saturating_sub(d_off_y)).saturating_sub(half_tl);
    let y_d_bot = (mid_y + d_off_y).saturating_sub(half_tl);
    let x_d_left = (mid_x.saturating_sub(d_off_x)).saturating_sub(half_tl);
    let x_d_right = (mid_x + d_off_x).saturating_sub(half_tl);

    let y_s_light = mid_y.saturating_sub(half_tl);
    let x_s_light = mid_x.saturating_sub(half_tl);
    let y_s_heavy = mid_y.saturating_sub(half_th);
    let x_s_heavy = mid_x.saturating_sub(half_th);

    let cell_right = px + cell_w;
    let cell_bottom = py + cell_h;

    let has_double = up == LineStyle::Double
        || down == LineStyle::Double
        || left == LineStyle::Double
        || right == LineStyle::Double;

    if has_double {
        match (up, down, left, right) {
            // Pure double
            (LineStyle::None, LineStyle::None, LineStyle::Double, LineStyle::Double) => {
                // ═
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::None, LineStyle::None) => {
                // ║
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
            }
            (LineStyle::None, LineStyle::Double, LineStyle::None, LineStyle::Double) => {
                // ╔
                fb.fill_span(x_d_left, y_d_top, cell_right.saturating_sub(x_d_left), t_light, fg);
                fb.fill_span(x_d_left, y_d_top, t_light, cell_bottom.saturating_sub(y_d_top), fg);
                fb.fill_span(x_d_right, y_d_bot, cell_right.saturating_sub(x_d_right), t_light, fg);
                fb.fill_span(x_d_right, y_d_bot, t_light, cell_bottom.saturating_sub(y_d_bot), fg);
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Double, LineStyle::None) => {
                // ╗
                fb.fill_span(px, y_d_top, (x_d_right + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_right, y_d_top, t_light, cell_bottom.saturating_sub(y_d_top), fg);
                fb.fill_span(px, y_d_bot, (x_d_left + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_left, y_d_bot, t_light, cell_bottom.saturating_sub(y_d_bot), fg);
            }
            (LineStyle::Double, LineStyle::None, LineStyle::None, LineStyle::Double) => {
                // ╚
                fb.fill_span(x_d_left, y_d_bot, cell_right.saturating_sub(x_d_left), t_light, fg);
                fb.fill_span(x_d_left, py, t_light, (y_d_bot + t_light).saturating_sub(py), fg);
                fb.fill_span(x_d_right, y_d_top, cell_right.saturating_sub(x_d_right), t_light, fg);
                fb.fill_span(x_d_right, py, t_light, (y_d_top + t_light).saturating_sub(py), fg);
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Double, LineStyle::None) => {
                // ╝
                fb.fill_span(px, y_d_bot, (x_d_right + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_right, py, t_light, (y_d_bot + t_light).saturating_sub(py), fg);
                fb.fill_span(px, y_d_top, (x_d_left + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_left, py, t_light, (y_d_top + t_light).saturating_sub(py), fg);
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::None, LineStyle::Double) => {
                // ╠
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, (y_d_top + t_light).saturating_sub(py), fg);
                fb.fill_span(x_d_right, y_d_bot, t_light, cell_bottom.saturating_sub(y_d_bot), fg);
                fb.fill_span(x_d_right, y_d_top, cell_right.saturating_sub(x_d_right), t_light, fg);
                fb.fill_span(x_d_right, y_d_bot, cell_right.saturating_sub(x_d_right), t_light, fg);
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::Double, LineStyle::None) => {
                // ╣
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
                fb.fill_span(x_d_left, py, t_light, (y_d_top + t_light).saturating_sub(py), fg);
                fb.fill_span(x_d_left, y_d_bot, t_light, cell_bottom.saturating_sub(y_d_bot), fg);
                fb.fill_span(px, y_d_top, (x_d_left + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(px, y_d_bot, (x_d_left + t_light).saturating_sub(px), t_light, fg);
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Double, LineStyle::Double) => {
                // ╦
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, (x_d_left + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_right, y_d_bot, cell_right.saturating_sub(x_d_right), t_light, fg);
                fb.fill_span(x_d_left, y_d_bot, t_light, cell_bottom.saturating_sub(y_d_bot), fg);
                fb.fill_span(x_d_right, y_d_bot, t_light, cell_bottom.saturating_sub(y_d_bot), fg);
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Double, LineStyle::Double) => {
                // ╩
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(px, y_d_top, (x_d_left + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_right, y_d_top, cell_right.saturating_sub(x_d_right), t_light, fg);
                fb.fill_span(x_d_left, py, t_light, (y_d_top + t_light).saturating_sub(py), fg);
                fb.fill_span(x_d_right, py, t_light, (y_d_top + t_light).saturating_sub(py), fg);
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::Double, LineStyle::Double) => {
                // ╬
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
            }

            // Mixed single and double
            (LineStyle::None, LineStyle::Light, LineStyle::None, LineStyle::Double) => {
                // ╒
                fb.fill_span(x_s_light, y_d_top, t_light, cell_bottom.saturating_sub(y_d_top), fg);
                fb.fill_span(x_s_light, y_d_top, cell_right.saturating_sub(x_s_light), t_light, fg);
                fb.fill_span(x_s_light, y_d_bot, cell_right.saturating_sub(x_s_light), t_light, fg);
            }
            (LineStyle::None, LineStyle::Double, LineStyle::None, LineStyle::Light) => {
                // ╓
                fb.fill_span(x_d_left, y_s_light, cell_right.saturating_sub(x_d_left), t_light, fg);
                fb.fill_span(x_d_left, y_s_light, t_light, cell_bottom.saturating_sub(y_s_light), fg);
                fb.fill_span(x_d_right, y_s_light, t_light, cell_bottom.saturating_sub(y_s_light), fg);
            }
            (LineStyle::None, LineStyle::Light, LineStyle::Double, LineStyle::None) => {
                // ╕
                fb.fill_span(x_s_light, y_d_top, t_light, cell_bottom.saturating_sub(y_d_top), fg);
                fb.fill_span(px, y_d_top, (x_s_light + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(px, y_d_bot, (x_s_light + t_light).saturating_sub(px), t_light, fg);
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Light, LineStyle::None) => {
                // ╖
                fb.fill_span(px, y_s_light, (x_d_right + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_left, y_s_light, t_light, cell_bottom.saturating_sub(y_s_light), fg);
                fb.fill_span(x_d_right, y_s_light, t_light, cell_bottom.saturating_sub(y_s_light), fg);
            }
            (LineStyle::Light, LineStyle::None, LineStyle::None, LineStyle::Double) => {
                // ╘
                fb.fill_span(x_s_light, py, t_light, (y_d_bot + t_light).saturating_sub(py), fg);
                fb.fill_span(x_s_light, y_d_top, cell_right.saturating_sub(x_s_light), t_light, fg);
                fb.fill_span(x_s_light, y_d_bot, cell_right.saturating_sub(x_s_light), t_light, fg);
            }
            (LineStyle::Double, LineStyle::None, LineStyle::None, LineStyle::Light) => {
                // ╙
                fb.fill_span(x_d_left, y_s_light, cell_right.saturating_sub(x_d_left), t_light, fg);
                fb.fill_span(x_d_left, py, t_light, (y_s_light + t_light).saturating_sub(py), fg);
                fb.fill_span(x_d_right, py, t_light, (y_s_light + t_light).saturating_sub(py), fg);
            }
            (LineStyle::Light, LineStyle::None, LineStyle::Double, LineStyle::None) => {
                // ╛
                fb.fill_span(x_s_light, py, t_light, (y_d_bot + t_light).saturating_sub(py), fg);
                fb.fill_span(px, y_d_top, (x_s_light + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(px, y_d_bot, (x_s_light + t_light).saturating_sub(px), t_light, fg);
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Light, LineStyle::None) => {
                // ╜
                fb.fill_span(px, y_s_light, (x_d_right + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(x_d_left, py, t_light, (y_s_light + t_light).saturating_sub(py), fg);
                fb.fill_span(x_d_right, py, t_light, (y_s_light + t_light).saturating_sub(py), fg);
            }
            (LineStyle::Light, LineStyle::Light, LineStyle::None, LineStyle::Double) => {
                // ╞
                fb.fill_span(x_s_light, py, t_light, cell_h, fg);
                fb.fill_span(x_s_light, y_d_top, cell_right.saturating_sub(x_s_light), t_light, fg);
                fb.fill_span(x_s_light, y_d_bot, cell_right.saturating_sub(x_s_light), t_light, fg);
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::None, LineStyle::Light) => {
                // ╟
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, y_s_light, cell_right.saturating_sub(x_d_right), t_light, fg);
            }
            (LineStyle::Light, LineStyle::Light, LineStyle::Double, LineStyle::None) => {
                // ╡
                fb.fill_span(x_s_light, py, t_light, cell_h, fg);
                fb.fill_span(px, y_d_top, (x_s_light + t_light).saturating_sub(px), t_light, fg);
                fb.fill_span(px, y_d_bot, (x_s_light + t_light).saturating_sub(px), t_light, fg);
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::Light, LineStyle::None) => {
                // ╢
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
                fb.fill_span(px, y_s_light, (x_d_left + t_light).saturating_sub(px), t_light, fg);
            }
            (LineStyle::None, LineStyle::Light, LineStyle::Double, LineStyle::Double) => {
                // ╤
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(x_s_light, y_d_bot, t_light, cell_bottom.saturating_sub(y_d_bot), fg);
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Light, LineStyle::Light) => {
                // ╥
                fb.fill_span(px, y_s_light, cell_w, t_light, fg);
                fb.fill_span(x_d_left, y_s_light, t_light, cell_bottom.saturating_sub(y_s_light), fg);
                fb.fill_span(x_d_right, y_s_light, t_light, cell_bottom.saturating_sub(y_s_light), fg);
            }
            (LineStyle::Light, LineStyle::None, LineStyle::Double, LineStyle::Double) => {
                // ╧
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(x_s_light, py, t_light, (y_d_top + t_light).saturating_sub(py), fg);
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Light, LineStyle::Light) => {
                // ╨
                fb.fill_span(px, y_s_light, cell_w, t_light, fg);
                fb.fill_span(x_d_left, py, t_light, (y_s_light + t_light).saturating_sub(py), fg);
                fb.fill_span(x_d_right, py, t_light, (y_s_light + t_light).saturating_sub(py), fg);
            }
            (LineStyle::Light, LineStyle::Light, LineStyle::Double, LineStyle::Double) => {
                // ╪
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(x_s_light, py, t_light, cell_h, fg);
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::Light, LineStyle::Light) => {
                // ╫
                fb.fill_span(px, y_s_light, cell_w, t_light, fg);
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
            }
            _ => {}
        }
        return;
    }

    // Single / Heavy lines
    let center_t = if up == LineStyle::Heavy
        || down == LineStyle::Heavy
        || left == LineStyle::Heavy
        || right == LineStyle::Heavy
    {
        t_heavy
    } else {
        t_light
    };
    let center_half = center_t / 2;

    // Up arm
    match up {
        LineStyle::Light => {
            let y_end = if down != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
                mid_y + center_half + 1
            } else {
                mid_y
            };
            fb.fill_span(x_s_light, py, t_light, y_end.saturating_sub(py), fg);
        }
        LineStyle::Heavy => {
            let y_end = if down != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
                mid_y + center_half + 1
            } else {
                mid_y
            };
            fb.fill_span(x_s_heavy, py, t_heavy, y_end.saturating_sub(py), fg);
        }
        _ => {}
    }

    // Down arm
    match down {
        LineStyle::Light => {
            let y_start = if up != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
                mid_y.saturating_sub(center_half)
            } else {
                mid_y
            };
            fb.fill_span(x_s_light, y_start, t_light, cell_bottom.saturating_sub(y_start), fg);
        }
        LineStyle::Heavy => {
            let y_start = if up != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
                mid_y.saturating_sub(center_half)
            } else {
                mid_y
            };
            fb.fill_span(x_s_heavy, y_start, t_heavy, cell_bottom.saturating_sub(y_start), fg);
        }
        _ => {}
    }

    // Left arm
    match left {
        LineStyle::Light => {
            let x_end = if right != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
                mid_x + center_half + 1
            } else {
                mid_x
            };
            fb.fill_span(px, y_s_light, x_end.saturating_sub(px), t_light, fg);
        }
        LineStyle::Heavy => {
            let x_end = if right != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
                mid_x + center_half + 1
            } else {
                mid_x
            };
            fb.fill_span(px, y_s_heavy, x_end.saturating_sub(px), t_heavy, fg);
        }
        _ => {}
    }

    // Right arm
    match right {
        LineStyle::Light => {
            let x_start = if left != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
                mid_x.saturating_sub(center_half)
            } else {
                mid_x
            };
            fb.fill_span(x_start, y_s_light, cell_right.saturating_sub(x_start), t_light, fg);
        }
        LineStyle::Heavy => {
            let x_start = if left != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
                mid_x.saturating_sub(center_half)
            } else {
                mid_x
            };
            fb.fill_span(x_start, y_s_heavy, cell_right.saturating_sub(x_start), t_heavy, fg);
        }
        _ => {}
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
    if let Some((up, down, left, right)) = get_box_drawing_arms(c) {
        draw_box_arms(up, down, left, right, px, py, cell_w, cell_h, fg, fb);
        return true;
    }

    let mid_x = px + cell_w / 2;
    let mid_y = py + cell_h / 2;
    let t_light = (cell_w / 10).max(1);
    let t_heavy = (t_light * 2).clamp(t_light + 1, cell_w);
    let half_tl = t_light / 2;
    let y_s_light = mid_y.saturating_sub(half_tl);
    let x_s_light = mid_x.saturating_sub(half_tl);
    let cell_right = px + cell_w;
    let cell_bottom = py + cell_h;

    match c {
        // Rounded corners
        '╭' => {
            fb.fill_span(mid_x + half_tl, y_s_light, cell_right.saturating_sub(mid_x + half_tl), t_light, fg);
            fb.fill_span(x_s_light, mid_y + half_tl, t_light, cell_bottom.saturating_sub(mid_y + half_tl), fg);
            if x_s_light + t_light < cell_right && y_s_light + t_light < cell_bottom {
                fb.fill_span(x_s_light + t_light, y_s_light + t_light, t_light, t_light, fg);
            }
            true
        }
        '╮' => {
            fb.fill_span(px, y_s_light, (mid_x.saturating_sub(half_tl)).saturating_sub(px), t_light, fg);
            fb.fill_span(x_s_light, mid_y + half_tl, t_light, cell_bottom.saturating_sub(mid_y + half_tl), fg);
            if x_s_light >= px + t_light && y_s_light + t_light < cell_bottom {
                fb.fill_span(x_s_light.saturating_sub(t_light), y_s_light + t_light, t_light, t_light, fg);
            }
            true
        }
        '╯' => {
            fb.fill_span(px, y_s_light, (mid_x.saturating_sub(half_tl)).saturating_sub(px), t_light, fg);
            fb.fill_span(x_s_light, py, t_light, (mid_y.saturating_sub(half_tl)).saturating_sub(py), fg);
            if x_s_light >= px + t_light && y_s_light >= py + t_light {
                fb.fill_span(x_s_light.saturating_sub(t_light), y_s_light.saturating_sub(t_light), t_light, t_light, fg);
            }
            true
        }
        '╰' => {
            fb.fill_span(mid_x + half_tl, y_s_light, cell_right.saturating_sub(mid_x + half_tl), t_light, fg);
            fb.fill_span(x_s_light, py, t_light, (mid_y.saturating_sub(half_tl)).saturating_sub(py), fg);
            if x_s_light + t_light < cell_right && y_s_light >= py + t_light {
                fb.fill_span(x_s_light + t_light, y_s_light.saturating_sub(t_light), t_light, t_light, fg);
            }
            true
        }

        // Diagonals
        '╱' => {
            for i in 0..cell_w {
                let x = px + i;
                let frac = (cell_w.saturating_sub(1).saturating_sub(i)) as f32 / (cell_w.max(2) - 1) as f32;
                let y = py + (frac * (cell_h.saturating_sub(1)) as f32).round() as u32;
                fb.fill_span(x, y, 1, t_light, fg);
            }
            true
        }
        '╲' => {
            for i in 0..cell_w {
                let x = px + i;
                let frac = i as f32 / (cell_w.max(2) - 1) as f32;
                let y = py + (frac * (cell_h.saturating_sub(1)) as f32).round() as u32;
                fb.fill_span(x, y, 1, t_light, fg);
            }
            true
        }
        '╳' => {
            for i in 0..cell_w {
                let x = px + i;
                let frac1 = (cell_w.saturating_sub(1).saturating_sub(i)) as f32 / (cell_w.max(2) - 1) as f32;
                let y1 = py + (frac1 * (cell_h.saturating_sub(1)) as f32).round() as u32;
                fb.fill_span(x, y1, 1, t_light, fg);
                let frac2 = i as f32 / (cell_w.max(2) - 1) as f32;
                let y2 = py + (frac2 * (cell_h.saturating_sub(1)) as f32).round() as u32;
                fb.fill_span(x, y2, 1, t_light, fg);
            }
            true
        }

        // Dashes: Horizontal
        '┄' | '┅' => {
            let t = if c == '┄' { t_light } else { t_heavy };
            let y = if c == '┄' { y_s_light } else { mid_y.saturating_sub(t / 2) };
            let dash_w = (cell_w / 5).max(1);
            fb.fill_span(px, y, dash_w, t, fg);
            fb.fill_span(px + (cell_w.saturating_sub(dash_w)) / 2, y, dash_w, t, fg);
            fb.fill_span(px + cell_w.saturating_sub(dash_w), y, dash_w, t, fg);
            true
        }
        '┈' | '┉' => {
            let t = if c == '┈' { t_light } else { t_heavy };
            let y = if c == '┈' { y_s_light } else { mid_y.saturating_sub(t / 2) };
            let dash_w = (cell_w / 7).max(1);
            for k in 0..4 {
                let dx = px + (k * cell_w.saturating_sub(dash_w)) / 3;
                fb.fill_span(dx, y, dash_w, t, fg);
            }
            true
        }
        '╌' | '╍' => {
            let t = if c == '╌' { t_light } else { t_heavy };
            let y = if c == '╌' { y_s_light } else { mid_y.saturating_sub(t / 2) };
            let dash_w = (cell_w / 3).max(1);
            fb.fill_span(px, y, dash_w, t, fg);
            fb.fill_span(px + cell_w.saturating_sub(dash_w), y, dash_w, t, fg);
            true
        }

        // Dashes: Vertical
        '┆' | '┇' => {
            let t = if c == '┆' { t_light } else { t_heavy };
            let x = if c == '┆' { x_s_light } else { mid_x.saturating_sub(t / 2) };
            let dash_h = (cell_h / 5).max(1);
            fb.fill_span(x, py, t, dash_h, fg);
            fb.fill_span(x, py + (cell_h.saturating_sub(dash_h)) / 2, t, dash_h, fg);
            fb.fill_span(x, py + cell_h.saturating_sub(dash_h), t, dash_h, fg);
            true
        }
        '┊' | '┋' => {
            let t = if c == '┊' { t_light } else { t_heavy };
            let x = if c == '┊' { x_s_light } else { mid_x.saturating_sub(t / 2) };
            let dash_h = (cell_h / 7).max(1);
            for k in 0..4 {
                let dy = py + (k * cell_h.saturating_sub(dash_h)) / 3;
                fb.fill_span(x, dy, t, dash_h, fg);
            }
            true
        }
        '╎' | '╏' => {
            let t = if c == '╎' { t_light } else { t_heavy };
            let x = if c == '╎' { x_s_light } else { mid_x.saturating_sub(t / 2) };
            let dash_h = (cell_h / 3).max(1);
            fb.fill_span(x, py, t, dash_h, fg);
            fb.fill_span(x, py + cell_h.saturating_sub(dash_h), t, dash_h, fg);
            true
        }

        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_all_double_line_table_chars() {
        let mut fb = Framebuffer::new(50, 50);
        let double_chars = [
            '═', '║', '╔', '╗', '╚', '╝', '╠', '╣', '╦', '╩', '╬',
            '╒', '╓', '╕', '╖', '╘', '╙', '╛', '╜', '╞', '╟', '╡', '╢', '╤', '╥', '╧', '╨', '╪', '╫',
        ];

        for &c in &double_chars {
            fb.clear(0);
            let handled = try_render_primitive(c, 5, 5, 10, 20, 0xFFFFFFFF, &mut fb);
            assert!(handled, "Character {} should be handled as primitive", c);
            assert!(fb.pixels.contains(&0xFFFFFFFF), "Character {} must draw pixels", c);
        }
    }

    #[test]
    fn test_render_quadrant_blocks() {
        let mut fb = Framebuffer::new(50, 50);
        let quadrants = ['▖', '▗', '▘', '▙', '▚', '▛', '▜', '▝', '▞', '▟'];

        for &c in &quadrants {
            fb.clear(0);
            let handled = try_render_primitive(c, 0, 0, 10, 10, 0xFF00FF00, &mut fb);
            assert!(handled, "Quadrant {} should be handled", c);
            assert!(fb.pixels.contains(&0xFF00FF00));
        }
    }

    #[test]
    fn test_render_rounded_and_diagonals() {
        let mut fb = Framebuffer::new(50, 50);
        let special_box = ['╭', '╮', '╯', '╰', '╱', '╲', '╳', '┄', '┆', '┈', '┊', '╌', '╎'];

        for &c in &special_box {
            fb.clear(0);
            let handled = try_render_primitive(c, 2, 2, 12, 24, 0xFFFFFF00, &mut fb);
            assert!(handled, "Special box char {} should be handled", c);
            assert!(fb.pixels.contains(&0xFFFFFF00));
        }
    }
}
