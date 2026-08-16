use super::atlas::{GlyphAtlas, GlyphRef};
use crate::font::fallback::FallbackManager;
use crate::font::loader::{is_nerd_font_or_pua, is_powerline};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use owned_ttf_parser::AsFaceRef;
use std::collections::HashMap;

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
    pub fn ascii_index(&self) -> Option<usize> {
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
        let mut cache = Self {
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
            ascii_table: [None; 512],
            unicode_table: HashMap::with_capacity(1024),
            max_unicode_entries: 4096,
        };

        cache.preload_common_glyphs();
        cache
    }

    pub fn from_font_family(font_family: &str, font_size: f32, font_scale_multiplier: f32) -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        let query = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };

        let font = crate::font::loader::load_font_face(&db, &query)
            .expect("Could not load any system monospace font");

        let query_bold = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let font_bold = crate::font::loader::load_font_face(&db, &query_bold);

        let regular_id = db.query(&query);

        let query_italic = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let italic_id = db.query(&query_italic);
        let font_italic = if italic_id.is_some() && italic_id != regular_id {
            crate::font::loader::load_font_face(&db, &query_italic)
        } else {
            None
        };

        let query_bold_id = db.query(&query_bold);
        let query_bold_italic = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let bold_italic_id = db.query(&query_bold_italic);
        let font_bold_italic = if bold_italic_id.is_some() && bold_italic_id != query_bold_id {
            crate::font::loader::load_font_face(&db, &query_bold_italic)
        } else {
            None
        };

        let fallback_manager = FallbackManager::with_database(db);

        let px_size = font_size * font_scale_multiplier;
        let scale = PxScale::from(px_size);
        let scaled_font = font.as_scaled(scale);
        let cell_width = scaled_font.h_advance(font.glyph_id('A')).ceil().max(1.0) as u32;
        let cell_height = (scaled_font.ascent() - scaled_font.descent()
            + scaled_font.line_gap().max(0.0))
        .ceil()
        .max(1.0) as u32;

        Self::new(
            font,
            font_bold,
            font_italic,
            font_bold_italic,
            fallback_manager,
            cell_width,
            cell_height,
            font_size,
            font_scale_multiplier,
        )
    }

    pub fn update_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        let px_size = font_size * self.font_scale_multiplier;
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

    /// Clear cached glyphs and atlas (e.g. on font size change).
    pub fn clear(&mut self) {
        self.atlas.clear();
        self.ascii_table = [None; 512];
        self.unicode_table.clear();
    }

    #[inline(always)]
    pub fn get(&self, key: &GlyphKey) -> Option<GlyphRef> {
        if let Some(idx) = key.ascii_index() {
            self.ascii_table[idx]
        } else {
            self.unicode_table.get(key).copied()
        }
    }

    /// Get cached glyph or rasterize and store into atlas.
    pub fn get_or_rasterize(&mut self, key: GlyphKey) -> Option<GlyphRef> {
        if let Some(g_ref) = self.get(&key) {
            return Some(g_ref);
        }

        let g_ref = self.rasterize_glyph(&key)?;

        if let Some(idx) = key.ascii_index() {
            self.ascii_table[idx] = Some(g_ref);
        } else {
            if self.unicode_table.len() >= self.max_unicode_entries {
                self.unicode_table.clear();
            }
            self.unicode_table.insert(key, g_ref);
        }

        Some(g_ref)
    }

    fn rasterize_glyph(&mut self, key: &GlyphKey) -> Option<GlyphRef> {
        let base_target_width = if key.wide {
            self.cell_width * 2
        } else {
            self.cell_width
        };

        // Try extracting embedded color PNG glyph if any (e.g., color emojis)
        let color_pixels = self.extract_color_png(key.codepoint);

        let is_pw_sep = is_powerline(key.codepoint);
        let is_nerd_or_pua = is_nerd_font_or_pua(key.codepoint);

        let mut glyph_w = 0.0f32;
        let mut glyph_h = 0.0f32;
        let mut bounds_min_x = 0.0f32;
        let mut bounds_min_y = 0.0f32;
        let mut has_outline = false;
        let mut char_font_arc: Option<FontArc> = None;
        let mut the_glyph: Option<ab_glyph::Glyph> = None;
        let mut is_synthetic_italic = false;
        let mut ascent = 0.0f32;

        if color_pixels.is_none() {
            let (mut char_font, synth) = match (key.bold, key.italic) {
                (true, true) => {
                    if let Some(ref f) = self.font_bold_italic {
                        (f, false)
                    } else if let Some(ref f) = self.font_bold {
                        (f, true)
                    } else if let Some(ref f) = self.font_italic {
                        (f, false)
                    } else {
                        (&self.font, true)
                    }
                }
                (true, false) => {
                    if let Some(ref f) = self.font_bold {
                        (f, false)
                    } else {
                        (&self.font, false)
                    }
                }
                (false, true) => {
                    if let Some(ref f) = self.font_italic {
                        (f, false)
                    } else {
                        (&self.font, true)
                    }
                }
                (false, false) => (&self.font, false),
            };
            is_synthetic_italic = synth;

            let mut char_glyph_id = char_font.glyph_id(key.codepoint);
            if char_glyph_id.0 == 0
                && let Some(idx) = self.fallback_manager.find_fallback_for_char(key.codepoint)
            {
                let fallback = &self.fallback_manager.fallbacks[idx];
                let id = fallback.font.glyph_id(key.codepoint);
                if id.0 != 0 {
                    char_font = &fallback.font;
                    char_glyph_id = id;
                }
            }

            if char_glyph_id.0 != 0 {
                let font_scale = self.font_size * self.font_scale_multiplier;
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

                let glyph = char_glyph_id.with_scale(scale);
                let scaled_font = char_font.as_scaled(scale);
                ascent = scaled_font.ascent();

                if let Some(outlined) = char_font.outline_glyph(glyph.clone()) {
                    let bounds = outlined.px_bounds();
                    glyph_w = bounds.width();
                    glyph_h = bounds.height();
                    bounds_min_x = bounds.min.x;
                    bounds_min_y = bounds.min.y;
                    has_outline = true;
                    char_font_arc = Some(char_font.clone());
                    the_glyph = Some(glyph);
                }
            }
        }

        const SHEAR_FACTOR: f32 = 0.22;
        let italic_bleed: u32 = if is_synthetic_italic && !is_nerd_or_pua && !is_pw_sep {
            (ascent * SHEAR_FACTOR).ceil() as u32
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

        if let Some((rgba, w, h)) = color_pixels {
            // Scale and convert RGBA to u32 ARGB (0x00RRGGBB)
            let tw = target_width;
            let th = self.cell_height;
            let mut u32_pixels = vec![0u32; (tw * th) as usize];

            let scale_x = tw as f32 / w as f32;
            let scale_y = th as f32 / h as f32;
            let scale = scale_x.min(scale_y);

            let new_w = (w as f32 * scale).round() as u32;
            let new_h = (h as f32 * scale).round() as u32;
            let x_offset = (tw as i32 - new_w as i32) / 2;
            let y_offset = (th as i32 - new_h as i32) / 2;

            for dy in 0..th {
                for dx in 0..tw {
                    let rx = dx as i32 - x_offset;
                    let ry = dy as i32 - y_offset;
                    if rx >= 0 && rx < new_w as i32 && ry >= 0 && ry < new_h as i32 {
                        let sx = ((rx as f32 / scale).floor() as u32).min(w - 1);
                        let sy = ((ry as f32 / scale).floor() as u32).min(h - 1);

                        let src_idx = (sy * w + sx) as usize * 4;
                        let dst_idx = (dy * tw + dx) as usize;

                        let r = rgba[src_idx] as u32;
                        let g = rgba[src_idx + 1] as u32;
                        let b = rgba[src_idx + 2] as u32;
                        let a = rgba[src_idx + 3] as u32;

                        if a > 0 {
                            u32_pixels[dst_idx] = (a << 24) | (r << 16) | (g << 8) | b;
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
                &u32_pixels,
            ))
        } else {
            let mut alpha_pixels = vec![0u8; (target_width * self.cell_height) as usize];

            if has_outline && let (Some(font), Some(glyph)) = (char_font_arc, the_glyph) {
                let (x_offset, y_offset) = if is_nerd_or_pua && !is_pw_sep {
                    let xo = (target_width as f32 - glyph_w) / 2.0;
                    let yo = (self.cell_height as f32 - glyph_h) / 2.0;
                    (xo, yo)
                } else if is_pw_sep {
                    let xo = (base_target_width as f32 - glyph_w) / 2.0;
                    let yo = (self.cell_height as f32 - glyph_h) / 2.0;
                    (xo, yo)
                } else if crate::font::loader::is_box_drawing_or_pipe(key.codepoint) {
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

                if let Some(outlined) = font.outline_glyph(glyph) {
                    outlined.draw(|gx, gy, alpha| {
                        let slant_shift = if is_synthetic_italic && !is_nerd_or_pua && !is_pw_sep {
                            let cell_y = y_offset + gy as f32;
                            (ascent - cell_y) * SHEAR_FACTOR
                        } else {
                            0.0
                        };
                        let px = (x_offset + gx as f32 + slant_shift).round() as i32;
                        let py = (y_offset + gy as f32).round() as i32;

                        if px >= 0
                            && px < target_width as i32
                            && py >= 0
                            && py < self.cell_height as i32
                        {
                            let idx = py as usize * target_width as usize + px as usize;
                            let old_alpha = alpha_pixels[idx] as f32 / 255.0;
                            let new_alpha = old_alpha.max(alpha);
                            alpha_pixels[idx] = (new_alpha * 255.0) as u8;
                        }
                    });
                }
            }

            Some(self.atlas.insert_alpha(
                target_width as u16,
                self.cell_height as u16,
                0,
                0,
                width_mult,
                &alpha_pixels,
            ))
        }
    }

    fn extract_color_png(&mut self, c: char) -> Option<(Vec<u8>, u32, u32)> {
        if self.font.glyph_id(c).0 == 0
            && let Some(idx) = self.fallback_manager.find_fallback_for_char(c)
        {
            let fallback = &self.fallback_manager.fallbacks[idx];
            let id = fallback.font.glyph_id(c);
            if id.0 != 0
                && let Some(ref face) = fallback.owned_face
            {
                let ttf_glyph_id = owned_ttf_parser::GlyphId(id.0);
                if let Some(img) = face
                    .as_face_ref()
                    .glyph_raster_image(ttf_glyph_id, self.cell_height as u16)
                    && img.format == owned_ttf_parser::RasterImageFormat::PNG
                {
                    let mut decoder = png::Decoder::new(std::io::Cursor::new(img.data));
                    decoder.set_transformations(png::Transformations::EXPAND);
                    if let Ok(mut reader) = decoder.read_info() {
                        let mut buf = vec![0; reader.output_buffer_size()];
                        if let Ok(info) = reader.next_frame(&mut buf) {
                            let (output_color, _) = reader.output_color_type();
                            if output_color == png::ColorType::Rgba {
                                return Some((buf, info.width, info.height));
                            }
                        }
                    }
                }
            }
        }
        None
    }
}
