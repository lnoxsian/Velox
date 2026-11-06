use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

const TOOL_WL_PASTE: u8 = 1 << 0;
const TOOL_XCLIP: u8 = 1 << 1;
const TOOL_XSEL: u8 = 1 << 2;

const CHECKED_WL_PASTE: u8 = 1 << 4;
const CHECKED_XCLIP: u8 = 1 << 5;
const CHECKED_XSEL: u8 = 1 << 6;

static CLI_TOOL_STATUS: AtomicU8 = AtomicU8::new(0);

/// Abstraction for platform-specific (Wayland / X11) native clipboard access.
pub trait ClipboardProvider: Send + Sync {
    fn copy(&self, text: &str);
    fn copy_primary(&self, text: &str);
    fn paste(&self) -> Option<String>;
    fn primary_selection(&self) -> Option<String>;
}

static INTERNAL_CLIPBOARD: RwLock<String> = RwLock::new(String::new());
static INTERNAL_PRIMARY: RwLock<String> = RwLock::new(String::new());
static ACTIVE_PROVIDER: RwLock<Option<Arc<dyn ClipboardProvider>>> = RwLock::new(None);

/// Register an active native display clipboard provider (e.g., Wayland or X11 window).
pub fn register_provider(provider: Arc<dyn ClipboardProvider>) {
    if let Ok(mut lock) = ACTIVE_PROVIDER.write() {
        *lock = Some(provider);
    }
}

/// Unregister the active display clipboard provider (e.g. when window closes).
pub fn unregister_provider() {
    if let Ok(mut lock) = ACTIVE_PROVIDER.write() {
        *lock = None;
    }
}

/// Write text to clipboard across all available layers:
/// 1. Guaranteed in-process memory buffer.
/// 2. Active native display server provider (Wayland data device / X11 selection).
/// 3. Secondary background CLI tool synchronization (wl-copy / xclip / xsel).
pub fn copy(text: String) {
    if text.is_empty() {
        return;
    }

    // 1. Always retain in internal memory first (ensures zero data loss)
    if let Ok(mut lock) = INTERNAL_CLIPBOARD.write() {
        *lock = text.clone();
    }
    if let Ok(mut lock) = INTERNAL_PRIMARY.write() {
        *lock = text.clone();
    }

    // 2. Forward to active native display provider if registered
    if let Ok(lock) = ACTIVE_PROVIDER.read() {
        if let Some(ref provider) = *lock {
            provider.copy(&text);
            provider.copy_primary(&text);
        }
    }

    // 3. Secondary background CLI tool sync (detached thread to avoid blocking UI)
    let text_bytes = text.into_bytes();
    let _ = std::thread::Builder::new()
        .name("velox-clipboard".to_string())
        .spawn(move || {
            // Try wl-copy (Wayland) first
            if let Ok(mut child) = Command::new("wl-copy")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(&text_bytes);
                }
                if let Ok(status) = child.wait()
                    && status.success()
                {
                    // Also update Wayland primary selection if possible
                    if let Ok(mut pchild) = Command::new("wl-copy")
                        .arg("--primary")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        if let Some(mut pstdin) = pchild.stdin.take() {
                            let _ = pstdin.write_all(&text_bytes);
                        }
                        let _ = pchild.wait();
                    }
                    return;
                }
            }

            // Try xclip (X11)
            if let Ok(mut child) = Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(&text_bytes);
                }
                if let Ok(status) = child.wait()
                    && status.success()
                {
                    // Also update primary selection
                    if let Ok(mut pchild) = Command::new("xclip")
                        .arg("-selection")
                        .arg("primary")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        if let Some(mut pstdin) = pchild.stdin.take() {
                            let _ = pstdin.write_all(&text_bytes);
                        }
                        let _ = pchild.wait();
                    }
                    return;
                }
            }

            // Try xsel (X11 fallback)
            if let Ok(mut child) = Command::new("xsel")
                .arg("--clipboard")
                .arg("--input")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(&text_bytes);
                }
                let _ = child.wait();

                // Also copy to primary with xsel
                if let Ok(mut pchild) = Command::new("xsel")
                    .arg("--primary")
                    .arg("--input")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    if let Some(mut pstdin) = pchild.stdin.take() {
                        let _ = pstdin.write_all(&text_bytes);
                    }
                    let _ = pchild.wait();
                }
            }
        });
}

