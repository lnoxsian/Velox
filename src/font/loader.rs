use std::collections::HashMap;
use std::sync::Arc;
use glow::HasContext;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use owned_ttf_parser::AsFaceRef;
use crate::font::fallback::FallbackManager;

pub const DEFAULT_FONT_SCALE_MULTIPLIER: f32 = 1.5;

#[derive(Clone, Copy)]
pub struct GlyphUv {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
    pub is_color: bool,
    pub width_mult: f32,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct CacheKey {
    pub c: char,
    pub is_wide: bool,
    pub is_bold: bool,
    pub is_italic: bool,
}

pub struct FontLoader {
    gl: Arc<glow::Context>,
    font: FontArc,
    font_bold: Option<FontArc>,
    font_italic: Option<FontArc>,
    font_bold_italic: Option<FontArc>,
    pub fallback_manager: FallbackManager,
    pub cell_width: u32,
    pub cell_height: u32,
    pub font_size: f32,
    pub font_scale_multiplier: f32,
    pub atlas_texture: glow::Texture,
    atlas_width: u32,
    atlas_height: u32,
    cache: HashMap<CacheKey, GlyphUv>,
    next_x: u32,
    next_y: u32,
}

fn load_font_face(db: &fontdb::Database, query: &fontdb::Query) -> Option<FontArc> {
    if let Some(id) = db.query(query) {
        if let Some(face) = db.face(id) {
            match &face.source {
                fontdb::Source::File(path) => {
                    if let Ok(data) = std::fs::read(path) {
                        if let Ok(font) = FontArc::try_from_vec(data) {
                            return Some(font);
                        }
                    }
                }
                fontdb::Source::Binary(data) => {
                    let bytes = data.as_ref().as_ref();
                    if let Ok(font) = FontArc::try_from_vec(bytes.to_vec()) {
                        return Some(font);
                    }
                }
                fontdb::Source::SharedFile(_, data) => {
                    let bytes = data.as_ref().as_ref();
                    if let Ok(font) = FontArc::try_from_vec(bytes.to_vec()) {
                        return Some(font);
                    }
                }
            }
        }
    }

    if query.style == fontdb::Style::Italic {
        let mut oblique_query = query.clone();
        oblique_query.style = fontdb::Style::Oblique;
        return load_font_face(db, &oblique_query);
    }

    None
}

pub fn is_nerd_font_or_pua(c: char) -> bool {
    matches!(c,
        '\u{2300}'..='\u{24ff}' |   // Misc Technical, Control Pictures, OCR, Enclosed Alphanumerics
        '\u{25a0}'..='\u{2bff}' |   // Geometric Shapes, Misc Symbols, Dingbats, Arrows (EXCLUDES 0x2500..=0x259F Box/Block)
        '\u{e000}'..='\u{f8ff}' |   // Private Use Area (Nerd Fonts, Powerline, Devicons, FontAwesome, Octicons)
        '\u{f0000}'..='\u{ffffd}' | // Supplementary Private Use Area A
        '\u{100000}'..='\u{10fffd}'|// Supplementary Private Use Area B
        '\u{1f300}'..='\u{1f9ff}'   // Emojis & Pictographs
    )
}

pub fn is_powerline(c: char) -> bool {
    matches!(c, '\u{e0b0}'..='\u{e0bf}')
}

pub fn is_box_drawing_or_pipe(c: char) -> bool {
    matches!(c,
        '\u{2500}'..='\u{257f}' | // Box Drawing
        '\u{2580}'..='\u{259f}' | // Block Elements
        '|'                      // ASCII vertical pipe
    )
}

impl FontLoader {
    pub fn new(gl: Arc<glow::Context>, font_family: &str, font_size: f32, font_scale_multiplier: f32) -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };

        let font = load_font_face(&db, &query).expect("Could not load any system monospace font");

        let query_bold = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let font_bold = load_font_face(&db, &query_bold);

        // Query italic and bold-italic faces.
        // If fontdb resolves italic to the same font file as regular (e.g. DejaVu Sans Mono has no italic),
        // treat them as None so the synthetic shear path activates instead of rendering un-slanted glyphs.
        let regular_id = db.query(&query);

        let query_italic = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let italic_id = db.query(&query_italic);
        let font_italic = if italic_id.is_some() && italic_id != regular_id {
            load_font_face(&db, &query_italic)
        } else {
            None // No distinct italic face — synthetic italic will be applied
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
            load_font_face(&db, &query_bold_italic)
        } else {
            None // No distinct bold-italic face — synthetic italic will be applied on bold
        };

        let fallback_manager = FallbackManager::new();

