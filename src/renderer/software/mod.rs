use crate::app::pane::PaneId;
use crate::app::split::{PaneRect, SplitDirection};
use crate::renderer::renderer::SeparatorRenderData;
pub mod atlas;
pub mod color;
pub mod damage;
pub mod decorations;
pub mod framebuffer;
pub mod glyph;
pub mod primitives;
pub mod raster;

pub use color::{PackedColor, PrecomputedPalette};
pub use damage::DamageMap;
pub use framebuffer::Framebuffer;
pub use glyph::{GlyphCache, GlyphKey};

use crate::screen::cell::{Cell, CellFlags};
use crate::screen::cursor::CursorShape;
use crate::screen::grid::Grid;
use crate::theme::theme::Theme;
use decorations::{
    draw_curly_underline, draw_cursor, draw_double_underline, draw_strike, draw_underline,
};
use primitives::try_render_primitive;
use raster::{blit_alpha_glyph, blit_color_glyph};
use std::time::Instant;

use std::collections::HashMap;

pub struct CpuPaneRenderData<'a> {
    pub pane_id: PaneId,
    pub rect: PaneRect,
    pub cells: &'a [Cell],
    pub grid: &'a Grid,
    pub font_size: f32,
    pub theme: &'a Theme,
    pub cursor_visible: bool,
    pub cursor_shape: CursorShape,
    pub display_cursor_x: usize,
    pub is_active: bool,
}

/// Pure-Rust, retained CPU software renderer for Velox.
pub struct CpuRenderer {
    pub framebuffer: Framebuffer,
    pub glyph_cache: GlyphCache,
    pub tab_glyph_cache: GlyphCache,
    pub pane_glyph_caches: HashMap<u32, GlyphCache>,
    pub damage: DamageMap,
    pub palette: PrecomputedPalette,
    pub viewport_width: u32,
    pub viewport_height: u32,
    prev_theme_fg: crate::screen::cell::Color,
    prev_theme_bg: crate::screen::cell::Color,
    prev_ansi_colors: [crate::screen::cell::Color; 16],
    prev_cursor_color: Option<crate::screen::cell::Color>,
    prev_cursor_text_color: Option<crate::screen::cell::Color>,
    prev_tab_accent_color: Option<crate::screen::cell::Color>,
    bold_is_bright: bool,
    pub start_time: Instant,
    pub prev_blink_on: bool,
    pub opacity: f32,
    prev_opacity: f32,
    prev_dim: f32,
    prev_tab_bar_hash: u64,
}

