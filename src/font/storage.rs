use ab_glyph::{
    CodepointIdIter, Font, FontArc, FontRef, Glyph, GlyphId, Outline, OutlinedGlyph, Rect,
};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// Storage backing for font bytes: memory-mapped file, owned byte slice, or shared dynamic buffer.
#[derive(Clone)]
pub enum FontStorage {
    Mmap(Arc<memmap2::Mmap>),
    #[allow(dead_code)]
    Owned(Arc<[u8]>),
    Shared(Arc<dyn AsRef<[u8]> + Send + Sync>),
}

impl FontStorage {
    /// Attempt to memory-map a font file from the filesystem.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(Self::Mmap(Arc::new(mmap)))
    }

    /// Construct from owned bytes in memory.
    #[allow(dead_code)]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::Owned(Arc::from(bytes.into_boxed_slice()))
    }

    /// Construct from fontdb's shared binary buffer.
    pub fn from_shared(shared: Arc<dyn AsRef<[u8]> + Send + Sync>) -> Self {
        Self::Shared(shared)
    }

    /// Returns the underlying byte slice.
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Mmap(mmap) => mmap.as_ref(),
            Self::Owned(bytes) => bytes.as_ref(),
            Self::Shared(shared) => shared.as_ref().as_ref(),
        }
    }

    /// Total size in bytes of the font storage.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }
}

/// A zero-copy `ab_glyph::Font` implementation anchored to an `Arc<FontStorage>`.
pub struct MmapFont {
    _storage: Arc<FontStorage>,
    font_ref: FontRef<'static>,
}

// Safety: `MmapFont` contains an `Arc<FontStorage>` which keeps the underlying memory
// pinned on the heap for the entire lifetime of `MmapFont`. The static lifetime on `FontRef`
// is sound because the referenced bytes will never move or be deallocated before `MmapFont` is dropped.
unsafe impl Send for MmapFont {}
unsafe impl Sync for MmapFont {}

impl MmapFont {
    pub fn try_from_storage(
        storage: Arc<FontStorage>,
        index: u32,
    ) -> Result<Self, ab_glyph::InvalidFont> {
        let slice = storage.as_bytes();
        let font_ref = FontRef::try_from_slice_and_index(slice, index)?;
        let font_ref_static: FontRef<'static> = unsafe { std::mem::transmute(font_ref) };
        Ok(Self {
            _storage: storage,
            font_ref: font_ref_static,
        })
    }
}

impl Font for MmapFont {
    #[inline(always)]
    fn units_per_em(&self) -> Option<f32> {
        self.font_ref.units_per_em()
    }

    #[inline(always)]
    fn ascent_unscaled(&self) -> f32 {
        self.font_ref.ascent_unscaled()
    }

    #[inline(always)]
    fn descent_unscaled(&self) -> f32 {
        self.font_ref.descent_unscaled()
    }

    #[inline(always)]
    fn line_gap_unscaled(&self) -> f32 {
        self.font_ref.line_gap_unscaled()
    }

    #[inline(always)]
    fn glyph_id(&self, c: char) -> GlyphId {
        self.font_ref.glyph_id(c)
    }

    #[inline(always)]
    fn h_advance_unscaled(&self, id: GlyphId) -> f32 {
        self.font_ref.h_advance_unscaled(id)
    }

    #[inline(always)]
    fn h_side_bearing_unscaled(&self, id: GlyphId) -> f32 {
        self.font_ref.h_side_bearing_unscaled(id)
    }

    #[inline(always)]
    fn v_advance_unscaled(&self, id: GlyphId) -> f32 {
        self.font_ref.v_advance_unscaled(id)
    }

    #[inline(always)]
    fn v_side_bearing_unscaled(&self, id: GlyphId) -> f32 {
        self.font_ref.v_side_bearing_unscaled(id)
    }

    #[inline(always)]
    fn kern_unscaled(&self, first: GlyphId, second: GlyphId) -> f32 {
        self.font_ref.kern_unscaled(first, second)
    }

    #[inline(always)]
    fn outline(&self, id: GlyphId) -> Option<Outline> {
        self.font_ref.outline(id)
    }

    #[inline(always)]
    fn outline_glyph(&self, glyph: Glyph) -> Option<OutlinedGlyph> {
        self.font_ref.outline_glyph(glyph)
    }

    #[inline(always)]
    fn glyph_bounds(&self, glyph: &Glyph) -> Rect {
        self.font_ref.glyph_bounds(glyph)
    }

    #[inline(always)]
    fn glyph_count(&self) -> usize {
        self.font_ref.glyph_count()
    }

    #[inline(always)]
    fn codepoint_ids(&self) -> CodepointIdIter<'_> {
        self.font_ref.codepoint_ids()
    }

    #[inline(always)]
    fn glyph_raster_image2(&self, id: GlyphId, size: u16) -> Option<ab_glyph::v2::GlyphImage<'_>> {
        self.font_ref.glyph_raster_image2(id, size)
    }
}

/// Construct a `FontArc` backed by zero-copy `FontStorage`.
pub fn create_font_arc(
    storage: Arc<FontStorage>,
    index: u32,
) -> Result<FontArc, ab_glyph::InvalidFont> {
    let mmap_font = MmapFont::try_from_storage(storage, index)?;
    Ok(FontArc::new(mmap_font))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_storage_bytes() {
        let dummy_data = vec![0u8; 128];
        let storage = FontStorage::from_bytes(dummy_data.clone());
        assert_eq!(storage.len(), 128);
        assert_eq!(storage.as_bytes(), &dummy_data[..]);
    }
}
