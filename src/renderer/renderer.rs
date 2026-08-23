use crate::font::loader::FontLoader;
use crate::screen::cell::{Cell, CellFlags, Color};
use crate::screen::cursor::CursorShape;
use crate::theme::theme::Theme;
use glow::HasContext;
use std::sync::Arc;

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

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Resolve the effective fg/bg colors for a cell, accounting for bold-bright
/// remapping, selection/cursor/reverse inversion, and dim attenuation.
fn compute_cell_colors(
    cell: &Cell,
    is_inverted: bool,
    bold_is_bright: bool,
    theme: &Theme,
) -> (Color, Color) {
    // Bold-bright: remap dim ANSI fg to its bright counterpart
    let mut cell_fg = cell.foreground;
    if bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
        for i in 0..8 {
            if cell_fg == theme.ansi_colors[i] {
                cell_fg = theme.ansi_colors[i + 8];
                break;
            }
        }
    }

    let (mut fg, bg) = if is_inverted {
        (cell.background, cell_fg)
    } else {
        (cell_fg, cell.background)
    };

    // Dim: attenuate foreground to 60%
    if cell.flags.contains(CellFlags::DIM) {
        fg.r = (fg.r as f32 * 0.6) as u8;
        fg.g = (fg.g as f32 * 0.6) as u8;
        fg.b = (fg.b as f32 * 0.6) as u8;
    }

    (fg, bg)
}