impl CpuRenderer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        font_family: &str,
        font_size: f32,
        font_scale_multiplier: f32,
        theme: &Theme,
        width: u32,
        height: u32,
        bold_is_bright: bool,
        opacity: f32,
    ) -> Self {
        let glyph_cache =
            GlyphCache::from_font_family(font_family, font_size, font_scale_multiplier);
        let tab_glyph_cache = glyph_cache.create_tab_cache(font_size);
        let framebuffer = Framebuffer::new(width, height);
        let opacity = opacity.clamp(0.0, 1.0);
        let palette = PrecomputedPalette::new(theme, opacity);
        let rows = (height / glyph_cache.cell_height.max(1)).max(1) as usize;

        Self {
            framebuffer,
            glyph_cache,
            tab_glyph_cache,
            pane_glyph_caches: HashMap::new(),
            damage: DamageMap::new(rows),
            palette,
            viewport_width: width,
            viewport_height: height,
            prev_theme_fg: theme.default_fg,
            prev_theme_bg: theme.default_bg,
            prev_ansi_colors: theme.ansi_colors,
            prev_cursor_color: theme.cursor_color,
            prev_cursor_text_color: theme.cursor_text_color,
            prev_tab_accent_color: theme.tab_accent_color,
            bold_is_bright,
            start_time: Instant::now(),
            prev_blink_on: true,
            opacity,
            prev_opacity: opacity,
            prev_dim: 0.0,
            prev_tab_bar_hash: 0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
        if self.framebuffer.resize(width, height) {
            let rows = (height / self.glyph_cache.cell_height.max(1)).max(1) as usize;
            self.damage.resize(rows);
            self.damage.mark_all();
        }
    }

    pub fn update_font_size(&mut self, font_size: f32) {
        self.glyph_cache.update_font_size(font_size);
        let rows = (self.viewport_height / self.glyph_cache.cell_height.max(1)).max(1) as usize;
        self.damage.resize(rows);
        self.damage.mark_all();
    }

    pub fn set_tab_font_size(&mut self, font_size: f32) {
        self.tab_glyph_cache.update_font_size(font_size);
    }

    /// Full memory cleanup: compacts glyph atlas, prunes fallback fonts, and shrinks scratch buffers.
    pub fn release_memory(&mut self) {
        self.glyph_cache.release_memory();
        self.tab_glyph_cache.release_memory();
        for cache in self.pane_glyph_caches.values_mut() {
            cache.release_memory();
        }
        self.damage.mark_all();
    }

    /// Backwards-compatible render entry point (no tab bar). Delegates to `render_with_tab_bar`.
    /// Only used in unit tests.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        cells: &[Cell],
        grid: &Grid,
        theme: &Theme,
        padding_x: f32,
        padding_y: f32,
        cursor_visible: bool,
        cursor_shape: CursorShape,
        display_cursor_x: usize,
        is_focused: bool,
        opacity: f32,
        target_buffer: &mut [u32],
    ) {
        self.render_with_tab_bar(
            cells,
            grid,
            theme,
            padding_x,
            padding_y,
            cursor_visible,
            cursor_shape,
            display_cursor_x,
            is_focused,
            opacity,
            0.0,
            target_buffer,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_with_tab_bar(
        &mut self,
        cells: &[Cell],
        grid: &Grid,
        theme: &Theme,
        padding_x: f32,
        padding_y: f32,
        cursor_visible: bool,
        cursor_shape: CursorShape,
        display_cursor_x: usize,
        is_focused: bool,
        opacity: f32,
        window_dim: f32,
        target_buffer: &mut [u32],
        tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>,
    ) {
        let cw = self.glyph_cache.cell_width as f32;
        let ch = self.glyph_cache.cell_height as f32;
        let bar_h = if let Some(tb) = tab_bar_info {
            tb.height
        } else {
            0.0
        };
        let pane_w = (grid.width as f32 * cw).max(1.0);
        let pane_h = (grid.height as f32 * ch).max(1.0);
        let pane_rect = PaneRect {
            pane_id: 0,
            x: 0.0,
            y: bar_h,
            width: pane_w + padding_x * 2.0,
            height: pane_h + padding_y * 2.0,
            padding_x,
            padding_y,
            cols: grid.width,
            rows: grid.height,
            cell_width: cw,
            cell_height: ch,
        };
        let pane_data = CpuPaneRenderData {
            pane_id: 0,
            rect: pane_rect,
            cells,
            grid,
            font_size: self.glyph_cache.font_size,
            theme,
            cursor_visible,
            cursor_shape,
            display_cursor_x,
            is_active: true,
        };
        self.render_splits(
            &[pane_data],
            &[],
            opacity,
            window_dim,
            is_focused,
            target_buffer,
            tab_bar_info,
            None,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_splits(
        &mut self,
        panes: &[CpuPaneRenderData],
        separators: &[SeparatorRenderData],
        opacity: f32,
        window_dim: f32,
        is_focused: bool,
        target_buffer: &mut [u32],
        tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>,
        separator_color_override: Option<crate::screen::cell::Color>,
        active_separator_color_override: Option<crate::screen::cell::Color>,
    ) {
        if panes.is_empty() {
            return;
        }

        let active_pane = panes.iter().find(|p| p.is_active).unwrap_or(&panes[0]);
        let base_theme = active_pane.theme;

        let opacity = opacity.clamp(0.0, 1.0);
        let effective_dim = if !is_focused {
            window_dim.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let effective_theme = if effective_dim > 0.0 {
            base_theme.dimmed(effective_dim)
        } else {
            base_theme.clone()
        };
        let theme = &effective_theme;

        // 1. Sync theme updates
        if theme.default_fg != self.prev_theme_fg
            || theme.default_bg != self.prev_theme_bg
            || theme.ansi_colors != self.prev_ansi_colors
            || theme.cursor_color != self.prev_cursor_color
            || theme.cursor_text_color != self.prev_cursor_text_color
            || theme.tab_accent_color != self.prev_tab_accent_color
            || (opacity - self.prev_opacity).abs() > f32::EPSILON
            || (effective_dim - self.prev_dim).abs() > f32::EPSILON
        {
            self.palette = PrecomputedPalette::new(theme, opacity);
            self.prev_theme_fg = theme.default_fg;
            self.prev_theme_bg = theme.default_bg;
            self.prev_ansi_colors = theme.ansi_colors;
            self.prev_cursor_color = theme.cursor_color;
            self.prev_cursor_text_color = theme.cursor_text_color;
            self.prev_tab_accent_color = theme.tab_accent_color;
            self.opacity = opacity;
            self.prev_opacity = opacity;
            self.prev_dim = effective_dim;
            self.damage.mark_all();
        }

        // Check if any pane has damage
        let mut any_pane_damage = self.damage.full_redraw;
        for pane in panes {
            if pane.grid.damage.full_redraw || pane.grid.damage.dirty_rows.iter().any(|&d| d) {
                any_pane_damage = true;
                break;
            }
        }

        // Blink animation handling
        let blink_on = (self.start_time.elapsed().as_millis() / 500).is_multiple_of(2);
        let blink_changed = blink_on != self.prev_blink_on;
        self.prev_blink_on = blink_on;

        // Tab bar dirty hash
        let new_tab_bar_hash: u64 = tab_bar_info
            .map(|tb| {
                let mut h: u64 = tb.tabs.len() as u64;
                for t in &tb.tabs {
                    h = h.wrapping_mul(0x9e3779b97f4a7c15);
                    h ^= t.is_active as u64
                        | ((t.is_hovered as u64) << 1)
                        | ((t.is_close_hovered as u64) << 2);
                    h ^= t.title.len() as u64;
                    for &b in t.title.as_bytes().iter().take(16) {
                        h = h.wrapping_mul(0x517cc1b727220a95).wrapping_add(b as u64);
                    }
                }
                h ^ (tb.is_new_tab_hovered as u64)
            })
            .unwrap_or(0);
        let tab_bar_dirty = new_tab_bar_hash != self.prev_tab_bar_hash;

        let force_redraw =
            self.damage.full_redraw || any_pane_damage || blink_changed || tab_bar_dirty;

        if !force_redraw && !self.damage.has_damage() {
            if target_buffer.len() == self.framebuffer.pixels.len() {
                target_buffer.copy_from_slice(self.framebuffer.as_slice());
            }
            return;
        }

        if self.damage.full_redraw {
            self.framebuffer.clear(self.palette.default_bg);
        }

        // Render each pane
        for pane in panes {
            let font_size = pane.font_size;
            let cell_w = pane.rect.cell_width.round().max(1.0) as u32;
            let cell_h = pane.rect.cell_height.round().max(1.0) as u32;
            let grid = pane.grid;
            let cells = pane.cells;
            let grid_w = grid.width;
            let grid_h = grid.height;
            let history_len = grid.scrollback.len();
            let selection_bounds = grid.selection.normalized_bounds();
            let ((sel_min_x, sel_min_abs_y), (sel_max_x, sel_max_abs_y)) = selection_bounds;

            let pane_effective_dim = if !pane.is_active {
                (effective_dim * 0.5 + 0.15).clamp(0.0, 1.0)
            } else {
                effective_dim
            };

            let tile_x = pane.rect.x.round() as u32;
            let tile_y = pane.rect.y.round() as u32;
            let tile_w = pane.rect.width.round() as u32;
            let tile_h = pane.rect.height.round() as u32;
            let default_pane_bg = if pane.theme.default_bg == self.palette.raw_default_bg {
                self.palette.default_bg
            } else {
                let alpha = (opacity * 255.0).round() as u8;
                PackedColor::from_premultiplied(pane.theme.default_bg, alpha).to_u32()
            };

            let pane_full_redraw = self.damage.full_redraw || grid.damage.full_redraw;
            if pane_full_redraw {
                self.framebuffer
                    .fill_span(tile_x, tile_y, tile_w, tile_h, default_pane_bg);
            }

            let px_offset = (pane.rect.x + pane.rect.padding_x).round() as u32;
            let py_offset = (pane.rect.y + pane.rect.padding_y).round() as u32;

            for y in 0..grid_h {
                let row_dirty = pane_full_redraw
                    || grid.damage.dirty_rows.get(y).copied().unwrap_or(true)
                    || blink_changed;

                if !row_dirty {
                    continue;
                }

                let py = py_offset + (y as u32) * cell_h;
                if py + cell_h > self.framebuffer.height {
                    break;
                }

                if !pane_full_redraw {
                    self.framebuffer
                        .fill_span(tile_x, py, tile_w, cell_h, default_pane_bg);
                }

                let abs_y = y + history_len;
                let (is_row_valid, abs_row) = if abs_y >= grid.scroll_offset {
                    (true, abs_y - grid.scroll_offset)
                } else {
                    (false, 0)
                };
                let is_row_in_selection = pane.is_active
                    && grid.selection.active
                    && !grid.selection.is_empty()
                    && is_row_valid
                    && abs_row >= sel_min_abs_y
                    && abs_row <= sel_max_abs_y;
                let is_active_grid_row = is_row_valid && abs_row >= history_len;
                let grid_y = if is_active_grid_row {
                    abs_row - history_len
                } else {
                    0
                };

                let default_cell = Cell {
                    character: ' ',
                    foreground: pane.theme.default_fg,
                    background: pane.theme.default_bg,
                    underline_color: None,
                    flags: CellFlags::empty(),
                };

                grid.with_display_row_slice(y, |row_cells| {
                    // Pass A: Coalesced Background Spans
                    let mut span_start_col = 0usize;
                    let mut span_bg = 0u32;
                    let mut in_span = false;

                    for col in 0..grid_w {
                        let cell = if col < row_cells.len() {
                            &row_cells[col]
                        } else {
                            &default_cell
                        };

                        let is_selected = if is_row_in_selection {
                            if sel_min_abs_y == sel_max_abs_y {
                                col >= sel_min_x && col <= sel_max_x
                            } else if abs_row == sel_min_abs_y {
                                col >= sel_min_x
                            } else if abs_row == sel_max_abs_y {
                                col <= sel_max_x
                            } else {
                                true
                            }
                        } else {
                            false
                        };
                        let is_reverse = cell.flags.contains(CellFlags::REVERSE);
                        let is_inverted = is_selected ^ is_reverse;

                        let (_, mut bg) = self.palette.resolve_cell_colors_pane(
                            cell,
                            is_inverted,
                            self.bold_is_bright,
                            pane_effective_dim,
                            pane.theme,
                            default_pane_bg,
                        );

                        let is_cursor = pane.is_active
                            && pane.cursor_visible
                            && is_active_grid_row
                            && col == pane.display_cursor_x
                            && grid_y == grid.cursor.y;
                        let is_block_cursor = is_cursor
                            && pane.cursor_shape == CursorShape::Block
                            && pane.is_active
                            && is_focused;
                        if is_block_cursor {
                            let mut cell_fg = cell.foreground;
                            if self.bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
                                for i in 0..8 {
                                    if cell_fg == pane.theme.ansi_colors[i] {
                                        cell_fg = pane.theme.ansi_colors[i + 8];
                                        break;
                                    }
                                }
                            }
                            let cell_fg_dimmed = cell_fg.dim(pane_effective_dim);
                            bg = PackedColor::from_color(
                                pane.theme
                                    .resolve_cursor_color(cell_fg_dimmed)
                                    .dim(pane_effective_dim),
                            )
                            .to_u32();
                        }

                        if !in_span {
                            span_start_col = col;
                            span_bg = bg;
                            in_span = true;
                        } else if bg != span_bg {
                            let span_px = px_offset + (span_start_col as u32) * cell_w;
                            let span_w = ((col - span_start_col) as u32) * cell_w;
                            self.framebuffer
                                .fill_span(span_px, py, span_w, cell_h, span_bg);
                            span_start_col = col;
                            span_bg = bg;
                        }
                    }

                    if in_span {
                        let span_px = px_offset + (span_start_col as u32) * cell_w;
                        let span_w = ((grid_w - span_start_col) as u32) * cell_w;
                        self.framebuffer
                            .fill_span(span_px, py, span_w, cell_h, span_bg);
                    }

                    // Pass B: Glyphs, Primitives, Decorations
                    for (col, cell) in row_cells.iter().enumerate() {
                        if cell.flags.contains(CellFlags::WIDE_CONTINUATION) {
                            continue;
                        }

                        let px = px_offset + (col as u32) * cell_w;
                        if px + cell_w > self.framebuffer.width {
                            break;
                        }

                        let is_selected = if is_row_in_selection {
                            if sel_min_abs_y == sel_max_abs_y {
                                col >= sel_min_x && col <= sel_max_x
                            } else if abs_row == sel_min_abs_y {
                                col >= sel_min_x
                            } else if abs_row == sel_max_abs_y {
                                col <= sel_max_x
                            } else {
                                true
                            }
                        } else {
                            false
                        };
                        let is_reverse = cell.flags.contains(CellFlags::REVERSE);
                        let is_inverted = is_selected ^ is_reverse;

                        let (mut fg, _) = self.palette.resolve_cell_colors_pane(
                            cell,
                            is_inverted,
                            self.bold_is_bright,
                            pane_effective_dim,
                            pane.theme,
                            default_pane_bg,
                        );

                        let is_cursor = pane.is_active
                            && pane.cursor_visible
                            && is_active_grid_row
                            && col == pane.display_cursor_x
                            && grid_y == grid.cursor.y;
                        let is_block_cursor = is_cursor
                            && pane.cursor_shape == CursorShape::Block
                            && pane.is_active
                            && is_focused;
                        if is_block_cursor {
                            fg = PackedColor::from_color(
                                pane.theme
                                    .resolve_cursor_text_color(cell.background)
                                    .dim(pane_effective_dim),
                            )
                            .to_u32();
                        }

                        let skip_fg = cell.flags.contains(CellFlags::HIDDEN)
                            || (cell.flags.contains(CellFlags::BLINK) && !blink_on);

                        if !skip_fg && cell.character != ' ' {
                            let is_wide = cell.flags.contains(CellFlags::WIDE);
                            let target_w = if is_wide { cell_w * 2 } else { cell_w };

                            if !try_render_primitive(
                                cell.character,
                                px,
                                py,
                                target_w,
                                cell_h,
                                fg,
                                &mut self.framebuffer,
                            ) {
                                let is_bold = cell.flags.contains(CellFlags::BOLD);
                                let is_italic = cell.flags.contains(CellFlags::ITALIC);
                                let glyph_key =
                                    GlyphKey::new(cell.character, is_bold, is_italic, is_wide);

                                let glyph_cache = if (self.glyph_cache.font_size - font_size).abs()
                                    < 0.01
                                {
                                    &mut self.glyph_cache
                                } else {
                                    let key = (font_size * 100.0).round() as u32;
                                    if !self.pane_glyph_caches.contains_key(&key) {
                                        let cache = self.glyph_cache.create_pane_cache(font_size);
                                        self.pane_glyph_caches.insert(key, cache);
                                    }
                                    self.pane_glyph_caches.get_mut(&key).unwrap()
                                };

                                if let Some(glyph_ref) = glyph_cache.get_or_rasterize(glyph_key) {
                                    if glyph_ref.is_color {
                                        let pixels = glyph_cache.atlas.get_color(&glyph_ref);
                                        blit_color_glyph(
                                            &mut self.framebuffer,
                                            px,
                                            py,
                                            pixels,
                                            glyph_ref.width,
                                            glyph_ref.height,
                                        );
                                    } else {
                                        let mask = glyph_cache.atlas.get_alpha(&glyph_ref);
                                        blit_alpha_glyph(
                                            &mut self.framebuffer,
                                            px,
                                            py,
                                            mask,
                                            glyph_ref.width,
                                            glyph_ref.height,
                                            fg,
                                        );
                                    }
                                }
                            }
                        }

                        // Decorations
                        let ul_color = if let Some(uc) = cell.underline_color {
                            PackedColor::from_color(uc.dim(pane_effective_dim)).to_u32()
                        } else {
                            fg
                        };

                        if cell.flags.contains(CellFlags::UNDERLINE) {
                            draw_underline(&mut self.framebuffer, px, py, cell_w, cell_h, ul_color);
                        }
                        if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                            draw_double_underline(
                                &mut self.framebuffer,
                                px,
                                py,
                                cell_w,
                                cell_h,
                                ul_color,
                            );
                        }
                        if cell.flags.contains(CellFlags::CURLY_UNDERLINE) {
                            draw_curly_underline(
                                &mut self.framebuffer,
                                px,
                                py,
                                cell_w,
                                cell_h,
                                ul_color,
                            );
                        }
                        if cell.flags.contains(CellFlags::STRIKE) {
                            draw_strike(&mut self.framebuffer, px, py, cell_w, cell_h, ul_color);
                        }
                    }
                });

                // Render Cursor for this row (non-block or unfocused cursor)
                let cursor_y = grid.cursor.y;
                if pane.is_active
                    && y == cursor_y
                    && pane.cursor_visible
                    && (grid.scroll_offset == 0)
                    && (!is_focused || pane.cursor_shape != CursorShape::Block)
                {
                    let cursor_x = pane.display_cursor_x.min(grid_w.saturating_sub(1));
                    let cursor_px = px_offset + (cursor_x as u32) * cell_w;
                    let cursor_py = py_offset + (cursor_y as u32) * cell_h;
                    let cursor_shape = pane.cursor_shape;

                    let physical_cursor_y = (grid.row_offset + cursor_y) % grid_h;
                    let cell_idx = physical_cursor_y * grid_w + cursor_x;
                    let cursor_color = if cell_idx < cells.len() {
                        let cell = &cells[cell_idx];
                        let mut cell_fg = cell.foreground;
                        if self.bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
                            for i in 0..8 {
                                if cell_fg == pane.theme.ansi_colors[i] {
                                    cell_fg = pane.theme.ansi_colors[i + 8];
                                    break;
                                }
                            }
                        }
                        let cell_fg_dimmed = cell_fg.dim(pane_effective_dim);
                        PackedColor::from_color(
                            pane.theme
                                .resolve_cursor_color(cell_fg_dimmed)
                                .dim(pane_effective_dim),
                        )
                        .to_u32()
                    } else {
                        self.palette.default_fg
                    };

                    draw_cursor(
                        &mut self.framebuffer,
                        cursor_px,
                        cursor_py,
                        cell_w,
                        cell_h,
                        cursor_shape,
                        is_focused && pane.is_active,
                        cursor_color,
                    );
                }
            }
        }

        // Render Separators
        for sep in separators {
            let active_color = if sep.is_dragging || sep.is_hovered {
                active_separator_color_override
                    .map(|c| PackedColor::from_color(c).to_u32())
                    .unwrap_or(self.palette.tab_accent)
            } else {
                active_separator_color_override
                    .map(|c| PackedColor::from_color(c).to_u32())
                    .unwrap_or(self.palette.tab_accent)
            };
            let inactive_color = separator_color_override
                .map(|c| PackedColor::from_color(c).to_u32())
                .unwrap_or(self.palette.ansi_colors[8]);

            if sep.is_dragging || sep.is_hovered {
                self.framebuffer.fill_span(
                    sep.rect.x as u32,
                    sep.rect.y as u32,
                    sep.rect.width as u32,
                    sep.rect.height as u32,
                    active_color,
                );
            } else if let Some((start, end)) = sep.active_segment {
                match sep.rect.direction {
                    SplitDirection::Vertical => {
                        // Inactive top segment
                        if start > sep.rect.y + 0.5 {
                            self.framebuffer.fill_span(
                                sep.rect.x as u32,
                                sep.rect.y as u32,
                                sep.rect.width as u32,
                                (start - sep.rect.y) as u32,
                                inactive_color,
                            );
                        }
                        // Active middle segment
                        if end > start + 0.5 {
                            self.framebuffer.fill_span(
                                sep.rect.x as u32,
                                start as u32,
                                sep.rect.width as u32,
                                (end - start) as u32,
                                active_color,
                            );
                        }
                        // Inactive bottom segment
                        let bottom_y = sep.rect.y + sep.rect.height;
                        if bottom_y > end + 0.5 {
                            self.framebuffer.fill_span(
                                sep.rect.x as u32,
                                end as u32,
                                sep.rect.width as u32,
                                (bottom_y - end) as u32,
                                inactive_color,
                            );
                        }
                    }
                    SplitDirection::Horizontal => {
                        // Inactive left segment
                        if start > sep.rect.x + 0.5 {
                            self.framebuffer.fill_span(
                                sep.rect.x as u32,
                                sep.rect.y as u32,
                                (start - sep.rect.x) as u32,
                                sep.rect.height as u32,
                                inactive_color,
                            );
                        }
                        // Active middle segment
                        if end > start + 0.5 {
                            self.framebuffer.fill_span(
                                start as u32,
                                sep.rect.y as u32,
                                (end - start) as u32,
                                sep.rect.height as u32,
                                active_color,
                            );
                        }
                        // Inactive right segment
                        let right_x = sep.rect.x + sep.rect.width;
                        if right_x > end + 0.5 {
                            self.framebuffer.fill_span(
                                end as u32,
                                sep.rect.y as u32,
                                (right_x - end) as u32,
                                sep.rect.height as u32,
                                inactive_color,
                            );
                        }
                    }
                }
            } else {
                self.framebuffer.fill_span(
                    sep.rect.x as u32,
                    sep.rect.y as u32,
                    sep.rect.width as u32,
                    sep.rect.height as u32,
                    inactive_color,
                );
            }
        }

        // Render Tab Bar
        if let Some(tab_bar) = tab_bar_info {
            let bar_h = tab_bar.height as u32;
            let tab_count = tab_bar.tabs.len();
            if tab_count > 0 && bar_h > 0 {
                let tab_w = tab_bar.compute_tab_width(self.viewport_width as f32);
                let tab_cw = self.tab_glyph_cache.cell_width;
                let tab_ch = self.tab_glyph_cache.cell_height;

                let tab_bar_bg = self.palette.tab_bar_bg;
                self.framebuffer
                    .fill_span(0, 0, self.viewport_width, bar_h, tab_bar_bg);

                for (i, tab) in tab_bar.tabs.iter().enumerate() {
                    let tab_x = (i as f32 * tab_w) as u32;
                    let actual_w = (tab_w - 2.0).max(1.0) as u32;

                    let tab_bg = if tab.is_active {
                        self.palette.default_bg
                    } else if tab.is_hovered {
                        self.palette.tab_hover_bg
                    } else {
                        self.palette.tab_inactive_bg
                    };
                    self.framebuffer
                        .fill_span(tab_x, 0, actual_w, bar_h, tab_bg);

                    if tab.is_active {
                        let accent = self.palette.tab_accent;
                        self.framebuffer.fill_span(tab_x, 0, actual_w, 2, accent);
                    }

                    // Text title
                    let text_fg = if tab.is_active {
                        self.palette.default_fg
                    } else {
                        self.palette.tab_inactive_fg
                    };

                    let close_space = if tab_bar.show_close_button { 24.0 } else { 8.0 };
                    let max_text_w = (actual_w as f32 - 16.0 - close_space).max(0.0);
                    let max_chars = (max_text_w / (tab_cw as f32)).floor() as usize;

                    let text_start_x = tab_x + 8;
                    let text_start_y = (bar_h.saturating_sub(tab_ch)) / 2;

                    let char_count = tab.title.chars().count();
                    if char_count <= max_chars {
                        for (c_idx, ch_char) in tab.title.chars().enumerate() {
                            let char_px = text_start_x + (c_idx as u32) * tab_cw;
                            let key = GlyphKey::new(ch_char, false, false, false);
                            if let Some(glyph_ref) = self.tab_glyph_cache.get_or_rasterize(key) {
                                let mask = self.tab_glyph_cache.atlas.get_alpha(&glyph_ref);
                                blit_alpha_glyph(
                                    &mut self.framebuffer,
                                    char_px,
                                    text_start_y,
                                    mask,
                                    glyph_ref.width,
                                    glyph_ref.height,
                                    text_fg,
                                );
                            }
                        }
                    } else if max_chars > 1 {
                        for (c_idx, ch_char) in tab.title.chars().take(max_chars - 1).enumerate() {
                            let char_px = text_start_x + (c_idx as u32) * tab_cw;
                            let key = GlyphKey::new(ch_char, false, false, false);
                            if let Some(glyph_ref) = self.tab_glyph_cache.get_or_rasterize(key) {
                                let mask = self.tab_glyph_cache.atlas.get_alpha(&glyph_ref);
                                blit_alpha_glyph(
                                    &mut self.framebuffer,
                                    char_px,
                                    text_start_y,
                                    mask,
                                    glyph_ref.width,
                                    glyph_ref.height,
                                    text_fg,
                                );
                            }
                        }
                        let char_px = text_start_x + ((max_chars - 1) as u32) * tab_cw;
                        let key = GlyphKey::new('…', false, false, false);
                        if let Some(glyph_ref) = self.tab_glyph_cache.get_or_rasterize(key) {
                            let mask = self.tab_glyph_cache.atlas.get_alpha(&glyph_ref);
                            blit_alpha_glyph(
                                &mut self.framebuffer,
                                char_px,
                                text_start_y,
                                mask,
                                glyph_ref.width,
                                glyph_ref.height,
                                text_fg,
                            );
                        }
                    } else if max_chars == 1
                        && let Some(ch_char) = tab.title.chars().next()
                    {
                        let char_px = text_start_x;
                        let key = GlyphKey::new(ch_char, false, false, false);
                        if let Some(glyph_ref) = self.tab_glyph_cache.get_or_rasterize(key) {
                            let mask = self.tab_glyph_cache.atlas.get_alpha(&glyph_ref);
                            blit_alpha_glyph(
                                &mut self.framebuffer,
                                char_px,
                                text_start_y,
                                mask,
                                glyph_ref.width,
                                glyph_ref.height,
                                text_fg,
                            );
                        }
                    }

                    // Close button
                    if tab_bar.show_close_button {
                        let close_x = tab_x + actual_w.saturating_sub(20);
                        let close_y = (bar_h.saturating_sub(tab_ch)) / 2;
                        let close_fg = if tab.is_close_hovered {
                            self.palette.ansi_colors[1]
                        } else {
                            self.palette.tab_close_fg
                        };
                        let key = GlyphKey::new('×', false, false, false);
                        if let Some(glyph_ref) = self.tab_glyph_cache.get_or_rasterize(key) {
                            let mask = self.tab_glyph_cache.atlas.get_alpha(&glyph_ref);
                            blit_alpha_glyph(
                                &mut self.framebuffer,
                                close_x,
                                close_y,
                                mask,
                                glyph_ref.width,
                                glyph_ref.height,
                                close_fg,
                            );
                        }
                    }
                }

                // New tab button '+'
                if tab_bar.show_new_tab {
                    let btn_x = (tab_count as f32 * tab_w + 4.0) as u32;
                    let btn_w = 24u32;
                    let btn_h = bar_h.saturating_sub(4);
                    let btn_y = 2u32;

                    if tab_bar.is_new_tab_hovered {
                        self.framebuffer.fill_span(
                            btn_x,
                            btn_y,
                            btn_w,
                            btn_h,
                            self.palette.tab_hover_bg,
                        );
                    }

                    let plus_x = btn_x + (btn_w.saturating_sub(tab_cw)) / 2;
                    let plus_y = (bar_h.saturating_sub(tab_ch)) / 2;
                    let plus_fg = if tab_bar.is_new_tab_hovered {
                        self.palette.default_fg
                    } else {
                        self.palette.tab_inactive_fg
                    };
                    let key = GlyphKey::new('+', false, false, false);
                    if let Some(glyph_ref) = self.tab_glyph_cache.get_or_rasterize(key) {
                        let mask = self.tab_glyph_cache.atlas.get_alpha(&glyph_ref);
                        blit_alpha_glyph(
                            &mut self.framebuffer,
                            plus_x,
                            plus_y,
                            mask,
                            glyph_ref.width,
                            glyph_ref.height,
                            plus_fg,
                        );
                    }
                }
            }
        }

        self.prev_tab_bar_hash = new_tab_bar_hash;

        // Present to target slice
        if target_buffer.len() == self.framebuffer.pixels.len() {
            target_buffer.copy_from_slice(self.framebuffer.as_slice());
        }

        self.damage.clear();
    }
}
