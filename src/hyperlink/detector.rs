/// Opens a URL using the system's default handler (xdg-open on Linux).
pub fn open(url: &str) -> std::io::Result<()> {
    #[cfg(all(target_os = "linux", not(test)))]
    {
        let formatted_url = if url.starts_with("www.") {
            format!("https://{}", url)
        } else {
            url.to_string()
        };
        let mut child = std::process::Command::new("xdg-open")
            .arg(&formatted_url)
            .spawn()?;

        // Spawn a background thread to wait for the process to exit.
        // This reaps the child process, preventing it from becoming a zombie,
        // without blocking the main event loop.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    #[cfg(any(not(target_os = "linux"), test))]
    {
        let _ = url;
    }
    Ok(())
}

/// Internal helper to scan raw byte ranges of URLs in `text`.
/// Uses a stack-allocated `SmallVec` to avoid heap allocations for common lines.
fn scan_raw_matches(text: &str) -> smallvec::SmallVec<[(usize, usize); 4]> {
    let bytes = text.as_bytes();
    // Fast-exit: all supported schemes require ':' or '.'
    if !bytes.iter().any(|&b| b == b':' || b == b'.') {
        return smallvec::SmallVec::new();
    }

    let mut raw_matches: smallvec::SmallVec<[(usize, usize); 4]> = smallvec::SmallVec::new();
    let schemes = [
        "https://", "http://", "file://", "mailto:", "ftp://", "git://", "ssh://", "www.",
    ];

    for scheme in &schemes {
        let mut start_search = 0;
        while start_search < bytes.len() {
            while start_search < bytes.len() && !text.is_char_boundary(start_search) {
                start_search += 1;
            }
            if start_search >= bytes.len() {
                break;
            }

            let Some(start_idx) = text[start_search..].find(scheme) else {
                break;
            };
            let abs_start = start_search + start_idx;

            // Find the end bound of the URL
            let mut end_idx = abs_start + scheme.len();
            while end_idx < bytes.len() {
                let b = bytes[end_idx];
                // URL characters cannot be whitespace, quotes, or angle brackets
                if b.is_ascii_whitespace()
                    || b == b'"'
                    || b == b'\''
                    || b == b'<'
                    || b == b'>'
                    || b == b'`'
                    || b == b'{'
                    || b == b'}'
                {
                    break;
                }
                end_idx += 1;
            }

            // Single-pass paren counting
            let (open_parens, mut close_parens) = bytes[abs_start..end_idx]
                .iter()
                .fold((0usize, 0usize), |(o, c), &b| {
                    (o + (b == b'(') as usize, c + (b == b')') as usize)
                });

            // Strip trailing punctuation common at the end of sentences
            while end_idx > abs_start + scheme.len() {
                let last_byte = bytes[end_idx - 1];
                if matches!(
                    last_byte,
                    b'.' | b',' | b';' | b'?' | b'!' | b':' | b'\'' | b'"'
                ) {
                    end_idx -= 1;
                } else if last_byte == b')' && close_parens > open_parens {
                    end_idx -= 1;
                    close_parens -= 1;
                } else {
                    break;
                }
            }

            // Ensure end_idx is on a valid char boundary
            while end_idx > abs_start && !text.is_char_boundary(end_idx) {
                end_idx -= 1;
            }

            if end_idx > abs_start + scheme.len() {
                // Ensure this range does not overlap with an already discovered URL
                let overlaps = raw_matches
                    .iter()
                    .any(|&(s, e)| !(end_idx <= s || abs_start >= e));
                if !overlaps {
                    raw_matches.push((abs_start, end_idx));
                }
            }

            start_search = end_idx;
        }
    }

    raw_matches.sort_by_key(|&(start, _)| start);
    raw_matches
}

/// Detects hyperlinks in a given text line without heap string allocations.
///
/// Returns a vector of tuples containing:
/// `(char_start_col, char_end_col, &str)`
/// where start and end indices are character (column) indices matching terminal cells.
pub fn detect(text: &str) -> Vec<(usize, usize, &str)> {
    let raw_matches = scan_raw_matches(text);
    if raw_matches.is_empty() {
        return Vec::new();
    }

    // Convert byte indices to character (cell/column) indices
    let mut results = Vec::with_capacity(raw_matches.len());
    let mut current_byte = 0;
    let mut current_char = 0;

    for (b_start, b_end) in raw_matches {
        let char_start = current_char
            + text
                .get(current_byte..b_start)
                .map(|s| s.chars().count())
                .unwrap_or(0);
        let char_len = text
            .get(b_start..b_end)
            .map(|s| s.chars().count())
            .unwrap_or(0);
        let char_end = char_start + char_len;

        current_byte = b_end;
        current_char = char_end;

        if let Some(slice) = text.as_bytes().get(b_start..b_end) {
            if let Ok(url) = std::str::from_utf8(slice) {
                results.push((char_start, char_end, url));
            }
        }
    }

    results
}

/// Zero-allocation scanner for URL ranges in a line of text.
///
/// Yields `(char_start_col, char_end_col)` matching terminal cell columns.
#[inline]
pub fn for_each_url(text: &str, mut f: impl FnMut(usize, usize)) {
    let raw_matches = scan_raw_matches(text);
    if raw_matches.is_empty() {
        return;
    }

    let mut current_byte = 0;
    let mut current_char = 0;

    for (b_start, b_end) in raw_matches {
        let char_start = current_char
            + text
                .get(current_byte..b_start)
                .map(|s| s.chars().count())
                .unwrap_or(0);
        let char_len = text
            .get(b_start..b_end)
            .map(|s| s.chars().count())
            .unwrap_or(0);
        let char_end = char_start + char_len;

        current_byte = b_end;
        current_char = char_end;

        f(char_start, char_end);
    }
}

/// Checks if a given character column falls within any detected URL.
/// Returns immediately without allocating any heap memory.
#[inline]
pub fn is_url_at(text: &str, char_col: usize) -> bool {
    let raw_matches = scan_raw_matches(text);
    if raw_matches.is_empty() {
        return false;
    }

    let mut current_byte = 0;
    let mut current_char = 0;

    for (b_start, b_end) in raw_matches {
        let char_start = current_char
            + text
                .get(current_byte..b_start)
                .map(|s| s.chars().count())
                .unwrap_or(0);
        if char_col < char_start {
            return false;
        }
        let char_len = text
            .get(b_start..b_end)
            .map(|s| s.chars().count())
            .unwrap_or(0);
        let char_end = char_start + char_len;
        if char_col < char_end {
            return true;
        }

        current_byte = b_end;
        current_char = char_end;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_urls() {
        let text = "Check out https://github.com/rust-lang/rust for more info, or email mailto:test@example.com.";
        let urls = detect(text);

        assert_eq!(urls.len(), 2);

        assert_eq!(urls[0].0, 10);
        assert_eq!(urls[0].1, 43);
        assert_eq!(urls[0].2, "https://github.com/rust-lang/rust");

        assert_eq!(urls[1].2, "mailto:test@example.com");
    }

    #[test]
    fn test_highlight_invalid_trailing() {
        let text = "Click here: https://example.com/page?ref=xyz. The page is cool.";
        let urls = detect(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].2, "https://example.com/page?ref=xyz");
    }

    #[test]
    fn test_unicode_prompt_url_character_indices() {
        let text = "Velox on  main [!] is 📦 v0.2.5 via 🦀 v1.99.0 ❯ echo https://github.com/rust-lang/rust here";
        let urls = detect(text);
        assert_eq!(urls.len(), 1);
        let (start, end, url) = urls[0];
        assert_eq!(url, "https://github.com/rust-lang/rust");

        // Verify that start and end match character indices
        let extracted: String = text.chars().skip(start).take(end - start).collect();
        assert_eq!(extracted, "https://github.com/rust-lang/rust");
    }

    #[test]
    fn test_www_and_parens_detection() {
        let text = "Visit www.rust-lang.org or (https://en.wikipedia.org/wiki/Rust_(programming_language)).";
        let urls = detect(text);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].2, "www.rust-lang.org");
        assert_eq!(
            urls[1].2,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );

        let extracted_wiki: String = text
            .chars()
            .skip(urls[1].0)
            .take(urls[1].1 - urls[1].0)
            .collect();
        assert_eq!(
            extracted_wiki,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );
    }

    #[test]
    fn test_is_url_at_and_for_each_url() {
        let text = "Prefix https://example.com/test suffix";
        // "Prefix " is 7 chars (0..6). URL is 7..31. " suffix" starts at 31.
        assert!(!is_url_at(text, 6));
        assert!(is_url_at(text, 7));
        assert!(is_url_at(text, 20));
        assert!(is_url_at(text, 30));
        assert!(!is_url_at(text, 31));

        // Test fast-exit with text containing no scheme triggers
        assert!(!is_url_at("plain text without any triggers", 5));

        // Test for_each_url zero-allocation callback
        let mut ranges = Vec::new();
        for_each_url(text, |s, e| ranges.push((s, e)));
        assert_eq!(ranges, vec![(7, 31)]);
    }

    #[test]
    fn test_multibyte_non_ascii_url_boundary_safety() {
        // Multi-byte chars before, inside, and immediately after URL
        let text = "🦀 网址: https://example.com/🚀/🎉 结尾。";
        let urls = detect(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].2, "https://example.com/🚀/🎉");

        // Verify is_url_at and for_each_url don't panic on character boundaries
        for col in 0..text.chars().count() {
            let _ = is_url_at(text, col);
        }
        for_each_url(text, |_, _| {});
    }
}
