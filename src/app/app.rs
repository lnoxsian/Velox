use std::collections::HashMap;
use std::sync::Arc;
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin_winit::GlWindow;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowId};

use crate::cli::CliOptions;
use crate::ipc::{start_ipc_server, IpcListenerHandle};
use crate::pty::master::PtyMaster;
use crate::pty::process::spawn_process;
use crate::renderer::renderer::Renderer;
use crate::screen::cell::{Cell, CellFlags};
use crate::terminal::terminal::Terminal;

pub enum CustomEvent {
    PtyData { window_id: WindowId, data: Vec<u8> },
    PtyExit { window_id: WindowId },
    IpcCreateWindow {
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        title: Option<String>,
        hold: Option<bool>,
    },
}

pub struct WindowState {
    pub renderer: Renderer,
    pub gl_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    pub gl_context: glutin::context::PossiblyCurrentContext,
    pub pty_master: Arc<PtyMaster>,
    pub terminal: Terminal,
    pub window: Window,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub render_cells_buf: Vec<Cell>,
    pub scroll_multiplier: f64,
    pub fps_limit: Option<u32>,
    pub last_frame_instant: std::time::Instant,
    pub current_title: String,
    pub default_font_size: f32,
    pub current_font_size: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub is_mouse_down: bool,
    pub last_click_instant: Option<std::time::Instant>,
    pub last_click_pos: (usize, usize),
    pub click_count: u8,
    pub last_mouse_cell: (usize, usize),
    pub is_focused: bool,
    pub needs_redraw: bool,
    pub content_dirty: bool,
    pub last_title_check: std::time::Instant,
    pub cursor_blink_enabled: bool,
    pub cursor_blink_on: bool,
    pub last_cursor_blink: std::time::Instant,
    pub hold: bool,
}

impl Drop for WindowState {
    fn drop(&mut self) {
        let _ = self.gl_context.make_current(&self.gl_surface);
    }
}

