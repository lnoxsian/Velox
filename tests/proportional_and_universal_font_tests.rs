use ab_glyph::{Font, PxScale, ScaleFont};
use velox::font::fallback::get_system_font_db;
use velox::font::resolved::ResolvedFontSet;
use velox::renderer::glyph::{GlyphCache, GlyphKey, compute_font_cell_metrics};
use velox::screen::cell::{Cell, CellFlags, Color};
use velox::screen::grid::Grid;

#[test]
fn test_universal_font_resolution_case_insensitivity_and_aliases() {
    let db = get_system_font_db();

    // 1. Lowercase resolution
    let res_lower = ResolvedFontSet::try_resolve(db, "ubuntu");
    assert!(
        res_lower.is_ok(),
        "Lowercase 'ubuntu' must resolve to a valid font face: {:?}",
        res_lower.err()
    );

    // 2. Uppercase resolution
    let res_upper = ResolvedFontSet::try_resolve(db, "UBUNTU");
    assert!(
        res_upper.is_ok(),
        "Uppercase 'UBUNTU' must resolve to a valid font face: {:?}",
        res_upper.err()
    );

    // 3. Qualifier-stripped resolution ("ubuntu non mono")
    let res_qual = ResolvedFontSet::try_resolve(db, "ubuntu non mono");
    assert!(
        res_qual.is_ok(),
        "'ubuntu non mono' must resolve to Ubuntu font face: {:?}",
        res_qual.err()
    );

    // 4. Comma-separated fallback chain
    let res_chain =
        ResolvedFontSet::try_resolve(db, "NonExistentFamilyName_XYZ_123, ubuntu, sans-serif");
    assert!(
        res_chain.is_ok(),
        "Comma-separated fallback chain must succeed when secondary candidate exists: {:?}",
        res_chain.err()
    );

    // 5. Generic CSS keyword "sans-serif"
    let res_sans = ResolvedFontSet::try_resolve(db, "sans-serif");
    assert!(
        res_sans.is_ok(),
        "Generic 'sans-serif' keyword must resolve to a system sans-serif font: {:?}",
        res_sans.err()
    );
}

