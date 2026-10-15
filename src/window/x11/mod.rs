pub mod icon;
pub mod keysym;
pub mod shm;

use super::event::{CursorIcon, ElementState, ModifiersState, MouseButton, PlatformEvent};
use crate::platform::{CANONICAL_WM_CLASS_GENERAL, CANONICAL_WM_CLASS_INSTANCE};
use icon::get_net_wm_icon_data;
use shm::X11Framebuffer;
use std::collections::VecDeque;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    self, AtomEnum, ChangeWindowAttributesAux, ColormapAlloc, ConnectionExt as _, CreateWindowAux,
    EventMask, VisualClass, WindowClass,
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
    pub is_visible: bool,
    pub root: xproto::Window,
    cursor_default: Option<xproto::Cursor>,
    cursor_text: Option<xproto::Cursor>,
    cursor_pointer: Option<xproto::Cursor>,
    cursor_col_resize: Option<xproto::Cursor>,
    cursor_row_resize: Option<xproto::Cursor>,
    events: VecDeque<PlatformEvent>,
    keymap: Option<Vec<u32>>,
    keysyms_per_keycode: usize,
    min_keycode: u8,
    max_keycode: u8,
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

fn create_font_cursor(conn: &RustConnection, glyph: u16) -> Option<xproto::Cursor> {
    let font = conn.generate_id().ok()?;
    conn.open_font(font, b"cursor").ok()?;
    let cursor = conn.generate_id().ok()?;
    conn.create_glyph_cursor(
        cursor,
        font,
        font,
        glyph,
        glyph + 1,
        0,
        0,
        0,
        0xFFFF,
        0xFFFF,
        0xFFFF,
    )
    .ok()?;
    let _ = conn.close_font(font);
    Some(cursor)
}

fn detect_desktop_cursor_theme() -> Option<String> {
    if let Ok(theme) = std::env::var("XCURSOR_THEME") {
        if !theme.is_empty() {
            return Some(theme);
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        let home_path = std::path::Path::new(&home);
        // 1. Check ~/.icons/default/index.theme
        let path = home_path.join(".icons/default/index.theme");
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(theme) = trimmed.strip_prefix("Inherits=") {
                    let theme = theme.trim().trim_matches('"').trim_matches('\'');
                    if !theme.is_empty() {
                        return Some(theme.to_string());
                    }
                }
            }
        }

        // 2. Check ~/.config/gtk-3.0/settings.ini
        let path = home_path.join(".config/gtk-3.0/settings.ini");
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(theme) = trimmed.strip_prefix("gtk-cursor-theme-name=") {
                    let theme = theme.trim().trim_matches('"').trim_matches('\'');
                    if !theme.is_empty() {
                        return Some(theme.to_string());
                    }
                }
            }
        }
    }

    // 3. Check /usr/share/icons/default/index.theme
    if let Ok(content) = std::fs::read_to_string("/usr/share/icons/default/index.theme") {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(theme) = trimmed.strip_prefix("Inherits=") {
                let theme = theme.trim().trim_matches('"').trim_matches('\'');
                if !theme.is_empty() {
                    return Some(theme.to_string());
                }
            }
        }
    }

    // 4. Check Cinnamon / GNOME gsettings
    for schema in ["org.cinnamon.desktop.interface", "org.gnome.desktop.interface"] {
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", schema, "cursor-theme"])
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                let theme = s.trim().trim_matches('\'').trim_matches('"').trim();
                if !theme.is_empty() {
                    return Some(theme.to_string());
                }
            }
        }
    }

    None
}

fn detect_desktop_cursor_size() -> Option<u32> {
    if let Ok(size_str) = std::env::var("XCURSOR_SIZE") {
        if let Ok(size) = size_str.parse::<u32>() {
            return Some(size);
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let path = std::path::Path::new(&home).join(".config/gtk-3.0/settings.ini");
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(size) = trimmed.strip_prefix("gtk-cursor-theme-size=") {
                    if let Ok(s) = size.trim().parse::<u32>() {
                        return Some(s);
                    }
                }
            }
        }
    }
    for schema in ["org.cinnamon.desktop.interface", "org.gnome.desktop.interface"] {
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", schema, "cursor-size"])
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                if let Ok(size) = s.trim().trim_matches('\'').trim_matches('"').trim().parse::<u32>() {
                    return Some(size);
                }
            }
        }
    }
    None
}

