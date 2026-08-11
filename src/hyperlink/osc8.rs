/// Parses an OSC 8 hyperlink sequence parameters and URL.
///
/// The OSC 8 sequence has the format: `OSC 8 ; params ; url`
/// - If `url` is empty, it indicates a closing of the current hyperlink context.
/// - Returns a tuple containing `(id, url)` if parsing is successful.
pub fn parse(params: &[&[u8]]) -> Option<(String, String)> {
    if params.len() >= 3 && params[0] == b"8" {
        let id_param = std::str::from_utf8(params[1]).unwrap_or("");
        let url = std::str::from_utf8(params[2]).unwrap_or("").to_string();

        // Extract the "id" parameter (e.g. "id=123" or "id=foo") if it exists
        let mut id = String::new();
        for part in id_param.split(':') {
            if let Some(rest) = part.strip_prefix("id=") {
                id = rest.to_string();
            }
        }

        Some((id, url))
    } else {
        None
    }
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
    }
}
