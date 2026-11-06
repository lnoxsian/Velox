use std::collections::VecDeque;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, AtomEnum, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

use crate::clipboard::clipboard::ClipboardProvider;

pub struct X11ClipboardProvider {
    pub conn: Arc<RustConnection>,
    pub window_id: xproto::Window,
    pub clipboard_atom: xproto::Atom,
    pub primary_atom: xproto::Atom,
    pub targets_atom: xproto::Atom,
    pub utf8_string_atom: xproto::Atom,
    pub velox_selection_atom: xproto::Atom,
    pub stored_clipboard: Arc<RwLock<String>>,
    pub stored_primary: Arc<RwLock<String>>,
    pub pending_events: Arc<Mutex<VecDeque<x11rb::protocol::Event>>>,
}

impl X11ClipboardProvider {
    pub fn new(
        conn: Arc<RustConnection>,
        window_id: xproto::Window,
        clipboard_atom: xproto::Atom,
        primary_atom: xproto::Atom,
        targets_atom: xproto::Atom,
        utf8_string_atom: xproto::Atom,
        velox_selection_atom: xproto::Atom,
        pending_events: Arc<Mutex<VecDeque<x11rb::protocol::Event>>>,
    ) -> Self {
        Self {
            conn,
            window_id,
            clipboard_atom,
            primary_atom,
            targets_atom,
            utf8_string_atom,
            velox_selection_atom,
            stored_clipboard: Arc::new(RwLock::new(String::new())),
            stored_primary: Arc::new(RwLock::new(String::new())),
            pending_events,
        }
    }

    fn read_selection(&self, selection: xproto::Atom) -> Option<String> {
        // Fast path: if our window already owns the selection, return the cached string directly
        if let Ok(cookie) = self.conn.get_selection_owner(selection) {
            if let Ok(reply) = cookie.reply() {
                if reply.owner == self.window_id {
                    let text = if selection == self.primary_atom {
                        self.stored_primary.read().unwrap().clone()
                    } else {
                        self.stored_clipboard.read().unwrap().clone()
                    };
                    if !text.is_empty() {
                        return Some(text);
                    }
                } else if reply.owner == 0 {
                    return None;
                }
            }
        }

        // Request selection conversion to UTF8_STRING
        if self
            .conn
            .convert_selection(
                self.window_id,
                selection,
                self.utf8_string_atom,
                self.velox_selection_atom,
                x11rb::CURRENT_TIME,
            )
            .is_err()
        {
            return None;
        }
        let _ = self.conn.flush();

        // Bounded poll for SelectionNotify event (150ms timeout to prevent hanging UI)
        let start = Instant::now();
        let timeout = Duration::from_millis(150);

        while start.elapsed() < timeout {
            match self.conn.poll_for_event() {
                Ok(Some(event)) => match event {
                    x11rb::protocol::Event::SelectionNotify(ev) => {
                        if ev.requestor == self.window_id && ev.selection == selection {
                            if ev.property == AtomEnum::NONE.into() {
                                return None;
                            }
                            if let Ok(cookie) = self.conn.get_property(
                                true, // delete property after retrieval
                                self.window_id,
                                ev.property,
                                AtomEnum::NONE,
                                0,
                                10 * 1024 * 1024,
                            ) {
                                if let Ok(reply) = cookie.reply() {
                                    if let Ok(text) = String::from_utf8(reply.value) {
                                        return Some(text);
                                    }
                                }
                            }
                            return None;
                        } else {
                            // Queue any unrelated events for the main window loop
                            self.pending_events.lock().unwrap().push_back(event);
                        }
                    }
                    other => {
                        self.pending_events.lock().unwrap().push_back(other);
                    }
                },
                Ok(None) => {
                    let conn_fd = self.conn.stream().as_raw_fd();
                    let mut pfd = libc::pollfd {
                        fd: conn_fd,
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let remaining = (timeout.saturating_sub(start.elapsed()).as_millis()
                        as libc::c_int)
                        .clamp(1, 15);
                    unsafe {
                        libc::poll(&mut pfd, 1, remaining);
                    }
                }
                Err(_) => break,
            }
        }

        None
    }
}

impl ClipboardProvider for X11ClipboardProvider {
    fn copy(&self, text: &str) {
        if let Ok(mut lock) = self.stored_clipboard.write() {
            *lock = text.to_string();
        }
        let _ =
            self.conn
                .set_selection_owner(self.window_id, self.clipboard_atom, x11rb::CURRENT_TIME);
        let _ = self.conn.flush();
    }

    fn copy_primary(&self, text: &str) {
        if let Ok(mut lock) = self.stored_primary.write() {
            *lock = text.to_string();
        }
        let _ =
            self.conn
                .set_selection_owner(self.window_id, self.primary_atom, x11rb::CURRENT_TIME);
        let _ = self.conn.flush();
    }

    fn paste(&self) -> Option<String> {
        self.read_selection(self.clipboard_atom)
    }

    fn primary_selection(&self) -> Option<String> {
        self.read_selection(self.primary_atom)
    }
}
