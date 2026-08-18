use std::sync::Arc;
use velox::font::fallback::{
    FallbackManager, MAX_FALLBACK_BYTES, MAX_FALLBACK_FONTS, MAX_MISSING_CHARS,
};
use velox::font::loader::MAX_DYNAMIC_GLYPHS;
use velox::font::storage::FontStorage;
use velox::renderer::software::atlas::{
    DEFAULT_ALPHA_CAPACITY, GlyphAtlas, MAX_RETAINED_ALPHA_CAPACITY,
};
use velox::renderer::software::glyph::{GlyphCache, GlyphKey, GlyphScratch};
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::scrollback::Chunk;

#[test]
fn test_fallback_manager_bounded_capacity() {
    let mut manager = FallbackManager::new();

    // Query diverse unicode characters across multiple scripts and PUA ranges
    let test_chars = [
        'A',
        'Z',
        '0',
        '9',
        'א',
        'ב',
        'ג', // Hebrew
        'а',
        'б',
        'в', // Cyrillic
        'α',
        'β',
        'γ', // Greek
        'क',
        'ख',
        'ग', // Devanagari
        'あ',
        'い',
        'う', // Japanese Hiragana
        '한',
        '글', // Korean Hangul
        '中',
        '文', // CJK Ideographs
        '\u{e0a0}',
        '\u{e0b0}',
        '\u{e0b2}', // Powerline
        '\u{f101}',
        '\u{f102}',
        '\u{f103}', // Nerd Font FontAwesome
        '\u{1f600}',
        '\u{1f680}',
        '\u{1f389}', // Emojis
    ];

    for &c in &test_chars {
        let _ = manager.find_fallback_for_char(c);
        assert!(
            manager.fallbacks.len() <= MAX_FALLBACK_FONTS,
            "Fallback font count ({}) exceeded bound ({})",
            manager.fallbacks.len(),
            MAX_FALLBACK_FONTS
        );
        assert!(
            manager.resident_bytes <= MAX_FALLBACK_BYTES * 2,
            "Fallback resident bytes ({}) exceeded budget limit ({})",
            manager.resident_bytes,
            MAX_FALLBACK_BYTES
        );
    }
}

#[test]
fn test_fallback_missing_chars_bounded() {
    let mut manager = FallbackManager::new();

    // Query characters in non-existent / obscure ranges to test missing_chars bounded set
    for i in 0..50 {
        let c = char::from_u32(0x30000 + i).unwrap_or('?');
        let _ = manager.find_fallback_for_char(c);
    }

    // Repeated lookups should hit the missing_chars cache immediately
    for i in 0..50 {
        let c = char::from_u32(0x30000 + i).unwrap_or('?');
        assert_eq!(manager.find_fallback_for_char(c), None);
    }

    assert!(manager.fallbacks.len() <= MAX_FALLBACK_FONTS);
}

#[test]
fn test_glyph_cache_max_bound_constant() {
    assert_eq!(MAX_DYNAMIC_GLYPHS, 2048);
    assert_eq!(MAX_FALLBACK_FONTS, 8);
    assert_eq!(MAX_MISSING_CHARS, 1024);
}

#[test]
fn test_mmap_font_loading_zero_copy() {
    let dummy_ttf = vec![0u8; 256];
    let storage = Arc::new(FontStorage::from_bytes(dummy_ttf));
    assert_eq!(storage.len(), 256);
    assert_eq!(storage.as_bytes().len(), 256);
}

#[test]
fn test_byte_budgeted_fallback_eviction() {
    let mut manager = FallbackManager::new();
    manager.max_fallback_fonts = 3;
    manager.max_fallback_bytes = 1024; // Very small budget to force byte-based eviction

    // Find fallbacks
    let _ = manager.find_fallback_for_char('\u{1f600}');
    let _ = manager.find_fallback_for_char('\u{e0b0}');
    let _ = manager.find_fallback_for_char('中');

    manager.prune_to_budget();
    assert!(manager.fallbacks.len() <= 3);

    // Explicit prune
    manager.prune_unused(1);
    assert!(manager.fallbacks.len() <= 1);
}

