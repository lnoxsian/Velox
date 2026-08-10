use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};
use winit::event::WindowEvent;
use winit::raw_window_handle::HasWindowHandle;
use glutin::prelude::*;
use glutin::display::GetGlDisplay;
use glutin_winit::GlWindow;

use crate::terminal::terminal::Terminal;
use crate::renderer::renderer::Renderer;
use crate::pty::process::spawn_shell;
use crate::pty::master::PtyMaster;

use crate::screen::cell::{Cell, CellFlags};


pub enum CustomEvent {
    PtyData(Vec<u8>),
    PtyExit,
}

pub struct App {
    pub(crate) event_loop_proxy: winit::event_loop::EventLoopProxy<CustomEvent>,
    pub(crate) modifiers: winit::keyboard::ModifiersState,
    pub(crate) window: Option<Window>,
    pub(crate) gl: Option<Arc<glow::Context>>,
    pub(crate) gl_context: Option<glutin::context::PossiblyCurrentContext>,
    pub(crate) gl_display: Option<glutin::display::Display>,
    pub(crate) gl_surface: Option<glutin::surface::Surface<glutin::surface::WindowSurface>>,
    pub(crate) renderer: Option<Renderer>,
    pub(crate) terminal: Option<Terminal>,
    pub(crate) pty_master: Option<Arc<PtyMaster>>,
    pub(crate) mouse_x: f64,
    pub(crate) mouse_y: f64,
    pub(crate) render_cells_buf: Vec<Cell>,
    pub(crate) scroll_multiplier: f64,
    pub(crate) fps_limit: Option<u32>,
    pub(crate) last_frame_instant: std::time::Instant,
    pub(crate) current_title: String,
    pub(crate) default_font_size: f32,
    pub(crate) current_font_size: f32,
    pub(crate) padding_x: f32,
    pub(crate) padding_y: f32,
    pub(crate) is_mouse_down: bool,
    pub(crate) last_click_instant: Option<std::time::Instant>,
    pub(crate) last_click_pos: (usize, usize),
    pub(crate) click_count: u8,
    pub(crate) last_mouse_cell: (usize, usize),
    pub(crate) is_focused: bool,
    // ── CPU-optimization state ───────────────────────────────────────────
    pub(crate) needs_redraw: bool,
    pub(crate) content_dirty: bool,
    pub(crate) last_title_check: std::time::Instant,
    pub(crate) cursor_blink_enabled: bool,
    pub(crate) cursor_blink_on: bool,
    pub(crate) last_cursor_blink: std::time::Instant,
}

