use ab_glyph::{Font, FontArc, GlyphId, OutlinedGlyph, PxScale, ScaleFont};
use fontdb::Database;

pub const SYNTHETIC_ITALIC_SHEAR: f32 = 0.20;

/// Error encountered when attempting to resolve font families.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    NoFontsInstalled,
    FontNotFound {
        requested: String,
    },
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFontsInstalled => write!(
                f,
                "No fonts are installed on the system. Please install a monospace font (such as DejaVu Sans Mono, Liberation Mono, or Noto Sans Mono) to run Velox."
            ),
            Self::FontNotFound { requested } => write!(
                f,
                "Configured font family '{}' was not found and no suitable fallback font could be loaded. Please verify your font configuration or install a monospace font.",
                requested
            ),
        }
    }
}

impl std::error::Error for FontError {}

/// Standard monospace font families queried in order of preference during fallback.
pub const STANDARD_MONOSPACE_FAMILIES: &[&str] = &[
    "monospace",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "Ubuntu Mono",
    "Cascadia Code",
    "Cascadia Mono",
    "Courier New",
    "Courier",
    "Consolas",
    "Menlo",
    "Monaco",
    "SF Mono",
    "Fira Code",
    "Fira Mono",
    "Inconsolata",
    "Source Code Pro",
    "JetBrains Mono",
    "Hack",
    "MesloLGS NF",
    "ComicShannsMono Nerd Font",
    "Droid Sans Mono",
    "FreeMono",
];