impl WindowState {
    pub fn draw(&mut self) {
        let _ = self.gl_context.make_current(&self.gl_surface);
        self.last_frame_instant = std::time::Instant::now();

        let active_grid = self.terminal.active_grid();
        let width = active_grid.width;
        let height = active_grid.height;
        let size = width * height;

        if self.render_cells_buf.len() != size {
            let default_cell = Cell {
                character: ' ',
                foreground: active_grid.default_fg,
                background: active_grid.default_bg,
                flags: CellFlags::empty(),
            };
            self.render_cells_buf.resize(size, default_cell);
        }

        let offset = active_grid.scroll_offset;
        let history_len = active_grid.scrollback.len();

        if offset == 0 {
            self.render_cells_buf.copy_from_slice(&active_grid.cells);
        } else {
            let default_cell = Cell {
                character: ' ',
                foreground: active_grid.default_fg,
                background: active_grid.default_bg,
                flags: CellFlags::empty(),
            };
            for y in 0..height {
                let dest_start = y * width;
                let dest_end = dest_start + width;

                let idx = (y + history_len).saturating_sub(offset);
                if y + history_len < offset {
                    self.render_cells_buf[dest_start..dest_end].fill(default_cell);
                } else if idx < history_len {
                    let row_data = active_grid
                        .scrollback
                        .get_row(idx)
                        .unwrap_or_else(|| crate::screen::scrollback::Row {
                            cells: vec![default_cell; width],
                            wrapped: false,
                        });
                    let line_slice = &row_data;
                    let copy_len = line_slice.len().min(width);
                    self.render_cells_buf[dest_start..dest_start + copy_len]
                        .copy_from_slice(&line_slice[..copy_len]);
                    if copy_len < width {
                        self.render_cells_buf[dest_start + copy_len..dest_end].fill(default_cell);
                    }
                } else {
                    let grid_y = idx - history_len;
                    let src_start = grid_y * width;
                    let src_end = src_start + width;
                    if src_end <= active_grid.cells.len() {
                        self.render_cells_buf[dest_start..dest_end]
                            .copy_from_slice(&active_grid.cells[src_start..src_end]);
                    } else {
                        self.render_cells_buf[dest_start..dest_end].fill(default_cell);
                    }
                }
            }
        }

        // Auto-detect URLs in visible rows and apply UNDERLINE styling
        for y in 0..height {
            let row_start = y * width;
            let line_text: String = (0..width)
                .map(|x| self.render_cells_buf[row_start + x].character)
                .collect();
            let urls = crate::hyperlink::detector::detect(&line_text);
            for (start_col, end_col, _) in urls {
                for col in start_col..end_col.min(width) {
                    self.render_cells_buf[row_start + col]
                        .flags
                        .insert(CellFlags::UNDERLINE);
                }
            }
        }

        let cursor_visible = if offset > 0 {
            false
        } else if self.cursor_blink_enabled {
            active_grid.cursor.visible && self.cursor_blink_on
        } else {
            active_grid.cursor.visible
        };

        // Throttle title updates
        if self.last_title_check.elapsed() >= std::time::Duration::from_secs(1) {
            self.last_title_check = std::time::Instant::now();
            let program = self
                .pty_master
                .get_foreground_process_name()
                .unwrap_or_else(|| "velox".to_string());
            let title = match &self.terminal.app_title {
                Some(tpl) => tpl.replace("{program}", &program),
                None => program,
            };
            if self.current_title != title {
                self.window.set_title(&title);
                self.current_title = title;
            }
        }

        let cursor_shape = if !self.is_focused
            && active_grid.cursor.shape == crate::screen::cursor::CursorShape::Block
        {
            crate::screen::cursor::CursorShape::HollowBlock
        } else {
            active_grid.cursor.shape
        };

        let display_cursor_x = active_grid.cursor.x.min((width as usize).saturating_sub(1));

        self.renderer.draw(
            &self.render_cells_buf,
            width,
            height,
            display_cursor_x,
            active_grid.cursor.y,
            cursor_visible,
            cursor_shape,
            &self.terminal.theme,
            self.terminal.bold_is_bright,
            &active_grid.selection,
            self.padding_x,
            self.padding_y,
        );

        let _ = self.gl_surface.swap_buffers(&self.gl_context);
    }
}

pub struct App {
    pub(crate) event_loop_proxy: EventLoopProxy<CustomEvent>,
    pub(crate) modifiers: winit::keyboard::ModifiersState,
    pub(crate) gl_display: Option<glutin::display::Display>,
    pub(crate) gl_config: Option<glutin::config::Config>,
    pub(crate) gl: Option<Arc<glow::Context>>,
    pub(crate) windows: HashMap<WindowId, WindowState>,
    pub(crate) daemon_mode: bool,
    pub(crate) single_instance_mode: bool,
    pub(crate) ipc_listener: Option<IpcListenerHandle>,
    pub(crate) initial_options: Option<CliOptions>,
}

impl App {
    pub fn new(event_loop_proxy: EventLoopProxy<CustomEvent>, options: CliOptions) -> Self {
        let daemon_mode = options.daemon;
        let single_instance_mode = options.single_instance;

        Self {
            event_loop_proxy,
            modifiers: winit::keyboard::ModifiersState::default(),
            gl_display: None,
            gl_config: None,
            gl: None,
            windows: HashMap::new(),
            daemon_mode,
            single_instance_mode,
            ipc_listener: None,
            initial_options: Some(options),
        }
    }

