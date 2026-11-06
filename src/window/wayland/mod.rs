pub mod clipboard;
pub mod cursor;
pub mod keyboard;
pub mod shm;

use super::event::{CursorIcon, ElementState, ModifiersState, MouseButton, PlatformEvent};
use crate::platform::{CANONICAL_APP_ID, CANONICAL_WM_CLASS_INSTANCE};
use crate::window::WindowResizeEdge;
use keyboard::{ActiveKeyRepeat, KeyRepeatConfig, XkbHandler};
use shm::WaylandBufferPool;
use std::collections::VecDeque;
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer,
    wl_data_source, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

fn resolve_wayland_app_id() -> &'static str {
    if std::path::Path::new("/usr/share/applications/io.github.lnoxsian.Velox.desktop").exists()
        || std::path::Path::new("/usr/local/share/applications/io.github.lnoxsian.Velox.desktop")
            .exists()
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
    pub decoration_manager: Option<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1>,
    pub toplevel_decoration: Option<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>,
    pub use_csd: bool,
    pub pointer_serial: u32,
    pub pointer: Option<wl_pointer::WlPointer>,
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    pub xkb_handler: XkbHandler,
    pub repeat_config: Option<KeyRepeatConfig>,
    pub active_repeat: Option<ActiveKeyRepeat>,
    pub buffer_pool: WaylandBufferPool,
    pub has_deferred_frame: bool,
    pub deferred_damage: Option<crate::window::DamageRect>,
    pub width: u32,
    pub height: u32,
    pub configured: bool,
    pub events: VecDeque<PlatformEvent>,
    pub modifiers: ModifiersState,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub is_maximized: bool,
    pub cursor_manager: Option<cursor::WaylandCursorManager>,
    pub current_cursor: CursorIcon,
    pub cursor_visible: bool,
    pub pointer_inside: bool,
    pub scroll_accumulator_y: f64,
    pub data_device_manager: Option<wl_data_device_manager::WlDataDeviceManager>,
    pub data_device: Option<wl_data_device::WlDataDevice>,
    pub current_selection_offer: Option<wl_data_offer::WlDataOffer>,
    pub active_offer: Arc<RwLock<Option<wl_data_offer::WlDataOffer>>>,
    pub active_mime_types: Arc<RwLock<Vec<String>>>,
    pub current_data_source: Option<wl_data_source::WlDataSource>,
    pub clipboard_text: Arc<RwLock<String>>,
    pub primary_text: Arc<RwLock<String>>,
    pub copy_pending_requests: Arc<Mutex<VecDeque<String>>>,
    pub last_input_serial: u32,
}

pub const CSD_TITLE_BAR_HEIGHT: u32 = 38;

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
            decoration_manager: None,
            toplevel_decoration: None,
            use_csd: true,
            pointer_serial: 0,
            pointer: None,
            keyboard: None,
            xkb_handler: XkbHandler::new(),
            repeat_config: None,
            active_repeat: None,
            buffer_pool: WaylandBufferPool::new(3),
            has_deferred_frame: false,
            deferred_damage: None,
            width: width.max(1),
            height: height.max(1),
            configured: false,
            events: VecDeque::new(),
            modifiers: ModifiersState::empty(),
            mouse_x: 0.0,
            mouse_y: 0.0,
            is_maximized: false,
            cursor_manager: None,
            current_cursor: CursorIcon::Default,
            cursor_visible: true,
            pointer_inside: false,
            scroll_accumulator_y: 0.0,
            data_device_manager: None,
            data_device: None,
            current_selection_offer: None,
            active_offer: Arc::new(RwLock::new(None)),
            active_mime_types: Arc::new(RwLock::new(Vec::new())),
            current_data_source: None,
            clipboard_text: Arc::new(RwLock::new(String::new())),
            primary_text: Arc::new(RwLock::new(String::new())),
            copy_pending_requests: Arc::new(Mutex::new(VecDeque::new())),
            last_input_serial: 0,
        }
    }
}

