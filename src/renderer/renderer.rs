use std::sync::Arc;
use glow::HasContext;
use crate::screen::cell::{Cell, Color, CellFlags};
use crate::font::loader::FontLoader;

pub struct Renderer {
    gl: Arc<glow::Context>,
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    pub font_loader: FontLoader,
    viewport_width: u32,
    viewport_height: u32,
    start_time: std::time::Instant,
    vertices: Vec<f32>,
}

impl Renderer {
    pub fn new(gl: Arc<glow::Context>, font_family: &str, font_size: f32, enable_nerdfont: bool) -> Self {
        unsafe {
            // Compile Shaders
            let vertex_shader_source = r#"
                #version 330 core
                layout (location = 0) in vec2 a_pos;
                layout (location = 1) in vec2 a_tex;
                layout (location = 2) in vec4 a_fg;
                layout (location = 3) in vec4 a_bg;
                out vec2 v_tex;
                out vec4 v_fg;
                out vec4 v_bg;
                uniform mat4 u_projection;
                void main() {
                    gl_Position = u_projection * vec4(a_pos, 0.0, 1.0);
                    v_tex = a_tex;
                    v_fg = a_fg;
                    v_bg = a_bg;
                }
            "#;

            let fragment_shader_source = r#"
                #version 330 core
                in vec2 v_tex;
                in vec4 v_fg;
                in vec4 v_bg;
                out vec4 FragColor;
                uniform sampler2D u_atlas;
                void main() {
                    vec4 tex_color = texture(u_atlas, v_tex);
                    if (v_fg.a < 0.5) {
                        FragColor = mix(v_bg, tex_color, tex_color.a);
                    } else {
                        float mask = tex_color.r;
                        FragColor = mix(v_bg, v_fg, mask);
                    }
                }
            "#;

            let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
            gl.shader_source(vs, vertex_shader_source);
            gl.compile_shader(vs);
            if !gl.get_shader_compile_status(vs) {
                panic!("Vertex shader compilation failed: {}", gl.get_shader_info_log(vs));
            }

            let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
            gl.shader_source(fs, fragment_shader_source);
            gl.compile_shader(fs);
            if !gl.get_shader_compile_status(fs) {
                panic!("Fragment shader compilation failed: {}", gl.get_shader_info_log(fs));
            }

            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("Shader program linking failed: {}", gl.get_program_info_log(program));
            }

            gl.delete_shader(vs);
            gl.delete_shader(fs);

            // VAO / VBO
            let vao = gl.create_vertex_array().unwrap();
            let vbo = gl.create_buffer().unwrap();

            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

            let stride = (2 + 2 + 4 + 4) * std::mem::size_of::<f32>() as i32;
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 2 * std::mem::size_of::<f32>() as i32);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, stride, 4 * std::mem::size_of::<f32>() as i32);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(3, 4, glow::FLOAT, false, stride, 8 * std::mem::size_of::<f32>() as i32);
            gl.enable_vertex_attrib_array(3);

            let font_loader = FontLoader::new(gl.clone(), font_family, font_size, enable_nerdfont);

            Self {
                gl,
                program,
                vao,
                vbo,
                font_loader,
                viewport_width: 800,
                viewport_height: 600,
                start_time: std::time::Instant::now(),
                vertices: Vec::with_capacity(80 * 24 * 6 * 12),
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
        }
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_loader.update_font_size(font_size);
    }

    pub fn draw(
        &mut self,
        cells: &[Cell],
        cols: usize,
        rows: usize,
        cursor_x: usize,
        cursor_y: usize,
        cursor_visible: bool,
        cursor_shape: crate::screen::cursor::CursorShape,
        theme: &crate::theme::theme::Theme,
        bold_is_bright: bool,
        selection: &crate::screen::selection::Selection,
    ) {
        let cw = self.font_loader.cell_width as f32;
        let ch = self.font_loader.cell_height as f32;
        
        let mut vertices = std::mem::take(&mut self.vertices);
        vertices.clear();
        let reserve_cap = cells.len() * 6 * 12;
        if vertices.capacity() < reserve_cap {
            vertices.reserve(reserve_cap - vertices.capacity());
        }

        let elapsed = self.start_time.elapsed().as_millis();
        let blink_on = (elapsed / 500).is_multiple_of(2);

        let push_quad = |vertices: &mut Vec<f32>, x: f32, y: f32, w: f32, h: f32, u_min: f32, v_min: f32, u_max: f32, v_max: f32, fg: Color, bg: Color, is_color: bool| {
            let fg_alpha = if is_color { 0.0 } else { 1.0 };
            let fg_f = [fg.r as f32 / 255.0, fg.g as f32 / 255.0, fg.b as f32 / 255.0, fg_alpha];
            let bg_f = [bg.r as f32 / 255.0, bg.g as f32 / 255.0, bg.b as f32 / 255.0, 1.0];
            
            // Triangle 1
            vertices.extend_from_slice(&[x, y, u_min, v_min]);
            vertices.extend_from_slice(&fg_f); vertices.extend_from_slice(&bg_f);

            vertices.extend_from_slice(&[x + w, y, u_max, v_min]);
            vertices.extend_from_slice(&fg_f); vertices.extend_from_slice(&bg_f);

            vertices.extend_from_slice(&[x, y + h, u_min, v_max]);
            vertices.extend_from_slice(&fg_f); vertices.extend_from_slice(&bg_f);

            // Triangle 2
            vertices.extend_from_slice(&[x, y + h, u_min, v_max]);
            vertices.extend_from_slice(&fg_f); vertices.extend_from_slice(&bg_f);

            vertices.extend_from_slice(&[x + w, y, u_max, v_min]);
            vertices.extend_from_slice(&fg_f); vertices.extend_from_slice(&bg_f);

            vertices.extend_from_slice(&[x + w, y + h, u_max, v_max]);
            vertices.extend_from_slice(&fg_f); vertices.extend_from_slice(&bg_f);
        };

        let selection_active = selection.active;
        let ((sel_min_x, sel_min_y), (sel_max_x, sel_max_y)) = if selection_active {
            selection.normalized_bounds()
        } else {
            ((0, 0), (0, 0))
        };

        for y in 0..rows {
            let mut x = 0;
            while x < cols {
                let cell = cells[y * cols + x];
                if cell.flags.contains(CellFlags::WIDE_CONTINUATION) {
                    x += 1;
                    continue;
                }

                let is_cursor = cursor_visible && x == cursor_x && y == cursor_y;
                let is_selected = selection_active && selection.contains_fast(sel_min_x, sel_min_y, sel_max_x, sel_max_y, x, y);
                
                let mut cell_fg = cell.foreground;
                if bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
                    for i in 0..8 {
                        if cell_fg == theme.ansi_colors[i] {
                            cell_fg = theme.ansi_colors[i + 8];
                            break;
                        }
                    }
                }

                let is_inverted = (is_cursor && cursor_shape == crate::screen::cursor::CursorShape::Block)
                    || is_selected
                    || cell.flags.contains(CellFlags::REVERSE);

                let (mut fg, bg) = if is_inverted {
                    (cell.background, cell_fg)
                } else {
                    (cell_fg, cell.background)
                };

                if cell.flags.contains(CellFlags::DIM) {
                    fg.r = (fg.r as f32 * 0.6) as u8;
                    fg.g = (fg.g as f32 * 0.6) as u8;
                    fg.b = (fg.b as f32 * 0.6) as u8;
                }

                if cell.flags.contains(CellFlags::HIDDEN) || (cell.flags.contains(CellFlags::BLINK) && !blink_on) {
                    fg = bg;
                }

                let is_wide = cell.flags.contains(CellFlags::WIDE);
                let cell_w_mult = if is_wide { 2.0 } else { 1.0 };

                let is_bold = cell.flags.contains(CellFlags::BOLD);
                let is_italic = cell.flags.contains(CellFlags::ITALIC);
                let uv = self.font_loader.get_glyph_uv(cell.character, is_wide, is_bold, is_italic);
                let px = x as f32 * cw;
                let py = y as f32 * ch;
                push_quad(&mut vertices, px, py, cw * cell_w_mult, ch, uv.u_min, uv.v_min, uv.u_max, uv.v_max, fg, bg, uv.is_color);

                if is_cursor {
                    let (u, v) = self.font_loader.white_pixel_uv();
                    let cursor_color = cell_fg;
                    match cursor_shape {
                        crate::screen::cursor::CursorShape::Beam => {
                            let beam_w = (cw * 0.15).max(2.0);
                            push_quad(&mut vertices, px, py, beam_w, ch, u, v, u, v, cursor_color, cursor_color, false);
                        }
                        crate::screen::cursor::CursorShape::Underline => {
                            let thick = (ch * 0.2).max(3.0);
                            push_quad(&mut vertices, px, py + ch - thick, cw * cell_w_mult, thick, u, v, u, v, cursor_color, cursor_color, false);
                        }
                        crate::screen::cursor::CursorShape::Block => {}
                    }
                }

                if cell.flags.contains(CellFlags::UNDERLINE) {
                    let thickness = 1.0f32.max((ch * 0.08).round());
                    let line_y = py + ch - thickness - 1.0;
                    let (u, v) = self.font_loader.white_pixel_uv();
                    push_quad(&mut vertices, px, line_y, cw * cell_w_mult, thickness, u, v, u, v, fg, bg, false);
                }

                if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                    let thickness = 1.0f32.max((ch * 0.08).round());
                    let line_y2 = py + ch - thickness - 1.0;
                    let line_y1 = line_y2 - thickness - 1.5;
                    let (u, v) = self.font_loader.white_pixel_uv();
                    push_quad(&mut vertices, px, line_y1, cw * cell_w_mult, thickness, u, v, u, v, fg, bg, false);
                    push_quad(&mut vertices, px, line_y2, cw * cell_w_mult, thickness, u, v, u, v, fg, bg, false);
                }

                if cell.flags.contains(CellFlags::CURLY_UNDERLINE) {
                    let thickness = 1.0f32.max((ch * 0.08).round());
                    let line_y = py + ch - thickness - 1.0;
                    let (u, v) = self.font_loader.white_pixel_uv();
                    let wave_w = cw * cell_w_mult;
                    let step = 2.0f32;
                    let mut sx = 0.0f32;
                    while sx < wave_w {
                        let angle = (sx / wave_w) * std::f32::consts::PI * 4.0;
                        let wave_offset = angle.sin() * thickness * 0.5;
                        let draw_w = step.min(wave_w - sx);
                        push_quad(&mut vertices, px + sx, line_y + wave_offset, draw_w, thickness, u, v, u, v, fg, bg, false);
                        sx += step;
                    }
                }

                if cell.flags.contains(CellFlags::STRIKE) {
                    let thickness = 1.0f32.max((ch * 0.08).round());
                    let line_y = py + (ch / 2.0).round() - (thickness / 2.0).round();
                    let (u, v) = self.font_loader.white_pixel_uv();
                    push_quad(&mut vertices, px, line_y, cw * cell_w_mult, thickness, u, v, u, v, fg, bg, false);
                }

                x += if is_wide { 2 } else { 1 };
            }
        }

        unsafe {
            self.gl.clear_color(
                theme.default_bg.r as f32 / 255.0,
                theme.default_bg.g as f32 / 255.0,
                theme.default_bg.b as f32 / 255.0,
                1.0,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            self.gl.use_program(Some(self.program));

            // Orthographic projection matrix: maps (0,0) to top-left and (width,height) to bottom-right
            let ortho = [
                2.0 / self.viewport_width as f32, 0.0, 0.0, 0.0,
                0.0, -2.0 / self.viewport_height as f32, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                -1.0, 1.0, 0.0, 1.0,
            ];
            let proj_loc = self.gl.get_uniform_location(self.program, "u_projection");
            self.gl.uniform_matrix_4_f32_slice(proj_loc.as_ref(), false, &ortho);

            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.font_loader.atlas_texture));
            let sampler_loc = self.gl.get_uniform_location(self.program, "u_atlas");
            self.gl.uniform_1_i32(sampler_loc.as_ref(), 0);

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&vertices), glow::DYNAMIC_DRAW);

            self.gl.draw_arrays(glow::TRIANGLES, 0, (vertices.len() / 12) as i32);
        }
        self.vertices = vertices;
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
        }
    }
}