        let px_size = font_size * font_scale_multiplier;
        let scale = PxScale::from(px_size);
        let scaled_font = font.as_scaled(scale);
        let cell_width = scaled_font.h_advance(font.glyph_id('A')).ceil().max(1.0) as u32;
        let cell_height = (scaled_font.ascent() - scaled_font.descent() + scaled_font.line_gap().max(0.0)).ceil().max(1.0) as u32;
        
        let atlas_width = 1024;
        let atlas_height = 1024;

        let atlas_texture = unsafe {
            let tex = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                atlas_width as i32,
                atlas_height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            
            let white_pixels = [255u8; 2 * 2 * 4];
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                2,
                2,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&white_pixels[..])),
            );
            tex
        };

        Self {
            gl,
            font,
            font_bold,
            font_italic,
            font_bold_italic,
            fallback_manager,
            cell_width,
            cell_height,
            font_size,
            font_scale_multiplier,
            atlas_texture,
            atlas_width,
            atlas_height,
            cache: HashMap::new(),
            next_x: 4,
            next_y: 0,
        }
    }

    pub fn white_pixel_uv(&self) -> (f32, f32) {
        (1.0 / self.atlas_width as f32, 1.0 / self.atlas_height as f32)
    }

    pub fn update_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        let px_size = font_size * self.font_scale_multiplier;
        let scale = PxScale::from(px_size);
        let scaled_font = self.font.as_scaled(scale);
        self.cell_width = scaled_font.h_advance(self.font.glyph_id('A')).ceil().max(1.0) as u32;
        self.cell_height = (scaled_font.ascent() - scaled_font.descent() + scaled_font.line_gap().max(0.0)).ceil().max(1.0) as u32;

        self.cache.clear();
        self.next_x = 4;
        self.next_y = 0;

        unsafe {
            let white_square = [255u8; 2 * 2 * 4];
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_texture));
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                self.atlas_width as i32,
                self.atlas_height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                2,
                2,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&white_square[..])),
            );
        }
    }

    pub fn get_glyph_uv(&mut self, c: char, is_wide: bool, is_bold: bool, is_italic: bool) -> GlyphUv {
        let key = CacheKey { c, is_wide, is_bold, is_italic };
        if let Some(uv) = self.cache.get(&key) {
            return *uv;
        }
        
        let seq = if ('\u{100000}'..='\u{10ffff}').contains(&c) {
            let reg_idx = (c as u32 - 0x100000) as usize;
            if let Ok(registry) = crate::screen::grid::get_combining_registry().lock() {
                if reg_idx < registry.len() {
                    registry[reg_idx].clone()
                } else {
                    c.to_string()
                }
            } else {
                c.to_string()
            }
        } else {
            c.to_string()
        };
        let base_c = seq.chars().next().unwrap_or(c);

        let active_font = match (is_bold, is_italic) {
            (true, true) => self.font_bold_italic.as_ref().or(self.font_bold.as_ref()).or(self.font_italic.as_ref()).unwrap_or(&self.font),
            (true, false) => self.font_bold.as_ref().unwrap_or(&self.font),
            (false, true) => self.font_italic.as_ref().unwrap_or(&self.font),
            (false, false) => &self.font,
        };
        let mut color_pixels = None;

        if active_font.glyph_id(base_c).0 == 0
            && let Some(idx) = self.fallback_manager.find_fallback_for_char(base_c) {
                let fallback = &self.fallback_manager.fallbacks[idx];
                let id = fallback.font.glyph_id(base_c);
                if id.0 != 0
                    && let Some(ref face) = fallback.owned_face {
                        let ttf_glyph_id = owned_ttf_parser::GlyphId(id.0);
                        if let Some(img) = face.as_face_ref().glyph_raster_image(ttf_glyph_id, self.cell_height as u16)
                            && img.format == owned_ttf_parser::RasterImageFormat::PNG {
                                let mut decoder = png::Decoder::new(std::io::Cursor::new(img.data));
                                decoder.set_transformations(png::Transformations::EXPAND);
                                if let Ok(mut reader) = decoder.read_info() {
                                    let mut buf = vec![0; reader.output_buffer_size()];
                                    if let Ok(info) = reader.next_frame(&mut buf) {
                                        let (output_color, _) = reader.output_color_type();
                                        if output_color == png::ColorType::Rgba {
                                            color_pixels = Some((buf, info.width, info.height));
                                        }
                                    }
                                }
                            }
                    }
            }

        let base_target_width = if is_wide { self.cell_width * 2 } else { self.cell_width };
        let mut glyph_w = 0.0f32;
        let mut glyph_h = 0.0f32;
        let mut bounds_min_x = 0.0f32;
        let mut bounds_min_y = 0.0f32;
        let mut ascent = 0.0f32;
        let mut has_outline = false;
        let is_nerd_or_pua = is_nerd_font_or_pua(base_c);
        let is_pw_sep = is_powerline(base_c);

        let mut is_synthetic_italic = false;
        let mut char_font_arc: Option<FontArc> = None;
        let mut the_glyph: ab_glyph::Glyph = ab_glyph::Glyph {
            id: ab_glyph::GlyphId(0),
            scale: PxScale::from(0.0),
            position: ab_glyph::point(0.0, 0.0),
        };

        if color_pixels.is_none() {
            let (mut char_font, synth) = match (is_bold, is_italic) {
                (true, true) => {
                    if let Some(ref f) = self.font_bold_italic {
                        (f, false)
                    } else if let Some(ref f) = self.font_italic {
                        (f, false)
                    } else if let Some(ref f) = self.font_bold {
                        (f, true)
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
            let mut char_glyph_id = char_font.glyph_id(base_c);
            if char_glyph_id.0 == 0
                && let Some(idx) = self.fallback_manager.find_fallback_for_char(base_c) {
                    let fallback = &self.fallback_manager.fallbacks[idx];
                    let id = fallback.font.glyph_id(base_c);
                    if id.0 != 0 {
                        char_font = &fallback.font;
                        char_glyph_id = id;
                    }
                }

            if char_glyph_id.0 != 0 {
                let font_scale = self.font_size * self.font_scale_multiplier;
                let scale;

                if is_pw_sep {
                    // Powerline prompt separators: scale to fill exactly the cell height
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
                    // Nerd Font icons / PUA / Emojis:
                    // Scale ONLY on height. Icons wider than one cell are cropped/centered horizontally.
                    // Previously using sw.min(sh) caused tiny icons because many icons are wider than one cell column.
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
                    // All other characters: use native font scale
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
                    the_glyph = glyph;
                }
            }
        }

        // Atlas region width:
        // - Nerd/PUA icons: icon's actual rendered pixel width, capped at 2×cell.
        //   width_mult reflects this so the renderer draws a correctly-sized quad.
        // - Synthetic italic regular text: add headroom for the italic slant bleed.
        //   The maximum horizontal shift is ascent × SHEAR_FACTOR pixels at the glyph top.
        //   The quad width (width_mult) stays at 1.0 — the extra pixels just overdraw slightly.
        // - Everything else: strictly base_target_width.
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

        let mut rgba_pixels = vec![0u8; (target_width * self.cell_height * 4) as usize];
        let is_color = color_pixels.is_some();

        if let Some((rgba, w, h)) = color_pixels {
            let tw = target_width;
            let th = self.cell_height;

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
                        let dst_idx = (dy * tw + dx) as usize * 4;

                        rgba_pixels[dst_idx]     = rgba[src_idx];
                        rgba_pixels[dst_idx + 1] = rgba[src_idx + 1];
                        rgba_pixels[dst_idx + 2] = rgba[src_idx + 2];
                        rgba_pixels[dst_idx + 3] = rgba[src_idx + 3];
                    }
                }
            }
        } else {
            let mut pixels = vec![0u8; (target_width * self.cell_height) as usize];

            if has_outline {
                // Compute draw offsets:
                // - Nerd/PUA icons: center horizontally within the (potentially wider) atlas slot, center vertically
                // - Powerline separators: center within cell
                // - Box Drawing & Vertical Pipes: exact font alignment (bounds_min_x)
                // - Regular text: baseline-aligned, horizontally centered if narrower than cell
                let (x_offset, y_offset) = if is_nerd_or_pua && !is_pw_sep {
                    // Nerd/PUA icons: center within the atlas slot (which is sized to the icon)
                    let xo = (target_width as f32 - glyph_w) / 2.0;
                    let yo = (self.cell_height as f32 - glyph_h) / 2.0;
                    (xo, yo)
                } else if is_pw_sep {
                    // Powerline separators: center within cell
                    let xo = (base_target_width as f32 - glyph_w) / 2.0;
                    let yo = (self.cell_height as f32 - glyph_h) / 2.0;
                    (xo, yo)
                } else if is_box_drawing_or_pipe(base_c) {
                    // Box drawing & vertical pipes: exact font origin so all glyphs share the same pixel grid
                    (bounds_min_x, ascent + bounds_min_y)
                } else {
                    // Regular text: baseline-aligned, horizontally centered if narrower than cell
                    let xo = if glyph_w < base_target_width as f32 {
                        let calc = (base_target_width as f32 - glyph_w) / 2.0;
                        if calc > 0.0 { calc } else { bounds_min_x }
                    } else {
                        bounds_min_x
                    };
                    let yo = ascent + bounds_min_y;
                    (xo, yo)
                };

                if let Some(ref cf) = char_font_arc {
                    if let Some(outlined) = cf.outline_glyph(the_glyph) {
                        outlined.draw(|gx, gy, alpha| {
                            let slant_shift = if is_synthetic_italic {
                                // Pivot the shear around the baseline.
                                // Pixels above the baseline lean right; pixels below lean left.
                                // cell_y is the pixel's position in the cell (0 = top).
                                let cell_y = y_offset + gy as f32;
                                (ascent - cell_y) * SHEAR_FACTOR
                            } else {
                                0.0
                            };
                            let px = (x_offset + gx as f32 + slant_shift).round() as i32;
                            let py = (y_offset + gy as f32).round() as i32;

                            if px >= 0 && px < target_width as i32 && py >= 0 && py < self.cell_height as i32 {
                                let idx = py as usize * target_width as usize + px as usize;
                                let old_alpha = pixels[idx] as f32 / 255.0;
                                let new_alpha = old_alpha.max(alpha);
                                pixels[idx] = (new_alpha * 255.0) as u8;
                            }
                        });
                    }
                }

                // Also draw combining chars
                for ch in seq.chars().skip(1) {
                    let mut char_font = match (is_bold, is_italic) {
                        (true, true) => self.font_bold_italic.as_ref().or(self.font_bold.as_ref()).or(self.font_italic.as_ref()).unwrap_or(&self.font),
                        (true, false) => self.font_bold.as_ref().unwrap_or(&self.font),
                        (false, true) => self.font_italic.as_ref().unwrap_or(&self.font),
                        (false, false) => &self.font,
                    };
                    let mut char_glyph_id = char_font.glyph_id(ch);

                    if char_glyph_id.0 == 0
                        && let Some(idx) = self.fallback_manager.find_fallback_for_char(ch) {
                            let fallback = &self.fallback_manager.fallbacks[idx];
                            let id = fallback.font.glyph_id(ch);
                            if id.0 != 0 {
                                char_font = &fallback.font;
                                char_glyph_id = id;
                            }
                        }

                    if char_glyph_id.0 != 0 {
                        let px_size = self.font_size * self.font_scale_multiplier;
                        let scale = PxScale::from(px_size);
                        let glyph = char_glyph_id.with_scale(scale);
                        let scaled_font = char_font.as_scaled(scale);
                        let ascent = scaled_font.ascent();

                        if let Some(outlined) = char_font.outline_glyph(glyph) {
                            let bounds = outlined.px_bounds();
                            let mut xo = bounds.min.x;
                            if unicode_width::UnicodeWidthChar::width(ch) == Some(0)
                                && bounds.max.x <= 1.0 {
                                    xo += base_target_width as f32;
                                }
                            let yo = ascent + bounds.min.y;

                            outlined.draw(|gx, gy, alpha| {
                                let px = (xo + gx as f32).round() as i32;
                                let py = (yo + gy as f32).round() as i32;

                                if px >= 0 && px < target_width as i32 && py >= 0 && py < self.cell_height as i32 {
                                    let idx = py as usize * target_width as usize + px as usize;
                                    let old_alpha = pixels[idx] as f32 / 255.0;
                                    let new_alpha = old_alpha.max(alpha);
                                    pixels[idx] = (new_alpha * 255.0) as u8;
                                }
                            });
                        }
                    }
                }
            }

            for i in 0..pixels.len() {
                let mask = pixels[i];
                let dst_idx = i * 4;
                rgba_pixels[dst_idx]     = mask;
                rgba_pixels[dst_idx + 1] = mask;
                rgba_pixels[dst_idx + 2] = mask;
                rgba_pixels[dst_idx + 3] = mask;
            }
        }

        let pad = 2;
        if self.next_x + target_width + pad > self.atlas_width {
            self.next_x = 0;
            self.next_y += self.cell_height + pad;
        }

        if self.next_y + self.cell_height + pad > self.atlas_height {
            self.cache.clear();
            self.next_x = 4;
            self.next_y = 0;
            unsafe {
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_texture));
                self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    self.atlas_width as i32,
                    self.atlas_height as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                let white_pixels = [255u8; 2 * 2 * 4];
                self.gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    2,
                    2,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&white_pixels[..])),
                );
            }
        }

        let ox = self.next_x;
        let oy = self.next_y;

        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_texture));
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                ox as i32,
                oy as i32,
                target_width as i32,
                self.cell_height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&rgba_pixels[..])),
            );
        }

        self.next_x += target_width + pad;

        let uv = GlyphUv {
            u_min: ox as f32 / self.atlas_width as f32,
            v_min: oy as f32 / self.atlas_height as f32,
            u_max: (ox + target_width) as f32 / self.atlas_width as f32,
            v_max: (oy + self.cell_height) as f32 / self.atlas_height as f32,
            is_color,
            // width_mult: ratio of atlas region to one cell width.
            // The renderer uses this to size the quad: quad_w = cw * width_mult.
            // For Nerd/PUA icons this will be >= 1.0, matching natural glyph width.
            width_mult: target_width as f32 / self.cell_width as f32,
        };

        self.cache.insert(key, uv);
        uv
    }
}
