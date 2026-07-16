use std::collections::HashMap;
use std::sync::Arc;
use glow::HasContext;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};

#[derive(Clone, Copy)]
pub struct GlyphUv {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

pub struct FontLoader {
    gl: Arc<glow::Context>,
    font: FontArc,
    fallbacks: Vec<FontArc>,
    pub cell_width: u32,
    pub cell_height: u32,
    pub font_size: f32,
    pub atlas_texture: glow::Texture,
    atlas_width: u32,
    atlas_height: u32,
    cache: HashMap<char, GlyphUv>,
    next_x: u32,
    next_y: u32,
}

impl FontLoader {
    pub fn new(gl: Arc<glow::Context>, font_family: &str, font_size: f32) -> Self {
        let mut loaded_font = None;

        // Try querying the system fonts database for the user's chosen family
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };

        if let Some(id) = db.query(&query) {
            if let Some(face) = db.face(id) {
                match &face.source {
                    fontdb::Source::File(path) => {
                        if let Ok(data) = std::fs::read(path) {
                            if let Ok(f) = FontArc::try_from_vec(data) {
                                loaded_font = Some(f);
                            }
                        }
                    }
                    fontdb::Source::Binary(data) => {
                        let bytes = data.as_ref().as_ref();
                        if let Ok(f) = FontArc::try_from_vec(bytes.to_vec()) {
                            loaded_font = Some(f);
                        }
                    }
                    fontdb::Source::SharedFile(_, data) => {
                        let bytes = data.as_ref().as_ref();
                        if let Ok(f) = FontArc::try_from_vec(bytes.to_vec()) {
                            loaded_font = Some(f);
                        }
                    }
                }
            }
        }

