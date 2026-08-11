/// Opens a URL using the system's default handler (xdg-open on Linux).
pub fn open(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let mut child = std::process::Command::new("xdg-open").arg(url).spawn()?;

        // Spawn a background thread to wait for the process to exit.
        // This reaps the child process, preventing it from becoming a zombie,
        // without blocking the main event loop.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = url;
    }
    Ok(())
}

/// Detects hyperlinks in a given text line.
///
/// Returns a vector of tuples containing:
/// `(start_index, end_index, url_string)`
pub fn detect(text: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    let schemes = ["http://", "https://", "mailto:", "file://"];

    for scheme in &schemes {
        let mut start_search = 0;
        while let Some(start_idx) = text[start_search..].find(scheme) {
            let abs_start = start_search + start_idx;

            // Find the end bound of the URL
            let mut end_idx = abs_start + scheme.len();
            let bytes = text.as_bytes();
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

            // Strip trailing punctuation common at the end of sentences
            while end_idx > abs_start + scheme.len() {
                let last_byte = bytes[end_idx - 1];
                if last_byte == b'.'
                    || last_byte == b','
                    || last_byte == b';'
                    || last_byte == b'?'
                    || last_byte == b'!'
                    || last_byte == b':'
                    || last_byte == b')'
                {
                    end_idx -= 1;
                } else {
                    break;
                }
            }

            if end_idx > abs_start + scheme.len()
                && let Ok(url) = std::str::from_utf8(&bytes[abs_start..end_idx])
            {
                results.push((abs_start, end_idx, url.to_string()));
            }

            start_search = end_idx;
        }
    }

    // Sort results by start index
    results.sort_by_key(|&(start, _, _)| start);
    results
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
}
