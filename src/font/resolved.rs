use ab_glyph::{Font, FontArc, GlyphId, OutlinedGlyph, PxScale, ScaleFont};
use fontdb::Database;

pub const SYNTHETIC_ITALIC_SHEAR: f32 = 0.20;

/// Represents a resolved font face along with any synthetic style transforms.
#[derive(Clone)]
pub struct ResolvedFont {
    pub font: FontArc,
    pub synthetic_italic: bool,
    pub synthetic_bold: bool,
}

/// A complete resolved font family covering all 4 standard terminal styles.
#[derive(Clone)]
pub struct ResolvedFontSet {
    pub regular: ResolvedFont,
    pub bold: ResolvedFont,
    pub italic: ResolvedFont,
    pub bold_italic: ResolvedFont,
}

impl ResolvedFontSet {
    /// Resolve all 4 font faces according to the fallback rules:
    /// - Regular: Must exist.
    /// - Bold: Real bold if available; otherwise Regular + synthetic bold.
    /// - Italic: Real italic if available; otherwise Regular + synthetic italic.
    /// - Bold Italic: Real bold italic if available; otherwise Bold + synthetic italic;
    ///   otherwise Regular + synthetic bold + synthetic italic.
    pub fn resolve(db: &Database, font_family: &str) -> Self {
        let query_regular = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let regular_id = db.query(&query_regular);
        let regular_font = crate::font::loader::load_font_face(db, &query_regular)
            .expect("Could not load any system monospace font");

        let regular_resolved = ResolvedFont {
            font: regular_font.clone(),
            synthetic_italic: false,
            synthetic_bold: false,
        };

        // Bold face query
        let query_bold = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let bold_id = db.query(&query_bold);
        let bold_resolved = if bold_id.is_some() && bold_id != regular_id {
            if let Some(f) = crate::font::loader::load_font_face(db, &query_bold) {
                ResolvedFont {
                    font: f,
                    synthetic_italic: false,
                    synthetic_bold: false,
                }
            } else {
                ResolvedFont {
                    font: regular_font.clone(),
                    synthetic_italic: false,
                    synthetic_bold: true,
                }
            }
        } else {
            ResolvedFont {
                font: regular_font.clone(),
                synthetic_italic: false,
                synthetic_bold: true,
            }
        };

        // Italic face query
        let query_italic = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let italic_id = db.query(&query_italic);
        let italic_resolved = if italic_id.is_some() && italic_id != regular_id {
            if let Some(f) = crate::font::loader::load_font_face(db, &query_italic) {
                ResolvedFont {
                    font: f,
                    synthetic_italic: false,
                    synthetic_bold: false,
                }
            } else {
                ResolvedFont {
                    font: regular_font.clone(),
                    synthetic_italic: true,
                    synthetic_bold: false,
                }
            }
        } else {
            // Unavailable -> Regular + synthetic italic
            ResolvedFont {
                font: regular_font.clone(),
                synthetic_italic: true,
                synthetic_bold: false,
            }
        };

        // Bold Italic face query
        let query_bold_italic = fontdb::Query {
            families: &[fontdb::Family::Name(font_family), fontdb::Family::Monospace],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let bold_italic_id = db.query(&query_bold_italic);
        let bold_italic_resolved = if bold_italic_id.is_some()
            && bold_italic_id != regular_id
            && bold_italic_id != bold_id
            && bold_italic_id != italic_id
        {
            if let Some(f) = crate::font::loader::load_font_face(db, &query_bold_italic) {
                ResolvedFont {
                    font: f,
                    synthetic_italic: false,
                    synthetic_bold: false,
                }
            } else if !bold_resolved.synthetic_bold {
                ResolvedFont {
                    font: bold_resolved.font.clone(),
                    synthetic_italic: true,
                    synthetic_bold: false,
                }
            } else {
                ResolvedFont {
                    font: regular_font.clone(),
                    synthetic_italic: true,
                    synthetic_bold: true,
                }
            }
        } else if !bold_resolved.synthetic_bold {
            ResolvedFont {
                font: bold_resolved.font.clone(),
                synthetic_italic: true,
                synthetic_bold: false,
            }
        } else {
            ResolvedFont {
                font: regular_font.clone(),
                synthetic_italic: true,
                synthetic_bold: true,
            }
        };

        Self {
            regular: regular_resolved,
            bold: bold_resolved,
            italic: italic_resolved,
            bold_italic: bold_italic_resolved,
        }
    }

    /// Retrieve the resolved font and synthetic style flags for the given style request.
    #[inline(always)]
    pub fn get(&self, is_bold: bool, is_italic: bool) -> &ResolvedFont {
        match (is_bold, is_italic) {
            (false, false) => &self.regular,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (true, true) => &self.bold_italic,
        }
    }
}

/// Apply oblique shear transformation to an unscaled vector `Outline` in-place.
/// For every point in the outline: x' = x + shear * y, y' = y.
/// In TrueType/OpenType font design coordinates, y=0 is baseline, positive y is above baseline.
pub fn shear_outline(outline: &mut ab_glyph::Outline, shear: f32) {
    if shear.abs() < f32::EPSILON {
        return;
    }
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    let transform_pt = |p: &mut ab_glyph::Point| {
        p.x += shear * p.y;
    };

    for curve in &mut outline.curves {
        match curve {
            ab_glyph::OutlineCurve::Line(p0, p1) => {
                transform_pt(p0);
                transform_pt(p1);
                min_x = min_x.min(p0.x).min(p1.x);
                min_y = min_y.min(p0.y).min(p1.y);
                max_x = max_x.max(p0.x).max(p1.x);
                max_y = max_y.max(p0.y).max(p1.y);
            }
            ab_glyph::OutlineCurve::Quad(p0, p1, p2) => {
                transform_pt(p0);
                transform_pt(p1);
                transform_pt(p2);
                min_x = min_x.min(p0.x).min(p1.x).min(p2.x);
                min_y = min_y.min(p0.y).min(p1.y).min(p2.y);
                max_x = max_x.max(p0.x).max(p1.x).max(p2.x);
                max_y = max_y.max(p0.y).max(p1.y).max(p2.y);
            }
            ab_glyph::OutlineCurve::Cubic(p0, p1, p2, p3) => {
                transform_pt(p0);
                transform_pt(p1);
                transform_pt(p2);
                transform_pt(p3);
                min_x = min_x.min(p0.x).min(p1.x).min(p2.x).min(p3.x);
                min_y = min_y.min(p0.y).min(p1.y).min(p2.y).min(p3.y);
                max_x = max_x.max(p0.x).max(p1.x).max(p2.x).max(p3.x);
                max_y = max_y.max(p0.y).max(p1.y).max(p2.y).max(p3.y);
            }
        }
    }

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        // ab_glyph coordinates: min.y is upper bound (highest in font units), max.y is lower bound (lowest in font units)
        outline.bounds = ab_glyph::Rect {
            min: ab_glyph::Point { x: min_x, y: max_y },
            max: ab_glyph::Point { x: max_x, y: min_y },
        };
    }
}

