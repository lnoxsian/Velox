use super::framebuffer::Framebuffer;

/// High-performance integer alpha blending: `(src * alpha + dst * (255 - alpha)) >> 8`.
/// Operates on 32-bit `0xAARRGGBB` pixels with zero floating-point arithmetic.
#[inline(always)]
pub fn blend_alpha(dst: u32, src: u32, alpha: u8) -> u32 {
    if alpha == 0 {
        return dst;
    }
    if alpha == 255 {
        return src;
    }

    let a = alpha as u32;
    let inv_a = 255 - a;

    // Isolate Red and Blue channels in one register
    let dst_rb = dst & 0x00FF00FF;
    let src_rb = src & 0x00FF00FF;
    let res_rb = ((src_rb * a + dst_rb * inv_a) >> 8) & 0x00FF00FF;

    // Isolate Green channel
    let dst_g = dst & 0x0000FF00;
    let src_g = src & 0x0000FF00;
    let res_g = ((src_g * a + dst_g * inv_a) >> 8) & 0x0000FF00;

    // Isolate Alpha channel
    let dst_a = (dst >> 24) & 0xFF;
    let src_a = (src >> 24) & 0xFF;
    let res_a = ((src_a * a + dst_a * inv_a) >> 8) & 0xFF;

    (res_a << 24) | res_rb | res_g
}

/// Blit an 8-bit alpha mask onto the framebuffer tinted by `fg` color.
#[inline(always)]
pub fn blit_alpha_glyph(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    mask: &[u8],
    glyph_w: u16,
    glyph_h: u16,
    fg: u32,
) {
    let gw = glyph_w as usize;
    let gh = glyph_h as usize;
    let fb_w = fb.width as usize;
    let fb_h = fb.height as usize;
    let stride = fb.stride;

    if (px as usize) >= fb_w || (py as usize) >= fb_h {
        return;
    }

    let max_y = (gh).min(fb_h.saturating_sub(py as usize));
    let max_x = (gw).min(fb_w.saturating_sub(px as usize));

    let pixels = fb.as_mut_slice();

    for y in 0..max_y {
        let src_row = y * gw;
        let dst_row = ((py as usize) + y) * stride + (px as usize);

        let src_slice = &mask[src_row..src_row + max_x];
        let dst_slice = &mut pixels[dst_row..dst_row + max_x];

        for (dst_pix, &alpha) in dst_slice.iter_mut().zip(src_slice.iter()) {
            if alpha == 0 {
                continue;
            }
            if alpha == 255 {
                *dst_pix = fg;
            } else {
                *dst_pix = blend_alpha(*dst_pix, fg, alpha);
            }
        }
    }
}

/// Blit an 8-bit alpha mask onto the framebuffer tinted by `fg` color with target dimensions.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn blit_alpha_glyph_scaled(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    mask: &[u8],
    glyph_w: u16,
    glyph_h: u16,
    target_w: u16,
    target_h: u16,
    fg: u32,
) {
    if glyph_w == target_w && glyph_h == target_h {
        blit_alpha_glyph(fb, px, py, mask, glyph_w, glyph_h, fg);
        return;
    }
    let tw = target_w as usize;
    let th = target_h as usize;
    let gw = glyph_w as usize;
    let gh = glyph_h as usize;
    if tw == 0 || th == 0 || gw == 0 || gh == 0 {
        return;
    }
    let fb_w = fb.width as usize;
    let fb_h = fb.height as usize;
    let stride = fb.stride;

    if (px as usize) >= fb_w || (py as usize) >= fb_h {
        return;
    }

    let max_y = th.min(fb_h.saturating_sub(py as usize));
    let max_x = tw.min(fb_w.saturating_sub(px as usize));
    let pixels = fb.as_mut_slice();

    for y in 0..max_y {
        let gy = (y as f32 + 0.5) * (gh as f32 / th as f32) - 0.5;
        let gy0 = (gy.floor() as isize).clamp(0, gh as isize - 1) as usize;
        let gy1 = (gy0 + 1).min(gh - 1);
        let fy = (gy - gy.floor()).clamp(0.0, 1.0);

        let dst_row = ((py as usize) + y) * stride + (px as usize);
        let r0 = gy0 * gw;
        let r1 = gy1 * gw;

        let dst_slice = &mut pixels[dst_row..dst_row + max_x];

        for (x, dst_pix) in dst_slice.iter_mut().enumerate() {
            let gx = (x as f32 + 0.5) * (gw as f32 / tw as f32) - 0.5;
            let gx0 = (gx.floor() as isize).clamp(0, gw as isize - 1) as usize;
            let gx1 = (gx0 + 1).min(gw - 1);
            let fx = (gx - gx.floor()).clamp(0.0, 1.0);

            let a00 = mask[r0 + gx0] as f32;
            let a10 = mask[r0 + gx1] as f32;
            let a01 = mask[r1 + gx0] as f32;
            let a11 = mask[r1 + gx1] as f32;

            let top = a00 * (1.0 - fx) + a10 * fx;
            let bottom = a01 * (1.0 - fx) + a11 * fx;
            let alpha = (top * (1.0 - fy) + bottom * fy).round() as u8;

            if alpha == 0 {
                continue;
            }

            if alpha == 255 {
                *dst_pix = fg;
            } else {
                *dst_pix = blend_alpha(*dst_pix, fg, alpha);
            }
        }
    }
}

/// Blit a 32-bit ARGB color bitmap (emoji) onto the framebuffer.
#[inline(always)]
pub fn blit_color_glyph(
    fb: &mut Framebuffer,
    px: u32,
    py: u32,
    color_pixels: &[u32],
    glyph_w: u16,
    glyph_h: u16,
) {
    let gw = glyph_w as usize;
    let gh = glyph_h as usize;
    let fb_w = fb.width as usize;
    let fb_h = fb.height as usize;
    let stride = fb.stride;

    if (px as usize) >= fb_w || (py as usize) >= fb_h {
        return;
    }

    let max_y = (gh).min(fb_h.saturating_sub(py as usize));
    let max_x = (gw).min(fb_w.saturating_sub(px as usize));

    let pixels = fb.as_mut_slice();

    for y in 0..max_y {
        let src_row = y * gw;
        let dst_row = ((py as usize) + y) * stride + (px as usize);

        let src_slice = &color_pixels[src_row..src_row + max_x];
        let dst_slice = &mut pixels[dst_row..dst_row + max_x];

        for (dst_pix, &src_px) in dst_slice.iter_mut().zip(src_slice.iter()) {
            let alpha = (src_px >> 24) as u8;
            if alpha == 0 {
                continue;
            }
            if alpha == 255 {
                *dst_pix = src_px;
            } else {
                *dst_pix = blend_alpha(*dst_pix, src_px, alpha);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_alpha_fast_paths() {
        let dst = 0x80000000;
        let src = 0xFFFFFFFF;

        assert_eq!(blend_alpha(dst, src, 0), dst);
        assert_eq!(blend_alpha(dst, src, 255), src);

        let mid = blend_alpha(0x00000000, 0xFFFFFFFF, 128);
        let a = (mid >> 24) & 0xFF;
        let r = (mid >> 16) & 0xFF;
        let g = (mid >> 8) & 0xFF;
        let b = mid & 0xFF;
        assert!((127..=128).contains(&a));
        assert!((127..=128).contains(&r));
        assert!((127..=128).contains(&g));
        assert!((127..=128).contains(&b));
    }
}
