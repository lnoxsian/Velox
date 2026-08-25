use std::collections::BTreeMap;
use std::sync::Arc;

/// An explicit OSC 8 hyperlink definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperlink {
    pub id: String,
    pub url: String,
}

/// A contiguous column span on a single row sharing a hyperlink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperlinkSpan {
    pub start_col: usize,
    pub end_col: usize,
    pub link: Arc<Hyperlink>,
}

/// Sparse, row-indexed storage for explicit OSC 8 hyperlinks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HyperlinkStore {
    pub rows: BTreeMap<usize, Vec<HyperlinkSpan>>,
}

impl HyperlinkStore {
    pub fn new() -> Self {
        Self {
            rows: BTreeMap::new(),
        }
    }

    /// Set a hyperlink on a single cell at `(abs_row, col)`.
    pub fn insert(&mut self, abs_row: usize, col: usize, link: Arc<Hyperlink>) {
        let spans = self.rows.entry(abs_row).or_default();

        // Fast path: extend the latest span if contiguous and identical
        if let Some(last) = spans.last_mut()
            && last.end_col == col
            && last.link.url == link.url
            && last.link.id == link.id
        {
            last.end_col = col + 1;
            return;
        }

        // If span already covers this column, replace or split
        for span in spans.iter_mut() {
            if col >= span.start_col && col < span.end_col {
                if span.link.url == link.url && span.link.id == link.id {
                    return;
                }
                if col + 1 == span.end_col {
                    span.end_col = col;
                }
            }
        }

        spans.push(HyperlinkSpan {
            start_col: col,
            end_col: col + 1,
            link,
        });
    }

    /// Remove any explicit hyperlink at `(abs_row, col)` if a cell is overwritten without a link.
    pub fn remove_cell(&mut self, abs_row: usize, col: usize) {
        if let Some(spans) = self.rows.get_mut(&abs_row) {
            let mut i = 0;
            while i < spans.len() {
                if col >= spans[i].start_col && col < spans[i].end_col {
                    let old_end = spans[i].end_col;
                    let old_start = spans[i].start_col;
                    let old_link = spans[i].link.clone();

                    if col == old_start {
                        spans[i].start_col = col + 1;
                        if spans[i].start_col >= spans[i].end_col {
                            spans.remove(i);
                            continue;
                        }
                    } else if col + 1 == old_end {
                        spans[i].end_col = col;
                    } else {
                        // Split span into two halves around the removed column
                        spans[i].end_col = col;
                        spans.insert(
                            i + 1,
                            HyperlinkSpan {
                                start_col: col + 1,
                                end_col: old_end,
                                link: old_link,
                            },
                        );
                        i += 1;
                    }
                }
                i += 1;
            }
            if spans.is_empty() {
                self.rows.remove(&abs_row);
            }
        }
    }

    /// Retrieve the explicit hyperlink at `(abs_row, col)` if one exists.
    pub fn get(&self, abs_row: usize, col: usize) -> Option<Arc<Hyperlink>> {
        let spans = self.rows.get(&abs_row)?;
        for span in spans {
            if col >= span.start_col && col < span.end_col {
                return Some(span.link.clone());
            }
        }
        None
    }

    /// Prune history rows older than `min_abs_row` when scrollback evicts lines.
    pub fn prune_before(&mut self, min_abs_row: usize) {
        if min_abs_row > 0 {
            self.rows.retain(|&abs_row, _| abs_row >= min_abs_row);
        }
    }

    /// Clear all stored hyperlinks.
    pub fn clear(&mut self) {
        self.rows.clear();
    }
}

/// Parses an OSC 8 hyperlink sequence parameters and URL.
///
/// The OSC 8 sequence format: `OSC 8 ; [params] ; [url]`
/// - If `url` is empty, it closes the active hyperlink context.
/// - Returns a tuple containing `(id, url)` if parsing is successful.
pub fn parse(params: &[&[u8]]) -> Option<(String, String)> {
    if params.is_empty() || params[0] != b"8" {
        return None;
    }
    let id_param = if params.len() >= 2 {
        std::str::from_utf8(params[1]).unwrap_or("")
    } else {
        ""
    };
    let url = if params.len() >= 3 {
        if params.len() > 3 {
            let mut full = Vec::new();
            for (idx, p) in params[2..].iter().enumerate() {
                if idx > 0 {
                    full.push(b';');
                }
                full.extend_from_slice(p);
            }
            String::from_utf8_lossy(&full).to_string()
        } else {
            String::from_utf8_lossy(params[2]).to_string()
        }
    } else {
        String::new()
    };

    let mut id = String::new();
    for part in id_param.split(':') {
        if let Some(rest) = part.strip_prefix("id=") {
            id = rest.to_string();
        }
    }

    Some((id, url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_osc8() {
        let params: &[&[u8]] = &[b"8", b"id=link123:foo=bar", b"https://example.com"];
        let parsed = parse(params).unwrap();
        assert_eq!(parsed.0, "link123");
        assert_eq!(parsed.1, "https://example.com");

        let params_empty_id: &[&[u8]] = &[b"8", b"", b"https://google.com"];
        let parsed = parse(params_empty_id).unwrap();
        assert_eq!(parsed.0, "");
        assert_eq!(parsed.1, "https://google.com");

        // Closing sequence (empty URL)
        let params_close: &[&[u8]] = &[b"8", b"", b""];
        let parsed_close = parse(params_close).unwrap();
        assert_eq!(parsed_close.0, "");
        assert_eq!(parsed_close.1, "");
    }

    #[test]
    fn test_hyperlink_store_insert_and_lookup() {
        let mut store = HyperlinkStore::new();
        let link = Arc::new(Hyperlink {
            id: "1".to_string(),
            url: "https://velox.dev".to_string(),
        });

        store.insert(10, 2, link.clone());
        store.insert(10, 3, link.clone());
        store.insert(10, 4, link.clone());

        assert_eq!(store.get(10, 1), None);
        assert_eq!(store.get(10, 2).as_deref(), Some(&*link));
        assert_eq!(store.get(10, 3).as_deref(), Some(&*link));
        assert_eq!(store.get(10, 4).as_deref(), Some(&*link));
        assert_eq!(store.get(10, 5), None);

        // Overwrite middle column
        store.remove_cell(10, 3);
        assert_eq!(store.get(10, 2).as_deref(), Some(&*link));
        assert_eq!(store.get(10, 3), None);
        assert_eq!(store.get(10, 4).as_deref(), Some(&*link));

        // Pruning older rows
        store.prune_before(11);
        assert_eq!(store.get(10, 2), None);
    }
}
