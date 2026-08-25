use super::atlas::{GlyphAtlas, GlyphRef};
use crate::font::fallback::FallbackManager;
use crate::font::loader::{is_nerd_font_or_pua, is_powerline};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use std::collections::HashMap;

/// Reusable temporary scratch storage for decoding PNG emojis and rasterizing glyph outlines.
#[derive(Default)]
pub struct GlyphScratch {
    pub png_buf: Vec<u8>,
    pub color_pixels: Vec<u32>,
    pub alpha_pixels: Vec<u8>,
}

impl GlyphScratch {
    pub fn new() -> Self {
        Self {
            png_buf: Vec::with_capacity(32 * 1024),
            color_pixels: Vec::with_capacity(4096),
            alpha_pixels: Vec::with_capacity(4096),
        }
    }

    /// Clear scratch buffers and release excessive capacity if overgrown.
    pub fn clear_and_release(&mut self, max_capacity: usize) {
        if self.png_buf.capacity() > max_capacity {
            self.png_buf = Vec::with_capacity(32 * 1024);
        } else {
            self.png_buf.clear();
        }

        if self.color_pixels.capacity() > max_capacity / 4 {
            self.color_pixels = Vec::with_capacity(4096);
        } else {
            self.color_pixels.clear();
        }

        if self.alpha_pixels.capacity() > max_capacity {
            self.alpha_pixels = Vec::with_capacity(4096);
        } else {
            self.alpha_pixels.clear();
        }
    }
}

/// Cache key containing only properties that change the visual outline/bitmap of a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub codepoint: char,
    pub bold: bool,
    pub italic: bool,
    pub wide: bool,
}

impl GlyphKey {
    #[inline(always)]
    pub fn new(codepoint: char, bold: bool, italic: bool, wide: bool) -> Self {
        Self {
            codepoint,
            bold,
            italic,
            wide,
        }
    }

    #[inline(always)]
    pub fn ascii_index(self) -> Option<usize> {
        let cp = self.codepoint as u32;
        if cp < 128 {
            let style = (self.bold as usize) | ((self.italic as usize) << 1);
            Some((cp as usize) * 4 + style)
        } else {
            None
        }
    }
}

pub struct GlyphCache {
    pub font_set: crate::font::resolved::ResolvedFontSet,
    pub font: FontArc,
    pub font_bold: Option<FontArc>,
    pub font_italic: Option<FontArc>,
    pub font_bold_italic: Option<FontArc>,
    pub fallback_manager: FallbackManager,
    pub cell_width: u32,
    pub cell_height: u32,
    pub font_size: f32,
    pub font_scale_multiplier: f32,
    pub atlas: GlyphAtlas,
    pub scratch: GlyphScratch,
    /// Fast direct lookup table for ASCII 0..127 across 4 styles (regular, bold, italic, bold_italic)
    ascii_table: [Option<GlyphRef>; 512],
    /// Bounded LRU-style cache for Unicode and emoji
    unicode_table: HashMap<GlyphKey, GlyphRef>,
    max_unicode_entries: usize,
}

