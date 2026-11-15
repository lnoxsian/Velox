use std::collections::VecDeque;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Entry in the clipboard history ring with timestamp.
#[derive(Debug, Clone)]
pub struct ClipboardHistoryEntry {
    pub text: String,
    pub timestamp: Instant,
}

/// Central Terminator-style clipboard manager.
pub struct ClipboardManager {
    clipboard: Mutex<String>,
    primary: Mutex<String>,
    clipboard_time: Mutex<Instant>,
    primary_time: Mutex<Instant>,
    history: Mutex<VecDeque<ClipboardHistoryEntry>>,
    history_limit: Mutex<usize>,
    owns_clipboard: AtomicBool,
    owns_primary: AtomicBool,
    middle_click_prefers_primary: AtomicBool,
    save_primary_to_history: AtomicBool,
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new(50)
    }
}

impl ClipboardManager {
    pub fn new(history_limit: usize) -> Self {
        let now = Instant::now();
        Self {
            clipboard: Mutex::new(String::new()),
            primary: Mutex::new(String::new()),
            clipboard_time: Mutex::new(now),
            primary_time: Mutex::new(now),
            history: Mutex::new(VecDeque::with_capacity(history_limit)),
            history_limit: Mutex::new(history_limit.max(1)),
            owns_clipboard: AtomicBool::new(false),
            owns_primary: AtomicBool::new(false),
            middle_click_prefers_primary: AtomicBool::new(false),
            save_primary_to_history: AtomicBool::new(false),
        }
    }