/// Resolve the primary regular font face from the database, falling back through known monospace
/// families, generic monospace queries, and any available system font face.
pub fn resolve_regular_font(
    db: &Database,
    requested_family: &str,
) -> Result<(FontArc, fontdb::ID, String), FontError> {
    if db.faces().count() == 0 {
        return Err(FontError::NoFontsInstalled);
    }

    // 1. Try requested font family (exact name match)
    let query_requested = fontdb::Query {
        families: &[fontdb::Family::Name(requested_family)],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    if let Some(id) = db.query(&query_requested)
        && let Some(font) = crate::font::loader::load_font_face_by_id(db, id)
    {
        let family_name = db
            .face(id)
            .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
            .unwrap_or_else(|| requested_family.to_string());
        return Ok((font, id, family_name));
    }

    // 2. Try standard monospace families
    for &family in STANDARD_MONOSPACE_FAMILIES {
        if family.eq_ignore_ascii_case(requested_family) {
            continue;
        }
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        if let Some(id) = db.query(&query)
            && let Some(font) = crate::font::loader::load_font_face_by_id(db, id)
        {
            let family_name = db
                .face(id)
                .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
                .unwrap_or_else(|| family.to_string());
            log::warn!(
                "Configured font family '{}' not found. Falling back to system font '{}'.",
                requested_family,
                family_name
            );
            return Ok((font, id, family_name));
        }
    }

    // 3. Try generic fontdb Monospace query
    let query_mono = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    if let Some(id) = db.query(&query_mono)
        && let Some(font) = crate::font::loader::load_font_face_by_id(db, id)
    {
        let family_name = db
            .face(id)
            .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
            .unwrap_or_else(|| "monospace".to_string());
        log::warn!(
            "Configured font family '{}' not found. Falling back to generic monospace font '{}'.",
            requested_family,
            family_name
        );
        return Ok((font, id, family_name));
    }

    // 4. Scan for any face marked monospaced with Normal style
    for face in db.faces() {
        if face.monospaced
            && face.style == fontdb::Style::Normal
            && let Some(font) = crate::font::loader::load_font_face_by_id(db, face.id)
        {
            let family_name = face
                .families
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| "monospace".to_string());
            log::warn!(
                "Configured font family '{}' not found. Falling back to monospaced face '{}'.",
                requested_family,
                family_name
            );
            return Ok((font, face.id, family_name));
        }
    }

    // 5. Scan for any face marked monospaced (any style)
    for face in db.faces() {
        if face.monospaced
            && let Some(font) = crate::font::loader::load_font_face_by_id(db, face.id)
        {
            let family_name = face
                .families
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| "monospace".to_string());
            log::warn!(
                "Configured font family '{}' not found. Falling back to monospaced face '{}'.",
                requested_family,
                family_name
            );
            return Ok((font, face.id, family_name));
        }
    }

    // 6. Scan for any face with Normal style
    for face in db.faces() {
        if face.style == fontdb::Style::Normal
            && let Some(font) = crate::font::loader::load_font_face_by_id(db, face.id)
        {
            let family_name = face
                .families
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| "system".to_string());
            log::warn!(
                "Configured font family '{}' and monospace fonts not found. Falling back to face '{}'.",
                requested_family,
                family_name
            );
            return Ok((font, face.id, family_name));
        }
    }

    // 7. Scan for ANY loadable face in db
    for face in db.faces() {
        if let Some(font) = crate::font::loader::load_font_face_by_id(db, face.id) {
            let family_name = face
                .families
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| "system".to_string());
            log::warn!(
                "Configured font family '{}' not found. Falling back to face '{}'.",
                requested_family,
                family_name
            );
            return Ok((font, face.id, family_name));
        }
    }

    Err(FontError::FontNotFound {
        requested: requested_family.to_string(),
    })
}

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
    /// Resolve all 4 font faces according to the fallback rules, returning a structured error if
    /// no font can be found or loaded.
    pub fn try_resolve(db: &Database, font_family: &str) -> Result<Self, FontError> {
        let (regular_font, regular_id, resolved_family) = resolve_regular_font(db, font_family)?;

        let regular_resolved = ResolvedFont {
            font: regular_font.clone(),
            synthetic_italic: false,
            synthetic_bold: false,
        };

        // Bold face query
        let query_bold = fontdb::Query {
            families: &[
                fontdb::Family::Name(&resolved_family),
                fontdb::Family::Name(font_family),
                fontdb::Family::Monospace,
            ],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let bold_id = db.query(&query_bold);
        let bold_resolved = if bold_id.is_some() && bold_id != Some(regular_id) {
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
            families: &[
                fontdb::Family::Name(&resolved_family),
                fontdb::Family::Name(font_family),
                fontdb::Family::Monospace,
            ],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let italic_id = db.query(&query_italic);
        let italic_resolved = if italic_id.is_some() && italic_id != Some(regular_id) {
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
            families: &[
                fontdb::Family::Name(&resolved_family),
                fontdb::Family::Name(font_family),
                fontdb::Family::Monospace,
            ],
            weight: fontdb::Weight::BOLD,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Italic,
        };
        let bold_italic_id = db.query(&query_bold_italic);
        let bold_italic_resolved = if bold_italic_id.is_some()
            && bold_italic_id != Some(regular_id)
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
                    font: regular_font,
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
                font: regular_font,
                synthetic_italic: true,
                synthetic_bold: true,
            }
        };

        Ok(Self {
            regular: regular_resolved,
            bold: bold_resolved,
            italic: italic_resolved,
            bold_italic: bold_italic_resolved,
        })
    }

    /// Resolve all 4 font faces according to the fallback rules:
    /// - Regular: Must exist (panics if no font found).
    /// - Bold: Real bold if available; otherwise Regular + synthetic bold.
    /// - Italic: Real italic if available; otherwise Regular + synthetic italic.
    /// - Bold Italic: Real bold italic if available; otherwise Bold + synthetic italic;
    ///   otherwise Regular + synthetic bold + synthetic italic.
    pub fn resolve(db: &Database, font_family: &str) -> Self {
        Self::try_resolve(db, font_family).unwrap_or_else(|e| panic!("{}", e))
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

    #[test]
    fn test_resolve_fallback_on_missing_family() {
        let db = crate::font::fallback::get_system_font_db();
        // Request a font family that does not exist on any system
        let result = ResolvedFontSet::try_resolve(db, "DefinitelyNonExistentFont_Velox_Test_12345");
        assert!(
            result.is_ok(),
            "Font resolution must fall back to an available system font rather than failing: {:?}",
            result.err()
        );
        let set = result.unwrap();
        assert_ne!(
            set.regular.font.glyph_id('A').0, 0,
            "Fallback font must have glyphs"
        );
    }

    #[test]
    fn test_resolve_fails_gracefully_when_no_fonts_installed() {
        // An empty database simulates an environment with zero fonts installed
        let empty_db = Database::new();
        let result = ResolvedFontSet::try_resolve(&empty_db, "monospace");
        assert_eq!(
            result.err(),
            Some(FontError::NoFontsInstalled),
            "Must return FontError::NoFontsInstalled cleanly when database has no fonts"
        );
    }

    #[test]
    fn test_font_error_display_formatting() {
        let err_no_fonts = FontError::NoFontsInstalled;
        assert!(
            err_no_fonts.to_string().contains("No fonts are installed on the system"),
            "Display message must explain no fonts installed: {}",
            err_no_fonts
        );

        let err_not_found = FontError::FontNotFound {
            requested: "CustomFont".to_string(),
        };
        assert!(
            err_not_found.to_string().contains("CustomFont"),
            "Display message must mention the requested font: {}",
            err_not_found
        );
    }
}