    pub fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        custom_title: Option<String>,
        hold: Option<bool>,
    ) {
        let config = crate::config::loader::load()
            .unwrap_or_else(|_| crate::config::defaults::default_config());

        let initial_title = match custom_title {
            Some(t) => t,
            None => match &config.app_title {
                Some(tpl) => tpl.replace("{program}", "velox"),
                None => "velox".to_string(),
            },
        };

        let icon = load_app_icon();

        let mut window_attributes = Window::default_attributes()
            .with_title(&initial_title)
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));

        if let Some(icon) = icon {
            window_attributes = window_attributes.with_window_icon(Some(icon));
        }

        let gl_config = match &self.gl_config {
            Some(cfg) => cfg.clone(),
            None => return,
        };
        let gl_display = match &self.gl_display {
            Some(disp) => disp.clone(),
            None => return,
        };
        let gl = match &self.gl {
            Some(g) => g.clone(),
            None => return,
        };

        let window = event_loop.create_window(window_attributes).unwrap();

        let context_attributes = glutin::context::ContextAttributesBuilder::new()
            .with_context_api(glutin::context::ContextApi::OpenGl(Some(
                glutin::context::Version::new(3, 3),
            )))
            .build(Some(window.window_handle().unwrap().as_raw()));

        let gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap()
        };

        let attrs = window.build_surface_attributes(<_>::default()).unwrap();
        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
                .unwrap()
        };

        let gl_context = gl_context.make_current(&gl_surface).unwrap();

        let scroll_multiplier = config.scroll_multiplier.unwrap_or(1.0);
        let cursor_blink_enabled = config.cursor_blink.unwrap_or(true);
        let gpu = config.gpu_acceleration.unwrap_or(true);
        let fps_limit = match config.fps_limit {
            Some(limit) => Some(limit),
            None => {
                if gpu {
                    Some(120)
                } else {
                    Some(60)
                }
            }
        };

        let font_size = config.font_size;
        let padding_x = config.padding_x.unwrap_or(8.0);
        let padding_y = config.padding_y.unwrap_or(4.0);
        let font_scale_multiplier = config.font_scale_multiplier.unwrap_or(1.5);

        let renderer = Renderer::new(
            gl.clone(),
            &config.font_family,
            font_size,
            font_scale_multiplier,
        );

        let avail_w = (800.0 - padding_x * 2.0).max(10.0);
        let avail_h = (600.0 - padding_y * 2.0).max(10.0);
        let cols = ((avail_w as u32) / renderer.font_loader.cell_width).max(20);
        let rows = ((avail_h as u32) / renderer.font_loader.cell_height).max(10);

        let terminal = Terminal::new(cols as usize, rows as usize);

        let shell_path = config
            .shell
            .clone()
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());

        let pty_master = Arc::new(
            spawn_process(&shell_path, command.as_deref(), working_directory.as_deref())
                .unwrap(),
        );
        pty_master.resize(cols as u16, rows as u16).unwrap();

        let window_id = window.id();
        let proxy = self.event_loop_proxy.clone();
        let pty_reader = pty_master.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = proxy.send_event(CustomEvent::PtyExit { window_id });
                        break;
                    }
                    Ok(n) => {
                        let _ = proxy.send_event(CustomEvent::PtyData {
                            window_id,
                            data: buf[..n].to_vec(),
                        });
                    }
                    Err(_) => {
                        let _ = proxy.send_event(CustomEvent::PtyExit { window_id });
                        break;
                    }
                }
            }
        });

        let window_state = WindowState {
            window,
            gl_surface,
            gl_context,
            renderer,
            terminal,
            pty_master,
            mouse_x: 0.0,
            mouse_y: 0.0,
            render_cells_buf: Vec::new(),
            scroll_multiplier,
            fps_limit,
            last_frame_instant: std::time::Instant::now(),
            current_title: initial_title,
            default_font_size: font_size,
            current_font_size: font_size,
            padding_x,
            padding_y,
            is_mouse_down: false,
            last_click_instant: None,
            last_click_pos: (0, 0),
            click_count: 0,
            last_mouse_cell: (usize::MAX, usize::MAX),
            is_focused: true,
            needs_redraw: true,
            content_dirty: true,
            last_title_check: std::time::Instant::now(),
            cursor_blink_enabled,
            cursor_blink_on: true,
            last_cursor_blink: std::time::Instant::now(),
            hold: hold.unwrap_or(false),
        };

        self.windows.insert(window_id, window_state);
    }
}

