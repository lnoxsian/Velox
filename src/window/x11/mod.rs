pub mod icon;
pub mod keysym;
pub mod shm;

use super::event::{CursorIcon, ElementState, ModifiersState, MouseButton, PlatformEvent};
use crate::platform::{CANONICAL_WM_CLASS_GENERAL, CANONICAL_WM_CLASS_INSTANCE};
use icon::get_net_wm_icon_data;
use shm::X11Framebuffer;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    self, AtomEnum, ColormapAlloc, ConnectionExt as _, CreateWindowAux, EventMask, KeyPressEvent,
    VisualClass, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub struct X11Window {
    pub conn: Arc<RustConnection>,
    pub window_id: xproto::Window,
    pub width: u32,
    pub height: u32,
    pub fb: X11Framebuffer,
    pub gc: xproto::Gcontext,
    pub visual: xproto::Visualid,
    pub depth: u8,
    pub colormap: Option<xproto::Colormap>,
    pub wm_delete_window: xproto::Atom,
    pub net_wm_name: xproto::Atom,
    pub net_wm_icon_name: xproto::Atom,
    pub utf8_string: xproto::Atom,
    pub modifiers: ModifiersState,
}

/// Find a 32-bit TrueColor visual supporting ARGB compositing with per-pixel alpha.
pub fn find_32bit_visual(
    conn: &RustConnection,
    screen_num: usize,
) -> Option<(xproto::Visualid, u8)> {
    let screen = conn.setup().roots.get(screen_num)?;
    for depth in &screen.allowed_depths {
        if depth.depth == 32 {
            for visual in &depth.visuals {
                if visual.class == VisualClass::TRUE_COLOR {
                    return Some((visual.visual_id, depth.depth));
                }
            }
        }
    }
    None
}

impl X11Window {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let (conn, screen_num) = RustConnection::connect(None)?;
        let conn = Arc::new(conn);
        let root = conn.setup().roots[screen_num].root;
        let root_visual = conn.setup().roots[screen_num].root_visual;
        let black_pixel = conn.setup().roots[screen_num].black_pixel;
        let window_id = conn.generate_id()?;
        let gc = conn.generate_id()?;

        let win_width = width.max(100);
        let win_height = height.max(100);

        let event_mask = EventMask::EXPOSURE
            | EventMask::STRUCTURE_NOTIFY
            | EventMask::KEY_PRESS
            | EventMask::KEY_RELEASE
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION
            | EventMask::FOCUS_CHANGE;

        // Try to find a 32-bit ARGB TrueColor visual for true alpha transparency
        let (visual, depth, colormap) =
            if let Some((vis32, depth32)) = find_32bit_visual(&conn, screen_num) {
                let colormap = conn.generate_id()?;
                conn.create_colormap(ColormapAlloc::NONE, colormap, root, vis32)?;
                (vis32, depth32, Some(colormap))
            } else {
                (root_visual, 24u8, None)
            };

        let mut win_aux = CreateWindowAux::new().event_mask(event_mask);
        if let Some(cmap) = colormap {
            // When creating a window with depth different from root (e.g. 32 on 24-bit root),
            // X11 protocol mandates colormap and border_pixel to avoid BadMatch.
            win_aux = win_aux
                .colormap(cmap)
                .border_pixel(0)
                .background_pixel(0);
        } else {
            win_aux = win_aux.background_pixel(black_pixel);
        }

        conn.create_window(
            depth,
            window_id,
            root,
            0,
            0,
            win_width as u16,
            win_height as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &win_aux,
        )?;

        conn.create_gc(gc, window_id, &xproto::CreateGCAux::new())?;

        // Intern atoms for window properties
        let net_wm_name = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let net_wm_icon_name = conn.intern_atom(false, b"_NET_WM_ICON_NAME")?.reply()?.atom;
        let net_wm_icon = conn.intern_atom(false, b"_NET_WM_ICON")?.reply()?.atom;
        let net_wm_pid = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
        let utf8_string = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;

