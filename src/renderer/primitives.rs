use super::framebuffer::Framebuffer;
use super::raster::blend_alpha;

/// Direct CPU rasterization for Box Drawing and Block Element characters.
/// Returns `true` if the character was handled as a geometric primitive.
#[inline(always)]
pub fn try_render_primitive(
    c: char,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    fg: u32,
    fb: &mut Framebuffer,
) -> bool {
    let cp = c as u32;
    // Fast-exit for common ASCII: ordinary ASCII (< 0x7c or 0x7d..0x2500 or > 0x259f) exit immediately
    if (cp < 0x2500 || cp > 0x259f) && cp != 0x7c {
        return false;
    }

    match c {
        // Full block
        '█' => {
            fb.fill_span(px, py, cell_w, cell_h, fg);
            true
        }
        // Upper half block
        '▀' => {
            let h = cell_h / 2;
            fb.fill_span(px, py, cell_w, h, fg);
            true
        }
        // Lower half block
        '▄' => {
            let h = cell_h / 2;
            let y = py + cell_h.saturating_sub(h);
            fb.fill_span(px, y, cell_w, h, fg);
            true
        }
        // Left half block
        '▌' => {
            let w = cell_w / 2;
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

        // Lower eighths:   ▂ ▃ ▄ ▅ ▆ ▇
        '\u{2581}'..='\u{2587}' => {
            let frac = (c as u32 - 0x2580) as f32 / 8.0;
            let h = ((cell_h as f32 * frac).round() as u32).max(1);
            let y = py + cell_h.saturating_sub(h);
            fb.fill_span(px, y, cell_w, h, fg);
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
            if px >= fb.width || py >= fb.height || cell_w == 0 || cell_h == 0 {
                return true;
            }
            let max_w = cell_w.min(fb.width.saturating_sub(px));
            let max_h = cell_h.min(fb.height.saturating_sub(py));
            let stride = fb.stride;
            let pixels = fb.as_mut_slice();

            for dy in 0..max_h {
                let y = py + dy;
                let row_offset = (y as usize) * stride;
                for dx in 0..max_w {
                    let x = px + dx;
                    let set = if c == '▒' {
                        (x + y).is_multiple_of(2)
                    } else if c == '░' {
                        x.is_multiple_of(2) && y.is_multiple_of(2)
                    } else {
                        !(x.is_multiple_of(2) && y.is_multiple_of(2))
                    };
                    if set {
                        pixels[row_offset + x as usize] = fg;
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
    let t_heavy = if cell_w <= t_light {
        cell_w
    } else {
        (t_light * 2).clamp(t_light + 1, cell_w)
    };

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
                fb.fill_span(
                    x_d_left,
                    y_d_top,
                    cell_right.saturating_sub(x_d_left),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    y_d_top,
                    t_light,
                    cell_bottom.saturating_sub(y_d_top),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Double, LineStyle::None) => {
                // ╗
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_d_right + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_top,
                    t_light,
                    cell_bottom.saturating_sub(y_d_top),
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::None, LineStyle::None, LineStyle::Double) => {
                // ╚
                fb.fill_span(
                    x_d_left,
                    y_d_bot,
                    cell_right.saturating_sub(x_d_left),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_d_bot + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_top,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Double, LineStyle::None) => {
                // ╝
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_d_right + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_d_bot + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::None, LineStyle::Double) => {
                // ╠
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_top,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::Double, LineStyle::None) => {
                // ╣
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Double, LineStyle::Double) => {
                // ╦
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Double, LineStyle::Double) => {
                // ╩
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_top,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::Double, LineStyle::Double) => {
                // ╬ - Open double cross junction (corridors pass through without blocking '#')
                // Top-left
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
                // Top-right
                fb.fill_span(
                    x_d_right,
                    y_d_top,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
                // Bottom-left
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
                // Bottom-right
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
            }

            // Mixed single and double
            (LineStyle::None, LineStyle::Light, LineStyle::None, LineStyle::Double) => {
                // ╒
                fb.fill_span(
                    x_s_light,
                    y_d_top,
                    t_light,
                    cell_bottom.saturating_sub(y_d_top),
                    fg,
                );
                fb.fill_span(
                    x_s_light,
                    y_d_top,
                    cell_right.saturating_sub(x_s_light),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_s_light,
                    y_d_bot,
                    cell_right.saturating_sub(x_s_light),
                    t_light,
                    fg,
                );
            }
            (LineStyle::None, LineStyle::Double, LineStyle::None, LineStyle::Light) => {
                // ╓
                fb.fill_span(
                    x_d_left,
                    y_s_light,
                    cell_right.saturating_sub(x_d_left),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    y_s_light,
                    t_light,
                    cell_bottom.saturating_sub(y_s_light),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_s_light,
                    t_light,
                    cell_bottom.saturating_sub(y_s_light),
                    fg,
                );
            }
            (LineStyle::None, LineStyle::Light, LineStyle::Double, LineStyle::None) => {
                // ╕
                fb.fill_span(
                    x_s_light,
                    y_d_top,
                    t_light,
                    cell_bottom.saturating_sub(y_d_top),
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_s_light + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_s_light + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Light, LineStyle::None) => {
                // ╖
                fb.fill_span(
                    px,
                    y_s_light,
                    (x_d_right + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    y_s_light,
                    t_light,
                    cell_bottom.saturating_sub(y_s_light),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_s_light,
                    t_light,
                    cell_bottom.saturating_sub(y_s_light),
                    fg,
                );
            }
            (LineStyle::Light, LineStyle::None, LineStyle::None, LineStyle::Double) => {
                // ╘
                fb.fill_span(
                    x_s_light,
                    py,
                    t_light,
                    (y_d_bot + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_s_light,
                    y_d_top,
                    cell_right.saturating_sub(x_s_light),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_s_light,
                    y_d_bot,
                    cell_right.saturating_sub(x_s_light),
                    t_light,
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::None, LineStyle::None, LineStyle::Light) => {
                // ╙
                fb.fill_span(
                    x_d_left,
                    y_s_light,
                    cell_right.saturating_sub(x_d_left),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_s_light + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_s_light + t_light).saturating_sub(py),
                    fg,
                );
            }
            (LineStyle::Light, LineStyle::None, LineStyle::Double, LineStyle::None) => {
                // ╛
                fb.fill_span(
                    x_s_light,
                    py,
                    t_light,
                    (y_d_bot + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_s_light + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_s_light + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Light, LineStyle::None) => {
                // ╜
                fb.fill_span(
                    px,
                    y_s_light,
                    (x_d_right + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_s_light + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_s_light + t_light).saturating_sub(py),
                    fg,
                );
            }
            (LineStyle::Light, LineStyle::Light, LineStyle::None, LineStyle::Double) => {
                // ╞
                fb.fill_span(x_s_light, py, t_light, cell_h, fg);
                fb.fill_span(
                    x_s_light,
                    y_d_top,
                    cell_right.saturating_sub(x_s_light),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    x_s_light,
                    y_d_bot,
                    cell_right.saturating_sub(x_s_light),
                    t_light,
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::None, LineStyle::Light) => {
                // ╟
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
                fb.fill_span(
                    x_d_right,
                    y_s_light,
                    cell_right.saturating_sub(x_d_right),
                    t_light,
                    fg,
                );
            }
            (LineStyle::Light, LineStyle::Light, LineStyle::Double, LineStyle::None) => {
                // ╡
                fb.fill_span(x_s_light, py, t_light, cell_h, fg);
                fb.fill_span(
                    px,
                    y_d_top,
                    (x_s_light + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
                fb.fill_span(
                    px,
                    y_d_bot,
                    (x_s_light + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::Double, LineStyle::Light, LineStyle::None) => {
                // ╢
                fb.fill_span(x_d_left, py, t_light, cell_h, fg);
                fb.fill_span(x_d_right, py, t_light, cell_h, fg);
                fb.fill_span(
                    px,
                    y_s_light,
                    (x_d_left + t_light).saturating_sub(px),
                    t_light,
                    fg,
                );
            }
            (LineStyle::None, LineStyle::Light, LineStyle::Double, LineStyle::Double) => {
                // ╤
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(
                    x_s_light,
                    y_d_bot,
                    t_light,
                    cell_bottom.saturating_sub(y_d_bot),
                    fg,
                );
            }
            (LineStyle::None, LineStyle::Double, LineStyle::Light, LineStyle::Light) => {
                // ╥
                fb.fill_span(px, y_s_light, cell_w, t_light, fg);
                fb.fill_span(
                    x_d_left,
                    y_s_light,
                    t_light,
                    cell_bottom.saturating_sub(y_s_light),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    y_s_light,
                    t_light,
                    cell_bottom.saturating_sub(y_s_light),
                    fg,
                );
            }
            (LineStyle::Light, LineStyle::None, LineStyle::Double, LineStyle::Double) => {
                // ╧
                fb.fill_span(px, y_d_top, cell_w, t_light, fg);
                fb.fill_span(px, y_d_bot, cell_w, t_light, fg);
                fb.fill_span(
                    x_s_light,
                    py,
                    t_light,
                    (y_d_top + t_light).saturating_sub(py),
                    fg,
                );
            }
            (LineStyle::Double, LineStyle::None, LineStyle::Light, LineStyle::Light) => {
                // ╨
                fb.fill_span(px, y_s_light, cell_w, t_light, fg);
                fb.fill_span(
                    x_d_left,
                    py,
                    t_light,
                    (y_s_light + t_light).saturating_sub(py),
                    fg,
                );
                fb.fill_span(
                    x_d_right,
                    py,
                    t_light,
                    (y_s_light + t_light).saturating_sub(py),
                    fg,
                );
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
            let y_end =
                if down != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
                    mid_y + center_half + 1
                } else {
                    mid_y
                };
            fb.fill_span(x_s_light, py, t_light, y_end.saturating_sub(py), fg);
        }
        LineStyle::Heavy => {
            let y_end =
                if down != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
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
            let y_start =
                if up != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
                    mid_y.saturating_sub(center_half)
                } else {
                    mid_y
                };
            fb.fill_span(
                x_s_light,
                y_start,
                t_light,
                cell_bottom.saturating_sub(y_start),
                fg,
            );
        }
        LineStyle::Heavy => {
            let y_start =
                if up != LineStyle::None || left != LineStyle::None || right != LineStyle::None {
                    mid_y.saturating_sub(center_half)
                } else {
                    mid_y
                };
            fb.fill_span(
                x_s_heavy,
                y_start,
                t_heavy,
                cell_bottom.saturating_sub(y_start),
                fg,
            );
        }
        _ => {}
    }

    // Left arm
    match left {
        LineStyle::Light => {
            let x_end =
                if right != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
                    mid_x + center_half + 1
                } else {
                    mid_x
                };
            fb.fill_span(px, y_s_light, x_end.saturating_sub(px), t_light, fg);
        }
        LineStyle::Heavy => {
            let x_end =
                if right != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
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
            let x_start =
                if left != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
                    mid_x.saturating_sub(center_half)
                } else {
                    mid_x
                };
            fb.fill_span(
                x_start,
                y_s_light,
                cell_right.saturating_sub(x_start),
                t_light,
                fg,
            );
        }
        LineStyle::Heavy => {
            let x_start =
                if left != LineStyle::None || up != LineStyle::None || down != LineStyle::None {
                    mid_x.saturating_sub(center_half)
                } else {
                    mid_x
                };
            fb.fill_span(
                x_start,
                y_s_heavy,
                cell_right.saturating_sub(x_start),
                t_heavy,
                fg,
            );
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
    let t_heavy = if cell_w <= t_light {
        cell_w
    } else {
        (t_light * 2).clamp(t_light + 1, cell_w)
    };
    let half_tl = t_light / 2;
    let y_s_light = mid_y.saturating_sub(half_tl);
    let x_s_light = mid_x.saturating_sub(half_tl);
    let cell_right = px + cell_w;
    let cell_bottom = py + cell_h;

    match c {
        // Rounded corners
        '╭' => {
            fb.fill_span(
                mid_x + half_tl,
                y_s_light,
                cell_right.saturating_sub(mid_x + half_tl),
                t_light,
                fg,
            );
            fb.fill_span(
                x_s_light,
                mid_y + half_tl,
                t_light,
                cell_bottom.saturating_sub(mid_y + half_tl),
                fg,
            );
            if x_s_light + t_light < cell_right && y_s_light + t_light < cell_bottom {
                fb.fill_span(
                    x_s_light + t_light,
                    y_s_light + t_light,
                    t_light,
                    t_light,
                    fg,
                );
            }
            true
        }
        '╮' => {
            fb.fill_span(
                px,
                y_s_light,
                (mid_x.saturating_sub(half_tl)).saturating_sub(px),
                t_light,
                fg,
            );
            fb.fill_span(
                x_s_light,
                mid_y + half_tl,
                t_light,
                cell_bottom.saturating_sub(mid_y + half_tl),
                fg,
            );
            if x_s_light >= px + t_light && y_s_light + t_light < cell_bottom {
                fb.fill_span(
                    x_s_light.saturating_sub(t_light),
                    y_s_light + t_light,
                    t_light,
                    t_light,
                    fg,
                );
            }
            true
        }
        '╯' => {
            fb.fill_span(
                px,
                y_s_light,
                (mid_x.saturating_sub(half_tl)).saturating_sub(px),
                t_light,
                fg,
            );
            fb.fill_span(
                x_s_light,
                py,
                t_light,
                (mid_y.saturating_sub(half_tl)).saturating_sub(py),
                fg,
            );
            if x_s_light >= px + t_light && y_s_light >= py + t_light {
                fb.fill_span(
                    x_s_light.saturating_sub(t_light),
                    y_s_light.saturating_sub(t_light),
                    t_light,
                    t_light,
                    fg,
                );
            }
            true
        }
        '╰' => {
            fb.fill_span(
                mid_x + half_tl,
                y_s_light,
                cell_right.saturating_sub(mid_x + half_tl),
                t_light,
                fg,
            );
            fb.fill_span(
                x_s_light,
                py,
                t_light,
                (mid_y.saturating_sub(half_tl)).saturating_sub(py),
                fg,
            );
            if x_s_light + t_light < cell_right && y_s_light >= py + t_light {
                fb.fill_span(
                    x_s_light + t_light,
                    y_s_light.saturating_sub(t_light),
                    t_light,
                    t_light,
                    fg,
                );
            }
            true
        }

        // Diagonals
        '╱' => {
            draw_diagonal_line(px, py, cell_w, cell_h, t_light, DiagonalType::Slash, fg, fb);
            true
        }
        '╲' => {
            draw_diagonal_line(px, py, cell_w, cell_h, t_light, DiagonalType::Backslash, fg, fb);
            true
        }
        '╳' => {
            draw_diagonal_line(px, py, cell_w, cell_h, t_light, DiagonalType::Cross, fg, fb);
            true
        }

        // Dashes: Horizontal
        '┄' | '┅' => {
            let t = if c == '┄' { t_light } else { t_heavy };
            let y = if c == '┄' {
                y_s_light
            } else {
                mid_y.saturating_sub(t / 2)
            };
            let dash_w = (cell_w / 5).max(1);
            fb.fill_span(px, y, dash_w, t, fg);
            fb.fill_span(px + (cell_w.saturating_sub(dash_w)) / 2, y, dash_w, t, fg);
            fb.fill_span(px + cell_w.saturating_sub(dash_w), y, dash_w, t, fg);
            true
        }
        '┈' | '┉' => {
            let t = if c == '┈' { t_light } else { t_heavy };
            let y = if c == '┈' {
                y_s_light
            } else {
                mid_y.saturating_sub(t / 2)
            };
            let dash_w = (cell_w / 7).max(1);
            for k in 0..4 {
                let dx = px + (k * cell_w.saturating_sub(dash_w)) / 3;
                fb.fill_span(dx, y, dash_w, t, fg);
            }
            true
        }
        '╌' | '╍' => {
            let t = if c == '╌' { t_light } else { t_heavy };
            let y = if c == '╌' {
                y_s_light
            } else {
                mid_y.saturating_sub(t / 2)
            };
            let dash_w = (cell_w / 3).max(1);
            fb.fill_span(px, y, dash_w, t, fg);
            fb.fill_span(px + cell_w.saturating_sub(dash_w), y, dash_w, t, fg);
            true
        }

        // Dashes: Vertical
        '┆' | '┇' => {
            let t = if c == '┆' { t_light } else { t_heavy };
            let x = if c == '┆' {
                x_s_light
            } else {
                mid_x.saturating_sub(t / 2)
            };
            let dash_h = (cell_h / 5).max(1);
            fb.fill_span(x, py, t, dash_h, fg);
            fb.fill_span(x, py + (cell_h.saturating_sub(dash_h)) / 2, t, dash_h, fg);
            fb.fill_span(x, py + cell_h.saturating_sub(dash_h), t, dash_h, fg);
            true
        }
        '┊' | '┋' => {
            let t = if c == '┊' { t_light } else { t_heavy };
            let x = if c == '┊' {
                x_s_light
            } else {
                mid_x.saturating_sub(t / 2)
            };
            let dash_h = (cell_h / 7).max(1);
            for k in 0..4 {
                let dy = py + (k * cell_h.saturating_sub(dash_h)) / 3;
                fb.fill_span(x, dy, t, dash_h, fg);
            }
            true
        }
        '╎' | '╏' => {
            let t = if c == '╎' { t_light } else { t_heavy };
            let x = if c == '╎' {
                x_s_light
            } else {
                mid_x.saturating_sub(t / 2)
            };
            let dash_h = (cell_h / 3).max(1);
            fb.fill_span(x, py, t, dash_h, fg);
            fb.fill_span(x, py + cell_h.saturating_sub(dash_h), t, dash_h, fg);
            true
        }

        _ => false,
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum DiagonalType {
    Slash,     // '╱'
    Backslash, // '╲'
    Cross,     // '╳'
}

#[inline(always)]
fn draw_diagonal_line(
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    t_light: u32,
    diag_type: DiagonalType,
    fg: u32,
    fb: &mut Framebuffer,
) {
    if cell_w == 0 || cell_h == 0 {
        return;
    }
    if cell_w == 1 {
        fb.fill_span(px, py, 1, cell_h, fg);
        return;
    }
    if cell_h == 1 {
        fb.fill_span(px, py, cell_w, 1, fg);
        return;
    }

    let w = cell_w as f32;
    let h = cell_h as f32;
    let len = w.hypot(h);
    if len <= 0.0 {
        return;
    }
    let inv_len = 1.0 / len;

    // Stroke width scales smoothly with cell dimensions/zoom
    let stroke_w = (t_light as f32).max(1.0) * 1.5;
    let radius = stroke_w / 2.0;

    let max_x = (px + cell_w).min(fb.width);
    let max_y = (py + cell_h).min(fb.height);
    let stride = fb.stride;
    let pixels = fb.as_mut_slice();

    for y in py..max_y {
        let dy = y - py;
        let y_rel = dy as f32 + 0.5;
        let row_offset = (y as usize) * stride;

        for x in px..max_x {
            let dx = x - px;
            let x_rel = dx as f32 + 0.5;

            let dist = match diag_type {
                DiagonalType::Slash => (h * x_rel + w * y_rel - w * h).abs() * inv_len,
                DiagonalType::Backslash => (h * x_rel - w * y_rel).abs() * inv_len,
                DiagonalType::Cross => {
                    let d1 = (h * x_rel + w * y_rel - w * h).abs() * inv_len;
                    let d2 = (h * x_rel - w * y_rel).abs() * inv_len;
                    d1.min(d2)
                }
            };

            if dist < radius + 0.5 {
                let cov = (radius + 0.5 - dist).min(1.0);
                let alpha = (cov * 255.0).round() as u8;
                if alpha > 0 {
                    let idx = row_offset + (x as usize);
                    pixels[idx] = blend_alpha(pixels[idx], fg, alpha);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_all_double_line_table_chars() {
        let mut fb = Framebuffer::new(50, 50);
        let double_chars = [
            '═', '║', '╔', '╗', '╚', '╝', '╠', '╣', '╦', '╩', '╬', '╒', '╓', '╕', '╖', '╘', '╙',
            '╛', '╜', '╞', '╟', '╡', '╢', '╤', '╥', '╧', '╨', '╪', '╫',
        ];

        for &c in &double_chars {
            fb.clear(0);
            let handled = try_render_primitive(c, 5, 5, 10, 20, 0xFFFFFFFF, &mut fb);
            assert!(handled, "Character {} should be handled as primitive", c);
            assert!(
                fb.pixels.contains(&0xFFFFFFFF),
                "Character {} must draw pixels",
                c
            );
        }
    }

    #[test]
    fn test_render_double_cross_open_junction() {
        let cell_w = 20;
        let cell_h = 30;
        let mut fb = Framebuffer::new(cell_w, cell_h);
        fb.clear(0);
        let handled = try_render_primitive('╬', 0, 0, cell_w, cell_h, 0xFFFFFFFF, &mut fb);
        assert!(handled);

        // Center junction between double arms must not be filled (corridor open)
        let mid_x = cell_w / 2;
        let mid_y = cell_h / 2;
        let center_pixel = fb.pixels[(mid_y * cell_w + mid_x) as usize];
        assert_eq!(
            center_pixel, 0,
            "Center crossroad of ╬ must be open and not blocked by hashtag grid"
        );
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
        let special_box = [
            '╭', '╮', '╯', '╰', '╱', '╲', '╳', '┄', '┆', '┈', '┊', '╌', '╎',
        ];

        for &c in &special_box {
            fb.clear(0);
            let handled = try_render_primitive(c, 2, 2, 12, 24, 0xFFFFFF00, &mut fb);
            assert!(handled, "Special box char {} should be handled", c);
            assert!(fb.pixels.contains(&0xFFFFFF00));
        }
    }

    #[test]
    fn test_diagonal_lines_continuity_and_connectivity() {
        let px = 5;
        let py = 5;
        let cell_w = 10;
        let cell_h = 24;
        let fg = 0xFF00FF00;
        let mut fb = Framebuffer::new(30, 40);

        for &c in &['╱', '╲', '╳'] {
            fb.clear(0);
            let handled = try_render_primitive(c, px, py, cell_w, cell_h, fg, &mut fb);
            assert!(handled, "Character {} must be handled", c);
            assert!(fb.pixels.contains(&fg), "Character {} must render full-color pixels", c);

            // 1. Every row within [py, py + cell_h) must have rendered pixels (no gaps / dotted appearance)
            for y in py..(py + cell_h) {
                let row_has_pixel = (px..(px + cell_w))
                    .any(|x| fb.pixels[(y as usize) * fb.stride + (x as usize)] != 0);
                assert!(
                    row_has_pixel,
                    "Character {} has empty row at y={}, causing dotted line appearance",
                    c, y
                );
            }

            // 2. Every column within [px, px + cell_w) must have rendered pixels
            for x in px..(px + cell_w) {
                let col_has_pixel = (py..(py + cell_h))
                    .any(|y| fb.pixels[(y as usize) * fb.stride + (x as usize)] != 0);
                assert!(
                    col_has_pixel,
                    "Character {} has empty col at x={}, causing gap",
                    c, x
                );
            }

            // 3. No pixels must be drawn outside the cell bounds
            for y in 0..fb.height {
                for x in 0..fb.width {
                    let pixel = fb.pixels[(y as usize) * fb.stride + (x as usize)];
                    if pixel != 0 {
                        assert!(
                            x >= px && x < px + cell_w && y >= py && y < py + cell_h,
                            "Character {} drew pixel outside cell at ({}, {})",
                            c, x, y
                        );
                    }
                }
            }

            // 4. Consecutive rows must be 8-connected (adjacent horizontally, vertically, or diagonally)
            if c == '╱' || c == '╲' {
                let mut prev_min_x: Option<u32> = None;
                let mut prev_max_x: Option<u32> = None;
                for y in py..(py + cell_h) {
                    let xs: Vec<u32> = (px..(px + cell_w))
                        .filter(|&x| fb.pixels[(y as usize) * fb.stride + (x as usize)] != 0)
                        .collect();
                    assert!(!xs.is_empty());
                    let min_x = *xs.first().unwrap();
                    let max_x = *xs.last().unwrap();
                    if let (Some(p_min), Some(p_max)) = (prev_min_x, prev_max_x) {
                        let connected = min_x <= p_max + 1 && p_min <= max_x + 1;
                        assert!(
                            connected,
                            "Character {} has disconnected step between row {} and {}: [{}, {}] vs [{}, {}]",
                            c, y - 1, y, p_min, p_max, min_x, max_x
                        );
                    }
                    prev_min_x = Some(min_x);
                    prev_max_x = Some(max_x);
                }
            }
        }
    }

    #[test]
    fn test_diagonal_lines_edge_cases_and_orientations() {
        let fg = 0xFFFFFFFF;
        let test_cases = [
            (25, 10), // wide cell (cell_w > cell_h)
            (16, 16), // square cell (cell_w == cell_h)
            (1, 20),  // 1px wide
            (20, 1),  // 1px high
            (1, 1),   // 1x1
            (30, 60), // large cell with thickness > 1
        ];

        for (cw, ch) in test_cases {
            let mut fb = Framebuffer::new(cw + 10, ch + 10);
            for &c in &['╱', '╲', '╳'] {
                fb.clear(0);
                let handled = try_render_primitive(c, 5, 5, cw, ch, fg, &mut fb);
                assert!(handled);
                assert!(fb.pixels.contains(&fg));

                // Every row must have at least one pixel when ch > 0
                for y in 5..(5 + ch) {
                    let has_pixel = (5..(5 + cw))
                        .any(|x| fb.pixels[(y as usize) * fb.stride + (x as usize)] != 0);
                    assert!(has_pixel, "Char {} at size {}x{} has empty row at y={}", c, cw, ch, y);
                }

                // Every col must have at least one pixel when cw > 0
                for x in 5..(5 + cw) {
                    let has_pixel = (5..(5 + ch))
                        .any(|y| fb.pixels[(y as usize) * fb.stride + (x as usize)] != 0);
                    assert!(has_pixel, "Char {} at size {}x{} has empty col at x={}", c, cw, ch, x);
                }
            }
        }
    }

    #[test]
    fn test_diagonal_collinear_across_cells_and_centered_cross() {
        let cw = 12;
        let ch = 28;
        let fg = 0xFFFFFFFF;
        let mut fb = Framebuffer::new(cw * 2, ch * 2);

        // 1. Test seamless straight diagonal line across two cells
        try_render_primitive('╲', 0, 0, cw, ch, fg, &mut fb);
        try_render_primitive('╲', cw, ch, cw, ch, fg, &mut fb);

        // The transition between cell (0,0) and cell (1,1):
        // Cell 0 bottom row (ch - 1) and Cell 1 top row (ch)
        let row_0_last: Vec<u32> = (0..cw)
            .filter(|&x| fb.pixels[((ch - 1) as usize) * fb.stride + (x as usize)] != 0)
            .collect();
        let row_1_first: Vec<u32> = (cw..(2 * cw))
            .filter(|&x| fb.pixels[(ch as usize) * fb.stride + (x as usize)] != 0)
            .collect();

        assert!(!row_0_last.is_empty());
        assert!(!row_1_first.is_empty());

        // Max x of row (ch-1) and min x of row (ch) must be seamlessly adjacent
        let max_x_row_prev = *row_0_last.last().unwrap();
        let min_x_row_next = *row_1_first.first().unwrap();
        assert_eq!(
            max_x_row_prev,
            cw - 1,
            "Diagonal must reach the exact right boundary of cell 0"
        );
        assert_eq!(
            min_x_row_next, cw,
            "Diagonal must continue seamlessly at exact left boundary of cell 1"
        );

        // 2. Test centered symmetry for Cross '╳'
        fb.clear(0);
        try_render_primitive('╳', 0, 0, cw, ch, fg, &mut fb);

        // For every row y, the pixels must have mirror symmetry around the center line (cw / 2.0)
        let mid_x = (cw as f32) / 2.0;
        for dy in 0..ch {
            let pixels_in_row: Vec<u32> = (0..cw)
                .filter(|&x| fb.pixels[(dy as usize) * fb.stride + (x as usize)] != 0)
                .collect();
            assert!(!pixels_in_row.is_empty());
            let min_x = *pixels_in_row.first().unwrap() as f32 + 0.5;
            let max_x = *pixels_in_row.last().unwrap() as f32 + 0.5;
            let center_est = (min_x + max_x) / 2.0;
            assert!(
                (center_est - mid_x).abs() <= 0.5,
                "Cross '╳' must be centered at row dy={}, expected mid {}, got {}",
                dy,
                mid_x,
                center_est
            );
        }
    }
}
