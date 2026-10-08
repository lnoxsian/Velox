pub mod keysym;
pub mod shm;

use super::event::{CursorIcon, ElementState, ModifiersState, MouseButton, PlatformEvent};
use crate::platform::{CANONICAL_APP_ID, CANONICAL_WM_CLASS_INSTANCE};
use shm::X11Framebuffer;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    self, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, KeyPressEvent, WindowClass,
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
    pub wm_delete_window: xproto::Atom,
    pub modifiers: ModifiersState,
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

        let win_aux = CreateWindowAux::new()
            .event_mask(event_mask)
            .background_pixel(black_pixel);

        conn.create_window(
            x11rb::COPY_FROM_PARENT as u8,
            window_id,
            root,
            0,
            0,
            win_width as u16,
            win_height as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            root_visual,
            &win_aux,
        )?;

        conn.create_gc(gc, window_id, &xproto::CreateGCAux::new())?;

        // Set WM_NAME / Title
        let wm_name = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let utf8_string = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
        conn.change_property8(
            xproto::PropMode::REPLACE,
            window_id,
            wm_name,
            utf8_string,
            title.as_bytes(),
        )?;

        // Set WM_CLASS
        let mut class_bytes = Vec::new();
        class_bytes.extend_from_slice(CANONICAL_WM_CLASS_INSTANCE.as_bytes());
        class_bytes.push(0);
        class_bytes.extend_from_slice(CANONICAL_APP_ID.as_bytes());
        class_bytes.push(0);
        conn.change_property8(
            xproto::PropMode::REPLACE,
            window_id,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            &class_bytes,
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

        let fb = X11Framebuffer::new(&conn, win_width, win_height);

        Ok(Self {
            conn,
            window_id,
            width: win_width,
            height: win_height,
            fb,
            gc,
            visual: root_visual,
            wm_delete_window,
            modifiers: ModifiersState::empty(),
        })
    }

    pub fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.conn.stream().as_raw_fd()
    }

    pub fn set_title(&self, title: &str) {
        if let Ok(wm_name) = self.conn.intern_atom(false, b"_NET_WM_NAME")
            && let Ok(reply) = wm_name.reply()
        {
            let _ = self.conn.change_property8(
                xproto::PropMode::REPLACE,
                self.window_id,
                reply.atom,
                AtomEnum::STRING,
                title.as_bytes(),
            );
            let _ = self.conn.flush();
        }
    }

    pub fn set_cursor(&self, _icon: CursorIcon) {
        // Standard X11 font cursor or xc_cursor can be bound without C libraries if desired
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
