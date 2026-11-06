use std::collections::VecDeque;
use std::os::fd::BorrowedFd;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use wayland_client::Connection;
use wayland_client::protocol::wl_data_offer::WlDataOffer;

use crate::clipboard::clipboard::ClipboardProvider;

pub struct WaylandClipboardProvider {
    pub clipboard_text: Arc<RwLock<String>>,
    pub primary_text: Arc<RwLock<String>>,
    pub copy_pending_requests: Arc<Mutex<VecDeque<String>>>,
    pub conn: Connection,
    pub active_offer: Arc<RwLock<Option<WlDataOffer>>>,
    pub active_mime_types: Arc<RwLock<Vec<String>>>,
}

impl WaylandClipboardProvider {
    pub fn new(
        clipboard_text: Arc<RwLock<String>>,
        primary_text: Arc<RwLock<String>>,
        copy_pending_requests: Arc<Mutex<VecDeque<String>>>,
        conn: Connection,
        active_offer: Arc<RwLock<Option<WlDataOffer>>>,
        active_mime_types: Arc<RwLock<Vec<String>>>,
    ) -> Self {
        Self {
            clipboard_text,
            primary_text,
            copy_pending_requests,
            conn,
            active_offer,
            active_mime_types,
        }
    }

    fn read_from_offer(&self) -> Option<String> {
        let offer = self.active_offer.read().ok()?.clone()?;
        let mimes = self.active_mime_types.read().ok()?.clone();

        let chosen_mime = mimes
            .iter()
            .find(|m| {
                *m == "text/plain;charset=utf-8" || *m == "text/plain" || *m == "UTF8_STRING"
            })?
            .clone();

        let mut fds = [0i32; 2];
        let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        if res != 0 {
            return None;
        }

        let read_fd = fds[0];
        let write_fd = fds[1];

        offer.receive(chosen_mime, unsafe { BorrowedFd::borrow_raw(write_fd) });
        unsafe { libc::close(write_fd) };
        let _ = self.conn.flush();

        // Use poll with 150ms timeout to avoid hanging if the source application stalls
        let mut pollfd = libc::pollfd {
            fd: read_fd,
            events: libc::POLLIN,
            revents: 0,
        };

        let start = Instant::now();
        let timeout_ms = 150u64;
        let mut data = Vec::new();

        while start.elapsed() < Duration::from_millis(timeout_ms) {
            let remaining =
                timeout_ms.saturating_sub(start.elapsed().as_millis() as u64) as libc::c_int;
            let ret = unsafe { libc::poll(&mut pollfd, 1, remaining.max(1)) };
            if ret > 0 && (pollfd.revents & (libc::POLLIN | libc::POLLHUP)) != 0 {
                let mut buf = [0u8; 4096];
                let bytes_read = unsafe {
                    libc::read(read_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                };
                if bytes_read > 0 {
                    data.extend_from_slice(&buf[..bytes_read as usize]);
                } else {
                    // EOF reached
                    break;
                }
            } else if ret == 0 {
                // Timeout
                break;
            } else {
                break;
            }
        }

        unsafe { libc::close(read_fd) };

        if !data.is_empty() {
            String::from_utf8(data).ok()
        } else {
            None
        }
    }
}

impl ClipboardProvider for WaylandClipboardProvider {
    fn copy(&self, text: &str) {
        if let Ok(mut lock) = self.clipboard_text.write() {
            *lock = text.to_string();
        }
        if let Ok(mut queue) = self.copy_pending_requests.lock() {
            queue.push_back(text.to_string());
        }
        let _ = self.conn.flush();
    }

    fn copy_primary(&self, text: &str) {
        if let Ok(mut lock) = self.primary_text.write() {
            *lock = text.to_string();
        }
    }

    fn paste(&self) -> Option<String> {
        if let Some(text) = self.read_from_offer() {
            if !text.is_empty() {
                return Some(text);
            }
        }
        let cached = self.clipboard_text.read().ok()?.clone();
        if !cached.is_empty() {
            Some(cached)
        } else {
            None
        }
    }

    fn primary_selection(&self) -> Option<String> {
        let cached = self.primary_text.read().ok()?.clone();
        if !cached.is_empty() {
            Some(cached)
        } else {
            None
        }
    }
}
