use std::collections::HashMap;
use std::sync::Arc;
use glow::HasContext;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use owned_ttf_parser::AsFaceRef;


#[derive(Clone, Copy)]
pub struct GlyphUv {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
    pub is_color: bool,
}

pub struct FallbackFont {
    pub font: FontArc,
    pub owned_face: owned_ttf_parser::OwnedFace,
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
    fallbacks: Vec<FallbackFont>,
    pub cell_width: u32,
    pub cell_height: u32,
    pub font_size: f32,
    pub enable_nerdfont: bool,
    pub atlas_texture: glow::Texture,
    atlas_width: u32,
    atlas_height: u32,
    cache: HashMap<CacheKey, GlyphUv>,
    next_x: u32,
    next_y: u32,
}

fn load_font_face(db: &fontdb::Database, query: &fontdb::Query) -> Option<FontArc> {
    let id = db.query(query)?;
    let face = db.face(id)?;
    match &face.source {
        fontdb::Source::File(path) => {
            let data = std::fs::read(path).ok()?;
            FontArc::try_from_vec(data).ok()
        }
        fontdb::Source::Binary(data) => {
            let bytes = data.as_ref().as_ref();
            FontArc::try_from_vec(bytes.to_vec()).ok()
        }
        fontdb::Source::SharedFile(_, data) => {
            let bytes = data.as_ref().as_ref();
            FontArc::try_from_vec(bytes.to_vec()).ok()
        }
    }
}

impl FontLoader {
    pub fn new(gl: Arc<glow::Context>, font_family: &str, font_size: f32, enable_nerdfont: bool) -> Self {
        // Try querying the system fonts database for the user's chosen family
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };

