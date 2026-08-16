use velox::font::fallback::{FallbackManager, MAX_FALLBACK_FONTS, MAX_MISSING_CHARS};
use velox::font::loader::MAX_DYNAMIC_GLYPHS;

#[test]
fn test_fallback_manager_bounded_capacity() {
    let mut manager = FallbackManager::new();

    // Query diverse unicode characters across multiple scripts and PUA ranges
    let test_chars = [
        'A',
        'Z',
        '0',
        '9',
        'א',
        'ב',
        'ג', // Hebrew
        'а',
        'б',
        'в', // Cyrillic
        'α',
        'β',
        'γ', // Greek
        'क',
        'ख',
        'ग', // Devanagari
        'あ',
        'い',
        'う', // Japanese Hiragana
        '한',
        '글', // Korean Hangul
        '中',
        '文', // CJK Ideographs
        '\u{e0a0}',
        '\u{e0b0}',
        '\u{e0b2}', // Powerline
        '\u{f101}',
        '\u{f102}',
        '\u{f103}', // Nerd Font FontAwesome
        '\u{1f600}',
        '\u{1f680}',
        '\u{1f389}', // Emojis
    ];

    for &c in &test_chars {
        let _ = manager.find_fallback_for_char(c);
        assert!(
            manager.fallbacks.len() <= MAX_FALLBACK_FONTS,
            "Fallback font count ({}) exceeded bound ({})",
            manager.fallbacks.len(),
            MAX_FALLBACK_FONTS
        );
    }
}

#[test]
fn test_fallback_missing_chars_bounded() {
    let mut manager = FallbackManager::new();

    // Query characters in non-existent / obscure ranges to test missing_chars bounded set
    for i in 0..50 {
        let c = char::from_u32(0x30000 + i).unwrap_or('?');
        let _ = manager.find_fallback_for_char(c);
    }

    // Repeated lookups should hit the missing_chars cache immediately
    for i in 0..50 {
        let c = char::from_u32(0x30000 + i).unwrap_or('?');
        assert_eq!(manager.find_fallback_for_char(c), None);
    }

    assert!(manager.fallbacks.len() <= MAX_FALLBACK_FONTS);
}

#[test]
fn test_glyph_cache_max_bound_constant() {
    assert_eq!(MAX_DYNAMIC_GLYPHS, 2048);
    assert_eq!(MAX_FALLBACK_FONTS, 8);
    assert_eq!(MAX_MISSING_CHARS, 1024);
}
