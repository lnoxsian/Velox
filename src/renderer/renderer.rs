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
}

impl Renderer {
    pub fn new(gl: Arc<glow::Context>, font_family: &str, font_size: f32) -> Self {
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
                    float mask = texture(u_atlas, v_tex).r;
                    FragColor = mix(v_bg, v_fg, mask);
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

            let font_loader = FontLoader::new(gl.clone(), font_family, font_size);

            Self {
                gl,
                program,
                vao,
                vbo,
                font_loader,
                viewport_width: 800,
                viewport_height: 600,
            }
        }
    }

    pub fn initialize() -> Self {
        panic!("Use Renderer::new instead of initialize")
    }

    pub fn draw_frame(&mut self) {
        // stub
    }

    pub fn draw_cursor(&mut self) {
        // stub
    }

    pub fn draw_selection(&mut self) {
        // stub
    }

    pub fn flush(&mut self) {
        // stub
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
        }
    }

    pub fn draw(&mut self, cells: &[Cell], cols: usize, rows: usize, cursor_x: usize, cursor_y: usize, cursor_visible: bool) {
        let cw = self.font_loader.cell_width as f32;
        let ch = self.font_loader.cell_height as f32;
        let mut vertices: Vec<f32> = Vec::with_capacity(cells.len() * 6 * 12);

        let push_quad = |vertices: &mut Vec<f32>, x: f32, y: f32, w: f32, h: f32, u_min: f32, v_min: f32, u_max: f32, v_max: f32, fg: Color, bg: Color| {
            let fg_f = [fg.r as f32 / 255.0, fg.g as f32 / 255.0, fg.b as f32 / 255.0, 1.0];
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

        for y in 0..rows {
            for x in 0..cols {
                let cell = cells[y * cols + x];
                let is_cursor = cursor_visible && x == cursor_x && y == cursor_y;
                
                let (fg, bg) = if is_cursor {
                    (cell.background, cell.foreground)
                } else if cell.flags.contains(CellFlags::REVERSE) {
                    (cell.background, cell.foreground)
                } else {
                    (cell.foreground, cell.background)
                };

                let uv = self.font_loader.get_glyph_uv(cell.character);
                let px = x as f32 * cw;
                let py = y as f32 * ch;
                push_quad(&mut vertices, px, py, cw, ch, uv.u_min, uv.v_min, uv.u_max, uv.v_max, fg, bg);
            }
        }

        unsafe {
            self.gl.clear_color(0.15, 0.15, 0.15, 1.0);
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
