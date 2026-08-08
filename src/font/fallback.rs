use std::collections::HashSet;
use std::path::PathBuf;
use fontdb::Database;
use ab_glyph::{Font, FontArc};

pub struct FallbackFont {
    pub font: FontArc,
    pub owned_face: Option<owned_ttf_parser::OwnedFace>,
}

pub struct FallbackManager {
    db: Database,
    loaded_paths: HashSet<PathBuf>,
    pub fallbacks: Vec<FallbackFont>,
    missing_chars: HashSet<char>,
}

impl Default for FallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FallbackManager {
    pub fn new() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();
        Self {
            db,
            loaded_paths: HashSet::new(),
            fallbacks: Vec::new(),
            missing_chars: HashSet::new(),
        }
    }

    pub fn find_fallback_for_char(&mut self, c: char, enable_nerdfont: bool) -> Option<usize> {
        // 1. Check existing loaded fallback fonts first
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            if fallback.font.glyph_id(c).0 != 0 {
                return Some(idx);
            }
        }

        // 2. Check if char is known to be missing across all system fonts to prevent redundant disk I/O
        if self.missing_chars.contains(&c) {
            return None;
        }

        // 3. Check popular Nerd Font & Symbol families directly
        let is_symbol_or_pua = enable_nerdfont
            || ('\u{e000}'..='\u{f8ff}').contains(&c)
            || ('\u{f0000}'..='\u{ffffd}').contains(&c)
            || ('\u{2300}'..='\u{2bff}').contains(&c);

        if is_symbol_or_pua {
            let nerd_families = [
                "Symbols Nerd Font",
                "Symbols Nerd Font Mono",
                "MesloLGS NF",
                "JetBrainsMono Nerd Font",
                "Hack Nerd Font",
                "FiraCode Nerd Font",
                "DejaVu Sans",
                "Noto Sans Symbols",
            ];

            for family in &nerd_families {
                let query = fontdb::Query {
                    families: &[fontdb::Family::Name(family)],
                    weight: fontdb::Weight::NORMAL,
                    stretch: fontdb::Stretch::Normal,
                    style: fontdb::Style::Normal,
                };
                if let Some(id) = self.db.query(&query)
                    && let Some(face) = self.db.face(id)
                        && let fontdb::Source::File(path) = &face.source
                            && !self.loaded_paths.contains(path)
                                && let Ok(data) = std::fs::read(path)
                                    && let Ok(font) = FontArc::try_from_vec(data)
                                        && font.glyph_id(c).0 != 0 {
                                            self.loaded_paths.insert(path.clone());
                                            self.fallbacks.push(FallbackFont { font, owned_face: None });
                                            return Some(self.fallbacks.len() - 1);
                                        }
            }
        }

        // 4. Scan system font faces in fontdb
        for face in self.db.faces() {
            if let fontdb::Source::File(path) = &face.source
                && !self.loaded_paths.contains(path)
                    && let Ok(data) = std::fs::read(path)
                        && let Ok(font) = FontArc::try_from_vec(data.clone())
                            && font.glyph_id(c).0 != 0 {
                                self.loaded_paths.insert(path.clone());
                                let path_str = path.to_string_lossy().to_lowercase();
                                let is_emoji = path_str.contains("emoji");
                                let owned_face = if is_emoji {
                                    owned_ttf_parser::OwnedFace::from_vec(data, 0).ok()
                                } else {
                                    None
                                };
                                self.fallbacks.push(FallbackFont { font, owned_face });
                                return Some(self.fallbacks.len() - 1);
                            }
        }

        // Mark as missing to optimize future lookups
        self.missing_chars.insert(c);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_manager_initialization() {
        let mut manager = FallbackManager::new();
        let _ = manager.find_fallback_for_char('A', true);
        let _ = manager.find_fallback_for_char('\u{1f600}', true); // 😀
        let _ = manager.find_fallback_for_char('\u{e0b0}', true); // 
    }
}