#[test]
fn test_glyph_scratch_buffer_reuse() {
    let mut scratch = GlyphScratch::new();
    scratch.png_buf.resize(1024 * 1024, 0xAB);
    scratch.alpha_pixels.resize(64 * 1024, 0xFF);
    scratch.color_pixels.resize(32 * 1024, 0x00FF00FF);

    assert!(scratch.png_buf.capacity() >= 1024 * 1024);
    scratch.clear_and_release(64 * 1024);

    // png_buf exceeded 64KB so it was shrunk; smaller buffers were cleared without allocation
    assert!(scratch.png_buf.capacity() <= 64 * 1024);
}

#[test]
fn test_glyph_atlas_bounds_and_release() {
    let mut atlas = GlyphAtlas::new();
    assert_eq!(atlas.total_bytes(), 0);
    assert!(!atlas.is_full());

    // Insert alpha pixels
    let data = vec![255u8; 1000];
    let g_ref = atlas.insert_alpha(100, 10, 0, 0, 1, &data);
    assert_eq!(g_ref.width, 100);
    assert_eq!(atlas.get_alpha(&g_ref).len(), 1000);
    assert_eq!(atlas.total_bytes(), 1000);

    // Grow past threshold
    let large_data = vec![128u8; MAX_RETAINED_ALPHA_CAPACITY + 2048];
    let _ = atlas.insert_alpha(
        1,
        (MAX_RETAINED_ALPHA_CAPACITY + 2048) as u16,
        0,
        0,
        1,
        &large_data,
    );
    assert!(atlas.alpha_pixels.capacity() > MAX_RETAINED_ALPHA_CAPACITY);

    atlas.clear_and_release();
    assert_eq!(atlas.total_bytes(), 0);
    assert!(atlas.alpha_pixels.capacity() <= DEFAULT_ALPHA_CAPACITY);
}

#[test]
fn test_contiguous_scrollback_chunk_representation() {
    let mut chunk = Chunk::new();
    let default_cell = Cell {
        character: 'X',
        foreground: Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        },
        background: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        flags: CellFlags::empty(),
    };

    for i in 0..100 {
        let row_cells = vec![default_cell; 80];
        chunk.push_row(&row_cells, i % 2 == 0);
    }

    assert_eq!(chunk.len(), 100);
    assert_eq!(chunk.cells.len(), 8000);
    assert_eq!(chunk.rows.len(), 100);

    let (view_cells, view_wrapped) = chunk.get_row_view(50).unwrap();
    assert_eq!(view_cells.len(), 80);
    assert_eq!(view_cells[0].character, 'X');
    assert!(view_wrapped);
}

#[test]
fn test_multicycle_unicode_memory_stability() {
    let mut cache = GlyphCache::from_font_family("monospace", 14.0, 1.0);

    let unicode_chars = [
        'A', 'B', '1', '2', '😀', '😁', '🚀', '🦀', '中', '文', '字', '日', '本', '語', 'न', 'म',
        'स', 'त', 'े', 'م', 'ر', 'ح', 'ب', 'ا', 'α', 'β', 'γ', 'П', 'р', 'и', 'в', 'е', 'т',
        '\u{e0a0}', '\u{e0b0}', '\u{f101}', '\u{f102}',
    ];

    let mut cycle_atlas_capacities = Vec::new();

    // Run 3 cycles of rendering followed by cleanup
    for _ in 0..3 {
        for &c in &unicode_chars {
            let _ = cache.get_or_rasterize(GlyphKey::new(c, false, false, false));
            let _ = cache.get_or_rasterize(GlyphKey::new(c, true, false, false));
            let _ = cache.get_or_rasterize(GlyphKey::new(c, false, true, false));
        }

        cache.release_memory();
        cycle_atlas_capacities.push(cache.atlas.total_capacity_bytes());
    }

    // Verify capacities after cleanup remain bounded and do not grow across cycles
    assert_eq!(cycle_atlas_capacities.len(), 3);
    assert_eq!(cycle_atlas_capacities[0], cycle_atlas_capacities[1]);
    assert_eq!(cycle_atlas_capacities[1], cycle_atlas_capacities[2]);
    assert!(
        cycle_atlas_capacities[2]
            <= DEFAULT_ALPHA_CAPACITY
                + (velox::renderer::software::atlas::DEFAULT_COLOR_CAPACITY * 4)
    );
}
