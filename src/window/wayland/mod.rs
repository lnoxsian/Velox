pub mod keyboard;
pub mod shm;

use super::event::{CursorIcon, ElementState, ModifiersState, MouseButton, PlatformEvent};
use crate::platform::{CANONICAL_APP_ID, CANONICAL_WM_CLASS_INSTANCE};
use shm::WaylandShmBuffer;
use std::collections::VecDeque;
use std::os::fd::{AsFd, AsRawFd};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

fn resolve_wayland_app_id() -> &'static str {
    if std::path::Path::new("/usr/share/applications/io.github.lnoxsian.Velox.desktop").exists()
        || std::path::Path::new("/usr/local/share/applications/io.github.lnoxsian.Velox.desktop").exists()
    {
        return CANONICAL_APP_ID;
    }
    if let Ok(home) = std::env::var("HOME")
        && std::path::PathBuf::from(home)
            .join(".local/share/applications/io.github.lnoxsian.Velox.desktop")
            .exists()
    {
        return CANONICAL_APP_ID;
    }
    CANONICAL_WM_CLASS_INSTANCE
}

pub struct WaylandState {
    pub running: bool,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub wm_base: Option<xdg_wm_base::XdgWmBase>,
    pub seat: Option<wl_seat::WlSeat>,
    pub surface: Option<wl_surface::WlSurface>,
    pub xdg_surface: Option<xdg_surface::XdgSurface>,
    pub xdg_toplevel: Option<xdg_toplevel::XdgToplevel>,
    pub pointer: Option<wl_pointer::WlPointer>,
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    pub buffer: Option<WaylandShmBuffer>,
    pub width: u32,
    pub height: u32,
    pub configured: bool,
    pub events: VecDeque<PlatformEvent>,
    pub modifiers: ModifiersState,
    pub mouse_x: f64,
    pub mouse_y: f64,
}

impl WaylandState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            running: true,
            compositor: None,
            shm: None,
            wm_base: None,
            seat: None,
            surface: None,
            xdg_surface: None,
            xdg_toplevel: None,
            pointer: None,
            keyboard: None,
            buffer: None,
            width: width.max(1),
            height: height.max(1),
            configured: false,
            events: VecDeque::new(),
            modifiers: ModifiersState::empty(),
            mouse_x: 0.0,
            mouse_y: 0.0,
        }
    }
}

pub struct WaylandWindow {
    pub conn: Connection,
    pub event_queue: EventQueue<WaylandState>,
    pub qh: QueueHandle<WaylandState>,
    pub state: WaylandState,
}

impl WaylandWindow {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::connect_to_env()?;
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let display = conn.display();
        display.get_registry(&qh, ());

        let mut state = WaylandState::new(width, height);

        // Roundtrip to bind globals
        event_queue.roundtrip(&mut state)?;

