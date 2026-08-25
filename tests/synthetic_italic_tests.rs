use ab_glyph::{Font, PxScale, ScaleFont};
use velox::font::fallback::get_system_font_db;
use velox::font::resolved::{ResolvedFontSet, SYNTHETIC_ITALIC_SHEAR, get_or_create_outlined_glyph, shear_outline};
use velox::renderer::software::glyph::{GlyphCache, GlyphKey};
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::grid::Grid;

#[test]
fn test_font_style_resolution_matrix() {
    let db = get_system_font_db();
    let set = ResolvedFontSet::resolve(db, "monospace");

    // 1. Regular is never synthetic italic
    assert!(!set.regular.synthetic_italic, "Regular must not be synthetic italic");

    // 2. Bold style
    let bold = set.get(true, false);
    assert!(bold.font.glyph_id('A').0 != 0, "Bold face must resolve");

    // 3. Italic style
    let italic = set.get(false, true);
    assert!(italic.font.glyph_id('A').0 != 0, "Italic face must resolve");

    // 4. Bold Italic style
    let bold_italic = set.get(true, true);
    assert!(bold_italic.font.glyph_id('A').0 != 0, "Bold Italic face must resolve");
}

#[test]
fn test_synthetic_italic_outline_shear_geometry() {
    let db = get_system_font_db();
    let query = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    let font = velox::font::loader::load_font_face(db, &query).expect("Monospace font");
    let scale = PxScale::from(16.0);
    let glyph_id = font.glyph_id('f');

    let mut outline = font.outline(glyph_id).expect("Outline for 'f'");
    let orig_bounds = outline.bounds;
    let orig_max_x = orig_bounds.max.x;

    shear_outline(&mut outline, SYNTHETIC_ITALIC_SHEAR);

    // Bounding box must expand horizontally to the right for positive ascent
    assert!(
        outline.bounds.max.x > orig_max_x,
        "Sheared bounds.max.x ({}) should exceed original ({})",
        outline.bounds.max.x,
        orig_max_x
    );

    let scaled = font.as_scaled(scale);
    let sf = scaled.scale_factor();
    let glyph = glyph_id.with_scale(scale);
    let outlined = ab_glyph::OutlinedGlyph::new(glyph, outline, sf);

    let mut non_zero_pixels = 0;
    outlined.draw(|_gx, _gy, alpha| {
        if alpha > 0.0 {
            non_zero_pixels += 1;
        }
    });

    assert!(
        non_zero_pixels > 0,
        "Transformed glyph must rasterize pixels with anti-aliasing"
    );
}

#[test]
fn test_multiple_font_sizes_synthetic_italic() {
    let sizes = [8.0, 10.0, 12.0, 14.0, 16.0, 20.0, 24.0];
    let test_chars = ['A', 'f', 'j', 'Q', '/', '@', 'W', '(', ')'];

    for &size in &sizes {
        let mut cache = GlyphCache::from_font_family("monospace", size, 1.0);
        for &ch in &test_chars {
            // Test both regular and italic styles
            let reg_key = GlyphKey::new(ch, false, false, false);
            let it_key = GlyphKey::new(ch, false, true, false);
            let bi_key = GlyphKey::new(ch, true, true, false);

            let reg_ref = cache.get_or_rasterize(reg_key);
            assert!(
                reg_ref.is_some(),
                "Failed to rasterize regular '{}' at size {}",
                ch,
                size
            );

            let it_ref = cache.get_or_rasterize(it_key);
            assert!(
                it_ref.is_some(),
                "Failed to rasterize synthetic italic '{}' at size {}",
                ch,
                size
            );

            let bi_ref = cache.get_or_rasterize(bi_key);
            assert!(
                bi_ref.is_some(),
                "Failed to rasterize bold italic '{}' at size {}",
                ch,
                size
            );
        }
    }
}

#[test]
fn test_clipping_prevention_challenging_glyphs() {
    let mut cache = GlyphCache::from_font_family("monospace", 16.0, 1.0);
    let challenging_chars = [
        '/', '\\', '(', ')', '[', ']', '{', '}', '<', '>',
        'f', 'j', 'y', 'Q', '@', '&', '%', 'W',
    ];

    for &ch in &challenging_chars {
        let key = GlyphKey::new(ch, false, true, false);
        let g_ref = cache.get_or_rasterize(key).expect("Rasterize italic char");
        assert!(g_ref.width > 0, "Glyph '{}' width must be non-zero", ch);
        assert!(g_ref.height > 0, "Glyph '{}' height must be non-zero", ch);
    }
}