/// Append a textured quad (two triangles) to the vertex buffer.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn push_quad(
    vertices: &mut Vec<f32>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    u_min: f32,
    v_min: f32,
    u_max: f32,
    v_max: f32,
    color: Color,
    is_color: bool,
) {
    // alpha == 0 signals the fragment shader to render the texture as-is (emoji/color glyphs).
    // alpha == 1 uses the texture red channel as coverage mask tinted by color.
    let alpha = if is_color { 0.0 } else { 1.0 };
    let c = [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        alpha,
    ];

    // Triangle 1
    vertices.extend_from_slice(&[x, y, u_min, v_min]);
    vertices.extend_from_slice(&c);
    vertices.extend_from_slice(&[x + w, y, u_max, v_min]);
    vertices.extend_from_slice(&c);
    vertices.extend_from_slice(&[x, y + h, u_min, v_max]);
    vertices.extend_from_slice(&c);
    // Triangle 2
    vertices.extend_from_slice(&[x, y + h, u_min, v_max]);
    vertices.extend_from_slice(&c);
    vertices.extend_from_slice(&[x + w, y, u_max, v_min]);
    vertices.extend_from_slice(&c);
    vertices.extend_from_slice(&[x + w, y + h, u_max, v_max]);
    vertices.extend_from_slice(&c);
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn try_render_block_element(
    c: char,
    px: f32,
    py: f32,
    cell_w: f32,
    cell_h: f32,
    wu: f32,
    wv: f32,
    fg: Color,
    vertices: &mut Vec<f32>,
) -> bool {
    match c {
        '█' => {
            push_quad(vertices, px, py, cell_w, cell_h, wu, wv, wu, wv, fg, false);
            true
        }
        '\u{2581}'..='\u{2587}' => {
            let frac = (c as u32 - 0x2580) as f32 / 8.0;
            let h = cell_h * frac;
            push_quad(
                vertices,
                px,
                py + cell_h - h,
                cell_w,
                h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{2589}' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w * (7.0 / 8.0),
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{258A}' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w * (6.0 / 8.0),
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{258B}' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w * (5.0 / 8.0),
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '▌' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w * 0.5,
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{258D}' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w * (3.0 / 8.0),
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{258E}' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w * (2.0 / 8.0),
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{258F}' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w * (1.0 / 8.0),
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '▀' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w,
                cell_h * 0.5,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{2594}' => {
            push_quad(
                vertices,
                px,
                py,
                cell_w,
                cell_h * (1.0 / 8.0),
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '▐' => {
            push_quad(
                vertices,
                px + cell_w * 0.5,
                py,
                cell_w * 0.5,
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        '\u{2595}' => {
            push_quad(
                vertices,
                px + cell_w * (7.0 / 8.0),
                py,
                cell_w * (1.0 / 8.0),
                cell_h,
                wu,
                wv,
                wu,
                wv,
                fg,
                false,
            );
            true
        }
        _ => false,
    }
}

// ─── Renderer ────────────────────────────────────────────────────────────────

impl Renderer {
    pub fn new(
        gl: Arc<glow::Context>,
        font_family: &str,
        font_size: f32,
        font_scale_multiplier: f32,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        unsafe {
            // ── Shaders ──────────────────────────────────────────────────────
            let vertex_src = r#"
                #version 330 core
                layout (location = 0) in vec2 a_pos;
                layout (location = 1) in vec2 a_tex;
                layout (location = 2) in vec4 a_color;
                out vec2 v_tex;
                out vec4 v_color;
                uniform mat4 u_projection;
                void main() {
                    gl_Position = u_projection * vec4(a_pos, 0.0, 1.0);
                    v_tex = a_tex;
                    v_color = a_color;
                }
            "#;

            let fragment_src = r#"
                #version 330 core
                in vec2 v_tex;
                in vec4 v_color;
                out vec4 FragColor;
                uniform sampler2D u_atlas;
                void main() {
                    vec4 tex_color = texture(u_atlas, v_tex);
                    // a_color.a < 0.5 → color glyph (emoji): use texture directly.
                    // Otherwise: tint by v_color using texture red channel as alpha mask.
                    if (v_color.a < 0.5) {
                        FragColor = tex_color;
                    } else {
                        FragColor = vec4(v_color.rgb, tex_color.r);
                    }
                }
            "#;

            let compile_shader = |gl: &glow::Context, kind, src| {
                let sh = gl.create_shader(kind).unwrap();
                gl.shader_source(sh, src);
                gl.compile_shader(sh);
                if !gl.get_shader_compile_status(sh) {
                    panic!("Shader compilation failed: {}", gl.get_shader_info_log(sh));
                }
                sh
            };

            let vs = compile_shader(&gl, glow::VERTEX_SHADER, vertex_src);
            let fs = compile_shader(&gl, glow::FRAGMENT_SHADER, fragment_src);

            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!(
                    "Shader program linking failed: {}",
                    gl.get_program_info_log(program)
                );
            }
            gl.delete_shader(vs);
            gl.delete_shader(fs);

            // ── VAO / VBO ─────────────────────────────────────────────────────
            // Vertex layout: [x, y, u, v, r, g, b, a] — 8 floats per vertex
            let vao = gl.create_vertex_array().unwrap();
            let vbo = gl.create_buffer().unwrap();
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

            let stride = 8 * std::mem::size_of::<f32>() as i32;
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0); // a_pos
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(
                1,
                2,
                glow::FLOAT,
                false,
                stride,
                2 * std::mem::size_of::<f32>() as i32,
            ); // a_tex
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(
                2,
                4,
                glow::FLOAT,
                false,
                stride,
                4 * std::mem::size_of::<f32>() as i32,
            ); // a_color
            gl.enable_vertex_attrib_array(2);

            gl.enable(glow::BLEND);
            gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );

            let font_loader =
                FontLoader::new(gl.clone(), font_family, font_size, font_scale_multiplier);

            let viewport_width = viewport_width.max(1);
            let viewport_height = viewport_height.max(1);
            gl.viewport(0, 0, viewport_width as i32, viewport_height as i32);

            Self {
                gl,
                program,
                vao,
                vbo,
                font_loader,
                viewport_width,
                viewport_height,
                start_time: std::time::Instant::now(),
                vertices: Vec::with_capacity(80 * 24 * 6 * 8),
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.viewport_width = width;
        self.viewport_height = height;
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
        }
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_loader.update_font_size(font_size);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        cells: &[Cell],
        cols: usize,
        rows: usize,
        cursor_x: usize,
        cursor_y: usize,
        cursor_visible: bool,
        cursor_shape: CursorShape,
        theme: &Theme,
        bold_is_bright: bool,
        selection: &crate::screen::selection::Selection,
        scroll_offset: usize,
        history_len: usize,
        padding_x: f32,
        padding_y: f32,
        opacity: f32,
        tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>,
    ) {
        let cw = self.font_loader.cell_width as f32;
        let ch = self.font_loader.cell_height as f32;
        let effective_padding_y = if let Some(tb) = tab_bar_info {
            padding_y + tb.height
        } else {
            padding_y
        };

        // Reuse the vertex buffer allocation across frames
        let mut vertices = std::mem::take(&mut self.vertices);
        vertices.clear();
        let needed = cells.len() * 6 * 8;
        if vertices.capacity() < needed {
            vertices.reserve(needed);
        }

        // Blink: toggle every 500 ms
        let blink_on = (self.start_time.elapsed().as_millis() / 500).is_multiple_of(2);

        // Pre-compute selection bounds once to avoid re-normalizing per cell
        let selection_active = selection.active;
        let ((sel_min_x, sel_min_abs_y), (sel_max_x, sel_max_abs_y)) = if selection_active {
            selection.normalized_bounds()
        } else {
            ((0, 0), (0, 0))
        };

        // White pixel UV (top-left 2×2 solid region in the atlas) used for solid quads
        let (wu, wv) = self.font_loader.white_pixel_uv();

        // ── Pass 1: Background quads ──────────────────────────────────────────
        for y in 0..rows {
            let abs_y = y + history_len;
            let (is_row_valid, abs_row) = if abs_y >= scroll_offset {
                (true, abs_y - scroll_offset)
            } else {
                (false, 0)
            };
            let is_row_in_selection = selection_active
                && is_row_valid
                && abs_row >= sel_min_abs_y
                && abs_row <= sel_max_abs_y;

            let mut x = 0;
            while x < cols {
                let cell = cells[y * cols + x];
                if cell.flags.contains(CellFlags::WIDE_CONTINUATION) {
                    x += 1;
                    continue;
                }

                let is_wide = cell.flags.contains(CellFlags::WIDE);
                let is_cursor = cursor_visible && x == cursor_x && y == cursor_y;
                let is_selected = if is_row_in_selection {
                    if sel_min_abs_y == sel_max_abs_y {
                        x >= sel_min_x && x <= sel_max_x
                    } else if abs_row == sel_min_abs_y {
                        x >= sel_min_x
                    } else if abs_row == sel_max_abs_y {
                        x <= sel_max_x
                    } else {
                        true
                    }
                } else {
                    false
                };
                let is_reversed = cell.flags.contains(CellFlags::REVERSE);
                let is_inverted = is_selected || is_reversed;

                let (_fg, mut bg) = compute_cell_colors(&cell, is_inverted, bold_is_bright, theme);

                let is_block_cursor = is_cursor && cursor_shape == CursorShape::Block;
                if is_block_cursor {
                    let mut cell_fg = cell.foreground;
                    if bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
                        for i in 0..8 {
                            if cell_fg == theme.ansi_colors[i] {
                                cell_fg = theme.ansi_colors[i + 8];
                                break;
                            }
                        }
                    }
                    bg = theme.resolve_cursor_color(cell_fg);
                }

                // Skip background quad if the cell has the default background and is not inverted/selected/cursor,
                // because gl.clear_color already cleared the entire window (including margins) with default_bg + opacity.
                if bg != theme.default_bg || is_inverted || is_block_cursor {
                    let px = padding_x + x as f32 * cw;
                    let py = effective_padding_y + y as f32 * ch;
                    let cell_w_mult = if is_wide { 2.0 } else { 1.0 };
                    push_quad(
                        &mut vertices,
                        px,
                        py,
                        cw * cell_w_mult,
                        ch,
                        wu,
                        wv,
                        wu,
                        wv,
                        bg,
                        false,
                    );
                }

                x += if is_wide { 2 } else { 1 };
            }
        }

        // ── Pass 2: Foreground glyphs + decorations ───────────────────────────
        for y in 0..rows {
            let abs_y = y + history_len;
            let (is_row_valid, abs_row) = if abs_y >= scroll_offset {
                (true, abs_y - scroll_offset)
            } else {
                (false, 0)
            };
            let is_row_in_selection = selection_active
                && is_row_valid
                && abs_row >= sel_min_abs_y
                && abs_row <= sel_max_abs_y;

            let mut x = 0;
            while x < cols {
                let cell = cells[y * cols + x];
                if cell.flags.contains(CellFlags::WIDE_CONTINUATION) {
                    x += 1;
                    continue;
                }

                let is_wide = cell.flags.contains(CellFlags::WIDE);
                let is_bold = cell.flags.contains(CellFlags::BOLD);
                let is_italic = cell.flags.contains(CellFlags::ITALIC);
                let is_cursor = cursor_visible && x == cursor_x && y == cursor_y;
                let is_selected = if is_row_in_selection {
                    if sel_min_abs_y == sel_max_abs_y {
                        x >= sel_min_x && x <= sel_max_x
                    } else if abs_row == sel_min_abs_y {
                        x >= sel_min_x
                    } else if abs_row == sel_max_abs_y {
                        x <= sel_max_x
                    } else {
                        true
                    }
                } else {
                    false
                };
                let is_reversed = cell.flags.contains(CellFlags::REVERSE);
                let is_inverted = is_selected || is_reversed;

                let (mut fg, _bg) = compute_cell_colors(&cell, is_inverted, bold_is_bright, theme);

                // Bold-bright cell_fg needed for cursor color (pre-dim value)
                let mut cell_fg = cell.foreground;
                if bold_is_bright && is_bold {
                    for i in 0..8 {
                        if cell_fg == theme.ansi_colors[i] {
                            cell_fg = theme.ansi_colors[i + 8];
                            break;
                        }
                    }
                }

                if is_cursor && cursor_shape == CursorShape::Block {
                    fg = theme.resolve_cursor_text_color(cell.background);
                }

                let px = padding_x + x as f32 * cw;
                let py = effective_padding_y + y as f32 * ch;
                let cell_w_mult = if is_wide { 2.0 } else { 1.0 };

                // ── Glyph ─────────────────────────────────────────────────────
                let skip_fg = cell.flags.contains(CellFlags::HIDDEN)
                    || (cell.flags.contains(CellFlags::BLINK) && !blink_on);

                if !skip_fg && cell.character != ' ' {
                    let quad_w = cw * cell_w_mult;
                    if !try_render_block_element(
                        cell.character,
                        px,
                        py,
                        quad_w,
                        ch,
                        wu,
                        wv,
                        fg,
                        &mut vertices,
                    ) {
                        let uv = self.font_loader.get_glyph_uv(
                            cell.character,
                            is_wide,
                            is_bold,
                            is_italic,
                        );
                        let quad_w_glyph = cw * uv.width_mult;
                        push_quad(
                            &mut vertices,
                            px,
                            py,
                            quad_w_glyph,
                            ch,
                            uv.u_min,
                            uv.v_min,
                            uv.u_max,
                            uv.v_max,
                            fg,
                            uv.is_color,
                        );
                    }
                }

                // ── Cursor (non-block shapes) ─────────────────────────────────
                if is_cursor {
                    let cursor_color = theme.resolve_cursor_color(cell_fg);
                    match cursor_shape {
                        CursorShape::Beam => {
                            let beam_w = (cw * 0.15).max(2.0);
                            push_quad(
                                &mut vertices,
                                px,
                                py,
                                beam_w,
                                ch,
                                wu,
                                wv,
                                wu,
                                wv,
                                cursor_color,
                                false,
                            );
                        }
                        CursorShape::Underline => {
                            let thick = (ch * 0.2).max(3.0);
                            push_quad(
                                &mut vertices,
                                px,
                                py + ch - thick,
                                cw * cell_w_mult,
                                thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                cursor_color,
                                false,
                            );
                        }
                        CursorShape::HollowBlock => {
                            let thick = 1.5f32;
                            let block_w = cw * cell_w_mult;
                            // Top border
                            push_quad(
                                &mut vertices,
                                px,
                                py,
                                block_w,
                                thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                cursor_color,
                                false,
                            );
                            // Bottom border
                            push_quad(
                                &mut vertices,
                                px,
                                py + ch - thick,
                                block_w,
                                thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                cursor_color,
                                false,
                            );
                            // Left border
                            push_quad(
                                &mut vertices,
                                px,
                                py + thick,
                                thick,
                                ch - 2.0 * thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                cursor_color,
                                false,
                            );
                            // Right border
                            push_quad(
                                &mut vertices,
                                px + block_w - thick,
                                py + thick,
                                thick,
                                ch - 2.0 * thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                cursor_color,
                                false,
                            );
                        }
                        CursorShape::Block => {} // handled as background in Pass 1
                    }
                }

                // ── Text decorations ──────────────────────────────────────────
                let deco_thick = 1.0f32.max((ch * 0.08).round());

                if cell.flags.contains(CellFlags::UNDERLINE) {
                    let line_y = py + ch - deco_thick - 1.0;
                    push_quad(
                        &mut vertices,
                        px,
                        line_y,
                        cw * cell_w_mult,
                        deco_thick,
                        wu,
                        wv,
                        wu,
                        wv,
                        fg,
                        false,
                    );
                }

                if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                    let line_y2 = py + ch - deco_thick - 1.0;
                    let line_y1 = line_y2 - deco_thick - 1.5;
                    push_quad(
                        &mut vertices,
                        px,
                        line_y1,
                        cw * cell_w_mult,
                        deco_thick,
                        wu,
                        wv,
                        wu,
                        wv,
                        fg,
                        false,
                    );
                    push_quad(
                        &mut vertices,
                        px,
                        line_y2,
                        cw * cell_w_mult,
                        deco_thick,
                        wu,
                        wv,
                        wu,
                        wv,
                        fg,
                        false,
                    );
                }

                if cell.flags.contains(CellFlags::CURLY_UNDERLINE) {
                    let line_y = py + ch - deco_thick - 1.0;
                    let wave_w = cw * cell_w_mult;
                    let step = 2.0f32;
                    let mut sx = 0.0f32;
                    while sx < wave_w {
                        let angle = (sx / wave_w) * std::f32::consts::PI * 4.0;
                        let wave_offset = angle.sin() * deco_thick * 0.5;
                        let draw_w = step.min(wave_w - sx);
                        push_quad(
                            &mut vertices,
                            px + sx,
                            line_y + wave_offset,
                            draw_w,
                            deco_thick,
                            wu,
                            wv,
                            wu,
                            wv,
                            fg,
                            false,
                        );
                        sx += step;
                    }
                }

                if cell.flags.contains(CellFlags::STRIKE) {
                    let line_y = py + (ch / 2.0).round() - (deco_thick / 2.0).round();
                    push_quad(
                        &mut vertices,
                        px,
                        line_y,
                        cw * cell_w_mult,
                        deco_thick,
                        wu,
                        wv,
                        wu,
                        wv,
                        fg,
                        false,
                    );
                }

                x += if is_wide { 2 } else { 1 };
            }
        }

        // ── Pass 3: Tab Bar (if visible) ───────────────────────────────────────
        if let Some(tab_bar) = tab_bar_info {
            let bar_h = tab_bar.height;
            let tab_count = tab_bar.tabs.len();
            if tab_count > 0 {
                let tab_w = tab_bar.compute_tab_width(self.viewport_width as f32);

                // 1. Tab bar background strip
                let tab_bar_bg = Color {
                    r: (theme.default_bg.r as f32 * 0.6) as u8,
                    g: (theme.default_bg.g as f32 * 0.6) as u8,
                    b: (theme.default_bg.b as f32 * 0.6) as u8,
                };
                push_quad(
                    &mut vertices,
                    0.0,
                    0.0,
                    self.viewport_width as f32,
                    bar_h,
                    wu,
                    wv,
                    wu,
                    wv,
                    tab_bar_bg,
                    false,
                );

                // 2. Render each tab
                for (i, tab) in tab_bar.tabs.iter().enumerate() {
                    let tab_x = (i as f32) * tab_w;
                    let actual_w = tab_w - 2.0;

                    let tab_bg = if tab.is_active {
                        theme.default_bg
                    } else if tab.is_hovered {
                        Color {
                            r: (theme.default_bg.r as f32 * 0.85) as u8,
                            g: (theme.default_bg.g as f32 * 0.85) as u8,
                            b: (theme.default_bg.b as f32 * 0.85) as u8,
                        }
                    } else {
                        Color {
                            r: (theme.default_bg.r as f32 * 0.72) as u8,
                            g: (theme.default_bg.g as f32 * 0.72) as u8,
                            b: (theme.default_bg.b as f32 * 0.72) as u8,
                        }
                    };

                    push_quad(
                        &mut vertices,
                        tab_x,
                        0.0,
                        actual_w,
                        bar_h,
                        wu,
                        wv,
                        wu,
                        wv,
                        tab_bg,
                        false,
                    );

                    // Active tab accent line at top
                    if tab.is_active {
                        let accent = theme.resolve_tab_accent_color();
                        push_quad(
                            &mut vertices,
                            tab_x,
                            0.0,
                            actual_w,
                            2.0,
                            wu,
                            wv,
                            wu,
                            wv,
                            accent,
                            false,
                        );
                    }

                    // Tab title text
                    let text_fg = if tab.is_active {
                        theme.default_fg
                    } else {
                        Color {
                            r: (theme.default_fg.r as f32 * 0.7) as u8,
                            g: (theme.default_fg.g as f32 * 0.7) as u8,
                            b: (theme.default_fg.b as f32 * 0.7) as u8,
                        }
                    };

                    let close_space = if tab_bar.show_close_button { 24.0 } else { 8.0 };
                    let max_text_w = (actual_w - 16.0 - close_space).max(0.0);
                    let max_chars = (max_text_w / cw).floor() as usize;

                    let text_start_x = tab_x + 8.0;
                    let text_start_y = (bar_h - ch) / 2.0;

                    let char_count = tab.title.chars().count();
                    if char_count <= max_chars {
                        for (c_idx, ch_char) in tab.title.chars().enumerate() {
                            let char_x = text_start_x + (c_idx as f32) * cw;
                            let uv = self.font_loader.get_glyph_uv(ch_char, false, false, false);
                            push_quad(
                                &mut vertices,
                                char_x,
                                text_start_y,
                                cw * uv.width_mult,
                                ch,
                                uv.u_min,
                                uv.v_min,
                                uv.u_max,
                                uv.v_max,
                                text_fg,
                                uv.is_color,
                            );
                        }
                    } else if max_chars > 1 {
                        for (c_idx, ch_char) in tab.title.chars().take(max_chars - 1).enumerate() {
                            let char_x = text_start_x + (c_idx as f32) * cw;
                            let uv = self.font_loader.get_glyph_uv(ch_char, false, false, false);
                            push_quad(
                                &mut vertices,
                                char_x,
                                text_start_y,
                                cw * uv.width_mult,
                                ch,
                                uv.u_min,
                                uv.v_min,
                                uv.u_max,
                                uv.v_max,
                                text_fg,
                                uv.is_color,
                            );
                        }
                        let char_x = text_start_x + ((max_chars - 1) as f32) * cw;
                        let uv = self.font_loader.get_glyph_uv('…', false, false, false);
                        push_quad(
                            &mut vertices,
                            char_x,
                            text_start_y,
                            cw * uv.width_mult,
                            ch,
                            uv.u_min,
                            uv.v_min,
                            uv.u_max,
                            uv.v_max,
                            text_fg,
                            uv.is_color,
                        );
                    } else if max_chars == 1
                        && let Some(ch_char) = tab.title.chars().next()
                    {
                        let char_x = text_start_x;
                        let uv = self.font_loader.get_glyph_uv(ch_char, false, false, false);
                        push_quad(
                            &mut vertices,
                            char_x,
                            text_start_y,
                            cw * uv.width_mult,
                            ch,
                            uv.u_min,
                            uv.v_min,
                            uv.u_max,
                            uv.v_max,
                            text_fg,
                            uv.is_color,
                        );
                    }

                    // Close button
                    if tab_bar.show_close_button {
                        let close_x = tab_x + actual_w - 20.0;
                        let close_y = (bar_h - ch) / 2.0;
                        let close_fg = if tab.is_close_hovered {
                            theme.ansi_colors[1]
                        } else {
                            Color {
                                r: (theme.default_fg.r as f32 * 0.6) as u8,
                                g: (theme.default_fg.g as f32 * 0.6) as u8,
                                b: (theme.default_fg.b as f32 * 0.6) as u8,
                            }
                        };
                        let uv = self.font_loader.get_glyph_uv('×', false, false, false);
                        push_quad(
                            &mut vertices,
                            close_x,
                            close_y,
                            cw * uv.width_mult,
                            ch,
                            uv.u_min,
                            uv.v_min,
                            uv.u_max,
                            uv.v_max,
                            close_fg,
                            uv.is_color,
                        );
                    }
                }

                // 3. New Tab Button '+'
                if tab_bar.show_new_tab {
                    let btn_x = (tab_count as f32) * tab_w + 4.0;
                    let btn_w = 24.0;
                    let btn_h = bar_h - 4.0;
                    let btn_y = 2.0;

                    if tab_bar.is_new_tab_hovered {
                        let hover_bg = Color {
                            r: (theme.default_bg.r as f32 * 0.85) as u8,
                            g: (theme.default_bg.g as f32 * 0.85) as u8,
                            b: (theme.default_bg.b as f32 * 0.85) as u8,
                        };
                        push_quad(
                            &mut vertices,
                            btn_x,
                            btn_y,
                            btn_w,
                            btn_h,
                            wu,
                            wv,
                            wu,
                            wv,
                            hover_bg,
                            false,
                        );
                    }

                    let plus_fg = if tab_bar.is_new_tab_hovered {
                        theme.default_fg
                    } else {
                        Color {
                            r: (theme.default_fg.r as f32 * 0.7) as u8,
                            g: (theme.default_fg.g as f32 * 0.7) as u8,
                            b: (theme.default_fg.b as f32 * 0.7) as u8,
                        }
                    };
                    let plus_x = btn_x + (btn_w - cw) / 2.0;
                    let plus_y = (bar_h - ch) / 2.0;
                    let uv = self.font_loader.get_glyph_uv('+', false, false, false);
                    push_quad(
                        &mut vertices,
                        plus_x,
                        plus_y,
                        cw * uv.width_mult,
                        ch,
                        uv.u_min,
                        uv.v_min,
                        uv.u_max,
                        uv.v_max,
                        plus_fg,
                        uv.is_color,
                    );
                }
            }
        }

        // ── GPU draw call ─────────────────────────────────────────────────────
        unsafe {
            self.gl.viewport(
                0,
                0,
                self.viewport_width as i32,
                self.viewport_height as i32,
            );
            let alpha = opacity.clamp(0.0, 1.0);
            self.gl.clear_color(
                (theme.default_bg.r as f32 / 255.0) * alpha,
                (theme.default_bg.g as f32 / 255.0) * alpha,
                (theme.default_bg.b as f32 / 255.0) * alpha,
                alpha,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            self.gl.use_program(Some(self.program));

            // Orthographic projection: (0,0) = top-left, (width,height) = bottom-right
            let ortho = [
                2.0 / self.viewport_width as f32,
                0.0,
                0.0,
                0.0,
                0.0,
                -2.0 / self.viewport_height as f32,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
                -1.0,
                1.0,
                0.0,
                1.0,
            ];
            let proj_loc = self.gl.get_uniform_location(self.program, "u_projection");
            self.gl
                .uniform_matrix_4_f32_slice(proj_loc.as_ref(), false, &ortho);

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.font_loader.atlas_texture));
            let sampler_loc = self.gl.get_uniform_location(self.program, "u_atlas");
            self.gl.uniform_1_i32(sampler_loc.as_ref(), 0);

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::DYNAMIC_DRAW,
            );
            self.gl
                .draw_arrays(glow::TRIANGLES, 0, (vertices.len() / 8) as i32);
        }

        // Retain the vertex buffer if within 2x the current viewport needs;
        // otherwise shrink to current needs to prevent unbounded growth.
        let current_viewport_capacity = cols * rows * 6 * 8;
        let max_retained = current_viewport_capacity * 2;
        if vertices.capacity() > max_retained {
            self.vertices = Vec::with_capacity(current_viewport_capacity);
        } else {
            self.vertices = vertices;
        }
    }

    /// Full memory cleanup: trims vertex capacity and prunes fallback font memory.
    pub fn release_memory(&mut self) {
        const DEFAULT_VERTEX_CAPACITY: usize = 80 * 24 * 6 * 8;
        if self.vertices.capacity() > DEFAULT_VERTEX_CAPACITY {
            self.vertices = Vec::with_capacity(DEFAULT_VERTEX_CAPACITY);
        } else {
            self.vertices.clear();
        }
        self.font_loader.release_memory();
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
