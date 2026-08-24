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

/// Pure-Rust, retained CPU software renderer for Velox.
pub struct CpuRenderer {
    pub framebuffer: Framebuffer,
    pub glyph_cache: GlyphCache,
    pub tab_glyph_cache: GlyphCache,
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
    prev_scroll_offset: usize,
    pub start_time: Instant,
    pub prev_blink_on: bool,
    pub opacity: f32,
    prev_opacity: f32,
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
            prev_scroll_offset: 0,
            start_time: Instant::now(),
            prev_blink_on: true,
            opacity,
            prev_opacity: opacity,
            prev_tab_bar_hash: 0,
        }
    }

    #[allow(dead_code)]
    pub fn set_opacity(&mut self, opacity: f32) {
        let opacity = opacity.clamp(0.0, 1.0);
        if (self.opacity - opacity).abs() > f32::EPSILON {
            self.opacity = opacity;
            self.damage.mark_all();
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
        self.damage.mark_all();
    }

    /// Backwards-compatible render entry point (no tab bar). Delegates to `render_with_tab_bar`.
    /// Only used in unit tests.
    #[allow(dead_code)]
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
        target_buffer: &mut [u32],
        tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>,
    ) {
        let opacity = opacity.clamp(0.0, 1.0);

        // 1. Sync theme updates
        if theme.default_fg != self.prev_theme_fg
            || theme.default_bg != self.prev_theme_bg
            || theme.ansi_colors != self.prev_ansi_colors
            || theme.cursor_color != self.prev_cursor_color
            || theme.cursor_text_color != self.prev_cursor_text_color
            || theme.tab_accent_color != self.prev_tab_accent_color
            || (opacity - self.prev_opacity).abs() > f32::EPSILON
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
            self.damage.mark_all();
        }

        // 2. Ingest terminal damage and scroll offset changes
        if grid.scroll_offset != self.prev_scroll_offset {
            self.damage.mark_all();
            self.prev_scroll_offset = grid.scroll_offset;
        }

        self.damage
            .sync_from_grid(&grid.damage.dirty_rows, grid.damage.full_redraw);
        self.damage
            .update_cursor(display_cursor_x, grid.cursor.y, cursor_visible);

        let history_len = grid.scrollback.len();
        let ((sel_min_x, sel_min_abs_y), (sel_max_x, sel_max_abs_y)) =
            grid.selection.normalized_bounds();
        self.damage
            .update_selection(grid.selection.active, sel_min_abs_y, sel_max_abs_y);

        let grid_w = grid.width;
        let grid_h = grid.height;

        // Blink animation handling
        let blink_on = (self.start_time.elapsed().as_millis() / 500).is_multiple_of(2);
        let blink_changed = blink_on != self.prev_blink_on;
        self.prev_blink_on = blink_on;

        if blink_changed {
            for y in 0..grid_h {
                let row_start = y * grid_w;
                let row_end = (row_start + grid_w).min(cells.len());
                if row_start < cells.len()
                    && cells[row_start..row_end]
                        .iter()
                        .any(|c| c.flags.contains(CellFlags::BLINK))
                {
                    self.damage.mark_row(y);
                }
            }
        }

        // Compute a lightweight hash of the tab bar state to detect changes
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

        // If no damage and tab bar unchanged, directly copy framebuffer to surface
        if !self.damage.has_damage() && !tab_bar_dirty {
            if target_buffer.len() == self.framebuffer.pixels.len() {
                target_buffer.copy_from_slice(self.framebuffer.as_slice());
            }
            return;
        }

        let cell_w = self.glyph_cache.cell_width;
        let cell_h = self.glyph_cache.cell_height;
        let bar_h = if let Some(tb) = tab_bar_info {
            tb.height as u32
        } else {
            0
        };
        let px_offset = padding_x as u32;
        let py_offset = padding_y as u32 + bar_h;

        if self.damage.full_redraw {
            self.framebuffer.clear(self.palette.default_bg);
        }

        // 3. Render each dirty row
        for y in 0..grid_h {
            if !self.damage.is_dirty(y) {
                continue;
            }

            let abs_y = y + history_len;
            let (is_row_valid, abs_row) = if abs_y >= grid.scroll_offset {
                (true, abs_y - grid.scroll_offset)
            } else {
                (false, 0)
            };
            let is_row_in_selection = grid.selection.active
                && is_row_valid
                && abs_row >= sel_min_abs_y
                && abs_row <= sel_max_abs_y;

            let py = py_offset + (y as u32) * cell_h;
            if py + cell_h > self.framebuffer.height {
                break;
            }

            let row_start = y * grid_w;
            let row_cells = &cells[row_start..(row_start + grid_w).min(cells.len())];

            // ─── Pass A: Coalesced Background Spans ───────────────────────────
            let mut span_start_col = 0usize;
            let mut span_bg = 0u32;
            let mut in_span = false;

            for (col, cell) in row_cells.iter().enumerate() {
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

                let (_, bg) =
                    self.palette
                        .resolve_cell_colors(cell, is_inverted, self.bold_is_bright);

                if !in_span {
                    span_start_col = col;
                    span_bg = bg;
                    in_span = true;
                } else if bg != span_bg {
                    // Flush span
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
                let span_w = ((row_cells.len() - span_start_col) as u32) * cell_w;
                self.framebuffer
                    .fill_span(span_px, py, span_w, cell_h, span_bg);
            }

            // ─── Pass B: Glyphs, Primitives, and Decorations ──────────────────
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

                let (fg, _) =
                    self.palette
                        .resolve_cell_colors(cell, is_inverted, self.bold_is_bright);

                // Render character (skip if HIDDEN or BLINK during off phase)
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
                        let key = GlyphKey::new(cell.character, is_bold, is_italic, is_wide);

                        if let Some(glyph_ref) = self.glyph_cache.get_or_rasterize(key) {
                            if glyph_ref.is_color {
                                let pixels = self.glyph_cache.atlas.get_color(&glyph_ref);
                                blit_color_glyph(
                                    &mut self.framebuffer,
                                    px,
                                    py,
                                    pixels,
                                    glyph_ref.width,
                                    glyph_ref.height,
                                );
                            } else {
                                let mask = self.glyph_cache.atlas.get_alpha(&glyph_ref);
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

                // Render text decorations
                if cell.flags.contains(CellFlags::UNDERLINE) {
                    draw_underline(&mut self.framebuffer, px, py, cell_w, cell_h, fg);
                }
                if cell.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                    draw_double_underline(&mut self.framebuffer, px, py, cell_w, cell_h, fg);
                }
                if cell.flags.contains(CellFlags::CURLY_UNDERLINE) {
                    draw_curly_underline(&mut self.framebuffer, px, py, cell_w, cell_h, fg);
                }
                if cell.flags.contains(CellFlags::STRIKE) {
                    draw_strike(&mut self.framebuffer, px, py, cell_w, cell_h, fg);
                }
            }
        }

        // 4. Render Cursor
        if cursor_visible && grid.cursor.y < grid_h && display_cursor_x < grid_w {
            let cur_x = display_cursor_x;
            let cur_y = grid.cursor.y;
            let px = px_offset + (cur_x as u32) * cell_w;
            let py = py_offset + (cur_y as u32) * cell_h;

            if px + cell_w <= self.framebuffer.width && py + cell_h <= self.framebuffer.height {
                let cell_idx = cur_y * grid_w + cur_x;
                let cell = if cell_idx < cells.len() {
                    &cells[cell_idx]
                } else {
                    &grid.cells[0]
                };

                let cell_fg_color = cell.foreground;
                let mut cell_fg = cell_fg_color;
                if self.bold_is_bright && cell.flags.contains(CellFlags::BOLD) {
                    for i in 0..8 {
                        if cell_fg_color == self.palette.ansi_colors_raw[i] {
                            cell_fg = theme.ansi_colors[i + 8];
                            break;
                        }
                    }
                }

                let cursor_color =
                    PackedColor::from_color(theme.resolve_cursor_color(cell_fg)).to_u32();
                let cursor_text_color =
                    PackedColor::from_color(theme.resolve_cursor_text_color(cell.background))
                        .to_u32();

                if is_focused && cursor_shape == CursorShape::Block {
                    // Block cursor: fill cursor block and render inverted cell character
                    self.framebuffer
                        .fill_span(px, py, cell_w, cell_h, cursor_color);
                    let skip_cursor_fg = cell.flags.contains(CellFlags::HIDDEN)
                        || (cell.flags.contains(CellFlags::BLINK) && !blink_on);

                    if !skip_cursor_fg && cell.character != ' ' {
                        let is_wide = cell.flags.contains(CellFlags::WIDE);
                        let target_w = if is_wide { cell_w * 2 } else { cell_w };
                        let inv_fg = cursor_text_color;

                        if !try_render_primitive(
                            cell.character,
                            px,
                            py,
                            target_w,
                            cell_h,
                            inv_fg,
                            &mut self.framebuffer,
                        ) {
                            let is_bold = cell.flags.contains(CellFlags::BOLD);
                            let is_italic = cell.flags.contains(CellFlags::ITALIC);
                            let key = GlyphKey::new(cell.character, is_bold, is_italic, is_wide);

                            if let Some(glyph_ref) = self.glyph_cache.get_or_rasterize(key)
                                && !glyph_ref.is_color
                            {
                                let mask = self.glyph_cache.atlas.get_alpha(&glyph_ref);
                                blit_alpha_glyph(
                                    &mut self.framebuffer,
                                    px,
                                    py,
                                    mask,
                                    glyph_ref.width,
                                    glyph_ref.height,
                                    inv_fg,
                                );
                            }
                        }
                    }
                } else {
                    draw_cursor(
                        &mut self.framebuffer,
                        px,
                        py,
                        cell_w,
                        cell_h,
                        cursor_shape,
                        is_focused,
                        cursor_color,
                    );
                }
            }
        }

        // 4.5. Render Tab Bar (only when content changed or full redraw forced)
        if (tab_bar_dirty || self.damage.full_redraw)
            && let Some(tab_bar) = tab_bar_info
        {
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
                        self.framebuffer
                            .fill_span(btn_x, btn_y, btn_w, btn_h, self.palette.tab_hover_bg);
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

        // Update tab bar hash for next frame
        self.prev_tab_bar_hash = new_tab_bar_hash;

        // 5. Present to Softbuffer target slice
        if target_buffer.len() == self.framebuffer.pixels.len() {
            target_buffer.copy_from_slice(self.framebuffer.as_slice());
        }

        // 6. Clear damage
        self.damage.clear();
    }
}