        let compositor = state
            .compositor
            .as_ref()
            .ok_or("Wayland compositor not found")?;
        let wm_base = state.wm_base.as_ref().ok_or("XDG WM Base not found")?;
        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());

        toplevel.set_title(title.to_string());
        toplevel.set_app_id(resolve_wayland_app_id().to_string());

        surface.commit();

        state.surface = Some(surface);
        state.xdg_surface = Some(xdg_surface);
        state.xdg_toplevel = Some(toplevel);

        // Wait for initial configure event from compositor
        while !state.configured {
            event_queue.blocking_dispatch(&mut state)?;
        }

        let shm = state.shm.as_ref().ok_or("Wayland SHM not found")?;
        let buffer = WaylandShmBuffer::new(shm, &qh, state.width, state.height)?;
        state.buffer = Some(buffer);

        let raw_fd = conn.as_fd().as_raw_fd();
        unsafe {
            let flags = libc::fcntl(raw_fd, libc::F_GETFL, 0);
            if flags >= 0 {
                libc::fcntl(raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        Ok(Self {
            conn,
            event_queue,
            qh,
            state,
        })
    }

    pub fn set_title(&self, title: &str) {
        if let Some(toplevel) = &self.state.xdg_toplevel {
            toplevel.set_title(title.to_string());
            if let Some(surface) = &self.state.surface {
                surface.commit();
            }
        }
    }

    pub fn set_cursor(&self, _icon: CursorIcon) {}

    pub fn present_frame(&mut self, src_pixels: &[u32]) {
        if let Some(buf) = &mut self.state.buffer {
            let dest = buf.pixels_mut();
            if dest.len() == src_pixels.len() {
                dest.copy_from_slice(src_pixels);
            }
            if let Some(surface) = &self.state.surface {
                surface.attach(Some(&buf.buffer), 0, 0);
                surface.damage_buffer(0, 0, buf.width as i32, buf.height as i32);
                surface.commit();
            }
        }
        let _ = self.conn.flush();
    }

    pub fn poll_event(&mut self) -> Option<PlatformEvent> {
        let _ = self.conn.flush();
        if let Some(guard) = self.conn.prepare_read() {
            let _ = guard.read();
        }
        let _ = self.event_queue.dispatch_pending(&mut self.state);
        self.state.events.pop_front()
    }
}

// Implement Dispatch for Wayland Protocols
impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(6), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "xdg_wm_base" => {
                    state.wm_base = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for WaylandState {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for WaylandState {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                let w = if width > 0 { width as u32 } else { state.width };
                let h = if height > 0 {
                    height as u32
                } else {
                    state.height
                };
                if w != state.width || h != state.height {
                    state.width = w;
                    state.height = h;
                    if let Some(shm) = &state.shm
                        && let Ok(buf) = WaylandShmBuffer::new(shm, qh, w, h)
                    {
                        state.buffer = Some(buf);
                    }
                    state.events.push_back(PlatformEvent::Resized {
                        width: w,
                        height: h,
                    });
                }
            }
            xdg_toplevel::Event::Close => {
                state.events.push_back(PlatformEvent::CloseRequested);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: wayland_client::WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.mouse_x = surface_x;
                state.mouse_y = surface_y;
                state.events.push_back(PlatformEvent::CursorMoved {
                    x: surface_x,
                    y: surface_y,
                });
            }
            wl_pointer::Event::Button {
                button,
                state: btn_state,
                ..
            } => {
                let s = match btn_state {
                    wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                        ElementState::Pressed
                    }
                    _ => ElementState::Released,
                };
                let b = match button {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    other => MouseButton::Other(other as u16),
                };
                state.events.push_back(PlatformEvent::MouseInput {
                    state: s,
                    button: b,
                });
            }
            wl_pointer::Event::Axis { value, .. } => {
                let delta = if value > 0.0 { -1.0 } else { 1.0 };
                state
                    .events
                    .push_back(PlatformEvent::MouseWheel { delta_y: delta });
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                let is_pressed = matches!(
                    key_state,
                    wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed)
                );
                // Wayland keycodes are Linux evdev scancodes; key + 8 aligns with standard keysyms
                let keysym = key + 8;
                let (k, text) = keyboard::xkb_keysym_to_key(keysym);
                if let Some(k) = k {
                    if is_pressed {
                        state.events.push_back(PlatformEvent::KeyPressed {
                            key: k,
                            text,
                            modifiers: state.modifiers,
                        });
                    } else {
                        state.events.push_back(PlatformEvent::KeyReleased {
                            key: k,
                            modifiers: state.modifiers,
                        });
                    }
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                ..
            } => {
                let mask = mods_depressed | mods_latched | mods_locked;
                let mut m = ModifiersState::empty();
                if mask & 1 != 0 {
                    m |= ModifiersState::SHIFT;
                }
                if mask & 4 != 0 {
                    m |= ModifiersState::CONTROL;
                }
                if mask & 8 != 0 {
                    m |= ModifiersState::ALT;
                }
                if mask & 64 != 0 {
                    m |= ModifiersState::SUPER;
                }
                state.modifiers = m;
            }
            _ => {}
        }
    }
}