        // Set EWMH & ICCCM Title
        conn.change_property8(
            xproto::PropMode::REPLACE,
            window_id,
            net_wm_name,
            utf8_string,
            title.as_bytes(),
        )?;
        conn.change_property8(
            xproto::PropMode::REPLACE,
            window_id,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;

        // Set EWMH & ICCCM Icon Name
        conn.change_property8(
            xproto::PropMode::REPLACE,
            window_id,
            net_wm_icon_name,
            utf8_string,
            title.as_bytes(),
        )?;
        conn.change_property8(
            xproto::PropMode::REPLACE,
            window_id,
            AtomEnum::WM_ICON_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;

        // Set WM_CLASS: instance_name \0 class_name \0 ("velox\0Velox\0")
        let mut class_bytes = Vec::new();
        class_bytes.extend_from_slice(CANONICAL_WM_CLASS_INSTANCE.as_bytes());
        class_bytes.push(0);
        class_bytes.extend_from_slice(CANONICAL_WM_CLASS_GENERAL.as_bytes());
        class_bytes.push(0);
        conn.change_property8(
            xproto::PropMode::REPLACE,
            window_id,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            &class_bytes,
        )?;

        // Set _NET_WM_PID
        let pid = std::process::id();
        conn.change_property32(
            xproto::PropMode::REPLACE,
            window_id,
            net_wm_pid,
            AtomEnum::CARDINAL,
            &[pid],
        )?;

        // Set _NET_WM_ICON (embedded multi-resolution ARGB icons)
        let icon_data = get_net_wm_icon_data();
        if !icon_data.is_empty() {
            conn.change_property32(
                xproto::PropMode::REPLACE,
                window_id,
                net_wm_icon,
                AtomEnum::CARDINAL,
                icon_data,
            )?;
        }

        // Set WM_HINTS (input = true, normal state)
        conn.change_property32(
            xproto::PropMode::REPLACE,
            window_id,
            AtomEnum::WM_HINTS,
            AtomEnum::WM_HINTS,
            &[3, 1, 1, 0, 0, 0, 0, 0, window_id],
        )?;

        // Handle WM_DELETE_WINDOW
        let wm_protocols = conn.intern_atom(false, b"WM_PROTOCOLS")?.reply()?.atom;
        let wm_delete_window = conn.intern_atom(false, b"WM_DELETE_WINDOW")?.reply()?.atom;
        conn.change_property32(
            xproto::PropMode::REPLACE,
            window_id,
            wm_protocols,
            AtomEnum::ATOM,
            &[wm_delete_window],
        )?;

        conn.map_window(window_id)?;
        conn.flush()?;

        let fb = X11Framebuffer::new(&conn, win_width, win_height, depth);

        Ok(Self {
            conn,
            window_id,
            width: win_width,
            height: win_height,
            fb,
            gc,
            visual,
            depth,
            colormap,
            wm_delete_window,
            net_wm_name,
            net_wm_icon_name,
            utf8_string,
            modifiers: ModifiersState::empty(),
        })
    }

    pub fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.conn.stream().as_raw_fd()
    }

    pub fn set_title(&self, title: &str) {
        let _ = self.conn.change_property8(
            xproto::PropMode::REPLACE,
            self.window_id,
            self.net_wm_name,
            self.utf8_string,
            title.as_bytes(),
        );
        let _ = self.conn.change_property8(
            xproto::PropMode::REPLACE,
            self.window_id,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        );
        let _ = self.conn.change_property8(
            xproto::PropMode::REPLACE,
            self.window_id,
            self.net_wm_icon_name,
            self.utf8_string,
            title.as_bytes(),
        );
        let _ = self.conn.change_property8(
            xproto::PropMode::REPLACE,
            self.window_id,
            AtomEnum::WM_ICON_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        );
        let _ = self.conn.flush();
    }

    pub fn set_cursor(&self, _icon: CursorIcon) {
        // Standard X11 font cursor or xc_cursor can be bound without C libraries if desired
    }

    pub fn set_opacity(&self, opacity: f32) {
        if self.colormap.is_none() {
            // Depth 24 fallback: set _NET_WM_WINDOW_OPACITY for window-level opacity
            if let Ok(cookie) = self.conn.intern_atom(false, b"_NET_WM_WINDOW_OPACITY")
                && let Ok(reply) = cookie.reply()
            {
                let op = opacity.clamp(0.0, 1.0);
                let cardinal = (op * (u32::MAX as f32)).round() as u32;
                let _ = self.conn.change_property32(
                    xproto::PropMode::REPLACE,
                    self.window_id,
                    reply.atom,
                    AtomEnum::CARDINAL,
                    &[cardinal],
                );
                let _ = self.conn.flush();
            }
        }
    }

    pub fn present_frame(&mut self, src_pixels: &[u32]) {
        let dest = self.fb.pixels_mut();
        if dest.len() == src_pixels.len() {
            dest.copy_from_slice(src_pixels);
        }
        self.fb
            .present(&self.conn, self.window_id, self.gc, self.visual);
    }

