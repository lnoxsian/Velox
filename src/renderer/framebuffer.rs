/// A persistent, CPU-resident 32-bit linear pixel framebuffer (`0x00RRGGBB`).
#[derive(Debug, Clone)]
pub struct Framebuffer {
    pub pixels: Vec<u32>,
    pub width: u32,
    pub height: u32,
    pub stride: usize,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let len = (w as usize)
            .checked_mul(h as usize)
            .expect("framebuffer dimensions too large");
        Self {
            pixels: vec![0; len],
            width: w,
            height: h,
            stride: w as usize,
        }
    }

    /// Resize the persistent framebuffer if dimensions change. Returns `true` if dimensions changed.
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        let w = width.max(1);
        let h = height.max(1);
        if self.width == w && self.height == h {
            return false;
        }

        crate::diagnostics::ResizeStats::record_framebuffer_resize();

        let pixel_count = (w as usize)
            .checked_mul(h as usize)
            .expect("framebuffer dimensions are too large");

        if self.pixels.len() < pixel_count {
            crate::diagnostics::ResizeStats::record_framebuffer_growth();
            self.pixels.resize(pixel_count, 0);
        } else {
            self.pixels.truncate(pixel_count);
        }

        self.width = w;
        self.height = h;
        self.stride = w as usize;
        true
    }

    /// Explicitly release excess capacity during idle memory compaction.
    pub fn release_memory(&mut self) {
        if self.pixels.capacity() > self.pixels.len() {
            crate::diagnostics::ResizeStats::record_framebuffer_shrink();
            self.pixels.shrink_to_fit();
        }
    }

    #[inline(always)]
    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u32] {
        &self.pixels
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    /// Fill a contiguous rectangular span across one or more horizontal rows.
    #[inline(always)]
    pub fn fill_span(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        if x >= self.width || y >= self.height || w == 0 || h == 0 {
            return;
        }
        let x_end = (x + w).min(self.width) as usize;
        let y_end = (y + h).min(self.height) as usize;
        let x_start = x as usize;
        let stride = self.stride;

        if x_start == 0 && x_end == stride {
            let start = (y as usize) * stride;
            let end = y_end * stride;
            if end <= self.pixels.len() {
                self.pixels[start..end].fill(color);
            }
            return;
        }

        let total_needed = (y_end - 1) * stride + x_end;
        if total_needed > self.pixels.len() {
            return;
        }

        let span_w = x_end - x_start;
        let pixels = &mut self.pixels;

        match span_w {
            1 => {
                let mut offset = (y as usize) * stride + x_start;
                for _ in (y as usize)..y_end {
                    pixels[offset] = color;
                    offset += stride;
                }
            }
            2 => {
                let mut offset = (y as usize) * stride + x_start;
                for _ in (y as usize)..y_end {
                    pixels[offset] = color;
                    pixels[offset + 1] = color;
                    offset += stride;
                }
            }
            _ => {
                let mut offset = (y as usize) * stride + x_start;
                for _ in (y as usize)..y_end {
                    pixels[offset..offset + span_w].fill(color);
                    offset += stride;
                }
            }
        }
    }

    /// Optimized vertical row shift in memory using `copy_within`.
    pub fn scroll_region_up(
        &mut self,
        top_px: u32,
        bottom_px: u32,
        lines_px: u32,
        fill_color: u32,
    ) {
        if top_px >= bottom_px || lines_px == 0 || bottom_px > self.height {
            return;
        }

        let stride = self.stride;
        let dst_start = (top_px as usize) * stride;
        let src_start = ((top_px + lines_px).min(bottom_px) as usize) * stride;
        let src_end = (bottom_px as usize) * stride;

        if src_start < src_end {
            self.pixels.copy_within(src_start..src_end, dst_start);
        }

        // Fill newly exposed rows at the bottom
        let fill_start = (bottom_px.saturating_sub(lines_px).max(top_px) as usize) * stride;
        let fill_end = (bottom_px as usize) * stride;
        if fill_start < fill_end {
            self.pixels[fill_start..fill_end].fill(fill_color);
        }
    }

    /// Optimized vertical row shift down in memory using `copy_within`.
    pub fn scroll_region_down(
        &mut self,
        top_px: u32,
        bottom_px: u32,
        lines_px: u32,
        fill_color: u32,
    ) {
        if top_px >= bottom_px || lines_px == 0 || bottom_px > self.height {
            return;
        }

        let stride = self.stride;
        let src_start = (top_px as usize) * stride;
        let src_end = (bottom_px.saturating_sub(lines_px).max(top_px) as usize) * stride;
        let dst_start = ((top_px + lines_px).min(bottom_px) as usize) * stride;

        if src_start < src_end {
            self.pixels.copy_within(src_start..src_end, dst_start);
        }

        // Fill newly exposed rows at the top
        let fill_start = (top_px as usize) * stride;
        let fill_end = ((top_px + lines_px).min(bottom_px) as usize) * stride;
        if fill_start < fill_end {
            self.pixels[fill_start..fill_end].fill(fill_color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framebuffer_allocation_and_fill() {
        let mut fb = Framebuffer::new(10, 10);
        assert_eq!(fb.pixels.len(), 100);

        fb.fill_span(2, 2, 4, 3, 0xFF00FF);
        // Check (2,2) to (5,4)
        for y in 2..5 {
            for x in 2..6 {
                assert_eq!(fb.pixels[y * 10 + x], 0xFF00FF);
            }
        }
        // Outside should be 0
        assert_eq!(fb.pixels[0], 0);
        assert_eq!(fb.pixels[2 * 10 + 1], 0);
        assert_eq!(fb.pixels[2 * 10 + 6], 0);
    }

    #[test]
    fn test_framebuffer_scroll_up() {
        let mut fb = Framebuffer::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                fb.pixels[y * 4 + x] = (y as u32) + 1;
            }
        }
        // Scroll up by 1 line
        fb.scroll_region_up(0, 4, 1, 0x99);
        // Row 0 should now have row 1's content (2)
        assert_eq!(fb.pixels[0], 2);
        // Row 1 should have row 2's content (3)
        assert_eq!(fb.pixels[4], 3);
        // Row 2 should have row 3's content (4)
        assert_eq!(fb.pixels[8], 4);
        // Row 3 should have fill color (0x99)
        assert_eq!(fb.pixels[12], 0x99);
    }

    #[test]
    fn test_framebuffer_scroll_down() {
        let mut fb = Framebuffer::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                fb.pixels[y * 4 + x] = (y as u32) + 1;
            }
        }
        // Scroll down by 1 line
        fb.scroll_region_down(0, 4, 1, 0x99);
        // Row 0 should have fill color (0x99)
        assert_eq!(fb.pixels[0], 0x99);
        // Row 1 should have old row 0's content (1)
        assert_eq!(fb.pixels[4], 1);
        // Row 2 should have old row 1's content (2)
        assert_eq!(fb.pixels[8], 2);
        // Row 3 should have old row 2's content (3)
        assert_eq!(fb.pixels[12], 3);
    }
}
