use crate::font::storage::{FontStorage, create_font_arc};
use ab_glyph::{Font, FontArc};
use fontdb::Database;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

pub const MAX_FALLBACK_FONTS: usize = 8;
pub const MAX_FALLBACK_BYTES: usize = 64 * 1024 * 1024; // 64 MB virtual/resident fallback budget
pub const MAX_MISSING_CHARS: usize = 1024;

static SYSTEM_FONT_DB: OnceLock<Arc<Database>> = OnceLock::new();

/// Returns the shared, lazily-initialized system font database.
/// Avoids loading and parsing font directories multiple times across loaders and fallback managers.
pub fn get_system_font_db() -> &'static Arc<Database> {
    SYSTEM_FONT_DB.get_or_init(|| {
        let mut db = Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
}

pub struct FallbackFont {
    pub font: FontArc,
    pub storage: Arc<FontStorage>,
    pub byte_size: usize,
    pub last_used: u64,
    pub path: PathBuf,
}

pub struct FallbackManager {
    db: Option<Arc<Database>>,
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
            db: None,
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
            db: Some(Arc::new(db)),
            loaded_paths: HashSet::new(),
            fallbacks: Vec::new(),
            missing_chars: HashSet::new(),
            usage_counter: 0,
            resident_bytes: 0,
            max_fallback_fonts: MAX_FALLBACK_FONTS,
            max_fallback_bytes: MAX_FALLBACK_BYTES,
        }
    }

    pub fn with_shared_database(db: Arc<Database>) -> Self {
        Self {
            db: Some(db),
            loaded_paths: HashSet::new(),
            fallbacks: Vec::new(),
            missing_chars: HashSet::new(),
            usage_counter: 0,
            resident_bytes: 0,
            max_fallback_fonts: MAX_FALLBACK_FONTS,
            max_fallback_bytes: MAX_FALLBACK_BYTES,
        }
    }

    #[inline]
    fn ensure_db(&mut self) -> Arc<Database> {
        Arc::clone(
            self.db
                .get_or_insert_with(|| Arc::clone(get_system_font_db())),
        )
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
        let emoji = crate::font::loader::is_emoji(c);

        // 1. Check existing loaded fallback fonts first and update LRU timestamp
        // If it is an emoji, prefer loaded fallbacks that have a color image for `c`
        if emoji {
            for (idx, fallback) in self.fallbacks.iter_mut().enumerate() {
                let id = fallback.font.glyph_id(c);
                if id.0 != 0
                    && let Ok(face) = owned_ttf_parser::Face::parse(fallback.storage.as_bytes(), 0)
                    && face
                        .glyph_raster_image(owned_ttf_parser::GlyphId(id.0), 32)
                        .is_some_and(|img| img.format == owned_ttf_parser::RasterImageFormat::PNG)
                {
                    self.usage_counter = self.usage_counter.wrapping_add(1);
                    fallback.last_used = self.usage_counter;
                    return Some(idx);
                }
            }
        }

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

        let db = self.ensure_db();

        // 3. If emoji, check popular color emoji families FIRST
        if emoji {
            let emoji_families = [
                "Noto Color Emoji",
                "Apple Color Emoji",
                "Segoe UI Emoji",
                "Twemoji Mozilla",
                "Twitter Color Emoji",
                "EmojiOne Color",
                "JoyPixels",
                "OpenMoji Color",
                "Noto Emoji",
            ];

            for family in &emoji_families {
                let query = fontdb::Query {
                    families: &[fontdb::Family::Name(family)],
                    weight: fontdb::Weight::NORMAL,
                    stretch: fontdb::Stretch::Normal,
                    style: fontdb::Style::Normal,
                };
                if let Some(id) = db.query(&query)
                    && let Some(face) = db.face(id)
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

        // 4. Check popular Nerd Font & Symbol families directly
        let is_symbol_or_pua = !emoji
            && (('\u{e000}'..='\u{f8ff}').contains(&c)
                || ('\u{f0000}'..='\u{ffffd}').contains(&c)
                || ('\u{2300}'..='\u{2bff}').contains(&c));

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
                if let Some(id) = db.query(&query)
                    && let Some(face) = db.face(id)
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

        // 5. Scan system font faces directly via iterator (zero Vec/PathBuf heap allocations)
        if emoji {
            for face in db.faces() {
                if let fontdb::Source::File(path) = &face.source
                    && !self.loaded_paths.contains(path)
                    && let Ok(storage) = FontStorage::from_file(path).map(Arc::new)
                    && let Ok(font) = create_font_arc(Arc::clone(&storage), face.index)
                    && font.glyph_id(c).0 != 0
                    && let Ok(face_parsed) =
                        owned_ttf_parser::Face::parse(storage.as_bytes(), face.index)
                    && face_parsed
                        .glyph_raster_image(owned_ttf_parser::GlyphId(font.glyph_id(c).0), 32)
                        .is_some_and(|img| img.format == owned_ttf_parser::RasterImageFormat::PNG)
                {
                    let idx = self.insert_fallback(path.clone(), font, storage);
                    return Some(idx);
                }
            }
        }

        for face in db.faces() {
            if let fontdb::Source::File(path) = &face.source
                && !self.loaded_paths.contains(path)
                && let Ok(storage) = FontStorage::from_file(path).map(Arc::new)
                && let Ok(font) = create_font_arc(Arc::clone(&storage), face.index)
                && font.glyph_id(c).0 != 0
            {
                let idx = self.insert_fallback(path.clone(), font, storage);
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

    #[test]
    fn test_emoji_vs_nerd_font_classification() {
        use crate::font::loader::{is_emoji, is_nerd_font_or_pua};

        // Unicode emoji codepoints
        assert!(is_emoji('\u{1f4e6}')); // 📦 Package
        assert!(is_emoji('\u{1f980}')); // 🦀 Crab
        assert!(is_emoji('\u{1f600}')); // 😀 Grinning Face
        assert!(is_emoji('\u{1f680}')); // 🚀 Rocket
        assert!(is_emoji('\u{2728}')); // ✨ Sparkles
        assert!(is_emoji('\u{26a0}')); // ⚠️ Warning

        // Emojis must NOT be classified as Nerd Font PUA
        assert!(!is_nerd_font_or_pua('\u{1f4e6}'));
        assert!(!is_nerd_font_or_pua('\u{1f980}'));
        assert!(!is_nerd_font_or_pua('\u{1f600}'));
        assert!(!is_nerd_font_or_pua('\u{1f680}'));

        // Nerd Font PUA codepoints
        assert!(is_nerd_font_or_pua('\u{e0b0}')); //  Powerline arrow
        assert!(is_nerd_font_or_pua('\u{e702}')); //  Git icon
        assert!(is_nerd_font_or_pua('\u{f07b}')); //  Folder icon
        assert!(is_nerd_font_or_pua('\u{f113}')); //  Github octicon
        assert!(is_nerd_font_or_pua('\u{f0001}')); // PUA-A

        // Nerd Fonts must NOT be classified as Emoji
        assert!(!is_emoji('\u{e0b0}'));
        assert!(!is_emoji('\u{e702}'));
        assert!(!is_emoji('\u{f07b}'));
        assert!(!is_emoji('\u{f113}'));
    }

    #[test]
    fn test_emoji_fallback_finds_color_font_for_package_and_crab() {
        let mut manager = FallbackManager::new();
        // If system has color emoji (e.g. Noto Color Emoji), both 📦 and 🦀 resolve successfully
        if let Some(idx_package) = manager.find_fallback_for_char('\u{1f4e6}') {
            let fallback = &manager.fallbacks[idx_package];
            let id = fallback.font.glyph_id('\u{1f4e6}');
            assert_ne!(id.0, 0);
        }
        if let Some(idx_crab) = manager.find_fallback_for_char('\u{1f980}') {
            let fallback = &manager.fallbacks[idx_crab];
            let id = fallback.font.glyph_id('\u{1f980}');
            assert_ne!(id.0, 0);
        }
    }
}
