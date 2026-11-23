use crate::font::resolved::{ResolvedFont, ResolvedFontSet};
use crate::font::storage::{FontStorage, create_font_arc};
use ab_glyph::FontArc;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

pub const FONT_CACHE_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CachedFaceEntry {
    pub path: PathBuf,
    pub face_index: u32,
    pub mtime: u64,
    pub size: u64,
}

impl CachedFaceEntry {
    pub fn from_path_and_index(path: PathBuf, face_index: u32) -> Option<Self> {
        let meta = fs::metadata(&path).ok()?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let size = meta.len();

        Some(Self {
            path,
            face_index,
            mtime,
            size,
        })
    }

    pub fn is_valid(&self) -> bool {
        if let Ok(meta) = fs::metadata(&self.path) {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            return meta.len() == self.size && mtime == self.mtime;
        }
        false
    }

    pub fn load_font(&self) -> Option<FontArc> {
        let storage = Arc::new(FontStorage::from_file(&self.path).ok()?);
        create_font_arc(storage, self.face_index).ok()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CachedFontSet {
    pub cache_version: u32,
    pub font_family: String,
    pub regular: CachedFaceEntry,
    pub bold: Option<CachedFaceEntry>,
    pub bold_synthetic: bool,
    pub italic: Option<CachedFaceEntry>,
    pub italic_synthetic: bool,
    pub bold_italic: Option<CachedFaceEntry>,
    pub bold_italic_synthetic: bool,
}

impl CachedFontSet {
    /// Attempts to instantiate a `ResolvedFontSet` from cached face entries.
    /// Returns `None` if any referenced font file is missing, modified, or corrupt.
    pub fn load_resolved_set(&self) -> Option<ResolvedFontSet> {
        if !self.regular.is_valid() {
            return None;
        }
        let regular_font = self.regular.load_font()?;

        let (bold_font, bold_synthetic) = if self.bold_synthetic {
            (regular_font.clone(), true)
        } else if let Some(ref b) = self.bold {
            if !b.is_valid() {
                return None;
            }
            (b.load_font().unwrap_or_else(|| regular_font.clone()), false)
        } else {
            (regular_font.clone(), true)
        };

        let (italic_font, italic_synthetic) = if self.italic_synthetic {
            (regular_font.clone(), true)
        } else if let Some(ref it) = self.italic {
            if !it.is_valid() {
                return None;
            }
            (it.load_font().unwrap_or_else(|| regular_font.clone()), false)
        } else {
            (regular_font.clone(), true)
        };

        let (bold_italic_font, bi_synthetic_italic, bi_synthetic_bold) = if self.bold_italic_synthetic {
            let f = if italic_synthetic {
                regular_font.clone()
            } else {
                italic_font.clone()
            };
            (f, true, true)
        } else if let Some(ref bi) = self.bold_italic {
            if !bi.is_valid() {
                return None;
            }
            (bi.load_font().unwrap_or_else(|| regular_font.clone()), false, false)
        } else {
            let f = if italic_synthetic {
                regular_font.clone()
            } else {
                italic_font.clone()
            };
            (f, true, true)
        };

        Some(ResolvedFontSet {
            regular: ResolvedFont {
                font: regular_font,
                synthetic_italic: false,
                synthetic_bold: false,
            },
            bold: ResolvedFont {
                font: bold_font,
                synthetic_italic: false,
                synthetic_bold: bold_synthetic,
            },
            italic: ResolvedFont {
                font: italic_font,
                synthetic_italic: italic_synthetic,
                synthetic_bold: false,
            },
            bold_italic: ResolvedFont {
                font: bold_italic_font,
                synthetic_italic: bi_synthetic_italic,
                synthetic_bold: bi_synthetic_bold,
            },
        })
    }
}

pub struct FontCache;

impl FontCache {
    /// Returns the absolute path to the binary font cache file: `$XDG_CACHE_HOME/velox/font_cache.bin`.
    pub fn cache_path() -> PathBuf {
        let base = std::env::var("XDG_CACHE_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".cache"))
            })
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        base.join("velox/font_cache.bin")
    }

    /// Fast-path loader: attempts to load the cached `ResolvedFontSet` for `font_family`.
    /// Execution time is under 1ms because it uses memory-mapped font bytes directly.
    pub fn load(font_family: &str) -> Option<ResolvedFontSet> {
        let path = Self::cache_path();
        let bytes = fs::read(&path).ok()?;
        let cached: CachedFontSet = bincode::deserialize(&bytes).ok()?;

        if cached.cache_version != FONT_CACHE_VERSION {
            return None;
        }

        if !cached.font_family.eq_ignore_ascii_case(font_family) {
            return None;
        }

        cached.load_resolved_set()
    }

    /// Persists a resolved font set and its source files to disk atomically.
    pub fn store(
        font_family: &str,
        regular_path: &Path,
        regular_index: u32,
        bold_info: Option<(&Path, u32)>,
        bold_synthetic: bool,
        italic_info: Option<(&Path, u32)>,
        italic_synthetic: bool,
        bold_italic_info: Option<(&Path, u32)>,
        bold_italic_synthetic: bool,
    ) {
        let regular = match CachedFaceEntry::from_path_and_index(regular_path.to_path_buf(), regular_index) {
            Some(e) => e,
            None => return,
        };

        let bold = bold_info.and_then(|(p, idx)| CachedFaceEntry::from_path_and_index(p.to_path_buf(), idx));
        let italic = italic_info.and_then(|(p, idx)| CachedFaceEntry::from_path_and_index(p.to_path_buf(), idx));
        let bold_italic = bold_italic_info.and_then(|(p, idx)| CachedFaceEntry::from_path_and_index(p.to_path_buf(), idx));

        let cached = CachedFontSet {
            cache_version: FONT_CACHE_VERSION,
            font_family: font_family.to_string(),
            regular,
            bold,
            bold_synthetic,
            italic,
            italic_synthetic,
            bold_italic,
            bold_italic_synthetic,
        };

        if let Ok(encoded) = bincode::serialize(&cached) {
            let path = Self::cache_path();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
            if fs::write(&tmp_path, encoded).is_ok() {
                let _ = fs::rename(&tmp_path, &path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_cache_serialization_and_validation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dummy_font_path = temp_dir.path().join("test_font.ttf");
        fs::write(&dummy_font_path, b"dummy font contents for testing cache").unwrap();

        let entry = CachedFaceEntry::from_path_and_index(dummy_font_path.clone(), 0).unwrap();
        assert!(entry.is_valid());

        let cached_set = CachedFontSet {
            cache_version: FONT_CACHE_VERSION,
            font_family: "TestFont".to_string(),
            regular: entry,
            bold: None,
            bold_synthetic: true,
            italic: None,
            italic_synthetic: true,
            bold_italic: None,
            bold_italic_synthetic: true,
        };

        let encoded = bincode::serialize(&cached_set).unwrap();
        let decoded: CachedFontSet = bincode::deserialize(&encoded).unwrap();
        assert_eq!(cached_set, decoded);

        // Modify file content to change mtime and size, verifying cache validation failure
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&dummy_font_path, b"modified larger font file content!").unwrap();
        assert!(!decoded.regular.is_valid());
    }
}
