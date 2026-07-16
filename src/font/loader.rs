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
    pub cell_width: u32,
    pub cell_height: u32,
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
            cell_width,
            cell_height,
            atlas_texture,
            atlas_width,
            atlas_height,
            cache: HashMap::new(),
            next_x: 0,
            next_y: 0,
        }
    }

    pub fn get_glyph_uv(&mut self, c: char) -> GlyphUv {
        if let Some(uv) = self.cache.get(&c) {
            return *uv;
        }
        
        // Rasterize using ab_glyph, aligning coordinates to baseline
        let scale = PxScale::from(self.cell_height as f32);
        let glyph = self.font.glyph_id(c).with_scale(scale);
        let mut pixels = vec![0u8; (self.cell_width * self.cell_height) as usize];
        
        let scaled_font = self.font.as_scaled(scale);
        let ascent = scaled_font.ascent();

        if let Some(outlined) = self.font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, alpha| {
                // Horizontal coordinate: standard rasterizer offset
                let px = (bounds.min.x + gx as f32).round() as i32;
                // Vertical coordinate: relative to the baseline (ascent)
                let py = (ascent + bounds.min.y + gy as f32).round() as i32;
                
                if px >= 0 && px < self.cell_width as i32 && py >= 0 && py < self.cell_height as i32 {
                    let idx = py as usize * self.cell_width as usize + px as usize;
                    pixels[idx] = (alpha * 255.0) as u8;
                }
            });
        }
        
        if self.next_x + self.cell_width > self.atlas_width {
            self.next_x = 0;
            self.next_y += self.cell_height;
        }
        
        if self.next_y + self.cell_height > self.atlas_height {
            self.cache.clear();
            self.next_x = 0;
            self.next_y = 0;
            unsafe {
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_texture));
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
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                ox as i32,
                oy as i32,
                self.cell_width as i32,
                self.cell_height as i32,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&pixels[..])),
            );
        }
        
        self.next_x += self.cell_width;

        let uv = GlyphUv {
            u_min: ox as f32 / self.atlas_width as f32,
            v_min: oy as f32 / self.atlas_height as f32,
            u_max: (ox + self.cell_width) as f32 / self.atlas_width as f32,
            v_max: (oy + self.cell_height) as f32 / self.atlas_height as f32,
        };

        self.cache.insert(c, uv);
        uv
    }
}
