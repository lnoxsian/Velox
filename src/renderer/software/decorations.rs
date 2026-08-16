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
    let thickness = (cell_h / 12).max(1);
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
    let thickness = (cell_h / 14).max(1);
    let y1 = py + cell_h.saturating_sub(thickness * 3 + 1);
    let y2 = py + cell_h.saturating_sub(thickness + 1);
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

/// Render curly / wave underline decoration without runtime trigonometry.
#[inline(always)]
pub fn draw_curly_underline(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
    color: u32,
) {
    let base_y = py + cell_h.saturating_sub(3);
    for x in 0..cell_w {
        let cycle = (px + x) % 4;
        let y_offset = match cycle {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 1,
            _ => 0,
        };
        let y = (base_y + y_offset).min(fb.height.saturating_sub(1));
        fb.fill_span(px + x, y, 1, 1, color);
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
    if !is_focused {
        // Unfocused windows display a hollow rectangular border
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
