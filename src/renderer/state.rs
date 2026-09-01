use crate::app::split::PaneRect;
use crate::screen::cursor::CursorShape;

/// Cached CPU-side render data for a single row of a terminal pane.
#[derive(Debug, Clone, Default)]
pub struct RowRenderCache {
    /// Cached background vertices for this row (8 floats per vertex, 6 vertices per quad).
    pub bg_vertices: Vec<f32>,
    /// Cached foreground vertices (glyphs, underlines, strike, decorations) for this row.
    pub fg_vertices: Vec<f32>,
    /// Cached cursor state when this row was built (to detect cursor move/shape/blink changes).
    pub last_cursor: Option<(usize, CursorShape, bool)>,
    /// Cached selection range for this row (to detect selection drag/changes).
    pub last_selection_range: Option<(usize, usize)>,
    /// Whether this row's cache is valid and can be reused directly.
    pub valid: bool,
}

impl RowRenderCache {
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    pub fn clear(&mut self) {
        self.bg_vertices.clear();
        self.fg_vertices.clear();
        self.last_cursor = None;
        self.last_selection_range = None;
        self.valid = false;
    }

    pub fn release_memory(&mut self) {
        self.bg_vertices = Vec::new();
        self.fg_vertices = Vec::new();
        self.last_cursor = None;
        self.last_selection_range = None;
        self.valid = false;
    }
}

/// Row-level damage tracker for a pane.
#[derive(Debug, Clone, Default)]
pub struct DirtyRowTracker {
    pub dirty: Vec<bool>,
}

impl DirtyRowTracker {
    pub fn new(rows: usize) -> Self {
        Self {
            dirty: vec![true; rows.max(1)],
        }
    }

    pub fn resize(&mut self, rows: usize) {
        self.dirty.resize(rows.max(1), true);
    }

    #[inline]
    pub fn mark_dirty(&mut self, row: usize) {
        if row < self.dirty.len() {
            self.dirty[row] = true;
        }
    }

    #[inline]
    pub fn mark_all(&mut self) {
        self.dirty.fill(true);
    }

    #[inline]
    pub fn clear(&mut self) {
        self.dirty.fill(false);
    }

    #[inline]
    pub fn is_dirty(&self, row: usize) -> bool {
        self.dirty.get(row).copied().unwrap_or(true)
    }

    #[inline]
    pub fn any_dirty(&self) -> bool {
        self.dirty.iter().any(|&d| d)
    }
}

/// Dedicated per-pane render state cache.
/// Separates authoritative TerminalState from derived rendering cache.
#[derive(Debug, Clone)]
pub struct PaneRenderState {
    pub dirty: bool,
    pub full_redraw: bool,
    pub dirty_rows: DirtyRowTracker,
    pub row_cache: Vec<RowRenderCache>,
    pub last_cols: usize,
    pub last_rows: usize,
    pub last_font_size: f32,
    pub last_rect: Option<PaneRect>,
    pub last_dim: f32,
    pub last_blink_on: bool,
    pub last_scroll_offset: usize,
}

impl Default for PaneRenderState {
    fn default() -> Self {
        Self::new()
    }
}

impl PaneRenderState {
    pub fn new() -> Self {
        Self {
            dirty: true,
            full_redraw: true,
            dirty_rows: DirtyRowTracker::new(24),
            row_cache: Vec::new(),
            last_cols: 0,
            last_rows: 0,
            last_font_size: 0.0,
            last_rect: None,
            last_dim: 0.0,
            last_blink_on: true,
            last_scroll_offset: 0,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn mark_full_redraw(&mut self) {
        self.dirty = true;
        self.full_redraw = true;
        self.dirty_rows.mark_all();
        for row in &mut self.row_cache {
            row.valid = false;
        }
    }

    pub fn mark_row_dirty(&mut self, row: usize) {
        self.dirty = true;
        self.dirty_rows.mark_dirty(row);
        if let Some(r) = self.row_cache.get_mut(row) {
            r.valid = false;
        }
    }

    pub fn ensure_rows(&mut self, rows: usize) {
        if self.row_cache.len() != rows {
            self.row_cache.resize_with(rows, RowRenderCache::default);
            self.dirty_rows.resize(rows);
            self.mark_full_redraw();
        }
    }

    pub fn clear_damage(&mut self) {
        self.dirty = false;
        self.full_redraw = false;
        self.dirty_rows.clear();
    }

    pub fn release_memory(&mut self) {
        for row in &mut self.row_cache {
            row.release_memory();
        }
        self.mark_full_redraw();
    }
}