    pub fn poll_event(&mut self) -> Option<PlatformEvent> {
        let raw_event = self.conn.poll_for_event().ok()??;
        match raw_event {
            x11rb::protocol::Event::ConfigureNotify(ev) => {
                let w = ev.width as u32;
                let h = ev.height as u32;
                if w != self.width || h != self.height {
                    self.width = w;
                    self.height = h;
                    self.fb.resize(&self.conn, w, h);
                    return Some(PlatformEvent::Resized {
                        width: w,
                        height: h,
                    });
                }
                None
            }
            x11rb::protocol::Event::Expose(ev) => {
                if ev.count == 0 {
                    return Some(PlatformEvent::RedrawRequested);
                }
                None
            }
            x11rb::protocol::Event::ClientMessage(ev) => {
                if ev.data.as_data32()[0] == self.wm_delete_window {
                    return Some(PlatformEvent::CloseRequested);
                }
                None
            }
            x11rb::protocol::Event::KeyPress(ev) => {
                self.update_modifiers(ev.state);
                let (key, text) = self.lookup_key(&ev);
                if let Some(key) = key {
                    Some(PlatformEvent::KeyPressed {
                        key,
                        text,
                        modifiers: self.modifiers,
                    })
                } else {
                    None
                }
            }
            x11rb::protocol::Event::KeyRelease(ev) => {
                self.update_modifiers(ev.state);
                let (key, _) = self.lookup_key(&KeyPressEvent {
                    detail: ev.detail,
                    state: ev.state,
                    time: ev.time,
                    root: ev.root,
                    event: ev.event,
                    child: ev.child,
                    root_x: ev.root_x,
                    root_y: ev.root_y,
                    event_x: ev.event_x,
                    event_y: ev.event_y,
                    same_screen: ev.same_screen,
                    response_type: ev.response_type,
                    sequence: ev.sequence,
                });
                if let Some(key) = key {
                    Some(PlatformEvent::KeyReleased {
                        key,
                        modifiers: self.modifiers,
                    })
                } else {
                    None
                }
            }
            x11rb::protocol::Event::MotionNotify(ev) => Some(PlatformEvent::CursorMoved {
                x: ev.event_x as f64,
                y: ev.event_y as f64,
            }),
            x11rb::protocol::Event::ButtonPress(ev) => match ev.detail {
                4 => Some(PlatformEvent::MouseWheel { delta_y: 1.0 }),
                5 => Some(PlatformEvent::MouseWheel { delta_y: -1.0 }),
                1 => Some(PlatformEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                }),
                2 => Some(PlatformEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Middle,
                }),
                3 => Some(PlatformEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Right,
                }),
                other => Some(PlatformEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Other(other as u16),
                }),
            },
            x11rb::protocol::Event::ButtonRelease(ev) => match ev.detail {
                1 => Some(PlatformEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                }),
                2 => Some(PlatformEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Middle,
                }),
                3 => Some(PlatformEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Right,
                }),
                4 | 5 => None,
                other => Some(PlatformEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Other(other as u16),
                }),
            },
            x11rb::protocol::Event::FocusIn(_) => Some(PlatformEvent::Focused(true)),
            x11rb::protocol::Event::FocusOut(_) => Some(PlatformEvent::Focused(false)),
            _ => None,
        }
    }

    fn update_modifiers(&mut self, state: xproto::KeyButMask) {
        let mut m = ModifiersState::empty();
        let val: u16 = state.into();
        if val & xproto::ModMask::SHIFT.bits() != 0 {
            m |= ModifiersState::SHIFT;
        }
        if val & xproto::ModMask::CONTROL.bits() != 0 {
            m |= ModifiersState::CONTROL;
        }
        if val & xproto::ModMask::M1.bits() != 0 {
            m |= ModifiersState::ALT;
        }
        if val & xproto::ModMask::M4.bits() != 0 {
            m |= ModifiersState::SUPER;
        }
        self.modifiers = m;
    }

    fn lookup_key(&self, ev: &KeyPressEvent) -> (Option<super::event::Key>, Option<String>) {
        let min_keycode = self.conn.setup().min_keycode;
        let max_keycode = self.conn.setup().max_keycode;
        if ev.detail < min_keycode || ev.detail > max_keycode {
            return (None, None);
        }

        // Fetch keyboard mapping row for keycode
        let count = max_keycode - min_keycode + 1;
        if let Ok(cookie) = self.conn.get_keyboard_mapping(min_keycode, count)
            && let Ok(reply) = cookie.reply()
        {
            let keysyms_per_code = reply.keysyms_per_keycode as usize;
            let index = (ev.detail - min_keycode) as usize * keysyms_per_code;
            if index < reply.keysyms.len() {
                let is_shift = self.modifiers.contains(ModifiersState::SHIFT);
                let col = if is_shift && keysyms_per_code > 1 {
                    1
                } else {
                    0
                };
                let mut keysym = reply.keysyms[index + col];
                if keysym == 0 && col == 1 {
                    keysym = reply.keysyms[index];
                }
                return keysym::keysym_to_key(keysym);
            }
        }
        (None, None)
    }
}

impl Drop for X11Window {
    fn drop(&mut self) {
        if let Some(cmap) = self.colormap.take() {
            let _ = self.conn.free_colormap(cmap);
        }
        let _ = self.conn.destroy_window(self.window_id);
        let _ = self.conn.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x11_32bit_visual_detection() {
        if let Ok((conn, screen_num)) = RustConnection::connect(None) {
            let vis32 = find_32bit_visual(&conn, screen_num);
            if let Some((visual_id, depth)) = vis32 {
                assert_eq!(depth, 32);
                assert_ne!(visual_id, 0);

                let win = X11Window::new("Velox ARGB32 Test", 300, 200);
                assert!(
                    win.is_ok(),
                    "X11 window creation with 32-bit visual failed: {:?}",
                    win.err()
                );
                let mut win = win.unwrap();
                assert_eq!(win.depth, 32);
                assert!(win.colormap.is_some());
                assert_eq!(win.fb.depth, 32);

                // Present a frame with semi-transparent ARGB pixels
                let frame = vec![0x80123456u32; (win.width * win.height) as usize];
                win.present_frame(&frame);
            }
        }
    }
}