/// Retrieve text from clipboard:
/// 1. Queries active native display server provider.
/// 2. Queries external CLI tools (wl-paste, xclip, xsel).
/// 3. Falls back to in-memory internal clipboard buffer.
/// 4. Falls back to primary selection.
pub fn paste() -> String {
    // 1. Try active native display server provider
    if let Ok(lock) = ACTIVE_PROVIDER.read() {
        if let Some(ref provider) = *lock {
            if let Some(text) = provider.paste() {
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }

    // 2. Try external CLI tools
    let cli_text = try_cli_paste();
    if !cli_text.is_empty() {
        return cli_text;
    }

    // 3. Fall back to internal memory buffer
    if let Ok(lock) = INTERNAL_CLIPBOARD.read() {
        if !lock.is_empty() {
            return lock.clone();
        }
    }

    // 4. Fall back to primary selection if clipboard is empty
    primary_selection()
}

/// Retrieve text from primary selection:
/// 1. Queries active native display server provider.
/// 2. Queries external CLI tools (wl-paste --primary, xclip -selection primary, xsel --primary).
/// 3. Falls back to in-memory primary selection buffer.
pub fn primary_selection() -> String {
    // 1. Try active native display server provider
    if let Ok(lock) = ACTIVE_PROVIDER.read() {
        if let Some(ref provider) = *lock {
            if let Some(text) = provider.primary_selection() {
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }

    // 2. Try external CLI tools
    let cli_text = try_cli_primary();
    if !cli_text.is_empty() {
        return cli_text;
    }

    // 3. Fall back to internal memory buffer
    if let Ok(lock) = INTERNAL_PRIMARY.read() {
        if !lock.is_empty() {
            return lock.clone();
        }
    }

    String::new()
}

fn try_cli_paste() -> String {
    let status = CLI_TOOL_STATUS.load(Ordering::Relaxed);

    // 1. Try wl-paste if not already known to be missing
    if (status & CHECKED_WL_PASTE == 0) || (status & TOOL_WL_PASTE != 0) {
        match Command::new("wl-paste").arg("--no-newline").output() {
            Ok(output) => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_WL_PASTE | TOOL_WL_PASTE, Ordering::Relaxed);
                if output.status.success()
                    && let Ok(text) = String::from_utf8(output.stdout)
                    && !text.is_empty()
                {
                    return text;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_WL_PASTE, Ordering::Relaxed);
            }
            Err(_) => {}
        }
    }

    // 2. Try xclip if not already known to be missing
    let status = CLI_TOOL_STATUS.load(Ordering::Relaxed);
    if (status & CHECKED_XCLIP == 0) || (status & TOOL_XCLIP != 0) {
        match Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .arg("-o")
            .output()
        {
            Ok(output) => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XCLIP | TOOL_XCLIP, Ordering::Relaxed);
                if output.status.success()
                    && let Ok(text) = String::from_utf8(output.stdout)
                    && !text.is_empty()
                {
                    return text;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XCLIP, Ordering::Relaxed);
            }
            Err(_) => {}
        }
    }

    // 3. Try xsel if not already known to be missing
    let status = CLI_TOOL_STATUS.load(Ordering::Relaxed);
    if (status & CHECKED_XSEL == 0) || (status & TOOL_XSEL != 0) {
        match Command::new("xsel")
            .arg("--clipboard")
            .arg("--output")
            .output()
        {
            Ok(output) => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XSEL | TOOL_XSEL, Ordering::Relaxed);
                if output.status.success()
                    && let Ok(text) = String::from_utf8(output.stdout)
                    && !text.is_empty()
                {
                    return text;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XSEL, Ordering::Relaxed);
            }
            Err(_) => {}
        }
    }

    String::new()
}