fn load_xcursor_or_fallback(
    conn: &RustConnection,
    handle: Option<&x11rb::cursor::Handle>,
    names: &[&str],
    fallback_glyph: u16,
) -> Option<xproto::Cursor> {
    if let Some(handle) = handle {
        for name in names {
            if let Ok(cursor) = handle.load_cursor(conn, name) {
                if cursor != 0 {
                    return Some(cursor);
                }
            }
        }
    }
    create_font_cursor(conn, fallback_glyph)
}

fn notify_startup_complete(conn: &RustConnection, root: xproto::Window, startup_id: &str) {
    let msg = format!("remove: ID=\"{}\"", startup_id);
    let bytes = msg.as_bytes();
    let begin_atom = conn
        .intern_atom(false, b"_NET_STARTUP_INFO_BEGIN")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom);
    let info_atom = conn
        .intern_atom(false, b"_NET_STARTUP_INFO")
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom);
    if let (Some(begin_atom), Some(info_atom)) = (begin_atom, info_atom) {
        for (i, chunk) in bytes.chunks(20).enumerate() {
            let mut data = [0u8; 20];
            data[..chunk.len()].copy_from_slice(chunk);
            let type_atom = if i == 0 { begin_atom } else { info_atom };
            let event = xproto::ClientMessageEvent {
                response_type: xproto::CLIENT_MESSAGE_EVENT,
                format: 8,
                sequence: 0,
                window: root,
                type_: type_atom,
                data: xproto::ClientMessageData::from(data),
            };
            let _ = conn.send_event(false, root, EventMask::PROPERTY_CHANGE, event);
        }
    }
}