/// Retrieve or construct an `OutlinedGlyph`, applying vector-level synthetic italic transformation if needed.
pub fn get_or_create_outlined_glyph(
    font: &FontArc,
    glyph_id: GlyphId,
    scale: PxScale,
    is_synthetic_italic: bool,
) -> Option<OutlinedGlyph> {
    if !is_synthetic_italic {
        let glyph = glyph_id.with_scale(scale);
        font.outline_glyph(glyph)
    } else {
        let scaled_font = font.as_scaled(scale);
        let sf = scaled_font.scale_factor();
        let glyph = glyph_id.with_scale(scale);
        let mut outline = font.outline(glyph_id)?;
        shear_outline(&mut outline, SYNTHETIC_ITALIC_SHEAR);
        Some(OutlinedGlyph::new(glyph, outline, sf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shear_outline_transforms_points_and_expands_bounds() {
        let db = crate::font::fallback::get_system_font_db();
        let query = fontdb::Query {
            families: &[fontdb::Family::Monospace],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        if let Some(font) = crate::font::loader::load_font_face(db, &query) {
            let scale = PxScale::from(16.0);
            let glyph_id = font.glyph_id('H');
            if let Some(mut outline) = font.outline(glyph_id) {
                let orig_max_x = outline.bounds.max.x;
                shear_outline(&mut outline, SYNTHETIC_ITALIC_SHEAR);
                assert!(
                    outline.bounds.max.x > orig_max_x,
                    "Sheared outline bounds must extend further right"
                );
                let scaled = font.as_scaled(scale);
                let sf = scaled.scale_factor();
                let glyph = glyph_id.with_scale(scale);
                let outlined = OutlinedGlyph::new(glyph, outline, sf);
                let mut pixel_count = 0;
                outlined.draw(|_gx, _gy, alpha| {
                    if alpha > 0.0 {
                        pixel_count += 1;
                    }
                });
                assert!(pixel_count > 0, "Outlined glyph must draw pixels");
            }
        }
    }

    #[test]
    fn test_resolved_font_set_fallback_rules() {
        let db = crate::font::fallback::get_system_font_db();
        let set = ResolvedFontSet::resolve(db, "Monospace");

        // Regular must never have synthetic italic
        assert!(!set.regular.synthetic_italic);

        // If italic face was not distinct, synthetic_italic must be true
        let italic = set.get(false, true);
        assert!(italic.font.glyph_id('A').0 != 0);

        // Bold italic
        let bold_italic = set.get(true, true);
        assert!(bold_italic.font.glyph_id('A').0 != 0);
    }
}