#[test]
fn test_combining_characters_synthetic_italic() {
    let db = get_system_font_db();
    let query = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    let font = velox::font::loader::load_font_face(db, &query).expect("Monospace font");
    let scale = PxScale::from(16.0);

    // Combining acute accent U+0301
    let acute_id = font.glyph_id('\u{0301}');
    if acute_id.0 != 0 {
        let outlined_shear = get_or_create_outlined_glyph(&font, acute_id, scale, true);
        assert!(
            outlined_shear.is_some(),
            "Combining accent must support synthetic italic transformation"
        );
    }
}

#[test]
fn test_cache_key_isolation_regular_vs_italic() {
    let mut cache = GlyphCache::from_font_family("monospace", 14.0, 1.0);

    let reg_key = GlyphKey::new('A', false, false, false);
    let it_key = GlyphKey::new('A', false, true, false);
    let bold_key = GlyphKey::new('A', true, false, false);
    let bi_key = GlyphKey::new('A', true, true, false);

    let reg_ref = cache.get_or_rasterize(reg_key).unwrap();
    let it_ref = cache.get_or_rasterize(it_key).unwrap();
    let bold_ref = cache.get_or_rasterize(bold_key).unwrap();
    let bi_ref = cache.get_or_rasterize(bi_key).unwrap();

    // Cache lookup must return isolated entries without collisions
    assert_eq!(cache.get(reg_key), Some(reg_ref));
    assert_eq!(cache.get(it_key), Some(it_ref));
    assert_eq!(cache.get(bold_key), Some(bold_ref));
    assert_eq!(cache.get(bi_key), Some(bi_ref));
}

#[test]
fn test_software_renderer_renders_italic_and_bold_italic_lines() {
    let theme = velox::theme::theme::Theme::new();
    let dummy_cache = velox::renderer::software::glyph::GlyphCache::from_font_family("monospace", 14.0, 1.0);
    let cell_w = dummy_cache.cell_width;
    let cell_h = dummy_cache.cell_height;
    let screen_w = 40 * cell_w;
    let screen_h = 10 * cell_h;

    let mut renderer = velox::renderer::software::CpuRenderer::new(
        "monospace",
        14.0,
        1.0,
        &theme,
        screen_w,
        screen_h,
        true,
        1.0,
    );
    let mut grid = Grid::new(
        40,
        10,
        Color { r: 255, g: 255, b: 255 },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    grid.damage.mark_all();

    // Write a test page with normal, italic, bold, and bold-italic text
    let test_line_norm = "Normal: ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123";
    let test_line_ital = "Italic: abcdefghijklmnopqrstuvwxyz /\\()";
    let test_line_bold = "Bold:   f j y Q @ & % AV To Wa";
    let test_line_bi   = "BoldIt: ABCDEFGHIJKLMNOPQRSTUVWXYZ /\\";

    for (x, ch) in test_line_norm.chars().enumerate() {
        grid.cells[x] = Cell {
            character: ch,
            foreground: Color { r: 240, g: 240, b: 240 },
            background: Color { r: 20, g: 20, b: 20 },
            flags: CellFlags::empty(),
        };
    }
    for (x, ch) in test_line_ital.chars().enumerate() {
        grid.cells[40 + x] = Cell {
            character: ch,
            foreground: Color { r: 200, g: 220, b: 255 },
            background: Color { r: 20, g: 20, b: 20 },
            flags: CellFlags::ITALIC,
        };
    }
    for (x, ch) in test_line_bold.chars().enumerate() {
        grid.cells[80 + x] = Cell {
            character: ch,
            foreground: Color { r: 255, g: 200, b: 200 },
            background: Color { r: 20, g: 20, b: 20 },
            flags: CellFlags::BOLD,
        };
    }
    for (x, ch) in test_line_bi.chars().enumerate() {
        grid.cells[120 + x] = Cell {
            character: ch,
            foreground: Color { r: 255, g: 255, b: 180 },
            background: Color { r: 20, g: 20, b: 20 },
            flags: CellFlags::BOLD | CellFlags::ITALIC,
        };
    }

    let cell_w = renderer.glyph_cache.cell_width;
    let cell_h = renderer.glyph_cache.cell_height;
    let mut fb = vec![0u32; (40 * cell_w * 10 * cell_h) as usize];

    // Render should complete cleanly with 0 panics
    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        true,
        velox::screen::cursor::CursorShape::Block,
        0,
        true,
        1.0,
        0.0,
        &mut fb,
        None,
    );

    // Verify framebuffer contains non-zero rendered pixels
    let non_zero_pixels = fb.iter().filter(|&&p| p != 0).count();
    assert!(non_zero_pixels > 0, "Rendered frame must contain glyph pixels");
}