impl X11Window {
    pub fn new(
        title: &str,
        width: u32,
        height: u32,
        bg_pixel: Option<u32>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

        // Detect desktop cursor theme & size if not already set in environment
        if std::env::var_os("XCURSOR_THEME").is_none() {
            if let Some(theme) = detect_desktop_cursor_theme() {
                unsafe {
                    std::env::set_var("XCURSOR_THEME", &theme);
                }
            }
        }
        if std::env::var_os("XCURSOR_SIZE").is_none() {
            if let Some(size) = detect_desktop_cursor_size() {
                unsafe {
                    std::env::set_var("XCURSOR_SIZE", size.to_string());
                }
            }
        }

        let db = x11rb::resource_manager::new_from_default(&conn).unwrap_or_default();
        let cursor_handle = x11rb::cursor::Handle::new(&conn, screen_num, &db)
            .ok()
            .and_then(|cookie| cookie.reply().ok());

        let cursor_default = load_xcursor_or_fallback(
            &conn,
            cursor_handle.as_ref(),
            &["left_ptr", "default", "arrow"],
            68, // XC_left_ptr
        );
        let cursor_text = load_xcursor_or_fallback(
            &conn,
            cursor_handle.as_ref(),
            &["xterm", "text", "ibeam"],
            152, // XC_xterm
        );
        let cursor_pointer = load_xcursor_or_fallback(
            &conn,
            cursor_handle.as_ref(),
            &["pointer", "pointing_hand", "hand2", "hand1"],
            58, // XC_hand2
        );
        let cursor_col_resize = load_xcursor_or_fallback(
            &conn,
            cursor_handle.as_ref(),
            &["col-resize", "sb_h_double_arrow", "ew-resize", "h_double_arrow"],
            108, // XC_sb_h_double_arrow
        );
        let cursor_row_resize = load_xcursor_or_fallback(
            &conn,
            cursor_handle.as_ref(),
            &["row-resize", "sb_v_double_arrow", "ns-resize", "v_double_arrow"],
            116, // XC_sb_v_double_arrow
        );

        let mut win_aux = CreateWindowAux::new().event_mask(event_mask);
        if let Some(c) = cursor_default {
            win_aux = win_aux.cursor(c);
        }
        if let Some(cmap) = colormap {
            // When creating a window with depth different from root (e.g. 32 on 24-bit root),
            // X11 protocol mandates colormap and border_pixel to avoid BadMatch.
            let initial_bg = bg_pixel.unwrap_or(0xFF000000);
            win_aux = win_aux
                .colormap(cmap)
                .border_pixel(0)
                .background_pixel(initial_bg);
        } else {
            let initial_bg = bg_pixel.map(|p| p & 0x00FFFFFF).unwrap_or(black_pixel);
            win_aux = win_aux.background_pixel(initial_bg);
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

        // Inform window manager of startup notification sequence if launched from desktop launcher
        if let Ok(startup_id) = std::env::var("DESKTOP_STARTUP_ID") {
            if let Ok(cookie) = conn.intern_atom(false, b"_NET_STARTUP_ID") {
                if let Ok(reply) = cookie.reply() {
                    let _ = conn.change_property8(
                        xproto::PropMode::REPLACE,
                        window_id,
                        reply.atom,
                        utf8_string,
                        startup_id.as_bytes(),
                    );
                }
            }
        }

        // Defer mapping the window until set_visible(true) after initial frame draw
        // to achieve zero-flicker launch without transparent artifacts.
        let fb = X11Framebuffer::new(&conn, win_width, win_height, depth);

        let (min_keycode, max_keycode, keysyms_per_keycode, keymap) =
            Self::load_keyboard_mapping(&conn);

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
            is_visible: false,
            root,
            cursor_default,
            cursor_text,
            cursor_pointer,
            cursor_col_resize,
            cursor_row_resize,
            events: VecDeque::new(),
            keymap,
            keysyms_per_keycode,
            min_keycode,
            max_keycode,
        })
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.is_visible == visible {
            return;
        }
        self.is_visible = visible;
        if visible {
            let _ = self.conn.map_window(self.window_id);
            self.fb
                .present(&self.conn, self.window_id, self.gc, self.visual);

            // Signal startup notification completion to WM/desktop environment (stops loading spinner)
            if let Ok(startup_id) = std::env::var("DESKTOP_STARTUP_ID") {
                notify_startup_complete(&self.conn, self.root, &startup_id);
                unsafe {
                    std::env::remove_var("DESKTOP_STARTUP_ID");
                }
            }
            unsafe {
                std::env::remove_var("XDG_ACTIVATION_TOKEN");
            }

            let _ = self.conn.flush();
        } else {
            let _ = self.conn.unmap_window(self.window_id);
            let _ = self.conn.flush();
        }
    }

    pub fn set_background_pixel(&self, bg_pixel: u32) {
        let aux = ChangeWindowAttributesAux::new().background_pixel(bg_pixel);
        let _ = self.conn.change_window_attributes(self.window_id, &aux);
        let _ = self.conn.flush();
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

    pub fn set_cursor(&self, icon: CursorIcon) {
        let cid = match icon {
            CursorIcon::Default => self.cursor_default,
            CursorIcon::Text => self.cursor_text,
            CursorIcon::Pointer => self.cursor_pointer,
            CursorIcon::ColResize => self.cursor_col_resize,
            CursorIcon::RowResize => self.cursor_row_resize,
        };
        if let Some(cid) = cid {
            let aux = ChangeWindowAttributesAux::new().cursor(cid);
            if let Ok(cookie) = self.conn.change_window_attributes(self.window_id, &aux) {
                if let Err(e) = cookie.check() {
                    eprintln!("X11 change_window_attributes cursor error: {:?}", e);
                }
            }
            let _ = self.conn.flush();
        }
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

    #[inline]
    pub fn flush(&self) {
        let _ = self.conn.flush();
    }

    pub fn poll_event(&mut self) -> Option<PlatformEvent> {
        if let Some(event) = self.events.pop_front() {
            return Some(event);
        }

        while let Ok(Some(raw_event)) = self.conn.poll_for_event() {
            self.handle_raw_event(raw_event);
            if let Some(event) = self.events.pop_front() {
                return Some(event);
            }
        }

        None
    }

    fn handle_raw_event(&mut self, raw_event: x11rb::protocol::Event) {
        match raw_event {
            x11rb::protocol::Event::ConfigureNotify(ev) => {
                let w = ev.width as u32;
                let h = ev.height as u32;
                if w != self.width || h != self.height {
                    self.width = w;
                    self.height = h;
                    self.fb.resize(&self.conn, w, h);
                    self.events.push_back(PlatformEvent::Resized {
                        width: w,
                        height: h,
                    });
                }
            }
            x11rb::protocol::Event::Expose(ev) => {
                if ev.count == 0 {
                    self.events.push_back(PlatformEvent::RedrawRequested);
                }
            }
            x11rb::protocol::Event::ClientMessage(ev) => {
                if ev.data.as_data32()[0] == self.wm_delete_window {
                    self.events.push_back(PlatformEvent::CloseRequested);
                }
            }
            x11rb::protocol::Event::KeyPress(ev) => {
                let old_mods = self.modifiers;
                let mut mods = Self::parse_modifiers(ev.state);
                if let Some(m) = self.modifier_for_keycode(ev.detail) {
                    mods.insert(m);
                }
                self.modifiers = mods;
                if self.modifiers != old_mods {
                    self.events.push_back(PlatformEvent::ModifiersChanged(self.modifiers));
                }
                let (key, text) = self.lookup_key(ev.detail, ev.state);
                if let Some(key) = key {
                    self.events.push_back(PlatformEvent::KeyPressed {
                        key,
                        text,
                        modifiers: self.modifiers,
                    });
                }
            }
            x11rb::protocol::Event::KeyRelease(ev) => {
                let old_mods = self.modifiers;
                let mut mods = Self::parse_modifiers(ev.state);
                if let Some(m) = self.modifier_for_keycode(ev.detail) {
                    mods.remove(m);
                }
                self.modifiers = mods;
                if self.modifiers != old_mods {
                    self.events.push_back(PlatformEvent::ModifiersChanged(self.modifiers));
                }
                let (key, _) = self.lookup_key(ev.detail, ev.state);
                if let Some(key) = key {
                    self.events.push_back(PlatformEvent::KeyReleased {
                        key,
                        modifiers: self.modifiers,
                    });
                }
            }
            x11rb::protocol::Event::MotionNotify(ev) => {
                let old_mods = self.modifiers;
                self.modifiers = Self::parse_modifiers(ev.state);
                if self.modifiers != old_mods {
                    self.events.push_back(PlatformEvent::ModifiersChanged(self.modifiers));
                }
                self.events.push_back(PlatformEvent::CursorMoved {
                    x: ev.event_x as f64,
                    y: ev.event_y as f64,
                });
            }
            x11rb::protocol::Event::ButtonPress(ev) => {
                let old_mods = self.modifiers;
                self.modifiers = Self::parse_modifiers(ev.state);
                if self.modifiers != old_mods {
                    self.events.push_back(PlatformEvent::ModifiersChanged(self.modifiers));
                }
                match ev.detail {
                    4 => self.events.push_back(PlatformEvent::MouseWheel { delta_y: 1.0 }),
                    5 => self.events.push_back(PlatformEvent::MouseWheel { delta_y: -1.0 }),
                    1 => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                    }),
                    2 => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Middle,
                    }),
                    3 => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Right,
                    }),
                    other => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Other(other as u16),
                    }),
                }
            }
            x11rb::protocol::Event::ButtonRelease(ev) => {
                let old_mods = self.modifiers;
                self.modifiers = Self::parse_modifiers(ev.state);
                if self.modifiers != old_mods {
                    self.events.push_back(PlatformEvent::ModifiersChanged(self.modifiers));
                }
                match ev.detail {
                    1 => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Left,
                    }),
                    2 => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Middle,
                    }),
                    3 => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Right,
                    }),
                    4 | 5 => {}
                    other => self.events.push_back(PlatformEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Other(other as u16),
                    }),
                }
            }
            x11rb::protocol::Event::FocusIn(_) => {
                self.events.push_back(PlatformEvent::Focused(true));
            }
            x11rb::protocol::Event::FocusOut(_) => {
                let old_mods = self.modifiers;
                self.modifiers = ModifiersState::empty();
                if self.modifiers != old_mods {
                    self.events.push_back(PlatformEvent::ModifiersChanged(self.modifiers));
                }
                self.events.push_back(PlatformEvent::Focused(false));
            }
            x11rb::protocol::Event::MappingNotify(ev) => {
                if ev.request == xproto::Mapping::KEYBOARD || ev.request == xproto::Mapping::MODIFIER {
                    self.refresh_keyboard_mapping();
                }
            }
            _ => {}
        }
    }

    fn parse_modifiers(state: xproto::KeyButMask) -> ModifiersState {
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
        m
    }

    fn load_keyboard_mapping(conn: &RustConnection) -> (u8, u8, usize, Option<Vec<u32>>) {
        let min_keycode = conn.setup().min_keycode;
        let max_keycode = conn.setup().max_keycode;
        let count = max_keycode.saturating_sub(min_keycode) + 1;
        if let Ok(cookie) = conn.get_keyboard_mapping(min_keycode, count)
            && let Ok(reply) = cookie.reply()
        {
            (
                min_keycode,
                max_keycode,
                reply.keysyms_per_keycode as usize,
                Some(reply.keysyms),
            )
        } else {
            (min_keycode, max_keycode, 0, None)
        }
    }

    fn refresh_keyboard_mapping(&mut self) {
        let (min, max, per_code, keysyms) = Self::load_keyboard_mapping(&self.conn);
        self.min_keycode = min;
        self.max_keycode = max;
        self.keysyms_per_keycode = per_code;
        self.keymap = keysyms;
    }

    fn keysym_for_keycode(&self, keycode: u8, col: usize) -> u32 {
        if keycode < self.min_keycode || keycode > self.max_keycode {
            return 0;
        }
        if let Some(keymap) = &self.keymap {
            let index = (keycode - self.min_keycode) as usize * self.keysyms_per_keycode;
            if index + col < keymap.len() {
                return keymap[index + col];
            }
        }
        0
    }

    fn modifier_for_keycode(&self, keycode: u8) -> Option<ModifiersState> {
        let keysym = self.keysym_for_keycode(keycode, 0);
        match keysym {
            0xffe1 | 0xffe2 => Some(ModifiersState::SHIFT),
            0xffe3 | 0xffe4 => Some(ModifiersState::CONTROL),
            0xffe7 | 0xffe8 | 0xffe9 | 0xffea => Some(ModifiersState::ALT),
            0xffeb | 0xffec | 0xffed | 0xffee => Some(ModifiersState::SUPER),
            _ => None,
        }
    }

    fn lookup_key(
        &self,
        keycode: u8,
        state: xproto::KeyButMask,
    ) -> (Option<super::event::Key>, Option<String>) {
        if keycode < self.min_keycode || keycode > self.max_keycode {
            return (None, None);
        }
        let val: u16 = state.into();
        let is_shift = val & xproto::ModMask::SHIFT.bits() != 0;
        let is_lock = val & xproto::ModMask::LOCK.bits() != 0;
        let keysym_col0 = self.keysym_for_keycode(keycode, 0);
        let is_letter = (b'a'..=b'z').contains(&(keysym_col0 as u8))
            || (b'A'..=b'Z').contains(&(keysym_col0 as u8));
        let col = if (if is_letter { is_shift ^ is_lock } else { is_shift })
            && self.keysyms_per_keycode > 1
        {
            1
        } else {
            0
        };
        let mut keysym = self.keysym_for_keycode(keycode, col);
        if keysym == 0 && col == 1 {
            keysym = keysym_col0;
        }
        if keysym == 0 {
            return (None, None);
        }
        keysym::keysym_to_key(keysym)
    }
}