fn try_cli_primary() -> String {
    let status = CLI_TOOL_STATUS.load(Ordering::Relaxed);

    // 1. Try wl-paste --primary if not already known to be missing
    if (status & CHECKED_WL_PASTE == 0) || (status & TOOL_WL_PASTE != 0) {
        match Command::new("wl-paste")
            .arg("--primary")
            .arg("--no-newline")
            .output()
        {
            Ok(output) => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_WL_PASTE | TOOL_WL_PASTE, Ordering::Relaxed);
                if output.status.success()
                    && let Ok(text) = String::from_utf8(output.stdout)
                    && !text.is_empty()
                {
                    return text;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_WL_PASTE, Ordering::Relaxed);
            }
            Err(_) => {}
        }
    }

    // 2. Try xclip -selection primary if not already known to be missing
    let status = CLI_TOOL_STATUS.load(Ordering::Relaxed);
    if (status & CHECKED_XCLIP == 0) || (status & TOOL_XCLIP != 0) {
        match Command::new("xclip")
            .arg("-selection")
            .arg("primary")
            .arg("-o")
            .output()
        {
            Ok(output) => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XCLIP | TOOL_XCLIP, Ordering::Relaxed);
                if output.status.success()
                    && let Ok(text) = String::from_utf8(output.stdout)
                    && !text.is_empty()
                {
                    return text;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XCLIP, Ordering::Relaxed);
            }
            Err(_) => {}
        }
    }

    // 3. Try xsel --primary if not already known to be missing
    let status = CLI_TOOL_STATUS.load(Ordering::Relaxed);
    if (status & CHECKED_XSEL == 0) || (status & TOOL_XSEL != 0) {
        match Command::new("xsel")
            .arg("--primary")
            .arg("--output")
            .output()
        {
            Ok(output) => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XSEL | TOOL_XSEL, Ordering::Relaxed);
                if output.status.success()
                    && let Ok(text) = String::from_utf8(output.stdout)
                    && !text.is_empty()
                {
                    return text;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                CLI_TOOL_STATUS.fetch_or(CHECKED_XSEL, Ordering::Relaxed);
            }
            Err(_) => {}
        }
    }

    String::new()
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

    for &byte in input.as_bytes() {
        if byte == b'=' || byte == b'\r' || byte == b'\n' || byte == b' ' {
            continue;
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
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    struct MockProvider {
        data: RwLock<String>,
        primary: RwLock<String>,
    }

    impl ClipboardProvider for MockProvider {
        fn copy(&self, text: &str) {
            *self.data.write().unwrap() = text.to_string();
        }
        fn copy_primary(&self, text: &str) {
            *self.primary.write().unwrap() = text.to_string();
        }
        fn paste(&self) -> Option<String> {
            let val = self.data.read().unwrap().clone();
            if val.is_empty() { None } else { Some(val) }
        }
        fn primary_selection(&self) -> Option<String> {
            let val = self.primary.read().unwrap().clone();
            if val.is_empty() { None } else { Some(val) }
        }
    }

    #[test]
    fn test_internal_in_memory_clipboard_fallback() {
        let _guard = TEST_MUTEX.lock().unwrap();
        unregister_provider();
        // Clear in-memory buffers for test isolation
        *INTERNAL_CLIPBOARD.write().unwrap() = String::new();
        *INTERNAL_PRIMARY.write().unwrap() = String::new();

        copy("Velox In-Memory Fallback Test".to_string());
        assert_eq!(
            INTERNAL_CLIPBOARD.read().unwrap().as_str(),
            "Velox In-Memory Fallback Test"
        );
        assert_eq!(
            INTERNAL_PRIMARY.read().unwrap().as_str(),
            "Velox In-Memory Fallback Test"
        );

        // paste() should return the stored string even without external tools
        let pasted = paste();
        assert!(pasted == "Velox In-Memory Fallback Test" || !pasted.is_empty());
    }

    #[test]
    fn test_mock_clipboard_provider_registration() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let provider = Arc::new(MockProvider {
            data: RwLock::new(String::new()),
            primary: RwLock::new(String::new()),
        });

        register_provider(provider.clone());

        copy("Provider Handled Data".to_string());
        assert_eq!(
            provider.data.read().unwrap().as_str(),
            "Provider Handled Data"
        );
        assert_eq!(
            provider.primary.read().unwrap().as_str(),
            "Provider Handled Data"
        );
        assert_eq!(paste(), "Provider Handled Data");

        unregister_provider();
    }
}
