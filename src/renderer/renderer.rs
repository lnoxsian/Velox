use crate::app::pane::PaneId;
use crate::app::split::{PaneRect, SeparatorRect, SplitDirection};
use crate::font::loader::FontLoader;
use crate::screen::cell::{Cell, CellFlags, Color};
use crate::screen::cursor::CursorShape;
use crate::theme::theme::Theme;
use glow::HasContext;
use std::sync::Arc;

use std::collections::HashMap;

pub struct PaneRenderData<'a> {
    pub pane_id: PaneId,
    pub rect: PaneRect,
    pub grid: Option<&'a crate::screen::grid::Grid>,
    pub cells: &'a [Cell],
    pub row_offset: usize,
    pub cols: usize,
    pub rows: usize,
    pub font_size: f32,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cursor_visible: bool,
    pub cursor_shape: CursorShape,
    pub theme: &'a Theme,
    pub bold_is_bright: bool,
    pub selection: &'a crate::screen::selection::Selection,
    pub scroll_offset: usize,
    pub history_len: usize,
    pub is_active: bool,
}

#[inline(always)]
fn with_pane_row_slice<R>(
    pane: &PaneRenderData,
    y: usize,
    f: impl FnOnce(&[Cell]) -> R,
) -> Option<R> {
    if let Some(grid) = pane.grid {
        grid.with_display_row_slice(y, f)
    } else {
        let physical_y = (y + pane.row_offset) % pane.rows;
        let start = physical_y * pane.cols;
        let end = (start + pane.cols).min(pane.cells.len());
        if start < pane.cells.len() {
            Some(f(&pane.cells[start..end]))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SeparatorRenderData {
    pub rect: SeparatorRect,
    pub is_active: bool,
    pub active_segment: Option<(f32, f32)>,
    pub is_hovered: bool,
    pub is_dragging: bool,
}

pub struct Renderer {
    gl: Arc<glow::Context>,
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    pub font_loader: FontLoader,
    pub tab_font_loader: FontLoader,
    pub pane_font_loaders: HashMap<u32, FontLoader>,
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
    dim: f32,
) -> (Color, Color) {
    let mut cell_fg = cell.foreground;
    let cell_bg = cell.background;

    // Bold-bright: remap dim ANSI fg to its bright counterpart
    if bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
        for i in 0..8 {
            if cell_fg == theme.ansi_colors[i] {
                cell_fg = theme.ansi_colors[i + 8];
                break;
            }
        }
    }

    let (mut fg, bg) = if is_inverted {
        let mut inv_fg = cell_bg;
        let mut inv_bg = cell_fg;

        let lum_fg = 0.299 * inv_fg.r as f32 + 0.587 * inv_fg.g as f32 + 0.114 * inv_fg.b as f32;
        let lum_bg = 0.299 * inv_bg.r as f32 + 0.587 * inv_bg.g as f32 + 0.114 * inv_bg.b as f32;

        let lum_theme_bg = 0.299 * theme.default_bg.r as f32
            + 0.587 * theme.default_bg.g as f32
            + 0.114 * theme.default_bg.b as f32;

        // When both colors are dark, both are light, or contrast is too low
        if (lum_fg < 128.0 && lum_bg < 128.0)
            || (lum_fg >= 128.0 && lum_bg >= 128.0)
            || (lum_fg - lum_bg).abs() < 30.0
        {
            if lum_theme_bg < 128.0 {
                // Dark theme: invert to light background
                inv_bg = theme.default_fg;
                let lum_orig_fg =
                    0.299 * cell_fg.r as f32 + 0.587 * cell_fg.g as f32 + 0.114 * cell_fg.b as f32;
                inv_fg = if lum_orig_fg < 120.0 {
                    cell_fg
                } else {
                    theme.default_bg
                };
            } else {
                // Light theme: invert to dark background
                inv_bg = theme.default_fg;
                let lum_orig_fg =
                    0.299 * cell_fg.r as f32 + 0.587 * cell_fg.g as f32 + 0.114 * cell_fg.b as f32;
                inv_fg = if lum_orig_fg >= 135.0 {
                    cell_fg
                } else {
                    theme.default_bg
                };
            }
        }

        (inv_fg.dim(dim), inv_bg)
    } else {
        (cell_fg.dim(dim), cell_bg)
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
    let alpha = if is_color { 0.0 } else { 1.0 };
    let cr = color.r as f32 / 255.0;
    let cg = color.g as f32 / 255.0;
    let cb = color.b as f32 / 255.0;
    let x2 = x + w;
    let y2 = y + h;

    let quad = [
        // Triangle 1
        x, y, u_min, v_min, cr, cg, cb, alpha, x2, y, u_max, v_min, cr, cg, cb, alpha, x, y2, u_min,
        v_max, cr, cg, cb, alpha, // Triangle 2
        x, y2, u_min, v_max, cr, cg, cb, alpha, x2, y, u_max, v_min, cr, cg, cb, alpha, x2, y2,
        u_max, v_max, cr, cg, cb, alpha,
    ];
    vertices.extend_from_slice(&quad);
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
            let tab_font_loader = font_loader.create_tab_loader(font_size);

            let viewport_width = viewport_width.max(1);
            let viewport_height = viewport_height.max(1);
            gl.viewport(0, 0, viewport_width as i32, viewport_height as i32);

            Self {
                gl,
                program,
                vao,
                vbo,
                font_loader,
                tab_font_loader,
                pane_font_loaders: HashMap::new(),
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

    pub fn set_tab_font_size(&mut self, font_size: f32) {
        self.tab_font_loader.update_font_size(font_size);
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
        window_dim: f32,
        tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>,
    ) {
        let cw = self.font_loader.cell_width as f32;
        let ch = self.font_loader.cell_height as f32;
        let bar_h = if let Some(tb) = tab_bar_info {
            tb.height
        } else {
            0.0
        };
        let pane_w = (cols as f32 * cw).max(1.0);
        let pane_h = (rows as f32 * ch).max(1.0);
        let pane_rect = PaneRect {
            pane_id: 0,
            x: 0.0,
            y: bar_h,
            width: pane_w + padding_x * 2.0,
            height: pane_h + padding_y * 2.0,
            padding_x,
            padding_y,
            cols,
            rows,
            cell_width: cw,
            cell_height: ch,
        };
        let pane_data = PaneRenderData {
            pane_id: 0,
            rect: pane_rect,
            grid: None,
            cells,
            row_offset: 0,
            cols,
            rows,
            font_size: self.font_loader.font_size,
            cursor_x,
            cursor_y,
            cursor_visible,
            cursor_shape,
            theme,
            bold_is_bright,
            selection,
            scroll_offset,
            history_len,
            is_active: true,
        };
        self.draw_splits(
            &[pane_data],
            &[],
            opacity,
            window_dim,
            tab_bar_info,
            None,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_splits(
        &mut self,
        panes: &[PaneRenderData],
        separators: &[SeparatorRenderData],
        opacity: f32,
        window_dim: f32,
        tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>,
        separator_color_override: Option<Color>,
        active_separator_color_override: Option<Color>,
    ) {
        if panes.is_empty() {
            return;
        }

        let effective_dim = window_dim.clamp(0.0, 1.0);

        let active_pane = panes.iter().find(|p| p.is_active).unwrap_or(&panes[0]);
        let base_theme = active_pane.theme;

        // Reuse the vertex buffer allocation across frames
        let mut vertices = std::mem::take(&mut self.vertices);
        vertices.clear();
        let total_cells: usize = panes.iter().map(|p| p.cols * p.rows).sum();
        let needed = (total_cells + (separators.len() + 10) * 2) * 12 * 8;
        if vertices.capacity() < needed {
            vertices.reserve(needed);
        }

        // Pre-ensure font loaders exist for all panes
        for pane in panes {
            let font_size = pane.font_size;
            let key = (font_size * 100.0).round() as u32;
            if (self.font_loader.font_size - font_size).abs() >= 0.01
                && !self.pane_font_loaders.contains_key(&key)
            {
                let loader = self.font_loader.create_pane_loader(font_size);
                self.pane_font_loaders.insert(key, loader);
            }
        }

        let mut draw_batches: Vec<(i32, i32, glow::Texture)> = Vec::with_capacity(panes.len() + 2);

        // Blink: toggle every 500 ms
        let blink_on = (self.start_time.elapsed().as_millis() / 500).is_multiple_of(2);

        // White pixel UV (top-left 2×2 solid region in the atlas) used for solid quads
        let (wu, wv) = self.font_loader.white_pixel_uv();

        // ── Pass 1: Background quads for all panes ────────────────────────────
        let pass1_start = (vertices.len() / 8) as i32;
        for pane in panes {
            let cw = pane.rect.cell_width;
            let ch = pane.rect.cell_height;
            let cols = pane.cols;
            let rows = pane.rows;
            let history_len = pane.history_len;
            let scroll_offset = pane.scroll_offset;
            let selection_active =
                pane.is_active && pane.selection.active && !pane.selection.is_empty();
            let ((sel_min_x, sel_min_abs_y), (sel_max_x, sel_max_abs_y)) = if selection_active {
                pane.selection.normalized_bounds()
            } else {
                ((0, 0), (0, 0))
            };

            let pane_effective_dim = if !pane.is_active {
                (effective_dim * 0.5 + 0.1).clamp(0.0, 1.0)
            } else {
                effective_dim
            };
            let pane_theme_dimmed;
            let pane_theme = if pane_effective_dim > 0.0 {
                pane_theme_dimmed = pane.theme.dimmed(pane_effective_dim);
                &pane_theme_dimmed
            } else {
                pane.theme
            };

            // Fill entire pane rectangle default background to eliminate gaps
            push_quad(
                &mut vertices,
                pane.rect.x,
                pane.rect.y,
                pane.rect.width,
                pane.rect.height,
                wu,
                wv,
                wu,
                wv,
                pane_theme.default_bg,
                false,
            );

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

                let mut span_start_x: Option<usize> = None;
                let mut span_end_x: usize = 0;
                let mut span_bg = pane_theme.default_bg;

                let is_active_grid_row = is_row_valid && abs_row >= history_len;
                let grid_y = if is_active_grid_row { abs_row - history_len } else { 0 };

                let default_cell = Cell {
                    character: ' ',
                    foreground: pane_theme.default_fg,
                    background: pane_theme.default_bg,
                    underline_color: None,
                    flags: CellFlags::empty(),
                };

                with_pane_row_slice(pane, y, |row_cells| {
                    let mut x = 0;
                    while x < cols {
                        let cell = if x < row_cells.len() {
                            row_cells[x]
                        } else {
                            default_cell
                        };
                        if cell.flags.contains(CellFlags::WIDE_CONTINUATION) {
                            x += 1;
                            continue;
                        }

                        let is_wide = cell.flags.contains(CellFlags::WIDE);
                        let is_cursor = pane.is_active
                            && pane.cursor_visible
                            && is_active_grid_row
                            && x == pane.cursor_x
                            && grid_y == pane.cursor_y;
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
                        let is_inverted = is_selected ^ is_reversed;

                        let (_fg, mut bg) = compute_cell_colors(
                            &cell,
                            is_inverted,
                            pane.bold_is_bright,
                            pane_theme,
                            pane_effective_dim,
                        );

                        let is_block_cursor =
                            is_cursor && pane.cursor_shape == CursorShape::Block && pane.is_active;
                        if is_block_cursor {
                            let mut cell_fg = cell.foreground.dim(pane_effective_dim);
                            if pane.bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
                                for i in 0..8 {
                                    if cell_fg == pane_theme.ansi_colors[i] {
                                        cell_fg = pane_theme.ansi_colors[i + 8];
                                        break;
                                    }
                                }
                            }
                            bg = pane_theme.resolve_cursor_color(cell_fg);
                        }

                        let next_x = x + if is_wide { 2 } else { 1 };

                        if bg != pane_theme.default_bg || is_inverted || is_block_cursor {
                            if let Some(start_x) = span_start_x {
                                if bg == span_bg {
                                    span_end_x = next_x;
                                } else {
                                    let px = pane.rect.x + pane.rect.padding_x + start_x as f32 * cw;
                                    let py = pane.rect.y + pane.rect.padding_y + y as f32 * ch;
                                    let span_w = (span_end_x - start_x) as f32 * cw;
                                    push_quad(&mut vertices, px, py, span_w, ch, wu, wv, wu, wv, span_bg, false);
                                    span_start_x = Some(x);
                                    span_end_x = next_x;
                                    span_bg = bg;
                                }
                            } else {
                                span_start_x = Some(x);
                                span_end_x = next_x;
                                span_bg = bg;
                            }
                        } else if let Some(start_x) = span_start_x {
                            let px = pane.rect.x + pane.rect.padding_x + start_x as f32 * cw;
                            let py = pane.rect.y + pane.rect.padding_y + y as f32 * ch;
                            let span_w = (span_end_x - start_x) as f32 * cw;
                            push_quad(&mut vertices, px, py, span_w, ch, wu, wv, wu, wv, span_bg, false);
                            span_start_x = None;
                        }

                        x = next_x;
                    }
                });

                if let Some(start_x) = span_start_x {
                    let px = pane.rect.x + pane.rect.padding_x + start_x as f32 * cw;
                    let py = pane.rect.y + pane.rect.padding_y + y as f32 * ch;
                    let span_w = (span_end_x - start_x) as f32 * cw;
                    push_quad(&mut vertices, px, py, span_w, ch, wu, wv, wu, wv, span_bg, false);
                }
            }
        }

        // ── Pass 1.5: Separators ──────────────────────────────────────────────
        for sep in separators {
            let active_color = active_separator_color_override
                .unwrap_or_else(|| base_theme.resolve_tab_accent_color());
            let inactive_color = separator_color_override.unwrap_or(base_theme.ansi_colors[8]);

            if sep.is_dragging || sep.is_hovered {
                push_quad(
                    &mut vertices,
                    sep.rect.x,
                    sep.rect.y,
                    sep.rect.width,
                    sep.rect.height,
                    wu,
                    wv,
                    wu,
                    wv,
                    active_color,
                    false,
                );
            } else if let Some((start, end)) = sep.active_segment {
                match sep.rect.direction {
                    SplitDirection::Vertical => {
                        // Inactive top segment
                        if start > sep.rect.y + 0.5 {
                            push_quad(
                                &mut vertices,
                                sep.rect.x,
                                sep.rect.y,
                                sep.rect.width,
                                start - sep.rect.y,
                                wu,
                                wv,
                                wu,
                                wv,
                                inactive_color,
                                false,
                            );
                        }
                        // Active middle segment
                        if end > start + 0.5 {
                            push_quad(
                                &mut vertices,
                                sep.rect.x,
                                start,
                                sep.rect.width,
                                end - start,
                                wu,
                                wv,
                                wu,
                                wv,
                                active_color,
                                false,
                            );
                        }
                        // Inactive bottom segment
                        let bottom_y = sep.rect.y + sep.rect.height;
                        if bottom_y > end + 0.5 {
                            push_quad(
                                &mut vertices,
                                sep.rect.x,
                                end,
                                sep.rect.width,
                                bottom_y - end,
                                wu,
                                wv,
                                wu,
                                wv,
                                inactive_color,
                                false,
                            );
                        }
                    }
                    SplitDirection::Horizontal => {
                        // Inactive left segment
                        if start > sep.rect.x + 0.5 {
                            push_quad(
                                &mut vertices,
                                sep.rect.x,
                                sep.rect.y,
                                start - sep.rect.x,
                                sep.rect.height,
                                wu,
                                wv,
                                wu,
                                wv,
                                inactive_color,
                                false,
                            );
                        }
                        // Active middle segment
                        if end > start + 0.5 {
                            push_quad(
                                &mut vertices,
                                start,
                                sep.rect.y,
                                end - start,
                                sep.rect.height,
                                wu,
                                wv,
                                wu,
                                wv,
                                active_color,
                                false,
                            );
                        }
                        // Inactive right segment
                        let right_x = sep.rect.x + sep.rect.width;
                        if right_x > end + 0.5 {
                            push_quad(
                                &mut vertices,
                                end,
                                sep.rect.y,
                                right_x - end,
                                sep.rect.height,
                                wu,
                                wv,
                                wu,
                                wv,
                                inactive_color,
                                false,
                            );
                        }
                    }
                }
            } else {
                push_quad(
                    &mut vertices,
                    sep.rect.x,
                    sep.rect.y,
                    sep.rect.width,
                    sep.rect.height,
                    wu,
                    wv,
                    wu,
                    wv,
                    inactive_color,
                    false,
                );
            }
        }

        let pass1_count = (vertices.len() / 8) as i32 - pass1_start;
        if pass1_count > 0 {
            draw_batches.push((pass1_start, pass1_count, self.font_loader.atlas_texture));
        }

        // ── Pass 2: Foreground glyphs + cursor + decorations for each pane ────
        for pane in panes {
            let pane_start = (vertices.len() / 8) as i32;
            let font_size = pane.font_size;
            let key = (font_size * 100.0).round() as u32;
            let font_loader = if (self.font_loader.font_size - font_size).abs() < 0.01 {
                &mut self.font_loader
            } else {
                self.pane_font_loaders.get_mut(&key).unwrap()
            };
            let atlas_texture = font_loader.atlas_texture;
            let (wu, wv) = font_loader.white_pixel_uv();

            let cw = pane.rect.cell_width;
            let ch = pane.rect.cell_height;
            let cols = pane.cols;
            let rows = pane.rows;
            let history_len = pane.history_len;
            let scroll_offset = pane.scroll_offset;
            let selection_active =
                pane.is_active && pane.selection.active && !pane.selection.is_empty();
            let ((sel_min_x, sel_min_abs_y), (sel_max_x, sel_max_abs_y)) = if selection_active {
                pane.selection.normalized_bounds()
            } else {
                ((0, 0), (0, 0))
            };

            let pane_effective_dim = if !pane.is_active {
                (effective_dim * 0.5 + 0.1).clamp(0.0, 1.0)
            } else {
                effective_dim
            };
            let pane_theme_dimmed;
            let pane_theme = if pane_effective_dim > 0.0 {
                pane_theme_dimmed = pane.theme.dimmed(pane_effective_dim);
                &pane_theme_dimmed
            } else {
                pane.theme
            };

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

                let is_active_grid_row = is_row_valid && abs_row >= history_len;
                let grid_y = if is_active_grid_row { abs_row - history_len } else { 0 };
                let default_cell = Cell {
                    character: ' ',
                    foreground: pane_theme.default_fg,
                    background: pane_theme.default_bg,
                    underline_color: None,
                    flags: CellFlags::empty(),
                };

                with_pane_row_slice(pane, y, |row_cells| {
                    let mut x = 0;
                    while x < cols {
                        let cell = if x < row_cells.len() {
                            row_cells[x]
                        } else {
                            default_cell
                        };
                        if cell.flags.contains(CellFlags::WIDE_CONTINUATION) {
                            x += 1;
                            continue;
                        }

                        let is_wide = cell.flags.contains(CellFlags::WIDE);
                        let is_bold = cell.flags.contains(CellFlags::BOLD);
                        let is_italic = cell.flags.contains(CellFlags::ITALIC);
                        let is_cursor = pane.is_active
                            && pane.cursor_visible
                            && is_active_grid_row
                            && x == pane.cursor_x
                            && grid_y == pane.cursor_y;
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
                        let is_inverted = is_selected ^ is_reversed;

                        let (mut fg, _bg) = compute_cell_colors(
                            &cell,
                            is_inverted,
                            pane.bold_is_bright,
                            pane_theme,
                            pane_effective_dim,
                        );

                        let mut cell_fg = cell.foreground.dim(pane_effective_dim);
                        if pane.bold_is_bright && is_bold {
                            for i in 0..8 {
                                if cell_fg == pane_theme.ansi_colors[i] {
                                    cell_fg = pane_theme.ansi_colors[i + 8];
                                    break;
                                }
                            }
                        }

                        if is_cursor && pane.cursor_shape == CursorShape::Block && pane.is_active {
                            fg = pane_theme
                                .resolve_cursor_text_color(cell.background)
                                .dim(pane_effective_dim);
                        }

                        let px = pane.rect.x + pane.rect.padding_x + x as f32 * cw;
                        let py = pane.rect.y + pane.rect.padding_y + y as f32 * ch;
                        let cell_w_mult = if is_wide { 2.0 } else { 1.0 };

                        // ── Glyph ──
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
                                let uv = font_loader.get_glyph_uv(
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

                        // ── Underline ──
                        if cell.flags.contains(CellFlags::UNDERLINE) {
                            let thick = (ch * 0.08).max(1.0);
                            let ul_fg = cell
                                .underline_color
                                .map(|c| c.dim(pane_effective_dim))
                                .unwrap_or(fg);
                            let line_y = py + ch - thick;
                            push_quad(
                                &mut vertices,
                                px,
                                line_y,
                                cw * cell_w_mult,
                                thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                ul_fg,
                                false,
                            );
                        }

                        // ── Cursor ──
                        if is_cursor && pane.is_active {
                            let cursor_color = pane_theme.resolve_cursor_color(cell_fg);
                            match pane.cursor_shape {
                                CursorShape::Block => {}
                                CursorShape::HollowBlock => {
                                    let thick = 1.0f32.max((ch * 0.05).floor());
                                    // Top
                                    push_quad(
                                        &mut vertices,
                                        px,
                                        py,
                                        cw * cell_w_mult,
                                        thick,
                                        wu,
                                        wv,
                                        wu,
                                        wv,
                                        cursor_color,
                                        false,
                                    );
                                    // Bottom
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
                                    // Left
                                    push_quad(
                                        &mut vertices,
                                        px,
                                        py + thick,
                                        thick,
                                        ch - thick * 2.0,
                                        wu,
                                        wv,
                                        wu,
                                        wv,
                                        cursor_color,
                                        false,
                                    );
                                    // Right
                                    push_quad(
                                        &mut vertices,
                                        px + cw * cell_w_mult - thick,
                                        py + thick,
                                        thick,
                                        ch - thick * 2.0,
                                        wu,
                                        wv,
                                        wu,
                                        wv,
                                        cursor_color,
                                        false,
                                    );
                                }
                                CursorShape::Beam => {
                                    let beam_w = 2.0f32.max((cw * 0.1).floor());
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
                                    let thick = 2.0f32.max((ch * 0.1).floor());
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
                            }
                        }

                        // ── Decorations ──
                        let deco_thick = 1.0f32.max((ch * 0.045).floor());
                        let ul_color = cell
                            .underline_color
                            .map(|c| c.dim(pane_effective_dim))
                            .unwrap_or(fg);

                        if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                            let double_thick = 1.0f32.max((ch * 0.035).floor());
                            let gap = 1.0f32.max((ch * 0.045).floor());
                            let line_y2 = py + ch - double_thick - 1.0;
                            let line_y1 = line_y2 - gap - double_thick;
                            push_quad(
                                &mut vertices,
                                px,
                                line_y1,
                                cw * cell_w_mult,
                                double_thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                ul_color,
                                false,
                            );
                            push_quad(
                                &mut vertices,
                                px,
                                line_y2,
                                cw * cell_w_mult,
                                double_thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                ul_color,
                                false,
                            );
                        }

                        if cell.flags.contains(CellFlags::CURLY_UNDERLINE) {
                            let curly_thick = 2.0f32.max((ch * 0.08).floor());
                            let wave_period = (cw * 0.75).clamp(6.0, 10.0);
                            let wave_amp = (ch * 0.05).clamp(1.0, 1.8);
                            let base_y = py + ch - wave_amp - curly_thick;
                            let wave_w = cw * cell_w_mult;
                            let step = 1.0f32;
                            let mut wx = 0.0f32;
                            while wx < wave_w {
                                let rel_x = px + wx;
                                let phase = (rel_x / wave_period) * std::f32::consts::TAU;
                                let wave_offset = phase.sin() * wave_amp;
                                let wy = base_y + wave_offset;
                                let segment_w = step.min(wave_w - wx);
                                push_quad(
                                    &mut vertices,
                                    rel_x,
                                    wy,
                                    segment_w,
                                    curly_thick,
                                    wu,
                                    wv,
                                    wu,
                                    wv,
                                    ul_color,
                                    false,
                                );
                                wx += step;
                            }
                        }

                        if cell.flags.contains(CellFlags::STRIKE) {
                            let strike_y = py + (ch * 0.5).floor();
                            push_quad(
                                &mut vertices,
                                px,
                                strike_y,
                                cw * cell_w_mult,
                                deco_thick,
                                wu,
                                wv,
                                wu,
                                wv,
                                ul_color,
                                false,
                            );
                        }

                        x += if is_wide { 2 } else { 1 };
                    }
                });
            }

            let pane_count = (vertices.len() / 8) as i32 - pane_start;
            if pane_count > 0 {
                draw_batches.push((pane_start, pane_count, atlas_texture));
            }
        }

        if let Some(tab_bar) = tab_bar_info {
            let tab_start = (vertices.len() / 8) as i32;
            let theme = base_theme;
            let bar_h = tab_bar.height;
            let tab_count = tab_bar.tabs.len();
            if tab_count > 0 {
                let (wu, wv) = self.tab_font_loader.white_pixel_uv();
                let tab_w = tab_bar.compute_tab_width(self.viewport_width as f32);
                let tab_cw = self.tab_font_loader.cell_width as f32;
                let tab_ch = self.tab_font_loader.cell_height as f32;

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

                // 2. Individual tabs
                for (i, tab) in tab_bar.tabs.iter().enumerate() {
                    let tab_x = i as f32 * tab_w;
                    let is_active = tab.is_active;
                    let is_hovered = tab.is_hovered;

                    let (tab_bg, title_fg) = if is_active {
                        let active_bg = Color {
                            r: (theme.default_bg.r as f32 * 1.15).min(255.0) as u8,
                            g: (theme.default_bg.g as f32 * 1.15).min(255.0) as u8,
                            b: (theme.default_bg.b as f32 * 1.15).min(255.0) as u8,
                        };
                        (active_bg, theme.default_fg)
                    } else if is_hovered {
                        let hover_bg = Color {
                            r: (theme.default_bg.r as f32 * 0.85) as u8,
                            g: (theme.default_bg.g as f32 * 0.85) as u8,
                            b: (theme.default_bg.b as f32 * 0.85) as u8,
                        };
                        let hover_fg = Color {
                            r: (theme.default_fg.r as f32 * 0.8) as u8,
                            g: (theme.default_fg.g as f32 * 0.8) as u8,
                            b: (theme.default_fg.b as f32 * 0.8) as u8,
                        };
                        (hover_bg, hover_fg)
                    } else {
                        let inactive_bg = Color {
                            r: (theme.default_bg.r as f32 * 0.6) as u8,
                            g: (theme.default_bg.g as f32 * 0.6) as u8,
                            b: (theme.default_bg.b as f32 * 0.6) as u8,
                        };
                        let inactive_fg = Color {
                            r: (theme.default_fg.r as f32 * 0.5) as u8,
                            g: (theme.default_fg.g as f32 * 0.5) as u8,
                            b: (theme.default_fg.b as f32 * 0.5) as u8,
                        };
                        (inactive_bg, inactive_fg)
                    };

                    push_quad(
                        &mut vertices,
                        tab_x,
                        0.0,
                        tab_w,
                        bar_h,
                        wu,
                        wv,
                        wu,
                        wv,
                        tab_bg,
                        false,
                    );

                    if is_active {
                        let accent_color = theme.tab_accent_color.unwrap_or(theme.ansi_colors[4]);
                        let line_h = 2.0;
                        push_quad(
                            &mut vertices,
                            tab_x,
                            bar_h - line_h,
                            tab_w,
                            line_h,
                            wu,
                            wv,
                            wu,
                            wv,
                            accent_color,
                            false,
                        );
                    }

                    let close_btn_w = if tab_bar.show_close_button {
                        tab_cw * 2.0
                    } else {
                        0.0
                    };
                    let title_max_w = tab_w - close_btn_w - tab_cw * 2.0;
                    let max_chars = ((title_max_w / tab_cw).floor() as usize).max(1);

                    let title_text = if tab.title.chars().count() > max_chars {
                        let truncated: String = tab
                            .title
                            .chars()
                            .take(max_chars.saturating_sub(1))
                            .collect();
                        format!("{}…", truncated)
                    } else {
                        tab.title.clone()
                    };

                    let title_y = (bar_h - tab_ch) / 2.0;
                    let mut cx = tab_x + tab_cw;
                    for ch in title_text.chars() {
                        if cx + tab_cw > tab_x + tab_w - close_btn_w {
                            break;
                        }
                        if ch != ' ' {
                            let uv = self.tab_font_loader.get_glyph_uv(ch, false, false, false);
                            push_quad(
                                &mut vertices,
                                cx,
                                title_y,
                                tab_cw,
                                tab_ch,
                                uv.u_min,
                                uv.v_min,
                                uv.u_max,
                                uv.v_max,
                                title_fg,
                                uv.is_color,
                            );
                        }
                        cx += tab_cw;
                    }

                    if tab_bar.show_close_button {
                        let close_hovered = tab.is_close_hovered;
                        let close_fg = if close_hovered {
                            theme.ansi_colors[1]
                        } else {
                            Color {
                                r: (theme.default_fg.r as f32 * 0.4) as u8,
                                g: (theme.default_fg.g as f32 * 0.4) as u8,
                                b: (theme.default_fg.b as f32 * 0.4) as u8,
                            }
                        };

                        let close_x = tab_x + tab_w - close_btn_w;
                        let close_y = (bar_h - tab_ch) / 2.0;
                        let uv = self.tab_font_loader.get_glyph_uv('×', false, false, false);
                        push_quad(
                            &mut vertices,
                            close_x,
                            close_y,
                            tab_cw,
                            tab_ch,
                            uv.u_min,
                            uv.v_min,
                            uv.u_max,
                            uv.v_max,
                            close_fg,
                            uv.is_color,
                        );
                    }

                    let sep_color = Color {
                        r: (theme.default_bg.r as f32 * 0.8) as u8,
                        g: (theme.default_bg.g as f32 * 0.8) as u8,
                        b: (theme.default_bg.b as f32 * 0.8) as u8,
                    };
                    push_quad(
                        &mut vertices,
                        tab_x + tab_w - 1.0,
                        4.0,
                        1.0,
                        bar_h - 8.0,
                        wu,
                        wv,
                        wu,
                        wv,
                        sep_color,
                        false,
                    );
                }

                if tab_bar.show_new_tab {
                    let total_tabs_w = tab_w * tab_count as f32;
                    let new_tab_x = total_tabs_w + 4.0;
                    let new_tab_y = (bar_h - tab_ch) / 2.0;
                    let new_tab_fg = if tab_bar.is_new_tab_hovered {
                        theme.default_fg
                    } else {
                        Color {
                            r: (theme.default_fg.r as f32 * 0.5) as u8,
                            g: (theme.default_fg.g as f32 * 0.5) as u8,
                            b: (theme.default_fg.b as f32 * 0.5) as u8,
                        }
                    };

                    let uv = self.tab_font_loader.get_glyph_uv('+', false, false, false);
                    push_quad(
                        &mut vertices,
                        new_tab_x,
                        new_tab_y,
                        tab_cw,
                        tab_ch,
                        uv.u_min,
                        uv.v_min,
                        uv.u_max,
                        uv.v_max,
                        new_tab_fg,
                        uv.is_color,
                    );
                }
            }

            let tab_count = (vertices.len() / 8) as i32 - tab_start;
            if tab_count > 0 {
                draw_batches.push((tab_start, tab_count, self.tab_font_loader.atlas_texture));
            }
        }

        // ── GPU draw call ─────────────────────────────────────────────────────
        unsafe {
            let ortho: [f32; 16] = [
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

            let sampler_loc = self.gl.get_uniform_location(self.program, "u_atlas");
            self.gl.uniform_1_i32(sampler_loc.as_ref(), 0);

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::DYNAMIC_DRAW,
            );

            let alpha = opacity.clamp(0.0, 1.0);
            self.gl.clear_color(
                (base_theme.default_bg.r as f32 / 255.0) * alpha,
                (base_theme.default_bg.g as f32 / 255.0) * alpha,
                (base_theme.default_bg.b as f32 / 255.0) * alpha,
                alpha,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            self.gl.use_program(Some(self.program));

            for (start, count, texture) in draw_batches {
                self.gl.active_texture(glow::TEXTURE0);
                self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                self.gl.draw_arrays(glow::TRIANGLES, start, count);
            }
        }

        // Retain the vertex buffer if within 2x the current viewport needs;
        // otherwise shrink to current needs to prevent unbounded growth.
        let current_viewport_capacity = total_cells * 6 * 8;
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
        self.tab_font_loader.release_memory();
        for loader in self.pane_font_loaders.values_mut() {
            loader.release_memory();
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
