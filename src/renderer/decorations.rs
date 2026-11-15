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
    let gap = 1.max(cell_h / 24);
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

#[allow(clippy::excessive_precision, clippy::approx_constant)]
#[rustfmt::skip]
const SINE_TABLE_256: [f32; 256] = [
    0.0000000, 0.0245412, 0.0490677, 0.0735646, 0.0980171, 0.1224107, 0.1467305, 0.1709619,
    0.1950903, 0.2191012, 0.2429802, 0.2667128, 0.2902847, 0.3136817, 0.3368899, 0.3598950,
    0.3826834, 0.4052413, 0.4275551, 0.4496113, 0.4713967, 0.4928982, 0.5141027, 0.5349976,
    0.5555702, 0.5758082, 0.5956993, 0.6152316, 0.6343933, 0.6531728, 0.6715590, 0.6895405,
    0.7071068, 0.7242471, 0.7409511, 0.7572088, 0.7730105, 0.7883464, 0.8032075, 0.8175848,
    0.8314696, 0.8448536, 0.8577286, 0.8700870, 0.8819213, 0.8932243, 0.9039893, 0.9142098,
    0.9238795, 0.9329928, 0.9415441, 0.9495282, 0.9569403, 0.9637761, 0.9700313, 0.9757021,
    0.9807853, 0.9852776, 0.9891765, 0.9924795, 0.9951847, 0.9972905, 0.9987955, 0.9996988,
    1.0000000, 0.9996988, 0.9987955, 0.9972905, 0.9951847, 0.9924795, 0.9891765, 0.9852776,
    0.9807853, 0.9757021, 0.9700313, 0.9637761, 0.9569403, 0.9495282, 0.9415441, 0.9329928,
    0.9238795, 0.9142098, 0.9039893, 0.8932243, 0.8819213, 0.8700870, 0.8577286, 0.8448536,
    0.8314696, 0.8175848, 0.8032075, 0.7883464, 0.7730105, 0.7572088, 0.7409511, 0.7242471,
    0.7071068, 0.6895405, 0.6715590, 0.6531728, 0.6343933, 0.6152316, 0.5956993, 0.5758082,
    0.5555702, 0.5349976, 0.5141027, 0.4928982, 0.4713967, 0.4496113, 0.4275551, 0.4052413,
    0.3826834, 0.3598950, 0.3368899, 0.3136817, 0.2902847, 0.2667128, 0.2429802, 0.2191012,
    0.1950903, 0.1709619, 0.1467305, 0.1224107, 0.0980171, 0.0735646, 0.0490677, 0.0245412,
    0.0000000, -0.0245412, -0.0490677, -0.0735646, -0.0980171, -0.1224107, -0.1467305, -0.1709619,
    -0.1950903, -0.2191012, -0.2429802, -0.2667128, -0.2902847, -0.3136817, -0.3368899, -0.3598950,
    -0.3826834, -0.4052413, -0.4275551, -0.4496113, -0.4713967, -0.4928982, -0.5141027, -0.5349976,
    -0.5555702, -0.5758082, -0.5956993, -0.6152316, -0.6343933, -0.6531728, -0.6715590, -0.6895405,
    -0.7071068, -0.7242471, -0.7409511, -0.7572088, -0.7730105, -0.7883464, -0.8032075, -0.8175848,
    -0.8314696, -0.8448536, -0.8577286, -0.8700870, -0.8819213, -0.8932243, -0.9039893, -0.9142098,
    -0.9238795, -0.9329928, -0.9415441, -0.9495282, -0.9569403, -0.9637761, -0.9700313, -0.9757021,
    -0.9807853, -0.9852776, -0.9891765, -0.9924795, -0.9951847, -0.9972905, -0.9987955, -0.9996988,
    -1.0000000, -0.9996988, -0.9987955, -0.9972905, -0.9951847, -0.9924795, -0.9891765, -0.9852776,
    -0.9807853, -0.9757021, -0.9700313, -0.9637761, -0.9569403, -0.9495282, -0.9415441, -0.9329928,
    -0.9238795, -0.9142098, -0.9039893, -0.8932243, -0.8819213, -0.8700870, -0.8577286, -0.8448536,
    -0.8314696, -0.8175848, -0.8032075, -0.7883464, -0.7730105, -0.7572088, -0.7409511, -0.7242471,
    -0.7071068, -0.6895405, -0.6715590, -0.6531728, -0.6343933, -0.6152316, -0.5956993, -0.5758082,
    -0.5555702, -0.5349976, -0.5141027, -0.4928982, -0.4713967, -0.4496113, -0.4275551, -0.4052413,
    -0.3826834, -0.3598950, -0.3368899, -0.3136817, -0.2902847, -0.2667128, -0.2429802, -0.2191012,
    -0.1950903, -0.1709619, -0.1467305, -0.1224107, -0.0980171, -0.0735646, -0.0490677, -0.0245412,
];

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
    if cell_w == 0 || cell_h == 0 {
        return;
    }
    let period = (cell_w as f32 * 0.75).clamp(6.0, 10.0);
    let amp = (cell_h as f32 * 0.05).clamp(1.0, 1.8);
    let thickness = 2.max(cell_h / 14);
    let base_y = (py as f32 + cell_h as f32 - amp - thickness as f32).max(0.0);
    let max_y_bound = (py + cell_h).saturating_sub(thickness) as i32;
    let fb_max_y = fb.height.saturating_sub(thickness);

    let inv_period = 1.0 / period;
    let mut phase = (px as f32 * inv_period).fract();
    if phase < 0.0 {
        phase += 1.0;
    }

    let mut run_start_x = 0u32;
    let mut run_y = 0u32;
    let mut run_len = 0u32;

    for x in 0..cell_w {
        let lut_idx = ((phase * 256.0).round() as usize) & 255;
        phase += inv_period;
        if phase >= 1.0 {
            phase -= 1.0;
        }
        let y_offset = (SINE_TABLE_256[lut_idx] * amp).round();
        let y = ((base_y + y_offset).round() as i32).clamp(py as i32, max_y_bound) as u32;
        let y = y.min(fb_max_y);

        if run_len == 0 {
            run_start_x = px + x;
            run_y = y;
            run_len = 1;
        } else if y == run_y {
            run_len += 1;
        } else {
            fb.fill_span(run_start_x, run_y, run_len, thickness, color);
            run_start_x = px + x;
            run_y = y;
            run_len = 1;
        }
    }

    if run_len > 0 {
        fb.fill_span(run_start_x, run_y, run_len, thickness, color);
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
        assert_eq!(
            y_drawn[1] - y_drawn[0],
            2,
            "There must be a 1px gap between the two underline bars"
        );
        assert_eq!(
            y_drawn[0], 20,
            "Double underline top line must start at y=20 matching curly underline descent separation"
        );
        assert_eq!(
            y_drawn[1], 22,
            "Double underline bottom line must sit at y=22"
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
        let mut drawn_pixels_per_col = vec![0; cell_w as usize];
        for y in 0..24 {
            for x in 0..cell_w {
                if fb.pixels[y as usize * fb.stride + x as usize] == color {
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                    drawn_pixels_per_col[x as usize] += 1;
                }
            }
        }
        assert!(max_y > min_y, "Curly underline must oscillate vertically");
        assert!(max_y - min_y >= 2, "Curly wave amplitude must be visible");
        assert!(
            min_y >= 19,
            "Curly underline must sit in descent area with separation from baseline"
        );
        for (x, &count) in drawn_pixels_per_col.iter().enumerate() {
            assert_eq!(count, 2, "Column {} should have 2px thickness", x);
        }
    }
}
