/// Optional frame statistics and performance counters for development and benchmarking.
#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub struct RenderStats {
    pub frame_time_ns: u64,
    pub dirty_rows: usize,
    pub dirty_cells: usize,
    pub glyph_cache_hits: u64,
    pub glyph_cache_misses: u64,
    pub glyph_rasterizations: u64,
    pub pixels_written: u64,
    pub spans_drawn: usize,
}

#[allow(dead_code)]
impl RenderStats {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