pub struct WaylandWindow {
    pub conn: Connection,
    pub event_queue: EventQueue<WaylandState>,
    pub qh: QueueHandle<WaylandState>,
    pub state: WaylandState,
    pub clipboard_provider: Arc<clipboard::WaylandClipboardProvider>,
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
            .clone()
            .ok_or("Wayland compositor not found")?;
        let wm_base = state.wm_base.clone().ok_or("XDG WM Base not found")?;
        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());

        toplevel.set_title(title.to_string());
        toplevel.set_app_id(resolve_wayland_app_id().to_string());

        // Request server-side window decorations if supported by compositor (e.g. KDE KWin)
        if let Some(dec_mgr) = &state.decoration_manager {
            let decoration = dec_mgr.get_toplevel_decoration(&toplevel, &qh, ());
            decoration.set_mode(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            state.toplevel_decoration = Some(decoration);
            // Default to CSD until compositor confirms Mode::ServerSide in configure event
            state.use_csd = true;
        } else {
            // Compositor lacks zxdg_decoration_manager_v1 (e.g. GNOME, Cinnamon) -> must use CSD
            state.use_csd = true;
        }

        surface.commit();

        state.surface = Some(surface);
        state.xdg_surface = Some(xdg_surface);
        state.xdg_toplevel = Some(toplevel);

        // Wait for initial configure event from compositor
        while !state.configured {
            event_queue.blocking_dispatch(&mut state)?;
        }

        let shm = state.shm.clone().ok_or("Wayland SHM not found")?;

        match cursor::WaylandCursorManager::new(&compositor, &shm, &qh) {
            Ok(cm) => state.cursor_manager = Some(cm),
            Err(e) => log::warn!("Failed to initialize Wayland cursor manager: {}", e),
        }

        let raw_fd = conn.as_fd().as_raw_fd();
        unsafe {
            let flags = libc::fcntl(raw_fd, libc::F_GETFL, 0);
            if flags >= 0 {
                libc::fcntl(raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        unsafe {
            if std::env::var("DESKTOP_STARTUP_ID").is_ok() {
                std::env::remove_var("DESKTOP_STARTUP_ID");
            }
            if std::env::var("XDG_ACTIVATION_TOKEN").is_ok() {
                std::env::remove_var("XDG_ACTIVATION_TOKEN");
            }
        }

        if state.data_device.is_none() {
            if let (Some(seat), Some(mgr)) = (&state.seat, &state.data_device_manager) {
                state.data_device = Some(mgr.get_data_device(seat, &qh, ()));
            }
        }

        let clipboard_provider = Arc::new(clipboard::WaylandClipboardProvider::new(
            Arc::clone(&state.clipboard_text),
            Arc::clone(&state.primary_text),
            Arc::clone(&state.copy_pending_requests),
            conn.clone(),
            Arc::clone(&state.active_offer),
            Arc::clone(&state.active_mime_types),
        ));
        crate::clipboard::clipboard::register_provider(clipboard_provider.clone());

        Ok(Self {
            conn,
            event_queue,
            qh,
            state,
            clipboard_provider,
        })
    }
}

impl Drop for WaylandWindow {
    fn drop(&mut self) {
        crate::clipboard::clipboard::unregister_provider();
    }
}

impl WaylandWindow {
    pub fn set_title(&self, title: &str) {
        if let Some(toplevel) = &self.state.xdg_toplevel {
            toplevel.set_title(title.to_string());
            if let Some(surface) = &self.state.surface {
                surface.commit();
            }
        }
    }

    pub fn set_cursor(&mut self, icon: CursorIcon) {
        self.state.current_cursor = icon;
        if self.state.pointer_inside && self.state.cursor_visible {
            if let (Some(cm), Some(pointer)) = (&mut self.state.cursor_manager, &self.state.pointer)
            {
                cm.apply_cursor(self.state.pointer_serial, icon, pointer);
                let _ = self.conn.flush();
            }
        }
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        if self.state.cursor_visible == visible {
            return;
        }
        self.state.cursor_visible = visible;
        if self.state.pointer_inside {
            if let (Some(cm), Some(pointer)) = (&mut self.state.cursor_manager, &self.state.pointer)
            {
                if visible {
                    cm.apply_cursor(
                        self.state.pointer_serial,
                        self.state.current_cursor,
                        pointer,
                    );
                } else {
                    cm.hide_cursor(self.state.pointer_serial, pointer);
                }
                let _ = self.conn.flush();
            }
        }
    }

    pub fn show_window_menu(&self, x: i32, y: i32) {
        if let (Some(toplevel), Some(seat)) = (&self.state.xdg_toplevel, &self.state.seat) {
            toplevel.show_window_menu(seat, self.state.pointer_serial, x, y);
            let _ = self.conn.flush();
        }
    }

    pub fn start_interactive_resize(&self, edge: WindowResizeEdge) {
        if let (Some(toplevel), Some(seat)) = (&self.state.xdg_toplevel, &self.state.seat) {
            let wayland_edge = match edge {
                WindowResizeEdge::Top => xdg_toplevel::ResizeEdge::Top,
                WindowResizeEdge::Bottom => xdg_toplevel::ResizeEdge::Bottom,
                WindowResizeEdge::Left => xdg_toplevel::ResizeEdge::Left,
                WindowResizeEdge::Right => xdg_toplevel::ResizeEdge::Right,
                WindowResizeEdge::TopLeft => xdg_toplevel::ResizeEdge::TopLeft,
                WindowResizeEdge::TopRight => xdg_toplevel::ResizeEdge::TopRight,
                WindowResizeEdge::BottomLeft => xdg_toplevel::ResizeEdge::BottomLeft,
                WindowResizeEdge::BottomRight => xdg_toplevel::ResizeEdge::BottomRight,
            };
            toplevel.resize(seat, self.state.pointer_serial, wayland_edge);
            let _ = self.conn.flush();
        }
    }

    #[inline]
    pub fn set_visible(&mut self, _visible: bool) {}

    #[inline]
    pub fn uses_csd(&self) -> bool {
        self.state.use_csd
    }

    #[inline]
    pub fn csd_title_bar_height(&self) -> u32 {
        if self.state.use_csd {
            CSD_TITLE_BAR_HEIGHT
        } else {
            0
        }
    }

    pub fn start_interactive_move(&self) {
        if let (Some(toplevel), Some(seat)) = (&self.state.xdg_toplevel, &self.state.seat) {
            toplevel._move(seat, self.state.pointer_serial);
            let _ = self.conn.flush();
        }
    }

    pub fn minimize(&self) {
        if let Some(toplevel) = &self.state.xdg_toplevel {
            toplevel.set_minimized();
            let _ = self.conn.flush();
        }
    }

    pub fn toggle_maximize(&mut self) {
        if let Some(toplevel) = &self.state.xdg_toplevel {
            if self.state.is_maximized {
                toplevel.unset_maximized();
            } else {
                toplevel.set_maximized();
            }
            let _ = self.conn.flush();
        }
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.state.is_maximized
    }

    #[inline]
    pub fn post_close_requested(&mut self) {
        self.state.events.push_back(PlatformEvent::CloseRequested);
    }

    pub fn present_frame(&mut self, src_pixels: &[u32], damage: Option<crate::window::DamageRect>) {
        let Some(rect) = damage else {
            return;
        };
        if rect.width == 0 || rect.height == 0 || self.state.width == 0 || self.state.height == 0 {
            return;
        }

        let width = self.state.width;
        let height = self.state.height;

        let effective_damage = match self.state.deferred_damage.take() {
            Some(prev) => prev.union(rect),
            None => rect,
        };

        let Some(shm) = &self.state.shm else {
            return;
        };

        let acquired = self.state.buffer_pool.acquire(shm, &self.qh, width, height);

        let Some(buf) = acquired else {
            self.state.has_deferred_frame = true;
            self.state.deferred_damage = Some(effective_damage);
            log::debug!(
                "Wayland SHM buffers exhausted; deferring frame for ({}, {})",
                width,
                height
            );
            return;
        };

        let damaged_y = effective_damage.y.min(height);
        let damaged_h = effective_damage
            .height
            .min(height.saturating_sub(damaged_y));
        if damaged_h == 0 {
            return;
        }

        let dest = buf.pixels_mut();
        let copy_len = dest.len().min(src_pixels.len());
        dest[..copy_len].copy_from_slice(&src_pixels[..copy_len]);

        use wayland_client::Proxy;
        let buffer_id = buf.buffer.id();
        let wl_buf_clone = buf.buffer.clone();

        if let Some(surface) = &self.state.surface {
            surface.attach(Some(&wl_buf_clone), 0, 0);
            surface.damage_buffer(0, damaged_y as i32, width as i32, damaged_h as i32);
            surface.commit();
        }

        self.state.buffer_pool.submit(&buffer_id);
        let _ = self.conn.flush();
    }

    #[inline]
    pub fn next_wake_time(&self) -> Option<Instant> {
        self.state.active_repeat.as_ref().map(|r| r.next_deadline)
    }

    pub fn poll_event(&mut self) -> Option<PlatformEvent> {
        // Process any pending native clipboard copy requests
        loop {
            let pending = self.state.copy_pending_requests.lock().unwrap().pop_front();
            match pending {
                Some(_text) => {
                    if let (Some(mgr), Some(device)) =
                        (&self.state.data_device_manager, &self.state.data_device)
                    {
                        let source = mgr.create_data_source(&self.qh, ());
                        source.offer("text/plain;charset=utf-8".to_string());
                        source.offer("text/plain".to_string());
                        source.offer("UTF8_STRING".to_string());
                        device.set_selection(Some(&source), self.state.last_input_serial);
                        self.state.current_data_source = Some(source);
                    }
                }
                None => break,
            }
        }

        let _ = self.conn.flush();
        if let Some(guard) = self.conn.prepare_read() {
            if let Err(e) = guard.read() {
                match e {
                    wayland_client::backend::WaylandError::Io(ref io_err)
                        if io_err.kind() == std::io::ErrorKind::WouldBlock => {}
                    _ => {
                        log::warn!("Wayland socket error: {}", e);
                        self.state.events.push_back(PlatformEvent::CloseRequested);
                    }
                }
            }
        }
        if let Err(e) = self.event_queue.dispatch_pending(&mut self.state) {
            log::warn!("Wayland dispatch error: {}", e);
            self.state.events.push_back(PlatformEvent::CloseRequested);
        }

        if let Some(event) = self.state.events.pop_front() {
            return Some(event);
        }

        // Handle active key repeat when no events arrived from compositor
        if let Some(repeat) = &mut self.state.active_repeat {
            let now = Instant::now();
            if now >= repeat.next_deadline {
                repeat.next_deadline += repeat.interval;
                if repeat.next_deadline <= now {
                    repeat.next_deadline = now + repeat.interval;
                }
                return Some(PlatformEvent::KeyPressed {
                    key: repeat.key.clone(),
                    text: repeat.text.clone(),
                    modifiers: repeat.modifiers,
                });
            }
        }

        None
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
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(7), qh, ());
                    if state.data_device.is_none() {
                        if let Some(mgr) = &state.data_device_manager {
                            state.data_device = Some(mgr.get_data_device(&seat, qh, ()));
                        }
                    }
                    state.seat = Some(seat);
                }
                "wl_data_device_manager" => {
                    let mgr = registry.bind::<wl_data_device_manager::WlDataDeviceManager, _, _>(
                        name,
                        version.min(3),
                        qh,
                        (),
                    );
                    if state.data_device.is_none() {
                        if let Some(seat) = &state.seat {
                            state.data_device = Some(mgr.get_data_device(seat, qh, ()));
                        }
                    }
                    state.data_device_manager = Some(mgr);
                }
                "zxdg_decoration_manager_v1" => {
                    state.decoration_manager = Some(registry.bind(name, version.min(1), qh, ()));
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
        state: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_buffer::Event::Release => {
                state.buffer_pool.release(buffer);
                if state.has_deferred_frame {
                    state.has_deferred_frame = false;
                    state.events.push_back(PlatformEvent::RedrawRequested);
                }
            }
            _ => {}
        }
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
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                let w = if width > 0 { width as u32 } else { state.width };
                let h = if height > 0 {
                    height as u32
                } else {
                    state.height
                };
                state.is_maximized = states.chunks_exact(4).any(|c| {
                    u32::from_ne_bytes(c.try_into().unwrap_or([0; 4]))
                        == xdg_toplevel::State::Maximized as u32
                });
                if w != state.width || h != state.height {
                    state.width = w;
                    state.height = h;
                    if let Some(PlatformEvent::Resized { width, height }) = state.events.back_mut()
                    {
                        *width = w;
                        *height = h;
                    } else {
                        state.events.push_back(PlatformEvent::Resized {
                            width: w,
                            height: h,
                        });
                    }
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
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_serial = serial;
                state.pointer_inside = true;
                state.mouse_x = surface_x;
                state.mouse_y = surface_y;
                if let Some(cm) = &mut state.cursor_manager {
                    if state.cursor_visible {
                        cm.apply_cursor(serial, state.current_cursor, pointer);
                    } else {
                        cm.hide_cursor(serial, pointer);
                    }
                }
                state.events.push_back(PlatformEvent::CursorMoved {
                    x: surface_x,
                    y: surface_y,
                });
            }
            wl_pointer::Event::Leave { .. } => {
                state.pointer_inside = false;
                state.scroll_accumulator_y = 0.0;
            }
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
                serial,
                button,
                state: btn_state,
                ..
            } => {
                state.pointer_serial = serial;
                state.last_input_serial = serial;
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
            wl_pointer::Event::Axis { axis, value, .. } => {
                if let wayland_client::WEnum::Value(wl_pointer::Axis::VerticalScroll) = axis {
                    if (state.scroll_accumulator_y > 0.0 && value < 0.0)
                        || (state.scroll_accumulator_y < 0.0 && value > 0.0)
                    {
                        state.scroll_accumulator_y = 0.0;
                    }
                    state.scroll_accumulator_y += value;
                    let threshold = 10.0;
                    while state.scroll_accumulator_y.abs() >= threshold {
                        let delta = if state.scroll_accumulator_y > 0.0 {
                            -1.0
                        } else {
                            1.0
                        };
                        if state.scroll_accumulator_y > 0.0 {
                            state.scroll_accumulator_y -= threshold;
                        } else {
                            state.scroll_accumulator_y += threshold;
                        }
                        state
                            .events
                            .push_back(PlatformEvent::MouseWheel { delta_y: delta });
                    }
                }
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
            wl_keyboard::Event::Keymap { fd, size, .. } => {
                state.xkb_handler.update_keymap(&fd, size);
            }
            wl_keyboard::Event::Enter { .. } => {
                state.events.push_back(PlatformEvent::Focused(true));
            }
            wl_keyboard::Event::Leave { .. } => {
                state.active_repeat = None;
                let prev_mods = state.modifiers;
                state.modifiers = ModifiersState::empty();
                if state.modifiers != prev_mods {
                    state
                        .events
                        .push_back(PlatformEvent::ModifiersChanged(state.modifiers));
                }
                state.events.push_back(PlatformEvent::Focused(false));
            }
            wl_keyboard::Event::Key {
                serial,
                key,
                state: key_state,
                ..
            } => {
                state.last_input_serial = serial;
                let is_pressed = matches!(
                    key_state,
                    wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed)
                );
                let (k, text, repeats, mods) =
                    state.xkb_handler.translate_key(key, state.modifiers);
                if is_pressed {
                    let prev_mods = state.modifiers;
                    state.modifiers = mods;
                    if state.modifiers != prev_mods {
                        state
                            .events
                            .push_back(PlatformEvent::ModifiersChanged(state.modifiers));
                    }
                }
                if let Some(k) = k {
                    if is_pressed {
                        state.events.push_back(PlatformEvent::KeyPressed {
                            key: k.clone(),
                            text: text.clone(),
                            modifiers: state.modifiers,
                        });

                        if repeats {
                            if let Some(cfg) = state.repeat_config {
                                let delay = Duration::from_millis(cfg.delay.max(1) as u64);
                                let interval = Duration::from_micros(
                                    (1_000_000.0 / (cfg.rate.max(1) as f64)) as u64,
                                );
                                let now = Instant::now();
                                state.active_repeat = Some(ActiveKeyRepeat {
                                    scancode: key,
                                    key: k,
                                    text,
                                    modifiers: state.modifiers,
                                    next_deadline: now + delay,
                                    interval,
                                });
                            }
                        } else {
                            state.active_repeat = None;
                        }
                    } else {
                        if let Some(active) = &state.active_repeat
                            && active.scancode == key
                        {
                            state.active_repeat = None;
                        }
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
                group,
                ..
            } => {
                let prev_mods = state.modifiers;
                state.modifiers = state.xkb_handler.update_modifiers(
                    mods_depressed,
                    mods_latched,
                    mods_locked,
                    group,
                );
                if let Some(repeat) = &mut state.active_repeat {
                    repeat.modifiers = state.modifiers;
                }
                if state.modifiers != prev_mods {
                    state
                        .events
                        .push_back(PlatformEvent::ModifiersChanged(state.modifiers));
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                if rate > 0 && delay > 0 {
                    state.repeat_config = Some(KeyRepeatConfig { rate, delay });
                } else {
                    state.repeat_config = None;
                    state.active_repeat = None;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        _: zxdg_decoration_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
            log::debug!("Wayland window decoration mode: {:?}", mode);
            match mode {
                wayland_client::WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ServerSide) => {
                    state.use_csd = false;
                    log::info!("Wayland compositor provides server-side decorations");
                }
                _ => {
                    state.use_csd = true;
                    log::info!("Wayland compositor requires client-side decorations (CSD)");
                }
            }
        }
    }
}

impl Dispatch<wl_data_device_manager::WlDataDeviceManager, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_data_device_manager::WlDataDeviceManager,
        _: wl_data_device_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Selection { id } => {
                if let Ok(mut lock) = state.active_offer.write() {
                    *lock = id.clone();
                }
                if let Ok(mut lock) = state.active_mime_types.write() {
                    lock.clear();
                }
                state.current_selection_offer = id;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_data_source::WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { mime_type: _, fd } => {
                let text = state.clipboard_text.read().unwrap().clone();
                let mut file = std::fs::File::from(fd);
                let _ = file.write_all(text.as_bytes());
            }
            wl_data_source::Event::Cancelled => {
                state.current_data_source = None;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_data_offer::WlDataOffer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_data_offer::WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            if let Ok(mut lock) = state.active_mime_types.write() {
                lock.push(mime_type);
            }
        }
    }
}