fn load_app_icon() -> Option<winit::window::Icon> {
    let icon_bytes = include_bytes!("../../assets/generated_icons/icon_128x128.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(icon_bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let raw_bytes = &buf[..info.buffer_size()];

    let rgba_bytes = match info.color_type {
        png::ColorType::Rgba => raw_bytes.to_vec(),
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
            for chunk in raw_bytes.chunks(3) {
                if chunk.len() == 3 {
                    rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                }
            }
            rgba
        }
        _ => return None,
    };

    winit::window::Icon::from_rgba(rgba_bytes, info.width, info.height).ok()
}

impl ApplicationHandler<CustomEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = crate::config::loader::load()
            .unwrap_or_else(|_| crate::config::defaults::default_config());

        if config.single_instance.unwrap_or(true) {
            self.single_instance_mode = true;
        }

        if self.gl_display.is_none() {
            let template = glutin::config::ConfigTemplateBuilder::new().with_alpha_size(8);

            let dummy_attrs = Window::default_attributes()
                .with_visible(false)
                .with_inner_size(winit::dpi::PhysicalSize::new(1, 1));
            let display_builder =
                glutin_winit::DisplayBuilder::new().with_window_attributes(Some(dummy_attrs));

            let (dummy_window, gl_config) = display_builder
                .build(event_loop, template, |configs| {
                    configs
                        .reduce(|accum, config| {
                            if config.num_samples() > accum.num_samples() {
                                config
                            } else {
                                accum
                            }
                        })
                        .unwrap()
                })
                .unwrap();

            let dummy_window = dummy_window.unwrap();
            let gl_display = gl_config.display();

            let context_attributes = glutin::context::ContextAttributesBuilder::new()
                .with_context_api(glutin::context::ContextApi::OpenGl(Some(
                    glutin::context::Version::new(3, 3),
                )))
                .build(Some(dummy_window.window_handle().unwrap().as_raw()));

            let dummy_context = unsafe {
                gl_display
                    .create_context(&gl_config, &context_attributes)
                    .unwrap()
            };

            let attrs = dummy_window.build_surface_attributes(<_>::default()).unwrap();
            let dummy_surface = unsafe {
                gl_config
                    .display()
                    .create_window_surface(&gl_config, &attrs)
                    .unwrap()
            };

            let _dummy_current = dummy_context.make_current(&dummy_surface).unwrap();

            let gl = unsafe {
                glow::Context::from_loader_function(|symbol| {
                    let c_str = std::ffi::CString::new(symbol).unwrap();
                    gl_display.get_proc_address(&c_str)
                })
            };

            self.gl_config = Some(gl_config);
            self.gl_display = Some(gl_display);
            self.gl = Some(Arc::new(gl));

            drop(_dummy_current);
            drop(dummy_surface);
            drop(dummy_window);
        }

        if (self.single_instance_mode || self.daemon_mode) && self.ipc_listener.is_none() {
            match start_ipc_server(self.event_loop_proxy.clone()) {
                Ok(handle) => {
                    self.ipc_listener = Some(handle);
                }
                Err(e) => {
                    log::warn!("Failed to start IPC server: {}", e);
                }
            }
        }

        if let Some(opts) = self.initial_options.take() {
            if !opts.daemon {
                self.create_window(
                    event_loop,
                    opts.working_directory,
                    opts.command,
                    opts.title,
                    Some(opts.hold),
                );
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.windows.remove(&window_id);
            if self.windows.is_empty() && !self.daemon_mode {
                event_loop.exit();
            }
            return;
        }

        if let WindowEvent::ModifiersChanged(ref mods) = event {
            self.modifiers = mods.state();
        }

        let modifiers = self.modifiers;
        if let Some(ws) = self.windows.get_mut(&window_id) {
            match event {
                WindowEvent::Focused(focused) => {
                    ws.is_focused = focused;
                    if ws.terminal.focus_tracking {
                        let seq = if focused { b"\x1b[I" } else { b"\x1b[O" };
                        let _ = ws.pty_master.write(seq);
                    }
                    ws.needs_redraw = true;
                }
                WindowEvent::Resized(size) => {
                    ws.window.resize_surface(&ws.gl_surface, &ws.gl_context);
                    ws.renderer.resize(size.width, size.height);

                    let avail_w = (size.width as f32 - ws.padding_x * 2.0).max(10.0);
                    let avail_h = (size.height as f32 - ws.padding_y * 2.0).max(10.0);
                    let cols = ((avail_w as u32) / ws.renderer.font_loader.cell_width).max(20);
                    let rows = ((avail_h as u32) / ws.renderer.font_loader.cell_height).max(10);
                    ws.terminal.resize(cols, rows);
                    let _ = ws.pty_master.resize(cols as u16, rows as u16);
                    ws.needs_redraw = true;
                    ws.content_dirty = true;
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    ws.handle_keyboard_input(event, modifiers);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    ws.handle_cursor_moved(position, modifiers);
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    ws.handle_mouse_wheel(delta, modifiers);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    ws.handle_mouse_input(state, button, modifiers);
                }
                WindowEvent::RedrawRequested => {
                    ws.draw();
                }
                _ => {}
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: CustomEvent) {
        match event {
            CustomEvent::PtyData { window_id, data } => {
                if let Some(ws) = self.windows.get_mut(&window_id) {
                    ws.terminal.feed(&data);
                    if !ws.terminal.outgoing.is_empty() {
                        let _ = ws.pty_master.write(&ws.terminal.outgoing);
                        ws.terminal.outgoing.clear();
                    }
                    ws.needs_redraw = true;
                    ws.content_dirty = true;
                }
            }
            CustomEvent::PtyExit { window_id } => {
                if let Some(ws) = self.windows.get(&window_id) {
                    if ws.hold {
                        ws.window.set_title("[Process exited]");
                        return;
                    }
                }
                self.windows.remove(&window_id);
                if self.windows.is_empty() && !self.daemon_mode {
                    event_loop.exit();
                }
            }
            CustomEvent::IpcCreateWindow {
                working_directory,
                command,
                title,
                hold,
            } => {
                self.create_window(event_loop, working_directory, command, title, hold);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = std::time::Instant::now();
        let mut min_next_wake: Option<std::time::Instant> = None;

        for ws in self.windows.values_mut() {
            // Cursor blink toggle
            if ws.cursor_blink_enabled
                && now.duration_since(ws.last_cursor_blink) >= std::time::Duration::from_millis(500)
            {
                ws.cursor_blink_on = !ws.cursor_blink_on;
                ws.last_cursor_blink = now;
                ws.needs_redraw = true;
            }

            if ws.cursor_blink_enabled {
                let next_blink = ws.last_cursor_blink + std::time::Duration::from_millis(500);
                min_next_wake = Some(min_next_wake.map_or(next_blink, |t| t.min(next_blink)));
            }

            // Redraw scheduling
            if ws.needs_redraw {
                if ws.terminal.is_synchronized_output_active() {
                    continue;
                }

                let frame_duration = ws
                    .fps_limit
                    .filter(|&l| l > 0)
                    .map(|l| std::time::Duration::from_secs_f64(1.0 / l as f64))
                    .unwrap_or(std::time::Duration::from_millis(8));

                let next_frame = ws.last_frame_instant + frame_duration;
                if now >= next_frame {
                    ws.window.request_redraw();
                    ws.needs_redraw = false;
                } else {
                    min_next_wake = Some(min_next_wake.map_or(next_frame, |t| t.min(next_frame)));
                }
            }
        }

        if let Some(wake_time) = min_next_wake {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(wake_time));
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }
}
