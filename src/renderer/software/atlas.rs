/// High-performance memory storage for CPU glyph alpha masks and color bitmaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphRef {
    pub offset: u32,
    pub width: u16,
    pub height: u16,
    pub bearing_x: i16,
    pub bearing_y: i16,
    pub is_color: bool,
    pub width_mult: u8,
}

#[derive(Debug, Clone, Default)]
pub struct GlyphAtlas {
    /// Contiguous buffer of 8-bit alpha masks for monochrome glyphs
    pub alpha_pixels: Vec<u8>,
    /// Contiguous buffer of 32-bit `0x00RRGGBB` pixels for color glyphs / emojis
    pub color_pixels: Vec<u32>,
}

impl GlyphAtlas {
    pub fn new() -> Self {
        Self {
            alpha_pixels: Vec::with_capacity(64 * 1024), // 64 KB initial capacity
            color_pixels: Vec::with_capacity(16 * 1024), // 64 KB initial capacity
        }
    }

    pub fn clear(&mut self) {
        self.alpha_pixels.clear();
        self.color_pixels.clear();
    }

    /// Insert an 8-bit alpha mask into the contiguous atlas buffer.
    pub fn insert_alpha(
        &mut self,
        width: u16,
        height: u16,
        bearing_x: i16,
        bearing_y: i16,
        width_mult: u8,
        data: &[u8],
    ) -> GlyphRef {
        let offset = self.alpha_pixels.len() as u32;
        self.alpha_pixels.extend_from_slice(data);
        GlyphRef {
            offset,
            width,
            height,
            bearing_x,
            bearing_y,
            is_color: false,
            width_mult,
        }
    }

    /// Insert a 32-bit color bitmap (emoji) into the color atlas buffer.
    pub fn insert_color(
        &mut self,
        width: u16,
        height: u16,
        bearing_x: i16,
        bearing_y: i16,
        width_mult: u8,
        data: &[u32],
    ) -> GlyphRef {
        let offset = self.color_pixels.len() as u32;
        self.color_pixels.extend_from_slice(data);
        GlyphRef {
            offset,
            width,
            height,
            bearing_x,
            bearing_y,
            is_color: true,
            width_mult,
        }
    }

    #[inline(always)]
    pub fn get_alpha(&self, glyph: &GlyphRef) -> &[u8] {
        let start = glyph.offset as usize;
        let len = (glyph.width as usize) * (glyph.height as usize);
        &self.alpha_pixels[start..start + len]
    }

    #[inline(always)]
    pub fn get_color(&self, glyph: &GlyphRef) -> &[u32] {
        let start = glyph.offset as usize;
        let len = (glyph.width as usize) * (glyph.height as usize);
        &self.color_pixels[start..start + len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_atlas_insertion_and_retrieval() {
        let mut atlas = GlyphAtlas::new();
        let mask = vec![0xFF, 0x80, 0x40, 0x00];
        let g_ref = atlas.insert_alpha(2, 2, 0, 0, 1, &mask);
        assert_eq!(atlas.get_alpha(&g_ref), &mask[..]);

        let color_data = vec![0xFF0000, 0x00FF00, 0x0000FF, 0xFFFFFF];
        let color_ref = atlas.insert_color(2, 2, 0, 0, 1, &color_data);
        assert_eq!(atlas.get_color(&color_ref), &color_data[..]);
    }
}