    /// Global singleton instance for application-wide clipboard management.
    pub fn global() -> &'static ClipboardManager {
        static INSTANCE: OnceLock<ClipboardManager> = OnceLock::new();
        INSTANCE.get_or_init(ClipboardManager::default)
    }

    pub fn set_owns_clipboard(&self, owns: bool) {
        self.owns_clipboard.store(owns, Ordering::SeqCst);
    }

    pub fn set_owns_primary(&self, owns: bool) {
        self.owns_primary.store(owns, Ordering::SeqCst);
    }

    pub fn notify_external_clipboard_claimed(&self) {
        self.set_owns_clipboard(false);
        *self.clipboard_time.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    pub fn notify_external_primary_claimed(&self) {
        self.set_owns_primary(false);
        *self.primary_time.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    pub fn owns_clipboard(&self) -> bool {
        self.owns_clipboard.load(Ordering::SeqCst)
    }

    pub fn owns_primary(&self) -> bool {
        self.owns_primary.load(Ordering::SeqCst)
    }

    pub fn set_middle_click_prefers_primary(&self, prefer: bool) {
        self.middle_click_prefers_primary.store(prefer, Ordering::SeqCst);
    }

    pub fn middle_click_prefers_primary(&self) -> bool {
        self.middle_click_prefers_primary.load(Ordering::SeqCst)
    }

    pub fn set_save_primary_to_history(&self, enable: bool) {
        self.save_primary_to_history.store(enable, Ordering::SeqCst);
    }

    pub fn save_primary_to_history(&self) -> bool {
        self.save_primary_to_history.load(Ordering::SeqCst)
    }

    /// Set the maximum number of items retained in the clipboard history ring.
    pub fn set_history_limit(&self, limit: usize) {
        let limit = limit.max(1);
        *self.history_limit.lock().unwrap_or_else(|e| e.into_inner()) = limit;
        let mut hist = self.history.lock().unwrap_or_else(|e| e.into_inner());
        while hist.len() > limit {
            hist.pop_back();
        }
    }

    /// Push text into the clipboard history ring, deduplicating identical consecutive entries.
    fn push_history(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let limit = *self.history_limit.lock().unwrap_or_else(|e| e.into_inner());
        let mut hist = self.history.lock().unwrap_or_else(|e| e.into_inner());
        // Deduplicate if identical to most recent entry
        if let Some(front) = hist.front() {
            if front.text == text {
                return;
            }
        }
        hist.push_front(ClipboardHistoryEntry {
            text: text.to_string(),
            timestamp: Instant::now(),
        });
        while hist.len() > limit {
            hist.pop_back();
        }
    }

    /// Retrieve all text entries currently stored in the clipboard history (newest first).
    pub fn get_history(&self) -> Vec<String> {
        let hist = self.history.lock().unwrap_or_else(|e| e.into_inner());
        hist.iter().map(|e| e.text.clone()).collect()
    }

    /// Clear all entries from the clipboard history ring.
    pub fn clear_history(&self) {
        self.history.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Write text to the system CLIPBOARD selection (Ctrl+Shift+C / Ctrl+Insert / OSC 52).
    pub fn copy(&self, text: String) {
        if text.is_empty() {
            return;
        }
        *self.clipboard.lock().unwrap_or_else(|e| e.into_inner()) = text.clone();
        *self.primary.lock().unwrap_or_else(|e| e.into_inner()) = text.clone();
        let now = Instant::now();
        *self.clipboard_time.lock().unwrap_or_else(|e| e.into_inner()) = now;
        *self.primary_time.lock().unwrap_or_else(|e| e.into_inner()) = now;
        self.set_owns_clipboard(true);
        self.set_owns_primary(true);
        self.push_history(&text);

        let text_bytes = text.into_bytes();
        let has_wayland = std::env::var_os("WAYLAND_DISPLAY")
            .map(|v| !v.is_empty())
            .unwrap_or(false);

        // Run external clipboard tool synchronization asynchronously in a background thread
        // so that slow or blocking subprocesses never freeze the terminal UI thread.
        // Note: On native X11, Velox's X11Window natively owns the selection and serves
        // SelectionRequest directly; spawning xsel/xclip to claim the selection would steal
        // ownership away from Velox and cause self-contention SelectionClear events.
        std::thread::spawn(move || {
            let mut written_system = false;

            // 1. Wayland clipboard & primary
            if has_wayland {
                let ok = write_command_input(
                    "wl-copy",
                    &["--type", "text/plain;charset=utf-8"],
                    &text_bytes,
                );
                if !ok {
                    let _ = write_command_input("wl-copy", &[], &text_bytes);
                }
                let _ = write_command_input(
                    "wl-copy",
                    &["--primary", "--type", "text/plain;charset=utf-8"],
                    &text_bytes,
                );
                if ok {
                    written_system = true;
                }
            }

            // 2. Desktop Clipboard Managers (CopyQ, GPaste, Termux fallback when system tools are missing)
            if !written_system {
                let _ = write_command_input("copyq", &["copy", "-"], &text_bytes);
                let _ = write_command_input("copyq", &["selection", "-"], &text_bytes);
                let _ = write_command_input("gpaste-client", &["add"], &text_bytes);
                let _ = write_command_input("termux-clipboard-set", &[], &text_bytes);
            }
        });
    }

    /// Write text to PRIMARY selection only (mouse highlight/selection).
    /// Does not overwrite CLIPBOARD, preserving text copied from browsers or editors.
    pub fn copy_primary(&self, text: String) {
        if text.is_empty() {
            return;
        }
        *self.primary.lock().unwrap_or_else(|e| e.into_inner()) = text.clone();
        let now = Instant::now();
        *self.primary_time.lock().unwrap_or_else(|e| e.into_inner()) = now;
        self.set_owns_primary(true);
        if self.save_primary_to_history.load(Ordering::SeqCst) {
            self.push_history(&text);
        }

        let text_bytes = text.into_bytes();
        let has_wayland = std::env::var_os("WAYLAND_DISPLAY")
            .map(|v| !v.is_empty())
            .unwrap_or(false);

        std::thread::spawn(move || {
            let mut written_system = false;
            if has_wayland {
                let ok = write_command_input(
                    "wl-copy",
                    &["--primary", "--type", "text/plain;charset=utf-8"],
                    &text_bytes,
                );
                if !ok {
                    let _ = write_command_input("wl-copy", &["--primary"], &text_bytes);
                }
                if ok {
                    written_system = true;
                }
            }

            if !written_system {
                let _ = write_command_input("copyq", &["selection", "-"], &text_bytes);
            }
        });
    }

    /// Retrieve text from system CLIPBOARD selection (Ctrl+Shift+V / Shift+Insert).
    pub fn paste(&self) -> String {
        // If this window currently owns the clipboard selection,
        // we already hold the most recent text in internal memory.
        // Querying xsel would cause a self-deadlock with our own event loop!
        // Even if the internal clipboard is empty, do NOT query the system —
        // the system would forward the request back to us, deadlocking the event loop.
        if self.owns_clipboard() {
            return self.get_internal_clipboard();
        }

        let has_wayland = std::env::var_os("WAYLAND_DISPLAY")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        let has_x11 = std::env::var_os("DISPLAY")
            .map(|v| !v.is_empty())
            .unwrap_or(false);

        // 1. Wayland clipboard
        if has_wayland {
            if let Some(text) = read_command_output(
                "wl-paste",
                &["--type", "text/plain;charset=utf-8", "--no-newline"],
            ) {
                self.update_internal_clipboard(&text);
                return text;
            }
            if let Some(text) =
                read_command_output("wl-paste", &["--type", "text/plain", "--no-newline"])
            {
                self.update_internal_clipboard(&text);
                return text;
            }
            if let Some(text) = read_command_output("wl-paste", &["--no-newline"]) {
                self.update_internal_clipboard(&text);
                return text;
            }
        }

        // 2. X11 clipboard
        if has_x11 {
            if let Some(text) = read_command_output(
                "xclip",
                &["-selection", "clipboard", "-t", "UTF8_STRING", "-o"],
            ) {
                self.update_internal_clipboard(&text);
                return text;
            }
            if let Some(text) = read_command_output(
                "xclip",
                &[
                    "-selection",
                    "clipboard",
                    "-t",
                    "text/plain;charset=utf-8",
                    "-o",
                ],
            ) {
                self.update_internal_clipboard(&text);
                return text;
            }
            if let Some(text) = read_command_output("xclip", &["-selection", "clipboard", "-o"]) {
                self.update_internal_clipboard(&text);
                return text;
            }

            if let Some(text) = read_command_output(
                "xsel",
                &["--clipboard", "--output", "--selectionTimeout", "300"],
            ) {
                self.update_internal_clipboard(&text);
                return text;
            }
        }

        // 3. Desktop Clipboard Managers
        if let Some(text) = read_command_output("copyq", &["clipboard"]) {
            self.update_internal_clipboard(&text);
            return text;
        }
        if let Some(text) = read_command_output("copyq", &["read", "0"]) {
            self.update_internal_clipboard(&text);
            return text;
        }
        if let Some(text) = read_command_output("gpaste-client", &["get", "0"]) {
            self.update_internal_clipboard(&text);
            return text;
        }
        if let Some(text) = read_command_output(
            "qdbus",
            &["org.kde.klipper", "/klipper", "getClipboardContents"],
        ) {
            self.update_internal_clipboard(&text);
            return text;
        }
        if let Some(text) = read_command_output("termux-clipboard-get", &[]) {
            self.update_internal_clipboard(&text);
            return text;
        }

        // 4. In-process fallback clipboard
        let internal = self.get_internal_clipboard();
        if !internal.is_empty() {
            return internal;
        }

        // 5. Fallback: check PRIMARY selection
        self.primary_selection()
    }

    /// Retrieve text from system PRIMARY selection (mouse highlight/selection).
    pub fn primary_selection(&self) -> String {
        // If this window currently owns the primary selection,
        // return internal primary directly without spawning xsel to avoid self-deadlock.
        // Even if the internal primary is empty we must not query the system — the system
        // would just bounce the request back to us, which would deadlock the event loop.
        if self.owns_primary() {
            return self.get_internal_primary();
        }

        let has_wayland = std::env::var_os("WAYLAND_DISPLAY")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        let has_x11 = std::env::var_os("DISPLAY")
            .map(|v| !v.is_empty())
            .unwrap_or(false);

        // 1. Wayland primary selection
        if has_wayland {
            if let Some(text) = read_command_output(
                "wl-paste",
                &[
                    "--primary",
                    "--type",
                    "text/plain;charset=utf-8",
                    "--no-newline",
                ],
            ) {
                self.update_internal_primary(&text);
                return text;
            }
            if let Some(text) = read_command_output(
                "wl-paste",
                &["--primary", "--type", "text/plain", "--no-newline"],
            ) {
                self.update_internal_primary(&text);
                return text;
            }
            if let Some(text) = read_command_output("wl-paste", &["--primary", "--no-newline"]) {
                self.update_internal_primary(&text);
                return text;
            }
        }

        // 2. X11 primary selection
        if has_x11 {
            if let Some(text) = read_command_output(
                "xclip",
                &["-selection", "primary", "-t", "UTF8_STRING", "-o"],
            ) {
                self.update_internal_primary(&text);
                return text;
            }
            if let Some(text) = read_command_output(
                "xclip",
                &[
                    "-selection",
                    "primary",
                    "-t",
                    "text/plain;charset=utf-8",
                    "-o",
                ],
            ) {
                self.update_internal_primary(&text);
                return text;
            }
            if let Some(text) = read_command_output("xclip", &["-selection", "primary", "-o"]) {
                self.update_internal_primary(&text);
                return text;
            }

            if let Some(text) = read_command_output(
                "xsel",
                &["--primary", "--output", "--selectionTimeout", "300"],
            ) {
                self.update_internal_primary(&text);
                return text;
            }
        }

        // 3. Desktop Clipboard Managers (CopyQ selection)
        if let Some(text) = read_command_output("copyq", &["selection"]) {
            self.update_internal_primary(&text);
            return text;
        }

        // 4. In-process fallback primary
        self.get_internal_primary()
    }

    /// Retrieve the appropriate text for middle-click paste.
    /// If `middle_click_prefers_primary` is enabled, strictly returns PRIMARY (falling back to CLIPBOARD if empty).
    /// Otherwise follows Terminator-style behavior: chooses the latest copy or selection action between CLIPBOARD and PRIMARY.
    pub fn middle_click_paste(&self) -> String {
        if self.middle_click_prefers_primary.load(Ordering::SeqCst) {
            let prim = self.primary_selection();
            if !prim.is_empty() {
                return prim;
            }
            return self.paste();
        }

        let clip = self.paste();
        let prim = self.primary_selection();

        if clip.is_empty() {
            return prim;
        }
        if prim.is_empty() {
            return clip;
        }

        let clip_time = *self.clipboard_time.lock().unwrap_or_else(|e| e.into_inner());
        let prim_time = *self.primary_time.lock().unwrap_or_else(|e| e.into_inner());

        if clip_time > prim_time { clip } else { prim }
    }

    fn update_internal_clipboard(&self, text: &str) {
        let mut lock = self.clipboard.lock().unwrap_or_else(|e| e.into_inner());
        let changed = *lock != text;
        if changed {
            *lock = text.to_string();
        }
        if changed || !self.owns_clipboard() {
            *self.clipboard_time.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        }
        drop(lock);
        self.push_history(text);
    }

    fn update_internal_primary(&self, text: &str) {
        let mut lock = self.primary.lock().unwrap_or_else(|e| e.into_inner());
        let changed = *lock != text;
        if changed {
            *lock = text.to_string();
        }
        if changed || !self.owns_primary() {
            *self.primary_time.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        }
    }

    pub fn get_internal_clipboard(&self) -> String {
        self.clipboard.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn get_internal_primary(&self) -> String {
        self.primary.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

fn command_exists(cmd: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let full_path = dir.join(cmd);
            if full_path.is_file() {
                return true;
            }
        }
    }
    false
}

struct CachedCmd {
    available: AtomicBool,
    last_check: Mutex<Option<Instant>>,
}

impl CachedCmd {
    const fn new() -> Self {
        Self {
            available: AtomicBool::new(false),
            last_check: Mutex::new(None),
        }
    }

    fn check(&self, cmd: &str) -> bool {
        if self.available.load(Ordering::Relaxed) {
            return true;
        }
        let now = Instant::now();
        let mut last = self.last_check.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(prev) = *last {
            if now.duration_since(prev) < std::time::Duration::from_secs(5) {
                return false;
            }
        }
        *last = Some(now);
        let exists = command_exists(cmd);
        if exists {
            self.available.store(true, Ordering::Relaxed);
        }
        exists
    }
}

fn has_command(cmd: &str) -> bool {
    match cmd {
        "xclip" => {
            static CMD: CachedCmd = CachedCmd::new();
            CMD.check("xclip")
        }
        "xsel" => {
            static CMD: CachedCmd = CachedCmd::new();
            CMD.check("xsel")
        }
        "wl-paste" => {
            static CMD: CachedCmd = CachedCmd::new();
            CMD.check("wl-paste")
        }
        "wl-copy" => {
            static CMD: CachedCmd = CachedCmd::new();
            CMD.check("wl-copy")
        }
        _ => command_exists(cmd),
    }
}

fn read_command_output(cmd: &str, args: &[&str]) -> Option<String> {
    if !has_command(cmd) {
        return None;
    }
    use std::io::Read;
    use std::os::unix::io::AsRawFd;

    let mut command = Command::new(cmd);
    command.args(args);
    command.env("LC_ALL", "C.UTF-8");
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::null());

    let mut child = command.spawn().ok()?;
    let stdout = child.stdout.take()?;
    let fd = stdout.as_raw_fd();

    // Use libc::poll with a 300ms timeout to avoid hanging the UI thread
    let mut poll_fd = libc::pollfd {
        fd,
        events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
        revents: 0,
    };

    let ret = unsafe { libc::poll(&mut poll_fd, 1, 300) };
    if ret <= 0 {
        // poll error or timeout — kill child immediately to avoid blocking on read
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }

    let mut bytes = Vec::new();
    // read_to_end returns immediately on EOF (pipe closed by child after it exits).
    if stdout.take(10 * 1024 * 1024).read_to_end(&mut bytes).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }

    // Reap child. We intentionally ignore the exit status: tools like xclip and xsel
    // return non-zero when the selection is owned by another client, yet they still
    // successfully emit the selection text to stdout.
    let _ = child.wait();

    if bytes.is_empty() {
        return None;
    }

    let text = String::from_utf8_lossy(&bytes)
        .trim_matches('\0')
        .to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn write_command_input_env(cmd: &str, args: &[&str], input: &[u8], envs: &[(&str, &str)]) -> bool {
    if !has_command(cmd) {
        return false;
    }
    let mut command = Command::new(cmd);
    command.args(args);
    for (k, v) in envs {
        command.env(k, v);
    }
    command.stdin(Stdio::piped());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Write to stdin then explicitly close it (drop) before waiting.
    // Clipboard tools (wl-copy, xclip, xsel) read until EOF — if we don't
    // close stdin first, child.wait() will deadlock waiting for the tool to
    // exit, while the tool is waiting for more input.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input);
        // stdin is dropped here → EOF is sent to the child process
    }

    // Blocking wait; clipboard write tools exit almost immediately after
    // receiving their input. This is safe on the UI thread for typical
    // clipboard sizes; for very large data the OS pipe buffer absorbs it.
    match child.wait() {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

fn write_command_input(cmd: &str, args: &[&str], input: &[u8]) -> bool {
    write_command_input_env(cmd, args, input, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_poisoned_mutex_recovery() {
        let manager = Arc::new(ClipboardManager::new(10));
        manager.copy("healthy before panic".to_string());

        let m_clone = Arc::clone(&manager);
        let _ = std::panic::catch_unwind(move || {
            let _lock = m_clone.clipboard.lock().unwrap();
            panic!("deliberate panic while holding clipboard lock");
        });

        // The mutex is poisoned now, but manager operations should recover gracefully
        assert!(manager.clipboard.is_poisoned());
        manager.copy("recovered after panic".to_string());
        assert_eq!(manager.get_internal_clipboard(), "recovered after panic");
    }

    #[test]
    fn test_middle_click_prefers_primary_toggle() {
        let manager = ClipboardManager::new(10);
        manager.copy("clipboard_value".to_string());
        manager.copy_primary("primary_value".to_string());

        // Default (latest timestamp - primary was set after copy, so primary)
        assert_eq!(manager.middle_click_paste(), "primary_value");

        // Now update clipboard with newer timestamp
        std::thread::sleep(std::time::Duration::from_millis(5));
        *manager.clipboard.lock().unwrap() = "newer_clipboard".to_string();
        *manager.clipboard_time.lock().unwrap() = Instant::now();

        // Default latest: returns newer_clipboard
        assert_eq!(manager.middle_click_paste(), "newer_clipboard");

        // When middle_click_prefers_primary = true: strictly returns primary
        manager.set_middle_click_prefers_primary(true);
        assert_eq!(manager.middle_click_paste(), "primary_value");

        // If primary is empty, falls back to clipboard
        *manager.primary.lock().unwrap() = String::new();
        assert_eq!(manager.middle_click_paste(), "newer_clipboard");
    }

    #[test]
    fn test_save_primary_to_history_toggle() {
        let manager = ClipboardManager::new(10);
        manager.copy_primary("primary_transient".to_string());
        assert!(manager.get_history().is_empty());

        manager.set_save_primary_to_history(true);
        manager.copy_primary("primary_saved".to_string());
        let hist = manager.get_history();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0], "primary_saved");
    }

    #[test]
    fn test_external_copy_precedence_over_previous_terminal_selection() {
        let manager = ClipboardManager::new(10);
        // Step 1: User selects text in terminal
        manager.copy_primary("terminal output line 1".to_string());
        assert_eq!(manager.middle_click_paste(), "terminal output line 1");

        // Step 2: User copies in external app (e.g. browser).
        // External app claims clipboard -> notify_external_clipboard_claimed
        std::thread::sleep(std::time::Duration::from_millis(5));
        manager.notify_external_clipboard_claimed();
        // External app's clipboard content is fetched during paste
        manager.update_internal_clipboard("https://github.com/lnoxsian/Velox");

        // Middle-click must now paste the external app's copy, NOT the old terminal selection!
        assert_eq!(manager.middle_click_paste(), "https://github.com/lnoxsian/Velox");

        // Step 3: User double-clicks a new word in the terminal
        std::thread::sleep(std::time::Duration::from_millis(5));
        manager.copy_primary("new_selected_word".to_string());

        // Middle-click must now paste the newly selected word!
        assert_eq!(manager.middle_click_paste(), "new_selected_word");
    }
}
