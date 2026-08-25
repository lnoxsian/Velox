use super::framebuffer::Framebuffer;
use crate::screen::cursor::CursorShape;

/// Render single underline decoration.
#[inline(always)]
pub fn draw_underline(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    color: u32,
) {
    let thickness = 1.max(cell_h / 24);
    let y = py + cell_h.saturating_sub(thickness + 1);
    fb.fill_span(px, y, cell_w, thickness, color);
}

/// Render double underline decoration.
#[inline(always)]
pub fn draw_double_underline(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    color: u32,
) {
    let thickness = 1.max(cell_h / 32);
    let gap = 1.max(cell_h / 24) + 1;
    let y2 = py + cell_h.saturating_sub(thickness + 1);
    let y1 = y2.saturating_sub(gap + thickness);
    fb.fill_span(px, y1, cell_w, thickness, color);
    fb.fill_span(px, y2, cell_w, thickness, color);
}

/// Render strikethrough decoration.
#[inline(always)]
pub fn draw_strike(fb: &mut Framebuffer, px: u32, py: u32, cell_w: u32, cell_h: u32, color: u32) {
    let thickness = (cell_h / 12).max(1);
    let y = py + cell_h / 2;
    fb.fill_span(px, y, cell_w, thickness, color);
}

/// Render curly / wave underline decoration with smooth continuous sinusoidal wave across columns.
#[inline(always)]
pub fn draw_curly_underline(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    color: u32,
) {
    let period = (cell_w as f32 * 0.75).clamp(6.0, 10.0);
    let amp = (cell_h as f32 * 0.08).clamp(1.5, 2.5);
    let thickness = 1.max(cell_h / 28);
    let base_y = (py + cell_h).saturating_sub((amp + thickness as f32 + 1.0) as u32);

    for x in 0..cell_w {
        let global_x = (px + x) as f32;
        let angle = (global_x / period) * std::f32::consts::TAU;
        let y_offset = (angle.sin() * amp).round() as i32;
        let y = ((base_y as i32 + y_offset).max(0) as u32).min(fb.height.saturating_sub(thickness));
        fb.fill_span(px + x, y, 1, thickness, color);
    }
}

/// Render cursor shape directly onto the framebuffer.
#[allow(clippy::too_many_arguments)]
pub fn draw_cursor(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    shape: CursorShape,
    is_focused: bool,
    color: u32,
) {
    if !is_focused && (shape == CursorShape::Block || shape == CursorShape::HollowBlock) {
        // Unfocused windows display a hollow rectangular border for block cursor
        draw_hollow_block(fb, px, py, cell_w, cell_h, color);
        return;
    }

    match shape {
        CursorShape::Block => {
            fb.fill_span(px, py, cell_w, cell_h, color);
        }
        CursorShape::HollowBlock => {
            draw_hollow_block(fb, px, py, cell_w, cell_h, color);
        }
        CursorShape::Beam => {
            let beam_w = (cell_w / 6).max(2);
            fb.fill_span(px, py, beam_w, cell_h, color);
        }
        CursorShape::Underline => {
            let underline_h = (cell_h / 6).max(2);
            let y = py + cell_h.saturating_sub(underline_h);
            fb.fill_span(px, y, cell_w, underline_h, color);
        }
    }
}

#[inline(always)]
fn draw_hollow_block(fb: &mut Framebuffer, px: u32, py: u32, cell_w: u32, cell_h: u32, color: u32) {
    let t = (cell_w / 12).max(1);
    // Top border
    fb.fill_span(px, py, cell_w, t, color);
    // Bottom border
    fb.fill_span(px, py + cell_h.saturating_sub(t), cell_w, t, color);
    // Left border
    fb.fill_span(px, py, t, cell_h, color);
    // Right border
    fb.fill_span(px + cell_w.saturating_sub(t), py, t, cell_h, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unfocused_cursor_rendering() {
        let mut fb = Framebuffer::new(40, 40);
        let cursor_color = 0xFFFFFFFF;

        let cell_w = 12;
        let cell_h = 24;
        // Beam when unfocused should render a vertical beam, NOT a hollow block
        draw_cursor(
            &mut fb,
            0,
            0,
            cell_w,
            cell_h,
            CursorShape::Beam,
            false,
            cursor_color,
        );
        let beam_w = (cell_w / 6).max(2);
        // Left column within beam_w should be filled
        for y in 0..cell_h {
            for x in 0..beam_w {
                assert_eq!(fb.pixels[y as usize * fb.stride + x as usize], cursor_color);
            }
        }
        // Right side should NOT have hollow block border
        assert_eq!(fb.pixels[10 * fb.stride + 11], 0);

        // Underline when unfocused should render an underline, NOT a hollow block
        let mut fb_under = Framebuffer::new(40, 40);
        draw_cursor(
            &mut fb_under,
            0,
            0,
            cell_w,
            cell_h,
            CursorShape::Underline,
            false,
            cursor_color,
        );
        let underline_h = (cell_h / 6).max(2);
        let under_y = cell_h - underline_h;
        for x in 0..cell_w {
            assert_eq!(
                fb_under.pixels[under_y as usize * fb_under.stride + x as usize],
                cursor_color
            );
        }
        // Top border should NOT be drawn
        assert_eq!(fb_under.pixels[5], 0);

        // Block when unfocused should render hollow block
        let mut fb_block = Framebuffer::new(40, 40);
        draw_cursor(
            &mut fb_block,
            0,
            0,
            cell_w,
            cell_h,
            CursorShape::Block,
            false,
            cursor_color,
        );
        // Top border and right border should be drawn
        assert_eq!(fb_block.pixels[5], cursor_color);
        assert_eq!(fb_block.pixels[10 * fb_block.stride + 11], cursor_color);
        // Center should be hollow (0)
        assert_eq!(fb_block.pixels[10 * fb_block.stride + 5], 0);
    }

    #[test]
    fn test_double_underline_thinner_and_separated() {
        let mut fb = Framebuffer::new(20, 24);
        let color = 0xFFFF0000;
        let cell_w = 10;
        let cell_h = 24;

        draw_double_underline(&mut fb, 0, 0, cell_w, cell_h, color);

        // Verify that there are two distinct lines separated by blank pixels
        let mut y_drawn = Vec::new();
        for y in 0..24 {
            if fb.pixels[y as usize * fb.stride] == color {
                y_drawn.push(y);
            }
        }
        assert_eq!(
            y_drawn.len(),
            2,
            "Double underline should draw exactly 2 single-pixel lines"
        );
        assert!(
            y_drawn[1] - y_drawn[0] >= 2,
            "There must be a gap between the two underline bars"
        );
    }

    #[test]
    fn test_curly_underline_continuous_wave() {
        let mut fb = Framebuffer::new(30, 24);
        let color = 0xFF00FF00;
        let cell_w = 10;
        let cell_h = 24;

        draw_curly_underline(&mut fb, 0, 0, cell_w, cell_h, color);

        let mut min_y = 24;
        let mut max_y = 0;
        for y in 0..24 {
            for x in 0..cell_w {
                if fb.pixels[y as usize * fb.stride + x as usize] == color {
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }
        assert!(max_y > min_y, "Curly underline must oscillate vertically");
        assert!(max_y - min_y >= 2, "Curly wave amplitude must be visible");
    }
}
