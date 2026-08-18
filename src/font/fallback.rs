use crate::font::storage::{FontStorage, create_font_arc};
use ab_glyph::{Font, FontArc};
use fontdb::Database;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub const MAX_FALLBACK_FONTS: usize = 8;
pub const MAX_FALLBACK_BYTES: usize = 64 * 1024 * 1024; // 64 MB virtual/resident fallback budget
pub const MAX_MISSING_CHARS: usize = 1024;

pub struct FallbackFont {
    pub font: FontArc,
    pub storage: Arc<FontStorage>,
    pub byte_size: usize,
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
    pub resident_bytes: usize,
    pub max_fallback_fonts: usize,
    pub max_fallback_bytes: usize,
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
            resident_bytes: 0,
            max_fallback_fonts: MAX_FALLBACK_FONTS,
            max_fallback_bytes: MAX_FALLBACK_BYTES,
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
            resident_bytes: 0,
            max_fallback_fonts: MAX_FALLBACK_FONTS,
            max_fallback_bytes: MAX_FALLBACK_BYTES,
        }
    }

    fn ensure_db_loaded(&mut self) {
        if !self.db_loaded {
            self.db.load_system_fonts();
            self.db_loaded = true;
        }
    }

    /// Prune cached fallback fonts so total resident bytes and count stay within budget limits.
    pub fn prune_to_budget(&mut self) {
        while self.fallbacks.len() > self.max_fallback_fonts
            || (self.resident_bytes > self.max_fallback_bytes && self.fallbacks.len() > 1)
        {
            let lru_idx = self
                .fallbacks
                .iter()
                .enumerate()
                .min_by_key(|(_, f)| f.last_used)
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            let evicted = self.fallbacks.remove(lru_idx);
            self.resident_bytes = self.resident_bytes.saturating_sub(evicted.byte_size);
            self.loaded_paths.remove(&evicted.path);
        }
    }

    /// Explicitly prune inactive fallback fonts down to a small target count (e.g. on idle cleanup).
    pub fn prune_unused(&mut self, keep_count: usize) {
        let keep = keep_count.min(self.max_fallback_fonts);
        while self.fallbacks.len() > keep {
            let lru_idx = self
                .fallbacks
                .iter()
                .enumerate()
                .min_by_key(|(_, f)| f.last_used)
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            let evicted = self.fallbacks.remove(lru_idx);
            self.resident_bytes = self.resident_bytes.saturating_sub(evicted.byte_size);
            self.loaded_paths.remove(&evicted.path);
        }
    }

    fn insert_fallback(
        &mut self,
        path: PathBuf,
        font: FontArc,
        storage: Arc<FontStorage>,
    ) -> usize {
        self.usage_counter = self.usage_counter.wrapping_add(1);
        let byte_size = storage.len();
        self.resident_bytes += byte_size;

        self.loaded_paths.insert(path.clone());
        self.fallbacks.push(FallbackFont {
            font,
            storage,
            byte_size,
            last_used: self.usage_counter,
            path,
        });

        // Enforce bounds and eviction
        self.prune_to_budget();

        self.fallbacks.len().saturating_sub(1)
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
                    && let Ok(storage) = FontStorage::from_file(path).map(Arc::new)
                    && let Ok(font) = create_font_arc(Arc::clone(&storage), face.index)
                    && font.glyph_id(c).0 != 0
                {
                    let idx = self.insert_fallback(path.clone(), font, storage);
                    return Some(idx);
                }
            }
        }

        // 4. Scan system font faces in fontdb with mmap
        let candidate_paths: Vec<(PathBuf, u32)> = self
            .db
            .faces()
            .filter_map(|face| {
                if let fontdb::Source::File(path) = &face.source
                    && !self.loaded_paths.contains(path)
                {
                    Some((path.clone(), face.index))
                } else {
                    None
                }
            })
            .collect();

        for (path, face_index) in candidate_paths {
            if !self.loaded_paths.contains(&path)
                && let Ok(storage) = FontStorage::from_file(&path).map(Arc::new)
                && let Ok(font) = create_font_arc(Arc::clone(&storage), face_index)
                && font.glyph_id(c).0 != 0
            {
                let idx = self.insert_fallback(path, font, storage);
                return Some(idx);
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

    #[test]
    fn test_fallback_manager_prune_to_budget() {
        let mut manager = FallbackManager::new();
        manager.max_fallback_fonts = 2;
        let _ = manager.find_fallback_for_char('\u{1f600}');
        let _ = manager.find_fallback_for_char('\u{e0b0}');
        let _ = manager.find_fallback_for_char('中');
        manager.prune_to_budget();
        assert!(manager.fallbacks.len() <= 2);
    }
}
