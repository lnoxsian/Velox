use std::process::{Command, Stdio};
use std::io::Write;

pub fn copy(text: &str) {
    if text.is_empty() {
        return;
    }
    let text_bytes = text.as_bytes().to_vec();

    std::thread::spawn(move || {
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
            let _ = child.wait();
            return;
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
            let _ = child.wait();
            return;
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
        }
    });
}

pub fn paste() -> String {
    // 1. Try wl-paste
    if let Ok(output) = Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout) {
                return text;
            }

    // 2. Try xclip
    if let Ok(output) = Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .arg("-o")
        .output()
        && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout) {
                return text;
            }

    // 3. Try xsel
    if let Ok(output) = Command::new("xsel")
        .arg("--clipboard")
        .arg("--output")
        .output()
        && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout) {
                return text;
            }

    String::new()
}

pub fn primary_selection() -> String {
    // 1. Try wl-paste --primary
    if let Ok(output) = Command::new("wl-paste")
        .arg("--primary")
        .arg("--no-newline")
        .output()
        && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout) {
                return text;
            }

    // 2. Try xclip -selection primary
    if let Ok(output) = Command::new("xclip")
        .arg("-selection")
        .arg("primary")
        .arg("-o")
        .output()
        && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout) {
                return text;
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
        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };

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
