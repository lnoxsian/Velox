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

        for x in 0..max_x {
            let alpha = mask[src_row + x];
            if alpha == 0 {
                continue;
            }

            let dst_idx = dst_row + x;
            if alpha == 255 {
                pixels[dst_idx] = fg;
            } else {
                pixels[dst_idx] = blend_alpha(pixels[dst_idx], fg, alpha);
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

        for x in 0..max_x {
            let src_px = color_pixels[src_row + x];
            let alpha = (src_px >> 24) as u8;
            if alpha == 0 {
                continue;
            }

            let dst_idx = dst_row + x;
            if alpha == 255 {
                pixels[dst_idx] = src_px;
            } else {
                pixels[dst_idx] = blend_alpha(pixels[dst_idx], src_px, alpha);
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