impl App {
    pub fn new(event_loop_proxy: winit::event_loop::EventLoopProxy<CustomEvent>) -> Self {
        Self {
            event_loop_proxy,
            modifiers: winit::keyboard::ModifiersState::default(),
            window: None,
            gl: None,
            gl_context: None,
            gl_display: None,
            gl_surface: None,
            renderer: None,
            terminal: None,
            pty_master: None,
            mouse_x: 0.0,
            mouse_y: 0.0,
            render_cells_buf: Vec::new(),
            scroll_multiplier: 1.0,
            fps_limit: None,
            last_frame_instant: std::time::Instant::now(),
            current_title: String::new(),
            default_font_size: 14.0,
            current_font_size: 14.0,
            padding_x: 8.0,
            padding_y: 4.0,
            is_mouse_down: false,
            last_click_instant: None,
            last_click_pos: (0, 0),
            click_count: 0,
            last_mouse_cell: (usize::MAX, usize::MAX),
            is_focused: true,
            needs_redraw: true,
            content_dirty: true,
            last_title_check: std::time::Instant::now(),
            cursor_blink_enabled: true,
            cursor_blink_on: true,
            last_cursor_blink: std::time::Instant::now(),
        }
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
        let config = crate::config::loader::load().unwrap_or_else(|_| {
            crate::config::defaults::default_config()
        });

        let initial_title = match &config.app_title {
            Some(tpl) => tpl.replace("{program}", "velox"),
            None => "velox".to_string(),
        };
        self.current_title = initial_title.clone();

        let icon = load_app_icon();

        let mut window_attributes = Window::default_attributes()
            .with_title(&initial_title)
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));

        if let Some(icon) = icon {
            window_attributes = window_attributes.with_window_icon(Some(icon));
        }

        let template = glutin::config::ConfigTemplateBuilder::new()
            .with_alpha_size(8);

        let display_builder = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes));

        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        let accum_samples = accum.num_samples();
                        let config_samples = config.num_samples();
                        if config_samples > accum_samples {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .unwrap();

        let window = window.unwrap();

        let gl_display = gl_config.display();
        let context_attributes = glutin::context::ContextAttributesBuilder::new()
            .with_context_api(glutin::context::ContextApi::OpenGl(Some(glutin::context::Version::new(3, 3))))
            .build(Some(window.window_handle().unwrap().as_raw()));
        
        let gl_context = unsafe {
            gl_display.create_context(&gl_config, &context_attributes).unwrap()
        };

        let attrs = window.build_surface_attributes(<_>::default()).unwrap();

        let gl_surface = unsafe {
            gl_config.display().create_window_surface(&gl_config, &attrs).unwrap()
        };

        let gl_context = gl_context.make_current(&gl_surface).unwrap();

        let gl = unsafe {
            glow::Context::from_loader_function(|symbol| {
                let c_str = std::ffi::CString::new(symbol).unwrap();
                gl_display.get_proc_address(&c_str)
            })
        };
        let gl = Arc::new(gl);

        self.scroll_multiplier = config.scroll_multiplier.unwrap_or(1.0);
        self.cursor_blink_enabled = config.cursor_blink.unwrap_or(true);
        // When software rendering, default to 60 fps to conserve CPU
        let gpu = config.gpu_acceleration.unwrap_or(true);
        self.fps_limit = match config.fps_limit {
            Some(limit) => Some(limit),
            None => if gpu { Some(120) } else { Some(60) },
        };
        self.default_font_size = config.font_size;
        self.current_font_size = config.font_size;
        self.padding_x = config.padding_x.unwrap_or(8.0);
        self.padding_y = config.padding_y.unwrap_or(4.0);
        let font_scale_multiplier = config.font_scale_multiplier.unwrap_or(1.5);
        let renderer = Renderer::new(gl.clone(), &config.font_family, config.font_size, font_scale_multiplier);
        
        let avail_w = (800.0 - self.padding_x * 2.0).max(10.0);
        let avail_h = (600.0 - self.padding_y * 2.0).max(10.0);
        let cols = ((avail_w as u32) / renderer.font_loader.cell_width).max(20);
        let rows = ((avail_h as u32) / renderer.font_loader.cell_height).max(10);

        let terminal = Terminal::new(cols as usize, rows as usize);
        
        // Spawn PTY shell
        let shell_path = config.shell.clone()
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());
        let pty_master = Arc::new(spawn_shell(&shell_path).unwrap());
        pty_master.resize(cols as u16, rows as u16).unwrap();

        // Spawn PTY Reader Thread
        let proxy = self.event_loop_proxy.clone();
        let pty_reader = pty_master.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = proxy.send_event(CustomEvent::PtyExit);
                        break;
                    }
                    Ok(n) => {
                        let _ = proxy.send_event(CustomEvent::PtyData(buf[..n].to_vec()));
                    }
                    Err(_) => {
                        let _ = proxy.send_event(CustomEvent::PtyExit);
                        break;
                    }
                }
            }
        });

        self.window = Some(window);
        self.gl = Some(gl);
        self.gl_context = Some(gl_context);
        self.gl_display = Some(gl_display);
        self.gl_surface = Some(gl_surface);
        self.renderer = Some(renderer);
        self.terminal = Some(terminal);
        self.pty_master = Some(pty_master);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Focused(focused) => {
                self.is_focused = focused;
                if let (Some(terminal), Some(pty_master)) = (&mut self.terminal, &self.pty_master)
                    && terminal.focus_tracking {
                        let seq = if focused { b"\x1b[I" } else { b"\x1b[O" };
                        let _ = pty_master.write(seq);
                    }
                self.needs_redraw = true;
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::Resized(size) => {
                if let (Some(window), Some(gl_surface), Some(gl_context), Some(renderer), Some(terminal), Some(pty_master)) = 
                   (&self.window, &self.gl_surface, &self.gl_context, &mut self.renderer, &mut self.terminal, &self.pty_master) 
                {
                    window.resize_surface(gl_surface, gl_context);
                    renderer.resize(size.width, size.height);
                    
                    let avail_w = (size.width as f32 - self.padding_x * 2.0).max(10.0);
                    let avail_h = (size.height as f32 - self.padding_y * 2.0).max(10.0);
                    let cols = ((avail_w as u32) / renderer.font_loader.cell_width).max(20);
                    let rows = ((avail_h as u32) / renderer.font_loader.cell_height).max(10);
                    terminal.resize(cols, rows);
                    let _ = pty_master.resize(cols as u16, rows as u16);
                }
                self.needs_redraw = true;
                self.content_dirty = true;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard_input(event);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(position);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(delta);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(state, button);
            }
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(renderer), Some(terminal), Some(gl_surface), Some(gl_context)) = 
                   (&self.window, &mut self.renderer, &self.terminal, &self.gl_surface, &self.gl_context) 
                {
                    self.last_frame_instant = std::time::Instant::now();

                    let active_grid = terminal.active_grid();
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

                            // Guard against usize underflow when offset > history_len + y.
                            // That means we're trying to show rows before the scrollback buffer exists;
                            // fill them with blank cells.
                            let idx = (y + history_len).saturating_sub(offset);
                            if y + history_len < offset {
                                // Row is before the start of scrollback — fill blank
                                self.render_cells_buf[dest_start..dest_end].fill(default_cell);
                            } else if idx < history_len {
                                // Row comes from the scrollback buffer
                                let row_data = active_grid.scrollback.get_row(idx).unwrap_or_else(|| crate::screen::scrollback::Row { cells: vec![default_cell; width], wrapped: false });
                                let line_slice = &row_data;
                                let copy_len = line_slice.len().min(width);
                                self.render_cells_buf[dest_start..dest_start + copy_len]
                                    .copy_from_slice(&line_slice[..copy_len]);
                                if copy_len < width {
                                    self.render_cells_buf[dest_start + copy_len..dest_end].fill(default_cell);
                                }
                            } else {
                                // Row comes from the live grid
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
                                self.render_cells_buf[row_start + col].flags.insert(CellFlags::UNDERLINE);
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

                    // Throttle title-bar updates to at most once per second
                    if self.last_title_check.elapsed() >= std::time::Duration::from_secs(1) {
                        self.last_title_check = std::time::Instant::now();
                        let program = self.pty_master
                            .as_ref()
                            .and_then(|pty| pty.get_foreground_process_name())
                            .unwrap_or_else(|| "velox".to_string());
                        let title = match &terminal.app_title {
                            Some(tpl) => tpl.replace("{program}", &program),
                            None => program,
                        };
                        if self.current_title != title {
                            window.set_title(&title);
                            self.current_title = title;
                        }
                    }

                    let cursor_shape = if !self.is_focused && active_grid.cursor.shape == crate::screen::cursor::CursorShape::Block {
                        crate::screen::cursor::CursorShape::HollowBlock
                    } else {
                        active_grid.cursor.shape
                    };

                    renderer.draw(
                        &self.render_cells_buf,
                        width,
                        height,
                        active_grid.cursor.x,
                        active_grid.cursor.y,
                        cursor_visible,
                        cursor_shape,
                        &terminal.theme,
                        terminal.bold_is_bright,
                        &active_grid.selection,
                        self.padding_x,
                        self.padding_y,
                    );
                    gl_surface.swap_buffers(gl_context).unwrap();
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: CustomEvent) {
        match event {
            CustomEvent::PtyData(data) => {
                if let Some(terminal) = &mut self.terminal {
                    terminal.feed(&data);
                    if !terminal.outgoing.is_empty() {
                        if let Some(pty_master) = &self.pty_master {
                            let _ = pty_master.write(&terminal.outgoing);
                        }
                        terminal.outgoing.clear();
                    }
                }
                self.needs_redraw = true;
                self.content_dirty = true;
            }
            CustomEvent::PtyExit => {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = std::time::Instant::now();

        // ── Cursor blink: toggle every 500 ms ────────────────────────────────
        if self.cursor_blink_enabled && now.duration_since(self.last_cursor_blink) >= std::time::Duration::from_millis(500) {
            self.cursor_blink_on = !self.cursor_blink_on;
            self.last_cursor_blink = now;
            self.needs_redraw = true;
        }

        // ── Schedule pending redraw with non-blocking FPS limiting ───────────
        if self.needs_redraw {
            if let Some(term) = &mut self.terminal
                && term.is_synchronized_output_active() {
                    return;
                }

            let frame_duration = self.fps_limit
                .filter(|&l| l > 0)
                .map(|l| std::time::Duration::from_secs_f64(1.0 / l as f64))
                .unwrap_or(std::time::Duration::from_millis(8));

            let next_frame = self.last_frame_instant + frame_duration;
            if now >= next_frame {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                self.needs_redraw = false;
            } else {
                // Defer until the FPS budget allows — no thread::sleep blocking
                event_loop.set_control_flow(
                    winit::event_loop::ControlFlow::WaitUntil(next_frame),
                );
                return;
            }
        }

        // ── Idle: sleep until the next cursor-blink toggle or wait ───────────
        if self.cursor_blink_enabled {
            let next_blink = self.last_cursor_blink + std::time::Duration::from_millis(500);
            event_loop.set_control_flow(
                winit::event_loop::ControlFlow::WaitUntil(next_blink),
            );
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }
}