impl GlyphCache {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        font: FontArc,
        font_bold: Option<FontArc>,
        font_italic: Option<FontArc>,
        font_bold_italic: Option<FontArc>,
        fallback_manager: FallbackManager,
        cell_width: u32,
        cell_height: u32,
        font_size: f32,
        font_scale_multiplier: f32,
    ) -> Self {
        let db = crate::font::fallback::get_system_font_db();
        let font_set = crate::font::resolved::ResolvedFontSet::resolve(db, "Monospace");
        let mut cache = Self {
            font_set,
            font,
            font_bold,
            font_italic,
            font_bold_italic,
            fallback_manager,
            cell_width,
            cell_height,
            font_size,
            font_scale_multiplier,
            atlas: GlyphAtlas::new(),
            scratch: GlyphScratch::new(),
            ascii_table: [None; 512],
            unicode_table: HashMap::with_capacity(1024),
            max_unicode_entries: 4096,
        };

        cache.preload_common_glyphs();
        cache
    }

    pub fn from_font_family(font_family: &str, font_size: f32, font_scale_multiplier: f32) -> Self {
        let db = crate::font::fallback::get_system_font_db();
        let font_set = crate::font::resolved::ResolvedFontSet::resolve(db, font_family);
        let font = font_set.regular.font.clone();
        let font_bold = if !font_set.bold.synthetic_bold {
            Some(font_set.bold.font.clone())
        } else {
            None
        };
        let font_italic = if !font_set.italic.synthetic_italic {
            Some(font_set.italic.font.clone())
        } else {
            None
        };
        let font_bold_italic = if !font_set.bold_italic.synthetic_italic && !font_set.bold_italic.synthetic_bold {
            Some(font_set.bold_italic.font.clone())
        } else {
            None
        };

        let fallback_manager = FallbackManager::with_shared_database(std::sync::Arc::clone(db));

        let px_size = (font_size * font_scale_multiplier).round().max(1.0);
        let scale = PxScale::from(px_size);
        let scaled_font = font.as_scaled(scale);
        let cell_width = scaled_font.h_advance(font.glyph_id('A')).ceil().max(1.0) as u32;
        let cell_height = (scaled_font.ascent() - scaled_font.descent()
            + scaled_font.line_gap().max(0.0))
        .ceil()
        .max(1.0) as u32;

        let mut cache = Self {
            font_set,
            font,
            font_bold,
            font_italic,
            font_bold_italic,
            fallback_manager,
            cell_width,
            cell_height,
            font_size,
            font_scale_multiplier,
            atlas: GlyphAtlas::new(),
            scratch: GlyphScratch::new(),
            ascii_table: [None; 512],
            unicode_table: HashMap::with_capacity(1024),
            max_unicode_entries: 4096,
        };
        cache.preload_common_glyphs();
        cache
    }

    /// Create an optimized, lightweight GlyphCache for tab bar text.
    /// Reuses already loaded FontArc handles and uses compact atlas and table capacities.
    pub fn create_tab_cache(&self, tab_font_size: f32) -> Self {
        let px_size = (tab_font_size * self.font_scale_multiplier)
            .round()
            .max(1.0);
        let scale = PxScale::from(px_size);
        let scaled_font = self.font.as_scaled(scale);
        let cell_width = scaled_font
            .h_advance(self.font.glyph_id('A'))
            .ceil()
            .max(1.0) as u32;
        let cell_height = (scaled_font.ascent() - scaled_font.descent()
            + scaled_font.line_gap().max(0.0))
        .ceil()
        .max(1.0) as u32;

        let mut cache = Self {
            font_set: self.font_set.clone(),
            font: self.font.clone(),
            font_bold: None,
            font_italic: None,
            font_bold_italic: None,
            fallback_manager: FallbackManager::new(),
            cell_width,
            cell_height,
            font_size: tab_font_size,
            font_scale_multiplier: self.font_scale_multiplier,
            atlas: GlyphAtlas::with_capacity(16 * 1024, 0),
            scratch: GlyphScratch::new(),
            ascii_table: [None; 512],
            unicode_table: HashMap::with_capacity(16),
            max_unicode_entries: 32,
        };
        cache.preload_tab_glyphs();
        cache
    }

    /// Preload printable ASCII characters and common tab symbols in regular style only.
    pub fn preload_tab_glyphs(&mut self) {
        for c in 32u8..=126u8 {
            let ch = c as char;
            self.get_or_rasterize(GlyphKey::new(ch, false, false, false));
        }
        self.get_or_rasterize(GlyphKey::new('…', false, false, false));
        self.get_or_rasterize(GlyphKey::new('×', false, false, false));
        self.get_or_rasterize(GlyphKey::new('+', false, false, false));
    }

    pub fn update_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        let px_size = (font_size * self.font_scale_multiplier).round().max(1.0);
        let scale = PxScale::from(px_size);
        let scaled_font = self.font.as_scaled(scale);
        self.cell_width = scaled_font
            .h_advance(self.font.glyph_id('A'))
            .ceil()
            .max(1.0) as u32;
        self.cell_height = (scaled_font.ascent() - scaled_font.descent()
            + scaled_font.line_gap().max(0.0))
        .ceil()
        .max(1.0) as u32;

        self.clear();
        self.preload_common_glyphs();
    }

    pub fn preload_common_glyphs(&mut self) {
        // Preload printable ASCII characters for regular, bold, and italic styles
        for c in 32u8..=126u8 {
            let ch = c as char;
            self.get_or_rasterize(GlyphKey::new(ch, false, false, false));
            self.get_or_rasterize(GlyphKey::new(ch, true, false, false));
            self.get_or_rasterize(GlyphKey::new(ch, false, true, false));
        }
    }

    /// Clear cached glyphs and atlas (retains memory capacity for immediate reuse).
    pub fn clear(&mut self) {
        self.atlas.clear();
        self.ascii_table = [None; 512];
        self.unicode_table.clear();
    }

    /// Full memory cleanup: releases oversized atlas buffers, prunes fallback fonts, and shrinks scratch.
    pub fn release_memory(&mut self) {
        self.atlas.clear_and_release();
        self.ascii_table = [None; 512];
        self.unicode_table.clear();
        self.fallback_manager.prune_unused(2);
        self.scratch.clear_and_release(64 * 1024);
        self.preload_common_glyphs();
    }

    #[inline(always)]
    pub fn get(&self, key: GlyphKey) -> Option<GlyphRef> {
        if let Some(idx) = key.ascii_index() {
            self.ascii_table[idx]
        } else {
            self.unicode_table.get(&key).copied()
        }
    }

    /// Get cached glyph or rasterize and store into atlas.
    pub fn get_or_rasterize(&mut self, key: GlyphKey) -> Option<GlyphRef> {
        if let Some(g_ref) = self.get(key) {
            return Some(g_ref);
        }

        let g_ref = self.rasterize_glyph(key)?;

        if let Some(idx) = key.ascii_index() {
            self.ascii_table[idx] = Some(g_ref);
        } else {
            if self.unicode_table.len() >= self.max_unicode_entries || self.atlas.is_full() {
                self.unicode_table.clear();
                self.atlas.clear();
                self.ascii_table = [None; 512];
                self.preload_common_glyphs();
                // Re-rasterize the requested key if it was flushed
                return self.get_or_rasterize(key);
            }
            self.unicode_table.insert(key, g_ref);
        }

        Some(g_ref)
    }

    fn rasterize_glyph(&mut self, key: GlyphKey) -> Option<GlyphRef> {
        let base_target_width = if key.wide {
            self.cell_width * 2
        } else {
            self.cell_width
        };

        // Try extracting embedded color PNG glyph if any (e.g., color emojis)
        let color_extract = self.extract_color_png(key.codepoint);

        let is_pw_sep = is_powerline(key.codepoint);
        let is_nerd_or_pua = is_nerd_font_or_pua(key.codepoint);
        let is_box = crate::font::loader::is_box_drawing_or_pipe(key.codepoint);

        let mut glyph_w = 0.0f32;
        let mut glyph_h = 0.0f32;
        let mut bounds_min_x = 0.0f32;
        let mut bounds_min_y = 0.0f32;
        let mut has_outline = false;
        let mut the_outlined: Option<ab_glyph::OutlinedGlyph> = None;
        let mut ascent = 0.0f32;

        let resolved_font = self.font_set.get(key.bold, key.italic);
        let mut char_font = &resolved_font.font;
        let mut is_synthetic_italic = resolved_font.synthetic_italic;

        if color_extract.is_none() {
            let mut char_glyph_id = char_font.glyph_id(key.codepoint);
            if char_glyph_id.0 == 0
                && let Some(idx) = self.fallback_manager.find_fallback_for_char(key.codepoint)
            {
                let fallback = &self.fallback_manager.fallbacks[idx];
                let id = fallback.font.glyph_id(key.codepoint);
                if id.0 != 0 {
                    char_font = &fallback.font;
                    char_glyph_id = id;
                    if key.italic {
                        is_synthetic_italic = true;
                    }
                }
            }

            if char_glyph_id.0 != 0 {
                let font_scale = (self.font_size * self.font_scale_multiplier)
                    .round()
                    .max(1.0);
                let scale: PxScale;

                if is_pw_sep {
                    let probe_scale = PxScale::from(font_scale);
                    let probe_glyph = char_glyph_id.with_scale(probe_scale);
                    if let Some(outlined) = char_font.outline_glyph(probe_glyph) {
                        let bounds = outlined.px_bounds();
                        let pw = bounds.width();
                        let ph = bounds.height();
                        if pw > 0.0 && ph > 0.0 {
                            let sw = base_target_width as f32 / pw;
                            let sh = self.cell_height as f32 / ph;
                            scale = PxScale::from(font_scale * sw.min(sh));
                        } else {
                            scale = PxScale::from(font_scale);
                        }
                    } else {
                        scale = PxScale::from(font_scale);
                    }
                } else if is_nerd_or_pua {
                    let probe_scale = PxScale::from(font_scale);
                    let probe_glyph = char_glyph_id.with_scale(probe_scale);
                    if let Some(outlined) = char_font.outline_glyph(probe_glyph) {
                        let bounds = outlined.px_bounds();
                        let ph = bounds.height();
                        if ph > self.cell_height as f32 && ph > 0.0 {
                            let sh = self.cell_height as f32 / ph;
                            scale = PxScale::from(font_scale * sh);
                        } else {
                            scale = PxScale::from(font_scale);
                        }
                    } else {
                        scale = PxScale::from(font_scale);
                    }
                } else {
                    scale = PxScale::from(font_scale);
                }

                let scaled_font = char_font.as_scaled(scale);
                ascent = scaled_font.ascent();

                let should_shear = is_synthetic_italic && !is_nerd_or_pua && !is_pw_sep && !is_box;
                let outlined_opt = crate::font::resolved::get_or_create_outlined_glyph(
                    char_font,
                    char_glyph_id,
                    scale,
                    should_shear,
                );

                if let Some(outlined) = outlined_opt {
                    let bounds = outlined.px_bounds();
                    glyph_w = bounds.width();
                    glyph_h = bounds.height();
                    bounds_min_x = bounds.min.x;
                    bounds_min_y = bounds.min.y;
                    has_outline = true;
                    the_outlined = Some(outlined);
                }
            }
        }

        let italic_bleed: u32 = if is_synthetic_italic && !is_nerd_or_pua && !is_pw_sep && !is_box {
            (ascent * crate::font::resolved::SYNTHETIC_ITALIC_SHEAR).ceil() as u32 + 2
        } else {
            0
        };

        let target_width: u32 = if is_nerd_or_pua && !is_pw_sep && glyph_w > 0.0 {
            (glyph_w.ceil() as u32)
                .max(base_target_width)
                .min(base_target_width * 2)
        } else {
            base_target_width + italic_bleed
        };

        let width_mult = if is_nerd_or_pua && !is_pw_sep && glyph_w > base_target_width as f32 {
            ((glyph_w / base_target_width as f32).ceil() as u8).clamp(1, 2)
        } else if key.wide {
            2
        } else {
            1
        };

        if let Some((w, h)) = color_extract {
            // Scale and convert RGBA to u32 ARGB (0x00RRGGBB) using scratch buffers
            let tw = target_width;
            let th = self.cell_height;
            let needed_u32 = (tw * th) as usize;
            self.scratch.color_pixels.clear();
            self.scratch.color_pixels.resize(needed_u32, 0);

            let scale_x = tw as f32 / w as f32;
            let scale_y = th as f32 / h as f32;
            let scale = scale_x.min(scale_y);

            let new_w = (w as f32 * scale).round() as u32;
            let new_h = (h as f32 * scale).round() as u32;
            let x_offset = (tw as i32 - new_w as i32) / 2;
            let y_offset = (th as i32 - new_h as i32) / 2;

            let rgba = &self.scratch.png_buf;
            for dy in 0..th {
                for dx in 0..tw {
                    let rx = dx as i32 - x_offset;
                    let ry = dy as i32 - y_offset;
                    if rx >= 0 && rx < new_w as i32 && ry >= 0 && ry < new_h as i32 {
                        let sx = ((rx as f32 / scale).floor() as u32).min(w - 1);
                        let sy = ((ry as f32 / scale).floor() as u32).min(h - 1);

                        let src_idx = (sy * w + sx) as usize * 4;
                        let dst_idx = (dy * tw + dx) as usize;

                        if src_idx + 3 < rgba.len() {
                            let r = rgba[src_idx] as u32;
                            let g = rgba[src_idx + 1] as u32;
                            let b = rgba[src_idx + 2] as u32;
                            let a = rgba[src_idx + 3] as u32;

                            if a > 0 {
                                self.scratch.color_pixels[dst_idx] =
                                    (a << 24) | (r << 16) | (g << 8) | b;
                            }
                        }
                    }
                }
            }

            Some(self.atlas.insert_color(
                target_width as u16,
                self.cell_height as u16,
                0,
                0,
                width_mult,
                &self.scratch.color_pixels,
            ))
        } else {
            let needed_alpha = (target_width * self.cell_height) as usize;
            self.scratch.alpha_pixels.clear();
            self.scratch.alpha_pixels.resize(needed_alpha, 0);

            if has_outline && let Some(outlined) = the_outlined {
                let (x_offset, y_offset) = if is_nerd_or_pua && !is_pw_sep {
                    let xo = (target_width as f32 - glyph_w) / 2.0;
                    let yo = (self.cell_height as f32 - glyph_h) / 2.0;
                    (xo, yo)
                } else if is_pw_sep {
                    let xo = (base_target_width as f32 - glyph_w) / 2.0;
                    let yo = (self.cell_height as f32 - glyph_h) / 2.0;
                    (xo, yo)
                } else if is_box {
                    (bounds_min_x, ascent + bounds_min_y)
                } else {
                    let xo = if glyph_w < base_target_width as f32 {
                        let calc = (base_target_width as f32 - glyph_w) / 2.0;
                        if calc > 0.0 { calc } else { bounds_min_x }
                    } else {
                        bounds_min_x
                    };
                    let yo = ascent + bounds_min_y;
                    (xo, yo)
                };

                let alpha_slice = &mut self.scratch.alpha_pixels;
                outlined.draw(|gx, gy, alpha| {
                    let px = (x_offset + gx as f32).round() as i32;
                    let py = (y_offset + gy as f32).round() as i32;

                    if px >= 0
                        && px < target_width as i32
                        && py >= 0
                        && py < self.cell_height as i32
                    {
                        let idx = py as usize * target_width as usize + px as usize;
                        let old_alpha = alpha_slice[idx] as f32 / 255.0;
                        let new_alpha = old_alpha.max(alpha);
                        alpha_slice[idx] = (new_alpha * 255.0) as u8;
                    }
                });
            }

            Some(self.atlas.insert_alpha(
                target_width as u16,
                self.cell_height as u16,
                0,
                0,
                width_mult,
                &self.scratch.alpha_pixels,
            ))
        }
    }

    fn extract_color_png(&mut self, c: char) -> Option<(u32, u32)> {
        let is_emoji_char = crate::font::loader::is_emoji(c);
        if (is_emoji_char || self.font.glyph_id(c).0 == 0)
            && let Some(idx) = self.fallback_manager.find_fallback_for_char(c)
        {
            let fallback = &self.fallback_manager.fallbacks[idx];
            let id = fallback.font.glyph_id(c);
            if id.0 != 0
                && let Ok(face) = owned_ttf_parser::Face::parse(fallback.storage.as_bytes(), 0)
            {
                let ttf_glyph_id = owned_ttf_parser::GlyphId(id.0);
                if let Some(img) = face.glyph_raster_image(ttf_glyph_id, self.cell_height as u16)
                    && img.format == owned_ttf_parser::RasterImageFormat::PNG
                {
                    let mut decoder = png::Decoder::new(std::io::Cursor::new(img.data));
                    decoder.set_transformations(png::Transformations::EXPAND);
                    if let Ok(mut reader) = decoder.read_info() {
                        let out_size = reader.output_buffer_size();
                        if self.scratch.png_buf.len() < out_size {
                            self.scratch.png_buf.resize(out_size, 0);
                        }
                        if let Ok(info) = reader.next_frame(&mut self.scratch.png_buf[..out_size]) {
                            let (output_color, _) = reader.output_color_type();
                            if output_color == png::ColorType::Rgba {
                                return Some((info.width, info.height));
                            }
                        }
                    }
                }
            }
        }
        None
    }
}