        // Hardcoded fallback list if fontdb fails or returns an invalid face
        if loaded_font.is_none() {
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
                        loaded_font = Some(f);
                        break;
                    }
                }
            }
        }

        let font = loaded_font.expect("Could not load any system monospace font");

        // Load fallback fonts for symbols, Nerd Font icons, and emojis
        let mut fallbacks = Vec::new();
        let fallback_families = [
            "Symbols Nerd Font",
            "Hack Nerd Font",
            "FiraCode Nerd Font",
            "DejaVu Sans",
            "Noto Sans Symbols",
            "Noto Sans Symbols2",
            "Noto Color Emoji",
        ];
        
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
                                if let Ok(f) = FontArc::try_from_vec(data) {
                                    fallbacks.push(f);
                                }
                            }
                        }
                        fontdb::Source::Binary(data) => {
                            let bytes = data.as_ref().as_ref();
                            if let Ok(f) = FontArc::try_from_vec(bytes.to_vec()) {
                                fallbacks.push(f);
                            }
                        }
                        fontdb::Source::SharedFile(_, data) => {
                            let bytes = data.as_ref().as_ref();
                            if let Ok(f) = FontArc::try_from_vec(bytes.to_vec()) {
                                fallbacks.push(f);
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
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ];
        for path in &extra_paths {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(f) = FontArc::try_from_vec(data) {
                    fallbacks.push(f);
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
                glow::R8 as i32,
                atlas_width as i32,
                atlas_height as i32,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            tex
        };

        Self {
            gl,
            font,
            fallbacks,
            cell_width,
            cell_height,
            font_size,
            atlas_texture,
            atlas_width,
            atlas_height,
            cache: HashMap::new(),
            next_x: 0,
            next_y: 0,
        }
    }

    pub fn get_glyph_uv(&mut self, c: char, is_wide: bool) -> GlyphUv {
        if let Some(uv) = self.cache.get(&c) {
            return *uv;
        }
        
        // Select either the main font or check fallback fonts for glyph support
        let mut active_font = &self.font;
        let mut glyph_id = self.font.glyph_id(c);
        let mut is_fallback = false;

        if glyph_id.0 == 0 {
            for fallback in &self.fallbacks {
                let id = fallback.glyph_id(c);
                if id.0 != 0 {
                    active_font = fallback;
                    glyph_id = id;
                    is_fallback = true;
                    break;
                }
            }
        }

        // Identify if this character is a Nerd Font icon, emoji, or fallback character
        let is_icon_or_emoji = (c >= '\u{e000}' && c <= '\u{f8ff}') 
            || c >= '\u{1f000}' 
            || is_fallback;

        let target_width = if is_wide { self.cell_width * 2 } else { self.cell_width };

        // Rasterize using ab_glyph, aligning coordinates to baseline or centering/scaling if it is an icon/emoji
        let mut scale = PxScale::from(self.font_size);
        let mut glyph = glyph_id.with_scale(scale);
        let mut pixels = vec![0u8; (target_width * self.cell_height) as usize];
        
        let mut scaled_font = active_font.as_scaled(scale);
        let mut ascent = scaled_font.ascent();

        if is_icon_or_emoji {
            let temp_scale = PxScale::from(self.cell_height as f32);
            let temp_glyph = glyph_id.with_scale(temp_scale);
            if let Some(outlined) = active_font.outline_glyph(temp_glyph) {
                let bounds = outlined.px_bounds();
                let glyph_w = bounds.max.x - bounds.min.x;
                let glyph_h = bounds.max.y - bounds.min.y;
                if glyph_w > 0.0 && glyph_h > 0.0 {
                    let scale_factor_x = (target_width as f32) / glyph_w;
                    let scale_factor_y = (self.cell_height as f32) / glyph_h;
                    // Maximize size within the cell by allowing up to 50% horizontal overflow for single-width icons,
                    // or keeping aspect ratio within double-width for wide icons/emojis.
                    let scale_mult = if is_wide { 1.0 } else { 1.5 };
                    let scale_factor = (scale_factor_x * scale_mult).min(scale_factor_y).min(0.95);
                    
                    let fit_scale = self.cell_height as f32 * scale_factor;
                    scale = PxScale::from(fit_scale);
                    glyph = glyph_id.with_scale(scale);
                    scaled_font = active_font.as_scaled(scale);
                    ascent = scaled_font.ascent();
                }
            }
        }

        if let Some(outlined) = active_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            
            // Center icons and emojis in both directions
            let x_offset = if is_icon_or_emoji {
                (target_width as f32 - (bounds.max.x - bounds.min.x)) / 2.0
            } else {
                bounds.min.x
            };

            let y_offset = if is_icon_or_emoji {
                (self.cell_height as f32 - (bounds.max.y - bounds.min.y)) / 2.0
            } else {
                ascent + bounds.min.y
            };

            outlined.draw(|gx, gy, alpha| {
                let px = (x_offset + gx as f32).round() as i32;
                let py = (y_offset + gy as f32).round() as i32;
                
                if px >= 0 && px < target_width as i32 && py >= 0 && py < self.cell_height as i32 {
                    let idx = py as usize * target_width as usize + px as usize;
                    pixels[idx] = (alpha * 255.0) as u8;
                }
            });
        }
        
        if self.next_x + target_width > self.atlas_width {
            self.next_x = 0;
            self.next_y += self.cell_height;
        }
        
        if self.next_y + self.cell_height > self.atlas_height {
            self.cache.clear();
            self.next_x = 0;
            self.next_y = 0;
            unsafe {
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_texture));
                self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::R8 as i32,
                    self.atlas_width as i32,
                    self.atlas_height as i32,
                    0,
                    glow::RED,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
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
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&pixels[..])),
            );
        }
        
        self.next_x += target_width;

        let uv = GlyphUv {
            u_min: ox as f32 / self.atlas_width as f32,
            v_min: oy as f32 / self.atlas_height as f32,
            u_max: (ox + target_width) as f32 / self.atlas_width as f32,
            v_max: (oy + self.cell_height) as f32 / self.atlas_height as f32,
        };

        self.cache.insert(c, uv);
        uv
    }
}
