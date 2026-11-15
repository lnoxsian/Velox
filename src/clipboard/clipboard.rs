use super::manager::ClipboardManager;

/// Copy text to the system CLIPBOARD (and PRIMARY) selection.
#[inline(always)]
pub fn copy(text: String) {
    ClipboardManager::global().copy(text);
}

/// Copy text to PRIMARY selection only (used for terminal mouse drag/click selection).
/// Does not overwrite CLIPBOARD, preserving text copied from browsers or other applications.
#[inline(always)]
pub fn copy_primary(text: String) {
    ClipboardManager::global().copy_primary(text);
}

/// Retrieve text from system CLIPBOARD selection (Ctrl+Shift+V / Shift+Insert).
#[inline(always)]
pub fn paste() -> String {
    ClipboardManager::global().paste()
}

/// Retrieve text from system PRIMARY selection (mouse highlight/selection).
#[inline(always)]
pub fn primary_selection() -> String {
    ClipboardManager::global().primary_selection()
}

/// Retrieve the appropriate text for middle-click paste.
/// By default, follows Terminator-style timestamp comparison (choosing latest action between CLIPBOARD and PRIMARY),
/// or strict standard Linux behavior (preferring PRIMARY, falling back to CLIPBOARD if PRIMARY is empty) when enabled.
#[inline(always)]
pub fn middle_click_paste() -> String {
    ClipboardManager::global().middle_click_paste()
}

/// Configure whether middle-click paste strictly prefers PRIMARY (falling back to CLIPBOARD if empty).
#[inline(always)]
pub fn set_middle_click_prefers_primary(prefer: bool) {
    ClipboardManager::global().set_middle_click_prefers_primary(prefer);
}

/// Configure whether PRIMARY selections (mouse highlighting) are saved to the clipboard history ring.
#[inline(always)]
pub fn set_save_primary_to_history(save: bool) {
    ClipboardManager::global().set_save_primary_to_history(save);
}

/// Retrieve all text entries currently stored in the clipboard history ring (newest first).
#[inline(always)]
pub fn get_history() -> Vec<String> {
    ClipboardManager::global().get_history()
}

/// Clear all entries from the clipboard history ring.
#[inline(always)]
pub fn clear_history() {
    ClipboardManager::global().clear_history();
}

/// Set the maximum number of items retained in the clipboard history ring.
#[inline(always)]
pub fn set_history_limit(limit: usize) {
    ClipboardManager::global().set_history_limit(limit);
}

/// In-process internal clipboard value (used for headless/container fallbacks and diagnostics).
#[inline(always)]
pub fn get_internal_clipboard() -> String {
    ClipboardManager::global().get_internal_clipboard()
}

/// In-process internal primary value (used for headless/container fallbacks and diagnostics).
#[inline(always)]
pub fn get_internal_primary() -> String {
    ClipboardManager::global().get_internal_primary()
}

/// Mark whether the current window holds ownership of the system clipboard selection.
#[inline(always)]
pub fn set_owns_clipboard(owns: bool) {
    ClipboardManager::global().set_owns_clipboard(owns);
}

/// Mark whether the current window holds ownership of the system primary selection.
#[inline(always)]
pub fn set_owns_primary(owns: bool) {
    ClipboardManager::global().set_owns_primary(owns);
}

/// Notify that an external client claimed the system CLIPBOARD selection.
#[inline(always)]
pub fn notify_external_clipboard_claimed() {
    ClipboardManager::global().notify_external_clipboard_claimed();
}

/// Notify that an external client claimed the system PRIMARY selection.
#[inline(always)]
pub fn notify_external_primary_claimed() {
    ClipboardManager::global().notify_external_primary_claimed();
}

// Custom lightweight Base64 helper for OSC 52
const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < data.len() {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if i + 2 < data.len() {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }
    result
}

pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut table = [255u8; 256];
    for (idx, &ch) in BASE64_CHARS.iter().enumerate() {
        table[ch as usize] = idx as u8;
    }

    let mut buf = Vec::with_capacity(input.len() * 3 / 4);
    let mut val = 0u32;
    let mut valb = -8i32;
    let mut padding_seen = false;
    let mut pad_count = 0;

    for &byte in input.as_bytes() {
        if byte == b'\r' || byte == b'\n' || byte == b' ' || byte == b'\t' {
            continue;
        }
        if byte == b'=' {
            padding_seen = true;
            pad_count += 1;
            if pad_count > 2 {
                return None;
            }
            continue;
        }
        if padding_seen {
            // Non-padding character encountered after padding started
            return None;
        }
        let code = table[byte as usize];
        if code == 255 {
            return None;
        }
        val = (val << 6) | (code as u32);
        valb += 6;
        if valb >= 0 {
            buf.push(((val >> valb) & 0xFF) as u8);
            valb -= 8;
        }
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_clipboard_fallback() {
        let manager = ClipboardManager::new(50);
        manager.copy("test_velox_internal_copy".to_string());
        assert_eq!(manager.get_internal_clipboard(), "test_velox_internal_copy");
        assert_eq!(manager.get_internal_primary(), "test_velox_internal_copy");

        manager.copy_primary("test_primary_only".to_string());
        assert_eq!(manager.get_internal_primary(), "test_primary_only");
        // Ensure copy_primary did not overwrite internal clipboard
        assert_eq!(manager.get_internal_clipboard(), "test_velox_internal_copy");
    }

    #[test]
    fn test_history_ring() {
        let manager = ClipboardManager::new(3);
        manager.copy("item 1".to_string());
        manager.copy("item 2".to_string());
        manager.copy("item 3".to_string());
        manager.copy("item 4".to_string());

        let hist = manager.get_history();
        assert_eq!(hist.len(), 3);
        assert_eq!(hist[0], "item 4");
        assert_eq!(hist[1], "item 3");
        assert_eq!(hist[2], "item 2");
    }

    #[test]
    fn test_base64_encode_decode_roundtrip() {
        let data = b"Hello, World! 1234567890 \xf0\x9f\x91\x8d";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).expect("base64_decode failed");
        assert_eq!(decoded, data);

        // Reject invalid padding
        assert!(base64_decode("SGVsbG8=== ").is_none());
        assert!(base64_decode("=SGVsbG8=").is_none());
        assert!(base64_decode("SGVsbG8=A").is_none());
    }
}