impl Drop for X11Window {
    fn drop(&mut self) {
        if let Some(c) = self.cursor_default {
            let _ = self.conn.free_cursor(c);
        }
        if let Some(c) = self.cursor_text {
            let _ = self.conn.free_cursor(c);
        }
        if let Some(c) = self.cursor_pointer {
            let _ = self.conn.free_cursor(c);
        }
        if let Some(c) = self.cursor_col_resize {
            let _ = self.conn.free_cursor(c);
        }
        if let Some(c) = self.cursor_row_resize {
            let _ = self.conn.free_cursor(c);
        }
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

                let win = X11Window::new("Velox ARGB32 Test", 300, 200, None);
                assert!(
                    win.is_ok(),
                    "X11 window creation with 32-bit visual failed: {:?}",
                    win.err()
                );
                let mut win = win.unwrap();
                assert_eq!(win.depth, 32);
                assert!(win.colormap.is_some());
                assert_eq!(win.fb.depth, 32);
                assert!(!win.is_visible);

                // Present a frame with semi-transparent ARGB pixels
                let frame = vec![0x80123456u32; (win.width * win.height) as usize];
                win.present_frame(&frame);
                win.set_visible(true);
                assert!(win.is_visible);
            }
        }
    }

    #[test]
    fn test_x11_initial_background_pixel_and_deferred_map() {
        if let Ok((_conn, _screen_num)) = RustConnection::connect(None) {
            let expected_bg = 0xFF282828u32;
            let win = X11Window::new("Velox Zero Flicker Test", 300, 200, Some(expected_bg));
            if let Ok(mut win) = win {
                assert!(!win.is_visible);
                win.set_visible(true);
                assert!(win.is_visible);
                win.set_visible(false);
                assert!(!win.is_visible);
            }
        }
    }

    #[test]
    fn test_x11_parse_modifiers() {
        use x11rb::protocol::xproto::ModMask;

        let empty = X11Window::parse_modifiers(0u16.into());
        assert_eq!(empty, ModifiersState::empty());

        let shift = X11Window::parse_modifiers(ModMask::SHIFT.bits().into());
        assert_eq!(shift, ModifiersState::SHIFT);

        let ctrl = X11Window::parse_modifiers(ModMask::CONTROL.bits().into());
        assert_eq!(ctrl, ModifiersState::CONTROL);

        let alt = X11Window::parse_modifiers(ModMask::M1.bits().into());
        assert_eq!(alt, ModifiersState::ALT);

        let super_mod = X11Window::parse_modifiers(ModMask::M4.bits().into());
        assert_eq!(super_mod, ModifiersState::SUPER);

        let combined = X11Window::parse_modifiers(
            (ModMask::SHIFT.bits() | ModMask::CONTROL.bits() | ModMask::M1.bits()).into(),
        );
        assert!(combined.contains(ModifiersState::SHIFT));
        assert!(combined.contains(ModifiersState::CONTROL));
        assert!(combined.contains(ModifiersState::ALT));
    }

    #[test]
    fn test_x11_lookup_key_shift_and_lock() {
        use crate::window::Key;
        use x11rb::protocol::xproto::ModMask;

        if let Ok((_conn, _screen_num)) = RustConnection::connect(None) {
            let win = X11Window::new("Lookup Key Test", 200, 200, None);
            if let Ok(win) = win {
                // Find keycode for 'a' (0x61)
                if let Some(keymap) = &win.keymap {
                    for keycode in win.min_keycode..=win.max_keycode {
                        let idx = (keycode - win.min_keycode) as usize * win.keysyms_per_keycode;
                        if idx < keymap.len() && keymap[idx] == 0x61 {
                            // Test unshifted 'a'
                            let (key, text) = win.lookup_key(keycode, 0u16.into());
                            assert_eq!(key, Some(Key::Character("a".to_string())));
                            assert_eq!(text, Some("a".to_string()));

                            // Test shifted 'A'
                            let (key_shift, text_shift) =
                                win.lookup_key(keycode, ModMask::SHIFT.bits().into());
                            assert_eq!(key_shift, Some(Key::Character("A".to_string())));
                            assert_eq!(text_shift, Some("A".to_string()));

                            // Test CapsLock 'A'
                            let (key_lock, text_lock) =
                                win.lookup_key(keycode, ModMask::LOCK.bits().into());
                            assert_eq!(key_lock, Some(Key::Character("A".to_string())));
                            assert_eq!(text_lock, Some("A".to_string()));

                            // Test Shift + CapsLock (cancels out to lowercase 'a')
                            let (key_both, text_both) = win.lookup_key(
                                keycode,
                                (ModMask::SHIFT.bits() | ModMask::LOCK.bits()).into(),
                            );
                            assert_eq!(key_both, Some(Key::Character("a".to_string())));
                            assert_eq!(text_both, Some("a".to_string()));
                            break;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_x11_cursor_initialization_and_set_cursor() {
        if let Ok((_conn, _screen_num)) = RustConnection::connect(None) {
            let win = X11Window::new("Cursor Test", 200, 200, None);
            if let Ok(win) = win {
                assert!(win.cursor_default.is_some());
                assert!(win.cursor_text.is_some());
                assert!(win.cursor_pointer.is_some());
                assert!(win.cursor_col_resize.is_some());
                assert!(win.cursor_row_resize.is_some());

                // Test switching cursors
                win.set_cursor(CursorIcon::Text);
                win.set_cursor(CursorIcon::Pointer);
                win.set_cursor(CursorIcon::ColResize);
                win.set_cursor(CursorIcon::RowResize);
                win.set_cursor(CursorIcon::Default);
            }
        }
    }

    #[test]
    fn test_desktop_cursor_theme_and_size_detection() {
        let theme = detect_desktop_cursor_theme();
        if let Some(theme) = theme {
            assert!(!theme.is_empty());
        }
        let size = detect_desktop_cursor_size();
        if let Some(size) = size {
            assert!(size >= 16 && size <= 128);
        }
    }
}


