pub use crate::font::resolved::{
    FontError, ResolvedFont, ResolvedFontSet, STANDARD_MONOSPACE_FAMILIES, SYNTHETIC_ITALIC_SHEAR,
    get_or_create_outlined_glyph, shear_outline,
};
use crate::font::storage::{FontStorage, create_font_arc};
use ab_glyph::FontArc;
use std::sync::Arc;

pub fn load_font_face_by_id(db: &fontdb::Database, id: fontdb::ID) -> Option<FontArc> {
    let face = db.face(id)?;
    let storage = match &face.source {
        fontdb::Source::File(path) => FontStorage::from_file(path).ok().map(Arc::new),
        fontdb::Source::Binary(data) | fontdb::Source::SharedFile(_, data) => {
            Some(Arc::new(FontStorage::from_shared(Arc::clone(data))))
        }
    };

    if let Some(st) = storage
        && let Ok(font) = create_font_arc(st, face.index)
    {
        Some(font)
    } else {
        None
    }
}

pub fn load_font_face(db: &fontdb::Database, query: &fontdb::Query) -> Option<FontArc> {
    if let Some(id) = db.query(query)
        && let Some(face) = db.face(id)
    {
        if query.style == fontdb::Style::Normal && face.style != fontdb::Style::Normal {
            return None;
        }

        if let Some(font) = load_font_face_by_id(db, id) {
            return Some(font);
        }
    }

    if query.style == fontdb::Style::Italic {
        let mut oblique_query = *query;
        oblique_query.style = fontdb::Style::Oblique;
        return load_font_face(db, &oblique_query);
    }

    None
}

pub fn is_emoji(c: char) -> bool {
    matches!(c,
        '\u{1f300}'..='\u{1faff}' | // Misc Symbols & Pictographs, Emoticons, Transport, Supplemental, Extended-A
        '\u{1f1e6}'..='\u{1f1ff}' | // Regional indicator symbols (Flags)
        '\u{1f004}' | '\u{1f0cf}' | // Mahjong, Playing Card Joker
        '\u{2600}'..='\u{27bf}'   | // Misc Symbols & Dingbats
        '\u{2b50}' | '\u{2b55}' | '\u{2b1b}' | '\u{2b1c}' | // Star, Circle, Large Squares
        '\u{231a}' | '\u{231b}'   | // Watch, Hourglass
        '\u{23e9}'..='\u{23ec}'   | // Fast-forward, Rewind, Up, Down
        '\u{23f0}' | '\u{23f3}'   | // Alarm clock, Hourglass done
        '\u{23f8}'..='\u{23fa}'   | // Pause, Stop, Record
        '\u{25aa}'..='\u{25ab}'   | // Black/white small square
        '\u{25fb}'..='\u{25fe}'   | // Medium squares
        '\u{2934}'..='\u{2935}'   | // Arrow curving up/down
        '\u{3030}' | '\u{303d}'   | // Wavy dash, part alternation mark
        '\u{3297}' | '\u{3299}'     // Circled ideographs
    )
}

pub fn is_nerd_font_or_pua(c: char) -> bool {
    matches!(c,
        '\u{2300}'..='\u{24ff}' |   // Misc Technical, Control Pictures, OCR, Enclosed Alphanumerics
        '\u{25a0}'..='\u{2bff}' |   // Geometric Shapes, Misc Symbols, Dingbats, Arrows (EXCLUDES 0x2500..=0x259F Box/Block)
        '\u{e000}'..='\u{f8ff}' |   // Private Use Area (Nerd Fonts, Powerline, Devicons, FontAwesome, Octicons)
        '\u{f0000}'..='\u{ffffd}' | // Supplementary Private Use Area A
        '\u{100000}'..='\u{10fffd}' // Supplementary Private Use Area B
    ) && !is_emoji(c)
}

pub fn is_powerline(c: char) -> bool {
    matches!(c, '\u{e0b0}'..='\u{e0bf}')
}

pub fn is_box_drawing_or_pipe(c: char) -> bool {
    matches!(c,
        '\u{2500}'..='\u{257f}' | // Box Drawing
        '\u{2580}'..='\u{259f}' | // Block Elements
        '|'                      // ASCII vertical pipe
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ab_glyph::{Font, ScaleFont};

    #[test]
    fn test_ab_glyph_outline_types() {
        let db = crate::font::fallback::get_system_font_db();
        let query = fontdb::Query {
            families: &[fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        if let Some(font) = load_font_face(db, &query) {
            let scale = ab_glyph::PxScale::from(16.0);
            let glyph = font.glyph_id('H').with_scale(scale);
            let scaled = font.as_scaled(scale);
            let sf = scaled.scale_factor();
            if let Some(native_outlined) = font.outline_glyph(glyph.clone()) {
                eprintln!("Native px bounds: {:?}", native_outlined.px_bounds());
            }
            if let Some(mut outline) = font.outline(glyph.id) {
                eprintln!("Original outline bounds: {:?}", outline.bounds);
                let orig_max_x = outline.bounds.max.x;
                shear_outline(&mut outline, 0.20);
                assert!(
                    outline.bounds.max.x > orig_max_x,
                    "Sheared outline bounds should extend further right at top"
                );
                eprintln!("Outline bounds: {:?}", outline.bounds);
                eprintln!("Scale factor: {:?}", sf);
                let outlined = ab_glyph::OutlinedGlyph::new(glyph, outline, sf);
                eprintln!("Outlined px bounds: {:?}", outlined.px_bounds());
                let mut drawn_pixels = 0;
                outlined.draw(|gx, gy, alpha| {
                    eprintln!("gx: {}, gy: {}, alpha: {}", gx, gy, alpha);
                    if alpha > 0.0 {
                        drawn_pixels += 1;
                    }
                });
                assert!(
                    drawn_pixels > 0,
                    "Sheared glyph must rasterize successfully"
                );
            }
        }
    }

    #[test]
    fn test_character_positioning_preserves_font_bearing_and_uniform_padding() {
        let db = crate::font::fallback::get_system_font_db();
        let query = fontdb::Query {
            families: &[fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        if let Some(font) = load_font_face(db, &query) {
            let scale = ab_glyph::PxScale::from(16.0);
            let scaled_font = font.as_scaled(scale);
            let cell_width = scaled_font.h_advance(font.glyph_id('A')).ceil().max(1.0) as u32;

            let adv_a = scaled_font.h_advance(font.glyph_id('A'));
            let adv_1 = scaled_font.h_advance(font.glyph_id('1'));
            let adv_i = scaled_font.h_advance(font.glyph_id('i'));
            let adv_dot = scaled_font.h_advance(font.glyph_id('.'));

            let pad_a = ((cell_width as f32 - adv_a) / 2.0).max(0.0);
            let pad_1 = ((cell_width as f32 - adv_1) / 2.0).max(0.0);
            let pad_i = ((cell_width as f32 - adv_i) / 2.0).max(0.0);
            let pad_dot = ((cell_width as f32 - adv_dot) / 2.0).max(0.0);

            // In a monospace font, advances and padding must be identical across standard glyphs
            assert!(
                (pad_a - pad_1).abs() < 0.01,
                "Padding for '1' must match 'A'"
            );
            assert!(
                (pad_a - pad_i).abs() < 0.01,
                "Padding for 'i' must match 'A'"
            );
            assert!(
                (pad_a - pad_dot).abs() < 0.01,
                "Padding for '.' must match 'A'"
            );
        }
    }
}