        let font = load_font_face(&db, &query).or_else(|| {
            // Hardcoded fallback list if fontdb fails or returns an invalid face
            let paths = [
                "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
                "/usr/share/fonts/truetype/freefont/FreeMono.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            ];
            for path in &paths {
                if let Ok(data) = std::fs::read(path) {
                    if let Ok(f) = FontArc::try_from_vec(data) {
                        return Some(f);
                    }
                }
            }
            None
        }).expect("Could not load any system monospace font");

        let query_bold = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let font_bold = load_font_face(&db, &query_bold);

        let query_italic = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let font_italic = load_font_face(&db, &query_italic);

        let query_bold_italic = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let font_bold_italic = load_font_face(&db, &query_bold_italic);

        // Load fallback fonts for symbols, Nerd Font icons, and emojis
        let mut fallbacks = Vec::new();
        let mut fallback_families = vec![
            "Noto Color Emoji",
            "DejaVu Sans",
            "Noto Sans Symbols",
            "Noto Sans Symbols2",
            "Noto Emoji",
            // CJK Monospace and Fallbacks
            "Noto Sans Mono CJK JP",
            "Noto Sans Mono CJK KR",
            "Noto Sans Mono CJK SC",
            "Noto Sans Mono CJK TC",
            "Noto Sans Mono CJK HK",
            "Noto Sans CJK JP",
            "Noto Sans CJK KR",
            "Noto Sans CJK SC",
            "Noto Sans CJK TC",
            "Noto Sans CJK HK",
            "Droid Sans Fallback",
            // Flags and Regional Indicator Symbols
            "FreeSans",
            "FreeMono",
            "FreeSerif",
            // Vector Emoji Fallback (requires fonts-symbola)
            "Symbola",
        ];
        if enable_nerdfont {
            fallback_families.insert(0, "Symbols Nerd Font");
            fallback_families.insert(1, "Hack Nerd Font");
            fallback_families.insert(2, "FiraCode Nerd Font");
        }
        
        for family in &fallback_families {
            let query = fontdb::Query {
                families: &[fontdb::Family::Name(family)],
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            };
            if let Some(id) = db.query(&query) {
                if let Some(face) = db.face(id) {
                    match &face.source {
                        fontdb::Source::File(path) => {
                            if let Ok(data) = std::fs::read(path) {
                                if let Ok(f) = FontArc::try_from_vec(data.clone()) {
                                    if let Ok(owned_face) = owned_ttf_parser::OwnedFace::from_vec(data, 0) {
                                        fallbacks.push(FallbackFont { font: f, owned_face });
                                    }
                                }
                            }
                        }
                        fontdb::Source::Binary(data) => {
                            let bytes = data.as_ref().as_ref().to_vec();
                            if let Ok(f) = FontArc::try_from_vec(bytes.clone()) {
                                if let Ok(owned_face) = owned_ttf_parser::OwnedFace::from_vec(bytes, 0) {
                                    fallbacks.push(FallbackFont { font: f, owned_face });
                                }
                            }
                        }
                        fontdb::Source::SharedFile(_, data) => {
                            let bytes = data.as_ref().as_ref().to_vec();
                            if let Ok(f) = FontArc::try_from_vec(bytes.clone()) {
                                if let Ok(owned_face) = owned_ttf_parser::OwnedFace::from_vec(bytes, 0) {
                                    fallbacks.push(FallbackFont { font: f, owned_face });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Add additional common filesystem fallback paths for robustness
        let extra_paths = [
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/opentype/noto/NotoColorEmoji.otf",
            "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ];
        for path in &extra_paths {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(f) = FontArc::try_from_vec(data.clone()) {
                    if let Ok(owned_face) = owned_ttf_parser::OwnedFace::from_vec(data, 0) {
                        fallbacks.push(FallbackFont { font: f, owned_face });
                    }
                }
            }
        }
        
        // Measure monospace glyph dimensions
        let scale = PxScale::from(font_size);
        let scaled_font = font.as_scaled(scale);
        let cell_width = scaled_font.h_advance(font.glyph_id('A')).round() as u32;
        let cell_height = (scaled_font.ascent() - scaled_font.descent() + scaled_font.line_gap()).round() as u32;
        
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
            
            // Upload 2x2 white square at (0, 0)
            let white_pixels = vec![255u8; 2 * 2 * 4];
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
            fallbacks,
            cell_width,
            cell_height,
            font_size,
            enable_nerdfont,
            atlas_texture,
            atlas_width,
            atlas_height,
            cache: HashMap::new(),
            next_x: 2, // Start past the 2x2 white pixel
            next_y: 0,
        }
    }

    pub fn white_pixel_uv(&self) -> (f32, f32) {
        (1.0 / self.atlas_width as f32, 1.0 / self.atlas_height as f32)
    }

    pub fn get_glyph_uv(&mut self, c: char, is_wide: bool, is_bold: bool, is_italic: bool) -> GlyphUv {
        let key = CacheKey { c, is_wide, is_bold, is_italic };
        if let Some(uv) = self.cache.get(&key) {
            return *uv;
        }
        
        let seq = if c >= '\u{100000}' && c <= '\u{10ffff}' {
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

        // Select either the main font or check fallback fonts for glyph support
        let active_font = match (is_bold, is_italic) {
            (true, true) => self.font_bold_italic.as_ref().or(self.font_bold.as_ref()).or(self.font_italic.as_ref()).unwrap_or(&self.font),
            (true, false) => self.font_bold.as_ref().unwrap_or(&self.font),
            (false, true) => self.font_italic.as_ref().unwrap_or(&self.font),
            (false, false) => &self.font,
        };
        let mut color_pixels = None;

        if active_font.glyph_id(base_c).0 == 0 {
            for fallback in &self.fallbacks {
                let id = fallback.font.glyph_id(base_c);
                if id.0 != 0 {
                    // Try to get raster image first (for color emojis)
                    let face = fallback.owned_face.as_face_ref();
                    let ttf_glyph_id = owned_ttf_parser::GlyphId(id.0);
                    if let Some(img) = face.glyph_raster_image(ttf_glyph_id, self.cell_height as u16) {
                        if img.format == owned_ttf_parser::RasterImageFormat::PNG {
                            let mut decoder = png::Decoder::new(std::io::Cursor::new(img.data));
                            decoder.set_transformations(png::Transformations::EXPAND);
                            if let Ok(mut reader) = decoder.read_info() {
                                let mut buf = vec![0; reader.output_buffer_size()];
                                if let Ok(info) = reader.next_frame(&mut buf) {
                                    let (output_color, _) = reader.output_color_type();
                                    if output_color == png::ColorType::Rgba {
                                        color_pixels = Some((buf, info.width, info.height));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let target_width = if is_wide { self.cell_width * 2 } else { self.cell_width };
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
                        
                        rgba_pixels[dst_idx] = rgba[src_idx];
                        rgba_pixels[dst_idx + 1] = rgba[src_idx + 1];
                        rgba_pixels[dst_idx + 2] = rgba[src_idx + 2];
                        rgba_pixels[dst_idx + 3] = rgba[src_idx + 3];
                    }
                }
            }
        } else {
            let mut pixels = vec![0u8; (target_width * self.cell_height) as usize];
            
            for (ch_idx, ch) in seq.chars().enumerate() {
                let mut char_font = match (is_bold, is_italic) {
                    (true, true) => self.font_bold_italic.as_ref().or(self.font_bold.as_ref()).or(self.font_italic.as_ref()).unwrap_or(&self.font),
                    (true, false) => self.font_bold.as_ref().unwrap_or(&self.font),
                    (false, true) => self.font_italic.as_ref().unwrap_or(&self.font),
                    (false, false) => &self.font,
                };
                let mut char_glyph_id = char_font.glyph_id(ch);
                
                if char_glyph_id.0 == 0 {
                    for fallback in &self.fallbacks {
                        let id = fallback.font.glyph_id(ch);
                        if id.0 != 0 {
                            char_font = &fallback.font;
                            char_glyph_id = id;
                            break;
                        }
                    }
                }
                
                if char_glyph_id.0 != 0 {
                    let mut scale = PxScale::from(self.font_size);
                    let mut glyph = char_glyph_id.with_scale(scale);
                    let mut scaled_font = char_font.as_scaled(scale);
                    let mut ascent = scaled_font.ascent();

                    let is_emoji = ch >= '\u{1f000}' || (ch >= '\u{2600}' && ch <= '\u{27bf}');
                    let is_nerd_icon = self.enable_nerdfont && (ch >= '\u{e000}' && ch <= '\u{f8ff}');
                    let is_icon_or_emoji = is_emoji || is_nerd_icon;

                    if is_icon_or_emoji {
                        let temp_scale = PxScale::from(self.cell_height as f32);
                        let temp_glyph = char_glyph_id.with_scale(temp_scale);
                        if let Some(outlined) = char_font.outline_glyph(temp_glyph) {
                            let bounds = outlined.px_bounds();
                            let glyph_w = bounds.max.x - bounds.min.x;
                            let glyph_h = bounds.max.y - bounds.min.y;
                            if glyph_w > 0.0 && glyph_h > 0.0 {
                                let scale_factor_x = (target_width as f32) / glyph_w;
                                let scale_factor_y = (self.cell_height as f32) / glyph_h;
                                let scale_mult = if is_wide { 1.0 } else { 1.5 };
                                let scale_factor = (scale_factor_x * scale_mult).min(scale_factor_y).min(0.95);
                                
                                let fit_scale = self.cell_height as f32 * scale_factor;
                                scale = PxScale::from(fit_scale);
                                glyph = char_glyph_id.with_scale(scale);
                                scaled_font = char_font.as_scaled(scale);
                                ascent = scaled_font.ascent();
                            }
                        }
                    }

                    if let Some(outlined) = char_font.outline_glyph(glyph) {
                        let bounds = outlined.px_bounds();
                        
                        let mut x_offset = if is_icon_or_emoji {
                            (target_width as f32 - (bounds.max.x - bounds.min.x)) / 2.0
                        } else {
                            bounds.min.x
                        };

                        let y_offset = if is_icon_or_emoji {
                            (self.cell_height as f32 - (bounds.max.y - bounds.min.y)) / 2.0
                        } else {
                            ascent + bounds.min.y
                        };

                        if !is_icon_or_emoji && ch_idx > 0 && unicode_width::UnicodeWidthChar::width(ch) == Some(0) {
                            // If the combining character outline is centered to the left of the origin (e.g. proportional fallback fonts),
                            // shift it to the right so it aligns on top of the base glyph.
                            if bounds.max.x <= 1.0 {
                                x_offset += target_width as f32;
                            }
                        }

                        outlined.draw(|gx, gy, alpha| {
                            let px = (x_offset + gx as f32).round() as i32;
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
            }

            for i in 0..pixels.len() {
                let mask = pixels[i];
                let dst_idx = i * 4;
                rgba_pixels[dst_idx] = mask;
                rgba_pixels[dst_idx + 1] = mask;
                rgba_pixels[dst_idx + 2] = mask;
                rgba_pixels[dst_idx + 3] = mask;
            }
        }
        
        if self.next_x + target_width > self.atlas_width {
            self.next_x = 0;
            self.next_y += self.cell_height;
        }
        
        if self.next_y + self.cell_height > self.atlas_height {
            self.cache.clear();
            self.next_x = 2; // Reserve (0,0)-(2,2) for white pixel
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
                // Re-write the white square
                let white_pixels = vec![255u8; 2 * 2 * 4];
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
        
        self.next_x += target_width;

        let uv = GlyphUv {
            u_min: ox as f32 / self.atlas_width as f32,
            v_min: oy as f32 / self.atlas_height as f32,
            u_max: (ox + target_width) as f32 / self.atlas_width as f32,
            v_max: (oy + self.cell_height) as f32 / self.atlas_height as f32,
            is_color,
        };

        self.cache.insert(key, uv);
        uv
    }
}






