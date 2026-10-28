use crate::app::pane::PaneId;
use crate::app::split::{PaneRect, SeparatorRect, SplitDirection};

#[derive(Debug, Clone, Copy)]
pub struct SeparatorRenderData {
    pub rect: SeparatorRect,
    pub is_active: bool,
    pub active_segment: Option<(f32, f32)>,
    pub is_hovered: bool,
    pub is_dragging: bool,
}
pub mod atlas;
pub mod backend;
pub mod color;
pub mod damage;
pub mod decorations;
pub mod framebuffer;
pub mod glyph;
pub mod primitives;
pub mod raster;

pub use backend::{RendererInitError, WindowAndRenderer, create_window_and_renderer};
pub use color::{PackedColor, PrecomputedPalette};
pub use damage::DamageMap;
pub use framebuffer::Framebuffer;
pub use glyph::{GlyphCache, GlyphKey};

pub mod software {
    pub use super::*;
}

use crate::screen::cell::{Cell, CellFlags};
use crate::screen::cursor::CursorShape;
use crate::screen::grid::Grid;
use crate::theme::theme::Theme;
use decorations::{
    draw_curly_underline, draw_cursor, draw_double_underline, draw_strike, draw_underline,
};
use primitives::try_render_primitive;
use raster::{
    blit_alpha_glyph, blit_alpha_glyph_clipped, blit_alpha_glyph_clipped_opaque,
    blit_color_glyph_clipped, blit_color_glyph_clipped_opaque,
};
use std::time::Instant;

use ahash::AHashMap;

