use ab_glyph::{Font, FontArc};
use fontdb::Database;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub const MAX_FALLBACK_FONTS: usize = 8;
pub const MAX_MISSING_CHARS: usize = 1024;

pub struct FallbackFont {
    pub font: FontArc,
    pub raw_data: Option<Arc<[u8]>>,
    pub last_used: u64,
    pub path: PathBuf,
}

pub struct FallbackManager {
    db: Database,
    db_loaded: bool,
    loaded_paths: HashSet<PathBuf>,
    pub fallbacks: Vec<FallbackFont>,
    missing_chars: HashSet<char>,
    usage_counter: u64,
}

impl Default for FallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FallbackManager {
    pub fn new() -> Self {
        Self {
            db: Database::new(),
            db_loaded: false,
            loaded_paths: HashSet::new(),
            fallbacks: Vec::new(),
            missing_chars: HashSet::new(),
            usage_counter: 0,
        }
    }

    pub fn with_database(db: Database) -> Self {
        Self {
            db,
            db_loaded: true,
            loaded_paths: HashSet::new(),
            fallbacks: Vec::new(),
            missing_chars: HashSet::new(),
            usage_counter: 0,
        }
    }

    fn ensure_db_loaded(&mut self) {
        if !self.db_loaded {
            self.db.load_system_fonts();
            self.db_loaded = true;
        }
    }

    fn insert_fallback(
        &mut self,
        path: PathBuf,
        font: FontArc,
        raw_data: Option<Arc<[u8]>>,
    ) -> usize {
        self.usage_counter = self.usage_counter.wrapping_add(1);
        if self.fallbacks.len() >= MAX_FALLBACK_FONTS {
            // Evict the least recently used fallback font to bound RAM
            let lru_idx = self
                .fallbacks
                .iter()
                .enumerate()
                .min_by_key(|(_, f)| f.last_used)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let evicted = self.fallbacks.remove(lru_idx);
            self.loaded_paths.remove(&evicted.path);
        }

        self.loaded_paths.insert(path.clone());
        self.fallbacks.push(FallbackFont {
            font,
            raw_data,
            last_used: self.usage_counter,
            path,
        });
        self.fallbacks.len() - 1
    }

    pub fn find_fallback_for_char(&mut self, c: char) -> Option<usize> {
        // 1. Check existing loaded fallback fonts first and update LRU timestamp
        for (idx, fallback) in self.fallbacks.iter_mut().enumerate() {
            if fallback.font.glyph_id(c).0 != 0 {
                self.usage_counter = self.usage_counter.wrapping_add(1);
                fallback.last_used = self.usage_counter;
                return Some(idx);
            }
        }

        // 2. Check if char is known to be missing across all system fonts
        if self.missing_chars.contains(&c) {
            return None;
        }

        self.ensure_db_loaded();

        // 3. Check popular Nerd Font & Symbol families directly
        let is_symbol_or_pua = ('\u{e000}'..='\u{f8ff}').contains(&c)
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
                    && font.glyph_id(c).0 != 0
                {
                    let idx = self.insert_fallback(path.clone(), font, None);
                    return Some(idx);
                }
            }
        }

        // 4. Scan system font faces in fontdb
        let candidate_paths: Vec<(PathBuf, bool)> = self
            .db
            .faces()
            .filter_map(|face| {
                if let fontdb::Source::File(path) = &face.source
                    && !self.loaded_paths.contains(path)
                {
                    let path_str = path.to_string_lossy().to_lowercase();
                    let is_emoji = path_str.contains("emoji");
                    Some((path.clone(), is_emoji))
                } else {
                    None
                }
            })
            .collect();

        for (path, is_emoji) in candidate_paths {
            if !self.loaded_paths.contains(&path)
                && let Ok(data) = std::fs::read(&path)
            {
                let raw_data = if is_emoji {
                    Some(Arc::from(data.as_slice()))
                } else {
                    None
                };
                if let Ok(font) = FontArc::try_from_vec(data)
                    && font.glyph_id(c).0 != 0
                {
                    let idx = self.insert_fallback(path, font, raw_data);
                    return Some(idx);
                }
            }
        }

        // Mark as missing to optimize future lookups (bounded capacity)
        if self.missing_chars.len() >= MAX_MISSING_CHARS {
            self.missing_chars.clear();
        }
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
        let _ = manager.find_fallback_for_char('A');
        let _ = manager.find_fallback_for_char('\u{1f600}'); // 😀
        let _ = manager.find_fallback_for_char('\u{e0b0}'); // 
        assert!(manager.fallbacks.len() <= MAX_FALLBACK_FONTS);
    }
}