#[test]
fn test_proportional_font_cell_metrics_and_baseline_calculation() {
    let db = get_system_font_db();
    let query = fontdb::Query {
        families: &[fontdb::Family::Name("Ubuntu")],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    if let Some(id) = db.query(&query)
        && let Some(font) = velox::font::loader::load_font_face_by_id(db, id)
    {
        let scale = PxScale::from(16.0);
        let metrics = compute_font_cell_metrics(&font, scale);

        assert!(
            metrics.cell_width >= 8,
            "Cell width for Ubuntu 16pt should accommodate representative characters: {}",
            metrics.cell_width
        );
        assert!(
            metrics.cell_height >= 16,
            "Cell height for Ubuntu 16pt must accommodate ascenders and descenders: {}",
            metrics.cell_height
        );
        assert!(
            metrics.baseline_y > 0.0 && metrics.baseline_y < metrics.cell_height as f32,
            "Baseline Y must sit within cell bounds: baseline={}, cell_height={}",
            metrics.baseline_y,
            metrics.cell_height
        );
    }
}

#[test]
fn test_clipping_prevention_on_wide_characters_in_proportional_fonts() {
    // When using proportional fonts like Ubuntu, wide characters ('W', 'm', '@') must not be clipped
    let mut cache = match GlyphCache::try_from_font_family("Ubuntu", 16.0, 1.0) {
        Ok(c) => c,
        Err(_) => return, // Skip test if Ubuntu font is not installed on this system
    };

    let wide_test_chars = ['W', 'M', 'm', 'w', '@', '%', '&'];
    for ch in wide_test_chars {
        let key = GlyphKey::new(ch, false, false, false);
        let gref = cache
            .get_or_rasterize(key)
            .expect("Rasterize wide character");
        assert!(
            gref.width >= cache.cell_width as u16,
            "Wide character '{}' width in atlas must be at least cell_width (not clipped)",
            ch
        );
        assert_eq!(
            gref.height, cache.cell_height as u16,
            "Wide character '{}' height in atlas must match cell_height",
            ch
        );

        let mask = cache.atlas.get_alpha(&gref);
        let w = gref.width as usize;
        let h = gref.height as usize;

        // Verify character is not clipped on the right (rightmost column has headroom or valid boundary)
        let mut non_zero_pixels = 0;
        for y in 0..h {
            for x in 0..w {
                if mask[y * w + x] > 0 {
                    non_zero_pixels += 1;
                }
            }
        }
        assert!(
            non_zero_pixels > 0,
            "Character '{}' must render non-zero pixels",
            ch
        );
    }
}

#[test]
fn test_clipping_prevention_on_descenders_and_accents() {
    let mut cache = match GlyphCache::try_from_font_family("Ubuntu", 16.0, 1.0) {
        Ok(c) => c,
        Err(_) => return, // Skip test if Ubuntu font is not installed
    };

    let descender_chars = ['g', 'y', 'j', 'p', 'q', '|', '(', ')', '[', ']', '{', '}'];
    for ch in descender_chars {
        let key = GlyphKey::new(ch, false, false, false);
        let gref = cache.get_or_rasterize(key).expect("Rasterize descender");
        let mask = cache.atlas.get_alpha(&gref);
        let non_zero = mask.iter().filter(|&&a| a > 0).count();
        assert!(
            non_zero > 0,
            "Descender '{}' must render pixels without being clipped away",
            ch
        );
    }

    let accent_chars = ['É', 'Â', 'Å', 'Ç'];
    for ch in accent_chars {
        let key = GlyphKey::new(ch, false, false, false);
        let gref = cache
            .get_or_rasterize(key)
            .expect("Rasterize accented capital");
        let mask = cache.atlas.get_alpha(&gref);
        let non_zero = mask.iter().filter(|&&a| a > 0).count();
        assert!(
            non_zero > 0,
            "Accented character '{}' must render pixels without being clipped away",
            ch
        );
    }
}

#[test]
fn test_monospace_font_metrics_and_paddings_preserved() {
    let db = get_system_font_db();
    let query = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    if let Some(font) = velox::font::loader::load_font_face(db, &query) {
        let scale = PxScale::from(16.0);
        let metrics = compute_font_cell_metrics(&font, scale);
        let scaled_font = font.as_scaled(scale);
        let adv_a = scaled_font.h_advance(font.glyph_id('A'));

        assert_eq!(
            metrics.cell_width,
            adv_a.ceil().max(1.0) as u32,
            "Monospace cell_width must equal ceil of character advance"
        );
    }
}

#[test]
fn test_wide_characters_preserve_separation_gap_in_grid() {
    let cache = match GlyphCache::try_from_font_family("Ubuntu", 16.0, 1.0) {
        Ok(c) => c,
        Err(_) => return, // Skip test if Ubuntu font is not installed
    };

    // For proportional fonts, cell_width must accommodate the widest glyph so adjacent characters do not collide
    let scale = PxScale::from(16.0);
    let scaled_font = cache.font.as_scaled(scale);
    let adv_w = scaled_font.h_advance(cache.font.glyph_id('W'));
    let adv_m = scaled_font.h_advance(cache.font.glyph_id('m'));

    assert!(
        cache.cell_width as f32 >= adv_w,
        "Proportional cell_width ({}) must be at least advance of 'W' ({:.2}) to prevent collision",
        cache.cell_width,
        adv_w
    );
    assert!(
        cache.cell_width as f32 >= adv_m,
        "Proportional cell_width ({}) must be at least advance of 'm' ({:.2}) to prevent collision",
        cache.cell_width,
        adv_m
    );
}

#[test]
fn test_software_renderer_draws_full_proportional_grid_cleanly() {
    let theme = velox::theme::theme::Theme::new();
    let cache = match GlyphCache::try_from_font_family("Ubuntu", 14.0, 1.0) {
        Ok(c) => c,
        Err(_) => return, // Skip test if Ubuntu font is not installed
    };

    let cell_w = cache.cell_width;
    let cell_h = cache.cell_height;
    let screen_w = 40 * cell_w;
    let screen_h = 8 * cell_h;

    let mut renderer = velox::renderer::CpuRenderer::new(
        "Ubuntu", 14.0, 1.0, &theme, screen_w, screen_h, true, 1.0,
    );

    let mut grid = Grid::new(
        40,
        8,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color {
            r: 20,
            g: 20,
            b: 20,
        },
        50,
        false,
    );
    grid.damage.mark_all();

    // Line with wide chars, descenders, and punctuation
    let line1 = "Welcome to Velox: @user & %status = 100%";
    let line2 = "Quick brown fox jumps over lazy dog /|\\";
    let line3 = "Accents: É Â Å Ç and brackets: ([{}])";

    for (x, ch) in line1.chars().enumerate() {
        grid.cells[x] = Cell::new(
            ch,
            Color {
                r: 240,
                g: 240,
                b: 240,
            },
            Color {
                r: 20,
                g: 20,
                b: 20,
            },
            CellFlags::empty(),
        );
    }
    for (x, ch) in line2.chars().enumerate() {
        grid.cells[40 + x] = Cell::new(
            ch,
            Color {
                r: 220,
                g: 240,
                b: 255,
            },
            Color {
                r: 20,
                g: 20,
                b: 20,
            },
            CellFlags::empty(),
        );
    }
    for (x, ch) in line3.chars().enumerate() {
        grid.cells[80 + x] = Cell::new(
            ch,
            Color {
                r: 255,
                g: 220,
                b: 200,
            },
            Color {
                r: 20,
                g: 20,
                b: 20,
            },
            CellFlags::empty(),
        );
    }

    let mut fb = vec![0u32; (screen_w * screen_h) as usize];
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

    let non_zero_pixels = fb.iter().filter(|&&p| p != 0).count();
    assert!(
        non_zero_pixels > 0,
        "Rendered proportional font frame must contain pixels"
    );
}

#[test]
fn test_line_height_multiplier_metrics_and_rasterization() {
    let db = get_system_font_db();
    let query = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    if let Some(font) = velox::font::loader::load_font_face(db, &query) {
        let scale = PxScale::from(16.0);
        let m_default = compute_font_cell_metrics(&font, scale);
        let m_tight =
            velox::renderer::glyph::compute_font_cell_metrics_with_line_height(&font, scale, 0.85);
        let m_loose =
            velox::renderer::glyph::compute_font_cell_metrics_with_line_height(&font, scale, 1.25);

        assert!(
            m_tight.cell_height < m_default.cell_height,
            "Tight line_height (0.85) must produce strictly smaller cell_height ({} vs {})",
            m_tight.cell_height,
            m_default.cell_height
        );
        assert!(
            m_loose.cell_height > m_default.cell_height,
            "Loose line_height (1.25) must produce strictly larger cell_height ({} vs {})",
            m_loose.cell_height,
            m_default.cell_height
        );
        assert_eq!(
            m_tight.cell_width, m_default.cell_width,
            "Cell width must be unaffected by line_height"
        );
        assert!(
            m_tight.baseline_y <= m_tight.cell_height as f32,
            "Baseline Y must stay within tight cell bounds"
        );
    }

    // Verify GlyphCache with custom line_height rasterizes cleanly without panic or clipping
    let mut cache = GlyphCache::try_from_font_family_with_line_height("Monospace", 14.0, 1.5, 0.85)
        .expect("Create cache with compact line_height");
    let test_chars = ['A', 'g', 'y', 'Å', 'É', 'W', '@', '%', '_', '|'];
    for ch in test_chars {
        let key = GlyphKey::new(ch, false, false, false);
        let gref = cache.get_or_rasterize(key).expect("Rasterize character");
        assert_eq!(
            gref.height, cache.cell_height as u16,
            "Glyph height must equal compact cell_height"
        );
        let mask = cache.atlas.get_alpha(&gref);
        let non_zero = mask.iter().filter(|&&a| a > 0).count();
        assert!(
            non_zero > 0,
            "Character '{}' must render non-zero pixels under compact line_height",
            ch
        );
    }

    // Dynamic set_line_height
    cache.set_line_height(1.2);
    assert_eq!(cache.line_height, 1.2);
    let gref_loose = cache
        .get_or_rasterize(GlyphKey::new('M', false, false, false))
        .unwrap();
    assert_eq!(gref_loose.height, cache.cell_height as u16);
}

#[test]
fn test_devanagari_combining_characters_rasterization_and_rendering() {
    let default_fg = Color {
        r: 255,
        g: 255,
        b: 255,
    };
    let default_bg = Color { r: 0, g: 0, b: 0 };
    let mut grid = Grid::new(30, 2, default_fg, default_bg, 100, true);
    let devanagari_text = "नमस्ते दुनिया";

    for c in devanagari_text.chars() {
        grid.put_char(c, default_fg, default_bg, None, CellFlags::empty());
    }

    // Check cells in row 0
    let row_cells: Vec<char> = grid.cells[0..grid.cursor.x]
        .iter()
        .map(|c| c.character)
        .collect();

    println!("Devanagari row cells: {:?}", row_cells);

    let mut cache = GlyphCache::try_from_font_family("Monospace", 14.0, 1.0)
        .expect("Create cache for devanagari test");

    for (i, &ch) in row_cells.iter().enumerate() {
        let key = GlyphKey::new(ch, false, false, false);
        let gref = cache
            .get_or_rasterize(key)
            .unwrap_or_else(|| panic!("Rasterize failed for cell {} ({:?})", i, ch));
        if ch != ' ' {
            let mask = cache.atlas.get_alpha(&gref);
            let non_zero = mask.iter().filter(|&&a| a > 0).count();
            assert!(
                non_zero > 0,
                "Cell {} with char {:?} (codepoint U+{:04X}) must have non-zero rasterized pixels",
                i,
                ch,
                ch as u32
            );
        }
    }

    // Verify selection text extraction expands combining sequences back to original UTF-8
    grid.selection.start_selection(0, 0);
    grid.selection
        .update_selection(grid.cursor.x.saturating_sub(1), 0);
    let extracted = grid.extract_selection_text();
    println!("Extracted text: {:?}", extracted);
    assert_eq!(
        extracted, devanagari_text,
        "Extracted selection text must match original Devanagari string"
    );

    // Also verify Accents and Zalgo combining text
    let other_tests = ["á é í ó ú", "Z͑̄͆ͭ̒̅"];
    for test_str in other_tests {
        let mut test_grid = Grid::new(40, 2, default_fg, default_bg, 100, true);
        for c in test_str.chars() {
            test_grid.put_char(c, default_fg, default_bg, None, CellFlags::empty());
        }
        for (i, cell) in test_grid.cells[0..test_grid.cursor.x].iter().enumerate() {
            let key = GlyphKey::new(cell.character, false, false, false);
            let gref = cache.get_or_rasterize(key).unwrap_or_else(|| {
                panic!(
                    "Rasterize failed for {:?} in {:?}",
                    cell.character, test_str
                )
            });
            if cell.character != ' ' {
                let mask = cache.atlas.get_alpha(&gref);
                let non_zero = mask.iter().filter(|&&a| a > 0).count();
                assert!(
                    non_zero > 0,
                    "Cell {} with char {:?} in {:?} must have non-zero pixels",
                    i,
                    cell.character,
                    test_str
                );
            }
        }
        test_grid.selection.start_selection(0, 0);
        test_grid
            .selection
            .update_selection(test_grid.cursor.x.saturating_sub(1), 0);
        let ext = test_grid.extract_selection_text();
        assert_eq!(
            ext, test_str,
            "Extracted text must match original combining string"
        );
    }
}