pub struct CpuPaneRenderData<'a> {
    pub pane_id: PaneId,
    pub rect: PaneRect,
    pub cells: &'a [Cell],
    pub grid: &'a Grid,
    pub font_size: f32,
    pub theme: &'a Theme,
    pub bold_is_bright: bool,
    pub cursor_visible: bool,
    pub cursor_shape: CursorShape,
    pub display_cursor_x: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CpuRowState {
    pub last_cursor: Option<(usize, CursorShape, bool)>,
    pub last_selection_range: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedCell {
    pub fg: u32,
    pub bg: u32,
    pub glyph_key: Option<GlyphKey>,
    pub flags: CellFlags,
}

#[derive(Debug, Clone, Default)]
pub struct CpuPaneState {
    pub row_states: Vec<CpuRowState>,
    pub last_scroll_offset: usize,
    pub last_is_active: bool,
    pub last_rect: Option<PaneRect>,
    pub last_is_focused: bool,
    pub last_dim: f32,
    pub last_font_size: f32,
    pub last_cursor_info: Option<(usize, CursorShape, usize, bool)>,
    pub last_selection_bounds: ((usize, usize), (usize, usize)),
}

impl CpuPaneState {
    #[inline]
    pub fn mark_full_redraw(&mut self) {
        self.last_rect = None;
        crate::diagnostics::ResizeStats::record_pane_state_invalidation();
    }
}

/// Pure-Rust, retained CPU software renderer for Velox.
pub struct CpuRenderer {
    pub framebuffer: Framebuffer,
    pub glyph_cache: GlyphCache,
    pub tab_glyph_cache: GlyphCache,
    pub pane_glyph_caches: AHashMap<u32, GlyphCache>,
    pub pane_states: AHashMap<PaneId, CpuPaneState>,
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
    pub default_font_size: f32,
    pub start_time: Instant,
    pub prev_blink_on: bool,
    pub opacity: f32,
    prev_opacity: f32,
    prev_dim: f32,
    prev_tab_bar_hash: u64,
    last_target_ptr: usize,
    pub csd_height: u32,
    pub prev_csd_hash: u64,
    pub scratch_resolved: Vec<ResolvedCell>,
    pub stats: RenderStats,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderStats {
    pub frames_attempted: u64,
    pub frames_rendered: u64,
    pub idle_frames: u64,
    pub rows_scanned: u64,
    pub rows_rendered: u64,
    pub cells_scanned: u64,
    pub cells_rendered: u64,
    pub glyph_cache_hits: u64,
    pub glyph_cache_misses: u64,
    pub color_resolutions: u64,
    pub framebuffer_pixels_written: u64,
    pub full_present_count: u64,
    pub damage_present_count: u64,
}

#[derive(Debug, Clone, Copy)]
struct RowLayoutInfo {
    is_row_in_selection: bool,
    abs_row: usize,
    is_active_grid_row: bool,
    grid_y: usize,
    current_cursor: Option<(usize, CursorShape, bool)>,
    current_selection: Option<(usize, usize)>,
}

#[inline(always)]
fn resolve_bold_bright_fg(
    fg: crate::screen::cell::Color,
    flags: CellFlags,
    bold_is_bright: bool,
    palette: &PrecomputedPalette,
    ansi_colors: &[crate::screen::cell::Color; 16],
) -> crate::screen::cell::Color {
    palette.bold_bright_fg(fg, flags, bold_is_bright, ansi_colors)
}

#[inline(always)]
fn is_column_selected(
    is_row_in_selection: bool,
    col: usize,
    abs_row: usize,
    sel_min_x: usize,
    sel_min_abs_y: usize,
    sel_max_x: usize,
    sel_max_abs_y: usize,
) -> bool {
    if !is_row_in_selection {
        return false;
    }
    if sel_min_abs_y == sel_max_abs_y {
        col >= sel_min_x && col <= sel_max_x
    } else if abs_row == sel_min_abs_y {
        col >= sel_min_x
    } else if abs_row == sel_max_abs_y {
        col <= sel_max_x
    } else {
        true
    }
}

// Precomputed 10x10 corner masks for GTK Adwaita 10px radius top window corners.
// 0: inside (retains title bar bg), 1: 1px border arc, 2: outside (transparent 0x00000000).
#[rustfmt::skip]
const CORNER_TL_10PX: [u8; 100] = [
    2, 2, 2, 2, 2, 2, 2, 1, 1, 1,
    2, 2, 2, 2, 2, 1, 1, 1, 0, 0,
    2, 2, 2, 1, 1, 0, 0, 0, 0, 0,
    2, 2, 1, 1, 0, 0, 0, 0, 0, 0,
    2, 2, 1, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[rustfmt::skip]
const CORNER_TR_10PX: [u8; 100] = [
    1, 1, 1, 1, 2, 2, 2, 2, 2, 2,
    0, 0, 0, 1, 1, 1, 2, 2, 2, 2,
    0, 0, 0, 0, 0, 0, 1, 1, 2, 2,
    0, 0, 0, 0, 0, 0, 0, 1, 1, 2,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 2,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

struct CircleRow {
    dy: i8,
    inner: Option<(i8, u8)>,
    edges: &'static [(i8, u8)],
}

#[rustfmt::skip]
const CIRCLE_24PX_SCANLINES: [CircleRow; 25] = [
    CircleRow { dy: -12, inner: None, edges: &[] },
    CircleRow { dy: -11, inner: Some((0, 1)), edges: &[(-4, 77), (-3, 155), (-2, 210), (-1, 243), (1, 243), (2, 210), (3, 155), (4, 77)] },
    CircleRow { dy: -10, inner: Some((-4, 9)), edges: &[(-6, 88), (-5, 210), (5, 210), (6, 88)] },
    CircleRow { dy: -9, inner: Some((-6, 13)), edges: &[(-7, 155), (7, 155)] },
    CircleRow { dy: -8, inner: Some((-7, 15)), edges: &[(-8, 177), (8, 177)] },
    CircleRow { dy: -7, inner: Some((-8, 17)), edges: &[(-9, 155), (9, 155)] },
    CircleRow { dy: -6, inner: Some((-9, 19)), edges: &[(-10, 88), (10, 88)] },
    CircleRow { dy: -5, inner: Some((-9, 19)), edges: &[(-10, 210), (10, 210)] },
    CircleRow { dy: -4, inner: Some((-10, 21)), edges: &[(-11, 77), (11, 77)] },
    CircleRow { dy: -3, inner: Some((-10, 21)), edges: &[(-11, 155), (11, 155)] },
    CircleRow { dy: -2, inner: Some((-10, 21)), edges: &[(-11, 210), (11, 210)] },
    CircleRow { dy: -1, inner: Some((-10, 21)), edges: &[(-11, 243), (11, 243)] },
    CircleRow { dy: 0, inner: Some((-11, 23)), edges: &[] },
    CircleRow { dy: 1, inner: Some((-10, 21)), edges: &[(-11, 243), (11, 243)] },
    CircleRow { dy: 2, inner: Some((-10, 21)), edges: &[(-11, 210), (11, 210)] },
    CircleRow { dy: 3, inner: Some((-10, 21)), edges: &[(-11, 155), (11, 155)] },
    CircleRow { dy: 4, inner: Some((-10, 21)), edges: &[(-11, 77), (11, 77)] },
    CircleRow { dy: 5, inner: Some((-9, 19)), edges: &[(-10, 210), (10, 210)] },
    CircleRow { dy: 6, inner: Some((-9, 19)), edges: &[(-10, 88), (10, 88)] },
    CircleRow { dy: 7, inner: Some((-8, 17)), edges: &[(-9, 155), (9, 155)] },
    CircleRow { dy: 8, inner: Some((-7, 15)), edges: &[(-8, 177), (8, 177)] },
    CircleRow { dy: 9, inner: Some((-6, 13)), edges: &[(-7, 155), (7, 155)] },
    CircleRow { dy: 10, inner: Some((-4, 9)), edges: &[(-6, 88), (-5, 210), (5, 210), (6, 88)] },
    CircleRow { dy: 11, inner: Some((0, 1)), edges: &[(-4, 77), (-3, 155), (-2, 210), (-1, 243), (1, 243), (2, 210), (3, 155), (4, 77)] },
    CircleRow { dy: 12, inner: None, edges: &[] },
];

impl CpuRenderer {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        font_family: &str,
        font_size: f32,
        font_scale_multiplier: f32,
        theme: &Theme,
        width: u32,
        height: u32,
        bold_is_bright: bool,
        opacity: f32,
    ) -> Result<Self, crate::font::resolved::FontError> {
        let font_size = font_size.max(1.0);
        let glyph_cache =
            GlyphCache::try_from_font_family(font_family, font_size, font_scale_multiplier)?;
        let tab_glyph_cache = glyph_cache.create_tab_cache(font_size);
        let framebuffer = Framebuffer::new(width, height);
        let opacity = opacity.clamp(0.0, 1.0);
        let palette = PrecomputedPalette::new(theme, opacity);
        let rows = (height / glyph_cache.cell_height.max(1)).max(1) as usize;

        Ok(Self {
            framebuffer,
            glyph_cache,
            tab_glyph_cache,
            pane_glyph_caches: AHashMap::new(),
            pane_states: AHashMap::new(),
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
            default_font_size: font_size,
            start_time: Instant::now(),
            prev_blink_on: true,
            opacity,
            prev_opacity: opacity,
            prev_dim: 0.0,
            prev_tab_bar_hash: 0,
            last_target_ptr: 0,
            csd_height: 0,
            prev_csd_hash: 0,
            scratch_resolved: Vec::new(),
            stats: RenderStats::default(),
        })
    }

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
        Self::try_new(
            font_family,
            font_size,
            font_scale_multiplier,
            theme,
            width,
            height,
            bold_is_bright,
            opacity,
        )
        .expect("Failed to initialize CpuRenderer")
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.viewport_width == width && self.viewport_height == height {
            return;
        }
        self.viewport_width = width;
        self.viewport_height = height;
        self.last_target_ptr = 0;
        self.prev_csd_hash = 0;
        if self.framebuffer.resize(width, height) {
            let rows = (height / self.glyph_cache.cell_height.max(1)).max(1) as usize;
            self.damage.resize(rows);
            self.damage.mark_all();
            for state in self.pane_states.values_mut() {
                state.mark_full_redraw();
            }
        }
    }

    pub fn update_font_size(&mut self, font_size: f32) {
        let font_size = crate::app::split::clamp_font_size(font_size, self.default_font_size);
        if (self.glyph_cache.font_size - font_size).abs() < 0.01 {
            return;
        }
        self.glyph_cache.update_font_size(font_size);
        let rows = (self.viewport_height / self.glyph_cache.cell_height.max(1)).max(1) as usize;
        self.damage.resize(rows);
        self.damage.mark_all();
        crate::diagnostics::ResizeStats::record_pane_state_clear();
        self.pane_states.clear();
        self.last_target_ptr = 0;
    }

    pub fn set_tab_font_size(&mut self, font_size: f32) {
        self.tab_glyph_cache.update_font_size(font_size);
    }

    /// Full memory cleanup: compacts glyph atlas, unloads fallback fonts, and shrinks scratch buffers.
    pub fn release_memory(&mut self) {
        self.glyph_cache.release_memory();
        self.tab_glyph_cache.release_tab_memory();
        self.pane_glyph_caches.clear();
        self.pane_glyph_caches.shrink_to_fit();
        crate::diagnostics::ResizeStats::record_pane_state_clear();
        self.pane_states.clear();
        self.pane_states.shrink_to_fit();
        self.framebuffer.release_memory();
        self.prev_csd_hash = 0;
        self.last_target_ptr = 0;
    }

    /// Prune fallback fonts that haven't been accessed for at least `max_idle`.
    pub fn prune_idle_fallbacks(&mut self, max_idle: std::time::Duration) -> usize {
        let mut evicted = 0;
        evicted += self.glyph_cache.fallback_manager.prune_idle(max_idle);
        evicted += self.tab_glyph_cache.fallback_manager.prune_idle(max_idle);
        for cache in self.pane_glyph_caches.values_mut() {
            evicted += cache.fallback_manager.prune_idle(max_idle);
        }
        evicted
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
            bold_is_bright: self.bold_is_bright,
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
            Some(target_buffer),
            tab_bar_info,
            None,
            None,
        );
    }

    #[inline(always)]
    fn get_row_cursor_and_selection(
        pane: &CpuPaneRenderData,
        y: usize,
        is_focused: bool,
        history_len: usize,
        sel_bounds: ((usize, usize), (usize, usize)),
    ) -> RowLayoutInfo {
        let grid = pane.grid;
        let abs_y = y + history_len;
        let (is_row_valid, abs_row) = if abs_y >= grid.scroll_offset {
            (true, abs_y - grid.scroll_offset)
        } else {
            (false, 0)
        };
        let ((sel_min_x, sel_min_abs_y), (sel_max_x, sel_max_abs_y)) = sel_bounds;
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

        let current_cursor = if pane.is_active
            && pane.cursor_visible
            && is_active_grid_row
            && grid_y == grid.cursor.y
        {
            Some((pane.display_cursor_x, pane.cursor_shape, is_focused))
        } else {
            None
        };

        let current_selection = if is_row_in_selection {
            let sel_range = if sel_min_abs_y == sel_max_abs_y {
                (sel_min_x, sel_max_x)
            } else if abs_row == sel_min_abs_y {
                (sel_min_x, grid.width)
            } else if abs_row == sel_max_abs_y {
                (0, sel_max_x)
            } else {
                (0, grid.width)
            };
            Some(sel_range)
        } else {
            None
        };

        RowLayoutInfo {
            is_row_in_selection,
            abs_row,
            is_active_grid_row,
            grid_y,
            current_cursor,
            current_selection,
        }
    }

    fn prune_stale_caches(&mut self, panes: &[CpuPaneRenderData]) {
        if self.pane_glyph_caches.len() > panes.len() {
            let default_size = self.default_font_size;
            self.pane_glyph_caches.retain(|&k, _| {
                panes.iter().any(|p| {
                    (crate::app::split::clamp_font_size(p.font_size, default_size) * 100.0).round()
                        as u32
                        == k
                })
            });
        }
        if self.pane_states.len() > panes.len() {
            self.pane_states
                .retain(|&k, _| panes.iter().any(|p| p.pane_id == k));
        }
    }

    fn sync_theme_and_palette(&mut self, theme: &Theme, opacity: f32, effective_dim: f32) {
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
    }

    fn check_panes_damage(
        &mut self,
        panes: &[CpuPaneRenderData],
        is_focused: bool,
        effective_dim: f32,
    ) -> bool {
        if self.damage.full_redraw {
            return true;
        }
        for pane in panes {
            let state = self.pane_states.entry(pane.pane_id).or_default();
            let grid = pane.grid;
            let selection_bounds = if grid.selection.active {
                grid.selection.normalized_bounds()
            } else {
                ((usize::MAX, usize::MAX), (usize::MAX, usize::MAX))
            };

            let pane_effective_dim = if !pane.is_active {
                (effective_dim * 0.5 + 0.15).clamp(0.0, 1.0)
            } else {
                effective_dim
            };

            let clamped_font_size =
                crate::app::split::clamp_font_size(pane.font_size, self.default_font_size);
            let pane_layout_changed = state.last_rect != Some(pane.rect)
                || state.last_scroll_offset != grid.scroll_offset
                || state.last_is_active != pane.is_active
                || state.last_is_focused != is_focused
                || (state.last_dim - pane_effective_dim).abs() > 0.001
                || (state.last_font_size - clamped_font_size).abs() > 0.01;

            if pane_layout_changed || grid.damage.has_damage() {
                return true;
            }

            let current_cursor_info =
                if pane.is_active && pane.cursor_visible && grid.scroll_offset == 0 {
                    Some((
                        pane.display_cursor_x,
                        pane.cursor_shape,
                        grid.cursor.y,
                        is_focused,
                    ))
                } else {
                    None
                };
            let cursor_changed = state.last_cursor_info != current_cursor_info;
            let selection_changed = state.last_selection_bounds != selection_bounds;

            if cursor_changed || selection_changed {
                return true;
            }
        }
        false
    }

    fn compute_tab_bar_hash(tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>) -> u64 {
        tab_bar_info
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
            .unwrap_or(0)
    }

    fn ensure_pane_glyph_caches(&mut self, panes: &[CpuPaneRenderData]) {
        for pane in panes {
            let font_size =
                crate::app::split::clamp_font_size(pane.font_size, self.default_font_size);
            let key = (font_size * 100.0).round() as u32;
            if (self.glyph_cache.font_size - font_size).abs() >= 0.01
                && !self.pane_glyph_caches.contains_key(&key)
            {
                let cache = self.glyph_cache.create_pane_cache(font_size);
                self.pane_glyph_caches.insert(key, cache);
            }
        }
    }

    fn blit_tab_char(&mut self, ch: char, x: u32, y: u32, fg: u32) {
        let key = GlyphKey::new(ch, false, false, false);
        if let Some(glyph_ref) = self.tab_glyph_cache.get_or_rasterize(key) {
            let mask = self.tab_glyph_cache.atlas.get_alpha(&glyph_ref);
            blit_alpha_glyph(
                &mut self.framebuffer,
                x,
                y,
                mask,
                glyph_ref.width,
                glyph_ref.height,
                fg,
            );
        }
    }

    fn blit_truncated_text(
        &mut self,
        text: &str,
        start_x: u32,
        y: u32,
        max_chars: usize,
        tab_cw: u32,
        fg: u32,
    ) {
        let char_count = text.chars().count();
        if char_count <= max_chars {
            for (idx, ch) in text.chars().enumerate() {
                let px = start_x + (idx as u32) * tab_cw;
                self.blit_tab_char(ch, px, y, fg);
            }
        } else if max_chars > 1 {
            let take_count = max_chars.saturating_sub(1);
            for (idx, ch) in text.chars().take(take_count).enumerate() {
                let px = start_x + (idx as u32) * tab_cw;
                self.blit_tab_char(ch, px, y, fg);
            }
            let px = start_x + (take_count as u32) * tab_cw;
            self.blit_tab_char('…', px, y, fg);
        } else if max_chars == 1
            && let Some(ch) = text.chars().next()
        {
            self.blit_tab_char(ch, start_x, y, fg);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_csd_title_bar(
        &mut self,
        title: &str,
        csd_height: u32,
        is_focused: bool,
        is_maximized: bool,
        hovered_button: Option<crate::window::csd::CsdButtonType>,
        pressed_button: Option<crate::window::csd::CsdButtonType>,
        button_layout: &crate::window::csd::CsdButtonLayout,
        csd_palette: &crate::window::csd::CsdPalette,
    ) -> bool {
        if csd_height == 0 {
            return false;
        }

        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        title.hash(&mut hasher);
        csd_height.hash(&mut hasher);
        is_focused.hash(&mut hasher);
        is_maximized.hash(&mut hasher);
        hovered_button.hash(&mut hasher);
        pressed_button.hash(&mut hasher);
        self.viewport_width.hash(&mut hasher);
        csd_palette.bg.hash(&mut hasher);
        csd_palette.border.hash(&mut hasher);
        let new_hash = hasher.finish();

        if !self.damage.full_redraw && self.prev_csd_hash == new_hash {
            return false;
        }
        self.prev_csd_hash = new_hash;

        let bar_bg = if is_focused {
            csd_palette.bg
        } else {
            csd_palette.inactive_bg
        };

        let w = self.viewport_width;
        let border_color = csd_palette.border;

        // 1. Fill title bar background
        self.framebuffer.fill_span(0, 0, w, csd_height, bar_bg);

        // 2. Render rounded top corners when not maximized (GTK Adwaita 10px radius)
        if !is_maximized && w >= 24 && csd_height >= 12 {
            let stride = self.framebuffer.stride;

            // Top-left corner (precomputed mask lookup)
            for py in 0..10 {
                let row_offset = py * stride;
                let table_offset = py * 10;
                for px in 0..10 {
                    match CORNER_TL_10PX[table_offset + px] {
                        2 => self.framebuffer.pixels[row_offset + px] = 0x00000000,
                        1 => self.framebuffer.pixels[row_offset + px] = border_color,
                        _ => {}
                    }
                }
            }

            // Top-right corner (precomputed mask lookup)
            let tr_start_x = (w - 10) as usize;
            for py in 0..10 {
                let row_offset = py * stride + tr_start_x;
                let table_offset = py * 10;
                for rx in 0..10 {
                    match CORNER_TR_10PX[table_offset + rx] {
                        2 => self.framebuffer.pixels[row_offset + rx] = 0x00000000,
                        1 => self.framebuffer.pixels[row_offset + rx] = border_color,
                        _ => {}
                    }
                }
            }

            // Top border line between corners
            let top_border_span = w.saturating_sub(20);
            if top_border_span > 0 {
                self.framebuffer
                    .fill_span(10, 0, top_border_span, 1, border_color);
            }
        } else {
            // Maximized: square 1px top border line
            self.framebuffer.fill_span(0, 0, w, 1, border_color);
        }

        // 3. Draw 1px bottom border line separating title bar from tab bar or panes (omit in seamless terminal mode)
        if csd_palette.border != csd_palette.bg {
            self.framebuffer
                .fill_span(0, csd_height.saturating_sub(1), w, 1, border_color);
        }

        // 4. Render GTK Window Control Buttons
        let button_rects = button_layout.compute_button_rects(w, csd_height);
        for rect in &button_rects {
            let cx = rect.center_x.round() as i32;
            let cy = rect.center_y.round() as i32;
            let is_hovered = hovered_button == Some(rect.button_type);
            let is_pressed = pressed_button == Some(rect.button_type);

            self.render_gtk_window_button(
                rect.button_type,
                cx,
                cy,
                is_hovered,
                is_pressed,
                is_focused,
                is_maximized,
                bar_bg,
                csd_palette,
            );
        }

        // 5. Render Centered Title Text with Safe Bounds
        let (safe_min_x, safe_max_x) = button_layout.safe_title_bounds(w, csd_height);
        let safe_width = (safe_max_x - safe_min_x).max(0.0);

        let tab_cw = self.tab_glyph_cache.cell_width;
        let tab_ch = self.tab_glyph_cache.cell_height;

        if safe_width > tab_cw as f32 && !title.is_empty() {
            let max_chars = (safe_width / tab_cw as f32).floor() as usize;
            let display_len = title.chars().count().min(max_chars);

            let title_pixel_width = display_len as u32 * tab_cw;
            let ideal_x = (w.saturating_sub(title_pixel_width)) / 2;
            let text_x = (ideal_x as f32).clamp(
                safe_min_x,
                (safe_max_x - title_pixel_width as f32).max(safe_min_x),
            ) as u32;
            let text_y = (csd_height.saturating_sub(tab_ch)) / 2;

            let text_fg = if is_focused {
                csd_palette.fg
            } else {
                csd_palette.inactive_fg
            };

            self.blit_truncated_text(title, text_x, text_y, max_chars, tab_cw, text_fg);
        }
        true
    }

    pub fn render_csd_window_borders(&mut self, border_color: u32) {
        if self.csd_height == 0 || border_color == self.palette.default_bg {
            return;
        }
        let w = self.viewport_width;
        let h = self.viewport_height;
        if w < 2 || h < 2 {
            return;
        }
        // Left border
        self.framebuffer.fill_span(
            0,
            self.csd_height,
            1,
            h.saturating_sub(self.csd_height),
            border_color,
        );
        // Right border
        self.framebuffer.fill_span(
            w.saturating_sub(1),
            self.csd_height,
            1,
            h.saturating_sub(self.csd_height),
            border_color,
        );
        // Bottom border
        self.framebuffer
            .fill_span(0, h.saturating_sub(1), w, 1, border_color);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_gtk_window_button(
        &mut self,
        btn_type: crate::window::csd::CsdButtonType,
        cx: i32,
        cy: i32,
        is_hovered: bool,
        is_pressed: bool,
        is_focused: bool,
        is_maximized: bool,
        bar_bg: u32,
        csd_palette: &crate::window::csd::CsdPalette,
    ) {
        use crate::renderer::raster::blend_alpha;
        use crate::window::csd::CsdButtonType;

        let width = self.viewport_width as i32;
        let height = self.viewport_height as i32;
        let stride = self.framebuffer.stride;

        // 1. Draw circular background button pill (diameter = 24px, radius = 12px)
        let bg_color = if is_hovered || is_pressed {
            match btn_type {
                CsdButtonType::Close => {
                    if is_pressed {
                        csd_palette.close_pressed_bg
                    } else {
                        csd_palette.close_hover_bg
                    }
                }
                CsdButtonType::Maximize | CsdButtonType::Minimize => {
                    let alpha = if is_pressed {
                        if csd_palette.is_light { 45 } else { 65 }
                    } else {
                        if csd_palette.is_light { 25 } else { 38 }
                    };
                    blend_alpha(bar_bg, csd_palette.button_hover_bg, alpha)
                }
            }
        } else {
            // Subtle idle circular button disk for native GTK / Mint styling
            let alpha = if is_focused {
                if csd_palette.is_light { 10 } else { 18 }
            } else {
                if csd_palette.is_light { 5 } else { 10 }
            };
            blend_alpha(bar_bg, csd_palette.button_hover_bg, alpha)
        };

        for row in &CIRCLE_24PX_SCANLINES {
            let py = cy + row.dy as i32;
            if py < 0 || py >= height {
                continue;
            }
            let row_offset = (py as usize) * stride;

            // Fill solid inner span using slice fill
            if let Some((start_dx, count)) = row.inner {
                let start_x = cx + start_dx as i32;
                let end_x = start_x + count as i32;
                let clamped_start = start_x.clamp(0, width) as usize;
                let clamped_end = end_x.clamp(0, width) as usize;
                if clamped_start < clamped_end {
                    self.framebuffer.pixels[row_offset + clamped_start..row_offset + clamped_end]
                        .fill(bg_color);
                }
            }

            // Blend antialiased edge pixels
            for &(edge_dx, alpha) in row.edges {
                let px = cx + edge_dx as i32;
                if px >= 0 && px < width {
                    let idx = row_offset + px as usize;
                    let existing = self.framebuffer.pixels[idx];
                    self.framebuffer.pixels[idx] = blend_alpha(existing, bg_color, alpha);
                }
            }
        }

        // 2. Icon Foreground Color
        let icon_fg = if is_hovered || is_pressed {
            match btn_type {
                CsdButtonType::Close => csd_palette.close_hover_fg,
                _ => {
                    if csd_palette.is_light {
                        0x00000000
                    } else {
                        0x00FFFFFF
                    }
                }
            }
        } else if is_focused {
            match btn_type {
                CsdButtonType::Close => csd_palette.close_fg,
                _ => csd_palette.fg,
            }
        } else {
            match btn_type {
                CsdButtonType::Close => {
                    if csd_palette.close_fg != csd_palette.fg {
                        blend_alpha(bar_bg, csd_palette.close_fg, 140)
                    } else {
                        csd_palette.inactive_fg
                    }
                }
                _ => csd_palette.inactive_fg,
            }
        };

        let mut put_pixel = |x: i32, y: i32, color: u32| {
            if x >= 0 && x < width && y >= 0 && y < height {
                self.framebuffer.pixels[(y as usize) * stride + (x as usize)] = color;
            }
        };

        match btn_type {
            CsdButtonType::Close => {
                // GTK crisp diagonal cross (2px stroke thickness)
                for d in -4..=4 {
                    put_pixel(cx + d, cy + d, icon_fg);
                    put_pixel(cx + d + 1, cy + d, icon_fg);
                    put_pixel(cx + d, cy - d, icon_fg);
                    put_pixel(cx + d + 1, cy - d, icon_fg);
                }
            }
            CsdButtonType::Minimize => {
                // 10px wide by 2px tall horizontal line
                for y in (cy + 3)..=(cy + 4) {
                    for x in (cx - 5)..=(cx + 4) {
                        put_pixel(x, y, icon_fg);
                    }
                }
            }
            CsdButtonType::Maximize => {
                if !is_maximized {
                    // Single 10x10 square outline with rounded corners
                    let x0 = cx - 5;
                    let x1 = cx + 4;
                    let y0 = cy - 5;
                    let y1 = cy + 4;

                    for x in (x0 + 1)..=x1 {
                        put_pixel(x, y0, icon_fg);
                        put_pixel(x, y1, icon_fg);
                    }
                    for y in (y0 + 1)..=y1 {
                        put_pixel(x0, y, icon_fg);
                        put_pixel(x1, y, icon_fg);
                    }
                } else {
                    // Two overlapping restore squares (GTK restore icon)
                    let bx0 = cx - 1;
                    let bx1 = cx + 5;
                    let by0 = cy - 5;
                    let by1 = cy + 1;

                    let fx0 = cx - 5;
                    let fx1 = cx + 1;
                    let fy0 = cy - 1;
                    let fy1 = cy + 5;

                    for x in bx0..=bx1 {
                        put_pixel(x, by0, icon_fg);
                    }
                    for y in by0..=by1 {
                        put_pixel(bx1, y, icon_fg);
                    }
                    for x in (fx1 + 1)..=bx1 {
                        put_pixel(x, by1, icon_fg);
                    }
                    for y in by0..=(fy0 - 1) {
                        put_pixel(bx0, y, icon_fg);
                    }

                    for x in fx0..=fx1 {
                        put_pixel(x, fy0, icon_fg);
                        put_pixel(x, fy1, icon_fg);
                    }
                    for y in fy0..=fy1 {
                        put_pixel(fx0, y, icon_fg);
                        put_pixel(fx1, y, icon_fg);
                    }
                }
            }
        }
    }

    fn render_tab_bar(&mut self, tab_bar: &crate::app::tab::TabBarRenderInfo, csd_y_offset: u32) {
        let bar_h = tab_bar.height as u32;
        let tab_count = tab_bar.tabs.len();
        if tab_count == 0 || bar_h == 0 {
            return;
        }

        let tab_w = tab_bar.compute_tab_width(self.viewport_width as f32);
        let tab_cw = self.tab_glyph_cache.cell_width;
        let tab_ch = self.tab_glyph_cache.cell_height;
        let char_baseline_y = csd_y_offset + bar_h.saturating_sub(tab_ch) / 2;

        self.framebuffer.fill_span(
            0,
            csd_y_offset,
            self.viewport_width,
            bar_h,
            self.palette.tab_bar_bg,
        );

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
                .fill_span(tab_x, csd_y_offset, actual_w, bar_h, tab_bg);

            if tab.is_active {
                self.framebuffer.fill_span(
                    tab_x,
                    csd_y_offset,
                    actual_w,
                    2,
                    self.palette.tab_accent,
                );
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

            self.blit_truncated_text(
                &tab.title,
                text_start_x,
                char_baseline_y,
                max_chars,
                tab_cw,
                text_fg,
            );

            // Close button
            if tab_bar.show_close_button {
                let close_x = tab_x + actual_w.saturating_sub(20);
                let close_fg = if tab.is_close_hovered {
                    self.palette.ansi_colors[1]
                } else {
                    self.palette.tab_close_fg
                };
                self.blit_tab_char('×', close_x, char_baseline_y, close_fg);
            }
        }

        // New tab button '+'
        if tab_bar.show_new_tab {
            let btn_x = (tab_count as f32 * tab_w + 4.0) as u32;
            let btn_w = 24u32;
            let btn_h = bar_h.saturating_sub(4);
            let btn_y = csd_y_offset + 2u32;

            if tab_bar.is_new_tab_hovered {
                self.framebuffer
                    .fill_span(btn_x, btn_y, btn_w, btn_h, self.palette.tab_hover_bg);
            }

            let plus_x = btn_x + (btn_w.saturating_sub(tab_cw)) / 2;
            let plus_fg = if tab_bar.is_new_tab_hovered {
                self.palette.default_fg
            } else {
                self.palette.tab_inactive_fg
            };
            self.blit_tab_char('+', plus_x, char_baseline_y, plus_fg);
        }
    }

    fn render_separators(
        &mut self,
        separators: &[SeparatorRenderData],
        separator_color_override: Option<crate::screen::cell::Color>,
        active_separator_color_override: Option<crate::screen::cell::Color>,
    ) {
        let active_color = active_separator_color_override
            .map(|c| PackedColor::from_color(c).to_u32())
            .unwrap_or(self.palette.tab_accent);
        let inactive_color = separator_color_override
            .map(|c| PackedColor::from_color(c).to_u32())
            .unwrap_or(self.palette.ansi_colors[8]);

        for sep in separators {
            let sep_x = sep.rect.x as u32;
            let sep_y = sep.rect.y as u32;
            let sep_w = sep.rect.width as u32;
            let sep_h = sep.rect.height as u32;

            if sep.is_dragging || sep.is_hovered {
                self.framebuffer
                    .fill_span(sep_x, sep_y, sep_w, sep_h, active_color);
            } else if let Some((start, end)) = sep.active_segment {
                match sep.rect.direction {
                    SplitDirection::Vertical => {
                        if start > sep.rect.y + 0.5 {
                            self.framebuffer.fill_span(
                                sep_x,
                                sep_y,
                                sep_w,
                                (start - sep.rect.y) as u32,
                                inactive_color,
                            );
                        }
                        if end > start + 0.5 {
                            self.framebuffer.fill_span(
                                sep_x,
                                start as u32,
                                sep_w,
                                (end - start) as u32,
                                active_color,
                            );
                        }
                        let bottom_y = sep.rect.y + sep.rect.height;
                        if bottom_y > end + 0.5 {
                            self.framebuffer.fill_span(
                                sep_x,
                                end as u32,
                                sep_w,
                                (bottom_y - end) as u32,
                                inactive_color,
                            );
                        }
                    }
                    SplitDirection::Horizontal => {
                        if start > sep.rect.x + 0.5 {
                            self.framebuffer.fill_span(
                                sep_x,
                                sep_y,
                                (start - sep.rect.x) as u32,
                                sep_h,
                                inactive_color,
                            );
                        }
                        if end > start + 0.5 {
                            self.framebuffer.fill_span(
                                start as u32,
                                sep_y,
                                (end - start) as u32,
                                sep_h,
                                active_color,
                            );
                        }
                        let right_x = sep.rect.x + sep.rect.width;
                        if right_x > end + 0.5 {
                            self.framebuffer.fill_span(
                                end as u32,
                                sep_y,
                                (right_x - end) as u32,
                                sep_h,
                                inactive_color,
                            );
                        }
                    }
                }
            } else {
                self.framebuffer
                    .fill_span(sep_x, sep_y, sep_w, sep_h, inactive_color);
            }
        }
    }

    fn render_single_pane(
        &mut self,
        pane: &CpuPaneRenderData,
        is_focused: bool,
        opacity: f32,
        effective_dim: f32,
        blink_on: bool,
        blink_changed: bool,
    ) -> Option<(u32, u32)> {
        let font_size = crate::app::split::clamp_font_size(pane.font_size, self.default_font_size);
        let cell_w = pane.rect.cell_width.round().max(1.0) as u32;
        let cell_h = pane.rect.cell_height.round().max(1.0) as u32;
        let grid = pane.grid;
        let grid_w = grid.width;
        let grid_h = grid.height;
        let history_len = grid.scrollback.len();
        let selection_bounds = if grid.selection.active {
            grid.selection.normalized_bounds()
        } else {
            ((usize::MAX, usize::MAX), (usize::MAX, usize::MAX))
        };
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
        let max_x = (tile_x + tile_w).min(self.framebuffer.width);
        let max_y = (tile_y + tile_h).min(self.framebuffer.height);
        let default_pane_bg = if pane.theme.default_bg == self.palette.raw_default_bg {
            self.palette.default_bg
        } else {
            let alpha = (opacity * 255.0).round() as u8;
            PackedColor::from_premultiplied(pane.theme.default_bg, alpha).to_u32()
        };

        let state = self.pane_states.entry(pane.pane_id).or_default();
        let pane_layout_changed = state.last_rect != Some(pane.rect)
            || state.last_scroll_offset != grid.scroll_offset
            || state.last_is_active != pane.is_active
            || state.last_is_focused != is_focused
            || (state.last_dim - pane_effective_dim).abs() > 0.001
            || (state.last_font_size - font_size).abs() > 0.01;

        let pane_full_redraw =
            self.damage.full_redraw || grid.damage.full_redraw || pane_layout_changed;
        if pane_full_redraw {
            self.framebuffer
                .fill_span(tile_x, tile_y, tile_w, tile_h, default_pane_bg);
        }

        if state.row_states.len() != grid_h {
            state.row_states.resize(grid_h, CpuRowState::default());
        }

        let px_offset = (pane.rect.x + pane.rect.padding_x).round() as u32;
        let py_offset = (pane.rect.y + pane.rect.padding_y).round() as u32;

        let mut pane_min_y = u32::MAX;
        let mut pane_max_y = 0u32;

        // Task 16: 1-line scroll fast path via copy_within
        let can_scroll_optimize = !pane_full_redraw
            && grid.damage.scrolled_lines == 1
            && grid.scroll_offset == 0
            && !grid.selection.active
            && !pane_layout_changed
            && opacity >= 0.999
            && (state.last_dim - pane_effective_dim).abs() <= 0.001
            && grid.damage.dirty_indices().len() <= 3;

        if can_scroll_optimize {
            let top_py = py_offset;
            let bottom_py = (py_offset + (grid_h as u32) * cell_h).min(max_y);
            if bottom_py > top_py + cell_h {
                self.framebuffer
                    .scroll_region_up(top_py, bottom_py, cell_h, default_pane_bg);
                pane_min_y = pane_min_y.min(top_py);
                pane_max_y = pane_max_y.max(bottom_py);
            }
        }

        // Task 3: Hoist pane glyph cache selection outside the row loop
        let is_default_font = (self.glyph_cache.font_size - font_size).abs() < 0.01;
        let font_key = (font_size * 100.0).round() as u32;
        if !is_default_font && !self.pane_glyph_caches.contains_key(&font_key) {
            self.pane_glyph_caches
                .insert(font_key, self.glyph_cache.create_pane_cache(font_size));
        }

        // Task 1: Reuse renderer-owned scratch storage to avoid heap allocations
        if grid_w > 512 && self.scratch_resolved.len() < grid_w {
            self.scratch_resolved.resize(
                grid_w,
                ResolvedCell {
                    fg: 0,
                    bg: default_pane_bg,
                    glyph_key: None,
                    flags: CellFlags::empty(),
                },
            );
        }

        let is_opaque = opacity >= 0.999;
        let default_cell = Cell {
            character: ' ',
            foreground: pane.theme.default_fg,
            background: pane.theme.default_bg,
            underline_color: None,
            flags: CellFlags::empty(),
        };

        let current_cursor_info =
            if pane.is_active && pane.cursor_visible && grid.scroll_offset == 0 {
                Some((
                    pane.display_cursor_x,
                    pane.cursor_shape,
                    grid.cursor.y,
                    is_focused,
                ))
            } else {
                None
            };

        // Task 6 & Task 8: Build list of dirty row indices instead of scanning 0..grid_h
        let mut dirty_rows: smallvec::SmallVec<[usize; 64]> = smallvec::SmallVec::new();
        if pane_full_redraw {
            for y in 0..grid_h {
                dirty_rows.push(y);
            }
        } else {
            let mut is_dirty = [false; 256];
            let mut heap_is_dirty = if grid_h > 256 {
                vec![false; grid_h]
            } else {
                Vec::new()
            };
            let dirty_map: &mut [bool] = if grid_h <= 256 {
                &mut is_dirty[..grid_h]
            } else {
                &mut heap_is_dirty[..grid_h]
            };

            let mut mark_damage = |dirty_indices: &[usize], dirty_rows: &[bool]| {
                if !dirty_indices.is_empty() {
                    for &r in dirty_indices {
                        if r < grid_h {
                            dirty_map[r] = true;
                        }
                    }
                } else {
                    for (idx, &is_d) in dirty_rows.iter().enumerate() {
                        if is_d && idx < grid_h {
                            dirty_map[idx] = true;
                        }
                    }
                }
            };
            mark_damage(grid.damage.dirty_indices(), &grid.damage.dirty_rows);
            mark_damage(self.damage.dirty_indices(), &self.damage.dirty_rows);
            if can_scroll_optimize {
                if grid_h > 0 {
                    dirty_map[grid_h - 1] = true;
                }
                if grid.cursor.y < grid_h {
                    dirty_map[grid.cursor.y] = true;
                }
            }
            if state.last_cursor_info != current_cursor_info {
                if let Some((_, _, old_y, _)) = state.last_cursor_info {
                    if old_y < grid_h {
                        dirty_map[old_y] = true;
                    }
                }
                if let Some((_, _, new_y, _)) = current_cursor_info {
                    if new_y < grid_h {
                        dirty_map[new_y] = true;
                    }
                }
            }
            if state.last_selection_bounds != selection_bounds {
                let ((_s1_x, s1_y), (_s2_x, s2_y)) = state.last_selection_bounds;
                let ((_n1_x, n1_y), (_n2_x, n2_y)) = selection_bounds;
                let vis_min = history_len.saturating_sub(grid.scroll_offset);
                let vis_max =
                    (history_len + grid_h.saturating_sub(1)).saturating_sub(grid.scroll_offset);

                let min_abs = s1_y.min(n1_y);
                let max_abs = if s2_y == usize::MAX {
                    n2_y
                } else if n2_y == usize::MAX {
                    s2_y
                } else {
                    s2_y.max(n2_y)
                };
                if max_abs != usize::MAX && max_abs >= vis_min && min_abs <= vis_max {
                    let start_y = min_abs.saturating_sub(vis_min);
                    let end_y = (max_abs.saturating_sub(vis_min) + 1).min(grid_h);
                    for y in start_y..end_y {
                        dirty_map[y] = true;
                    }
                }
            }
            if blink_changed {
                for y in 0..grid_h {
                    if !dirty_map[y] {
                        let has_blink = grid
                            .active_display_row_slice(y)
                            .map(|cells| cells.iter().any(|c| c.flags.contains(CellFlags::BLINK)))
                            .unwrap_or_else(|| {
                                grid.with_display_row_slice(y, |cells| {
                                    cells.iter().any(|c| c.flags.contains(CellFlags::BLINK))
                                })
                                .unwrap_or(false)
                            });
                        if has_blink {
                            dirty_map[y] = true;
                        }
                    }
                }
            }

            for y in 0..grid_h {
                if dirty_map[y] {
                    dirty_rows.push(y);
                }
            }
        }

        let fb = &mut self.framebuffer;
        let palette = &self.palette;
        let scratch_resolved = &mut self.scratch_resolved;
        let glyph_cache = if is_default_font {
            &mut self.glyph_cache
        } else {
            let base = &self.glyph_cache;
            self.pane_glyph_caches
                .entry(font_key)
                .or_insert_with(|| base.create_pane_cache(font_size))
        };

        let mut stack_resolved = [ResolvedCell {
            fg: 0,
            bg: default_pane_bg,
            glyph_key: None,
            flags: CellFlags::empty(),
        }; 512];

        for &y in &dirty_rows {
            let row_info = Self::get_row_cursor_and_selection(
                pane,
                y,
                is_focused,
                history_len,
                selection_bounds,
            );

            state.row_states[y].last_cursor = row_info.current_cursor;
            state.row_states[y].last_selection_range = row_info.current_selection;

            let py = py_offset + (y as u32) * cell_h;
            if py >= max_y {
                break;
            }
            let row_end_y = (py + cell_h).min(max_y);
            pane_min_y = pane_min_y.min(py);
            pane_max_y = pane_max_y.max(row_end_y);

            if !pane_full_redraw {
                fb.fill_span(tile_x, py, tile_w, cell_h, default_pane_bg);
            }

            let resolved: &mut [ResolvedCell] = if grid_w <= 512 {
                &mut stack_resolved[..grid_w]
            } else {
                &mut scratch_resolved[..grid_w]
            };

            let mut has_custom_bg = false;
            let mut is_blank_row = false;

            grid.with_display_row_slice(y, |row_cells| {
                // Task 5: Fast path for completely blank dirty row
                let is_cursor_row = pane.is_active
                    && pane.cursor_visible
                    && row_info.is_active_grid_row
                    && row_info.grid_y == grid.cursor.y;

                if !row_info.is_row_in_selection && !is_cursor_row {
                    let all_spaces = row_cells.iter().all(|c| {
                        c.character == ' '
                            && c.background == pane.theme.default_bg
                            && c.underline_color.is_none()
                            && !c.flags.intersects(
                                CellFlags::REVERSE
                                    | CellFlags::BLINK
                                    | CellFlags::UNDERLINE
                                    | CellFlags::DOUBLE_UNDERLINE
                                    | CellFlags::CURLY_UNDERLINE
                                    | CellFlags::STRIKE
                                    | CellFlags::HIDDEN,
                            )
                    });
                    if all_spaces {
                        is_blank_row = true;
                        return;
                    }
                }

                // Task 1: Single pass color resolution into resolved scratch buffer
                for col in 0..grid_w {
                    let cell = row_cells.get(col).unwrap_or(&default_cell);
                    let is_selected = is_column_selected(
                        row_info.is_row_in_selection,
                        col,
                        row_info.abs_row,
                        sel_min_x,
                        sel_min_abs_y,
                        sel_max_x,
                        sel_max_abs_y,
                    );
                    let is_reverse = cell.flags.contains(CellFlags::REVERSE);
                    let is_inverted = is_selected ^ is_reverse;

                    let (mut fg, mut bg) = palette.resolve_cell_colors_pane(
                        cell,
                        is_inverted,
                        pane.bold_is_bright,
                        pane_effective_dim,
                        pane.theme,
                        default_pane_bg,
                    );

                    let is_cursor = is_cursor_row && col == pane.display_cursor_x;
                    let is_block_cursor = is_cursor
                        && pane.cursor_shape == CursorShape::Block
                        && pane.is_active
                        && is_focused;

                    if is_block_cursor {
                        let cell_fg = resolve_bold_bright_fg(
                            cell.foreground,
                            cell.flags,
                            pane.bold_is_bright,
                            palette,
                            &pane.theme.ansi_colors,
                        );
                        let cell_fg_dimmed = cell_fg.dim(pane_effective_dim);
                        bg = PackedColor::from_color(
                            pane.theme
                                .resolve_cursor_color(cell_fg_dimmed)
                                .dim(pane_effective_dim),
                        )
                        .to_u32();
                        fg = PackedColor::from_color(
                            pane.theme
                                .resolve_cursor_text_color(cell.background)
                                .dim(pane_effective_dim),
                        )
                        .to_u32();
                    }

                    let glyph_key = if cell.character != ' '
                        && !cell.flags.contains(CellFlags::WIDE_CONTINUATION)
                    {
                        let is_bold = cell.flags.contains(CellFlags::BOLD);
                        let is_italic = cell.flags.contains(CellFlags::ITALIC);
                        let is_wide = cell.flags.contains(CellFlags::WIDE);
                        Some(GlyphKey::new(cell.character, is_bold, is_italic, is_wide))
                    } else {
                        None
                    };

                    resolved[col] = ResolvedCell {
                        fg,
                        bg,
                        glyph_key,
                        flags: cell.flags,
                    };

                    if bg != default_pane_bg {
                        has_custom_bg = true;
                    }
                }

                // Pass A: Coalesced Background Spans
                if has_custom_bg {
                    let mut span_start_col = 0usize;
                    let mut span_bg = 0u32;
                    let mut in_span = false;

                    let flush_span =
                        |start_col: usize, end_col: usize, bg: u32, fb: &mut Framebuffer| {
                            if bg != default_pane_bg {
                                let span_px = px_offset + (start_col as u32) * cell_w;
                                let span_w = (((end_col - start_col) as u32) * cell_w)
                                    .min(max_x.saturating_sub(span_px));
                                if span_w > 0 && span_px < max_x {
                                    fb.fill_span(span_px, py, span_w, cell_h, bg);
                                }
                            }
                        };

                    for col in 0..grid_w {
                        let cell = row_cells.get(col).unwrap_or(&default_cell);
                        let res = &resolved[col];
                        let is_cursor = is_cursor_row && col == pane.display_cursor_x;
                        let is_block_cursor = is_cursor
                            && pane.cursor_shape == CursorShape::Block
                            && pane.is_active
                            && is_focused;

                        let is_solid_block = cell.character == '█'
                            && !is_block_cursor
                            && cell.underline_color.is_none()
                            && !cell.flags.intersects(
                                CellFlags::UNDERLINE
                                    | CellFlags::DOUBLE_UNDERLINE
                                    | CellFlags::CURLY_UNDERLINE
                                    | CellFlags::STRIKE,
                            );

                        if is_solid_block {
                            if in_span {
                                flush_span(span_start_col, col, span_bg, fb);
                            }
                            in_span = false;
                        } else if !in_span {
                            span_start_col = col;
                            span_bg = res.bg;
                            in_span = true;
                        } else if res.bg != span_bg {
                            flush_span(span_start_col, col, span_bg, fb);
                            span_start_col = col;
                            span_bg = res.bg;
                        }
                    }

                    if in_span {
                        flush_span(span_start_col, grid_w, span_bg, fb);
                    }
                }

                // Pass B: Glyphs, Primitives, Decorations
                let max_col = row_cells.len().min(grid_w);
                let mut col = 0usize;
                while col < max_col {
                    let cell = &row_cells[col];
                    let res = &resolved[col];
                    if res.flags.contains(CellFlags::WIDE_CONTINUATION) {
                        col += 1;
                        continue;
                    }
                    if cell.character == ' '
                        && res.flags.is_empty()
                        && cell.underline_color.is_none()
                    {
                        col += 1;
                        continue;
                    }

                    let px = px_offset + (col as u32) * cell_w;
                    if px + cell_w > max_x {
                        break;
                    }

                    let fg = res.fg;
                    let skip_fg = res.flags.contains(CellFlags::HIDDEN)
                        || (res.flags.contains(CellFlags::BLINK) && !blink_on);

                    let is_wide = res.flags.contains(CellFlags::WIDE);
                    let target_w = if is_wide { cell_w * 2 } else { cell_w };
                    let clip_w = max_x.saturating_sub(px);
                    let clip_h = max_y.saturating_sub(py);

                    if !skip_fg && cell.character != ' ' && clip_w > 0 && clip_h > 0 {
                        let has_deco = cell.underline_color.is_some()
                            || cell.flags.intersects(
                                CellFlags::UNDERLINE
                                    | CellFlags::DOUBLE_UNDERLINE
                                    | CellFlags::CURLY_UNDERLINE
                                    | CellFlags::STRIKE,
                            );

                        let is_cursor = is_cursor_row && col == pane.display_cursor_x;

                        let is_coalescible_block = !is_wide
                            && !is_cursor
                            && !has_deco
                            && (cell.character == '█'
                                || cell.character == '▀'
                                || ('\u{2581}'..='\u{2587}').contains(&cell.character)
                                || cell.character == '▔');

                        if is_coalescible_block {
                            let mut span_count = 1usize;
                            while col + span_count < max_col {
                                let next_col = col + span_count;
                                let next_px = px_offset + (next_col as u32) * cell_w;
                                if next_px + cell_w > max_x {
                                    break;
                                }
                                let next_cell = &row_cells[next_col];
                                let next_res = &resolved[next_col];
                                if next_cell.character != cell.character
                                    || next_res.flags.contains(CellFlags::WIDE)
                                    || next_res.flags.contains(CellFlags::WIDE_CONTINUATION)
                                    || next_cell.underline_color.is_some()
                                    || next_res.flags.intersects(
                                        CellFlags::UNDERLINE
                                            | CellFlags::DOUBLE_UNDERLINE
                                            | CellFlags::CURLY_UNDERLINE
                                            | CellFlags::STRIKE,
                                    )
                                {
                                    break;
                                }

                                let next_is_cursor =
                                    is_cursor_row && next_col == pane.display_cursor_x;
                                if next_is_cursor {
                                    break;
                                }

                                if next_res.fg != fg {
                                    break;
                                }
                                span_count += 1;
                            }

                            let span_w = ((span_count as u32) * cell_w).min(clip_w);
                            match cell.character {
                                '█' => {
                                    fb.fill_span(px, py, span_w, cell_h.min(clip_h), fg);
                                }
                                '▀' => {
                                    let h = (cell_h / 2).min(clip_h);
                                    fb.fill_span(px, py, span_w, h, fg);
                                }
                                '\u{2581}'..='\u{2587}' => {
                                    let frac = (cell.character as u32 - 0x2580) as f32 / 8.0;
                                    let h = ((cell_h as f32 * frac).round() as u32).max(1);
                                    let y = py + cell_h.saturating_sub(h);
                                    let h_clip = max_y.saturating_sub(y);
                                    fb.fill_span(px, y, span_w, h.min(h_clip), fg);
                                }
                                '▔' => {
                                    let h = (cell_h / 8).max(1).min(clip_h);
                                    fb.fill_span(px, py, span_w, h, fg);
                                }
                                _ => unreachable!(),
                            }

                            col += span_count;
                            continue;
                        }

                        let prim_w = target_w.min(clip_w);
                        if !try_render_primitive(
                            cell.character,
                            px,
                            py,
                            prim_w,
                            cell_h.min(clip_h),
                            fg,
                            fb,
                        ) {
                            if let Some(glyph_key) = res.glyph_key {
                                if let Some(glyph_ref) = glyph_cache.get_or_rasterize(glyph_key) {
                                    if glyph_ref.is_color {
                                        let pixels = glyph_cache.atlas.get_color(&glyph_ref);
                                        if is_opaque {
                                            blit_color_glyph_clipped_opaque(
                                                fb,
                                                px,
                                                py,
                                                pixels,
                                                glyph_ref.width,
                                                glyph_ref.height,
                                                clip_w,
                                                clip_h,
                                            );
                                        } else {
                                            blit_color_glyph_clipped(
                                                fb,
                                                px,
                                                py,
                                                pixels,
                                                glyph_ref.width,
                                                glyph_ref.height,
                                                clip_w,
                                                clip_h,
                                            );
                                        }
                                    } else {
                                        let mask = glyph_cache.atlas.get_alpha(&glyph_ref);
                                        if is_opaque {
                                            blit_alpha_glyph_clipped_opaque(
                                                fb,
                                                px,
                                                py,
                                                mask,
                                                glyph_ref.width,
                                                glyph_ref.height,
                                                fg,
                                                clip_w,
                                                clip_h,
                                            );
                                        } else {
                                            blit_alpha_glyph_clipped(
                                                fb,
                                                px,
                                                py,
                                                mask,
                                                glyph_ref.width,
                                                glyph_ref.height,
                                                fg,
                                                clip_w,
                                                clip_h,
                                            );
                                        }
                                    }
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

                    let deco_w = target_w.min(clip_w);
                    if deco_w > 0 {
                        if res.flags.contains(CellFlags::UNDERLINE) {
                            draw_underline(fb, px, py, deco_w, cell_h, ul_color);
                        }
                        if res.flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                            draw_double_underline(fb, px, py, deco_w, cell_h, ul_color);
                        }
                        if res.flags.contains(CellFlags::CURLY_UNDERLINE) {
                            draw_curly_underline(fb, px, py, deco_w, cell_h, ul_color);
                        }
                        if res.flags.contains(CellFlags::STRIKE) {
                            draw_strike(fb, px, py, deco_w, cell_h, ul_color);
                        }
                    }

                    col += 1;
                }
            });

            if is_blank_row {
                continue;
            }

            // Render Cursor for this row (non-block or unfocused cursor)
            if let Some((cursor_x, cursor_shape, _)) = row_info.current_cursor
                && (!is_focused || cursor_shape != CursorShape::Block)
            {
                let cursor_px = px_offset + (cursor_x as u32) * cell_w;
                let cursor_py = py_offset + (y as u32) * cell_h;

                let cursor_color = if cursor_x < grid_w {
                    resolved[cursor_x].fg
                } else {
                    palette.default_fg
                };

                if cursor_px + cell_w <= max_x && cursor_py + cell_h <= max_y {
                    draw_cursor(
                        fb,
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

        state.last_scroll_offset = grid.scroll_offset;
        state.last_rect = Some(pane.rect);
        state.last_is_active = pane.is_active;
        state.last_is_focused = is_focused;
        state.last_dim = pane_effective_dim;
        state.last_font_size = font_size;
        state.last_cursor_info = current_cursor_info;
        state.last_selection_bounds = selection_bounds;

        if pane_full_redraw {
            Some((tile_y, (tile_y + tile_h).min(fb.height)))
        } else if pane_min_y < pane_max_y {
            Some((pane_min_y, pane_max_y))
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_splits(
        &mut self,
        panes: &[CpuPaneRenderData],
        separators: &[SeparatorRenderData],
        opacity: f32,
        window_dim: f32,
        is_focused: bool,
        target_buffer: Option<&mut [u32]>,
        tab_bar_info: Option<&crate::app::tab::TabBarRenderInfo>,
        separator_color_override: Option<crate::screen::cell::Color>,
        active_separator_color_override: Option<crate::screen::cell::Color>,
    ) -> Option<crate::window::DamageRect> {
        if panes.is_empty() {
            return None;
        }

        let active_pane = panes.iter().find(|p| p.is_active).unwrap_or(&panes[0]);
        let base_theme = active_pane.theme;

        self.prune_stale_caches(panes);

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

        // 1. Sync theme updates
        self.sync_theme_and_palette(&effective_theme, opacity, effective_dim);

        // 2. Check damage across panes
        let any_pane_damage = self.check_panes_damage(panes, is_focused, effective_dim);

        // 3. Blink animation handling
        let blink_on = (self.start_time.elapsed().as_millis() / 500).is_multiple_of(2);
        let blink_changed = blink_on != self.prev_blink_on;
        self.prev_blink_on = blink_on;

        // 4. Tab bar dirty check
        let new_tab_bar_hash = Self::compute_tab_bar_hash(tab_bar_info);
        let tab_bar_dirty = new_tab_bar_hash != self.prev_tab_bar_hash;

        let force_redraw =
            self.damage.full_redraw || any_pane_damage || blink_changed || tab_bar_dirty;

        if !force_redraw && !self.damage.has_damage() {
            if let Some(buf) = target_buffer {
                let current_ptr = buf.as_ptr() as usize;
                if current_ptr != self.last_target_ptr && buf.len() == self.framebuffer.pixels.len()
                {
                    buf.copy_from_slice(self.framebuffer.as_slice());
                    self.last_target_ptr = current_ptr;
                }
            }
            return None;
        }

        let mut min_damaged_y = u32::MAX;
        let mut max_damaged_y = 0u32;

        if self.damage.full_redraw {
            min_damaged_y = 0;
            max_damaged_y = self.viewport_height;
            self.framebuffer.clear(self.palette.default_bg);
        }

        // 5. Pre-ensure glyph caches exist for all panes
        self.ensure_pane_glyph_caches(panes);

        // 6. Render each pane
        for pane in panes {
            if let Some((p_min, p_max)) = self.render_single_pane(
                pane,
                is_focused,
                opacity,
                effective_dim,
                blink_on,
                blink_changed,
            ) {
                min_damaged_y = min_damaged_y.min(p_min);
                max_damaged_y = max_damaged_y.max(p_max);
            }
        }

        // 7. Render Separators
        if !separators.is_empty() {
            self.render_separators(
                separators,
                separator_color_override,
                active_separator_color_override,
            );
        }

        // 8. Render Tab Bar
        if let Some(tab_bar) = tab_bar_info {
            if tab_bar_dirty || self.damage.full_redraw {
                self.render_tab_bar(tab_bar, self.csd_height);
                let tab_h = tab_bar.height.ceil() as u32;
                min_damaged_y = min_damaged_y.min(self.csd_height);
                max_damaged_y = max_damaged_y.max(self.csd_height + tab_h);
            }
        }
        self.prev_tab_bar_hash = new_tab_bar_hash;

        // 9. Present to target buffer (if provided)
        if let Some(buf) = target_buffer
            && buf.len() == self.framebuffer.pixels.len()
        {
            buf.copy_from_slice(self.framebuffer.as_slice());
            self.last_target_ptr = buf.as_ptr() as usize;
        }

        self.damage.clear();

        if min_damaged_y < max_damaged_y {
            Some(crate::window::DamageRect {
                x: 0,
                y: min_damaged_y,
                width: self.viewport_width,
                height: max_damaged_y.saturating_sub(min_damaged_y).max(1),
            })
        } else {
            None
        }
    }
}
