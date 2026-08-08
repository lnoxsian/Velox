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

#[derive(Debug)]
pub enum AppError {
    Initialization(String),
}

pub enum CustomEvent {
    PtyData(Vec<u8>),
    PtyExit,
}

pub struct App {
    event_loop_proxy: winit::event_loop::EventLoopProxy<CustomEvent>,
    modifiers: winit::keyboard::ModifiersState,
    window: Option<Window>,
    gl: Option<Arc<glow::Context>>,
    gl_context: Option<glutin::context::PossiblyCurrentContext>,
    gl_display: Option<glutin::display::Display>,
    gl_surface: Option<glutin::surface::Surface<glutin::surface::WindowSurface>>,
    renderer: Option<Renderer>,
    terminal: Option<Terminal>,
    pty_master: Option<Arc<PtyMaster>>,
    mouse_x: f64,
    mouse_y: f64,
    render_cells_buf: Vec<Cell>,
    scroll_multiplier: f64,
    fps_limit: Option<u32>,
    last_frame_instant: std::time::Instant,
    current_title: String,
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
        }
    }

    pub fn initialize(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    pub fn render(&mut self) {
        // stub
    }

    pub fn shutdown(&mut self) {
        // stub
    }
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

        let window_attributes = Window::default_attributes()
            .with_title(&initial_title)
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));

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
        self.fps_limit = config.fps_limit;
        let renderer = Renderer::new(gl.clone(), &config.font_family, config.font_size, config.enable_nerdfont.unwrap_or(true));
        let cols = (800 / renderer.font_loader.cell_width).max(20);
        let rows = (600 / renderer.font_loader.cell_height).max(10);

        let terminal = Terminal::new(cols as usize, rows as usize);
        
        // Spawn PTY shell
        let shell_path = std::env::var("SHELL").unwrap_or_else(|_| {
            config.shell.clone()
        });
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
                    
                    let cols = (size.width / renderer.font_loader.cell_width).max(20);
                    let rows = (size.height / renderer.font_loader.cell_height).max(10);
                    terminal.resize(cols, rows);
                    let _ = pty_master.resize(cols as u16, rows as u16);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    if self.modifiers.shift_key() {
                        if let winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageUp) = event.logical_key {
                            if let Some(terminal) = &mut self.terminal {
                                let active_grid = if terminal.is_alt_screen { &mut terminal.alt_grid } else { &mut terminal.grid };
                                let history_len = active_grid.scrollback.lines.len();
                                active_grid.scroll_offset = (active_grid.scroll_offset + active_grid.height / 2).min(history_len);
                                return;
                            }
                        } else if let winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageDown) = event.logical_key {
                            if let Some(terminal) = &mut self.terminal {
                                let active_grid = if terminal.is_alt_screen { &mut terminal.alt_grid } else { &mut terminal.grid };
                                active_grid.scroll_offset = active_grid.scroll_offset.saturating_sub(active_grid.height / 2);
                                return;
                            }
                        }
                    }

                    if let Some(pty_master) = &self.pty_master {
                        let cursor_keys_mode = self.terminal.as_ref().map(|t| t.cursor_keys_mode).unwrap_or(false);
                        if let Some(bytes) = crate::input::keyboard::translate_key(&event.logical_key, self.modifiers, cursor_keys_mode) {
                            let _ = pty_master.write(&bytes);
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x;
                self.mouse_y = position.y;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines_f = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        y as f64
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        pos.y / 15.0
                    }
                };
                let lines = (lines_f * self.scroll_multiplier).round() as i32;
                if lines != 0 {
                    if let Some(pty_master) = &self.pty_master {
                        if let (Some(terminal), Some(renderer)) = (&mut self.terminal, &self.renderer) {
                            let cw = renderer.font_loader.cell_width as f64;
                            let ch = renderer.font_loader.cell_height as f64;
                            let col = ((self.mouse_x / cw).floor() as i32 + 1).max(1);
                            let row = ((self.mouse_y / ch).floor() as i32 + 1).max(1);

                            if terminal.mouse_mode > 0 {
                                let btn = if lines > 0 { 64 } else { 65 };
                                for _ in 0..lines.abs() {
                                    let seq = if terminal.mouse_sgr {
                                        format!("\x1b[<{};{};{}M", btn, col, row)
                                    } else {
                                        let cb = 32 + btn;
                                        let cx = 32 + col;
                                        let cy = 32 + row;
                                        if cx <= 255 && cy <= 255 {
                                            format!("\x1b[M{}{}{}", cb as u8 as char, cx as u8 as char, cy as u8 as char)
                                        } else {
                                            String::new()
                                        }
                                    };
                                    if !seq.is_empty() {
                                        let _ = pty_master.write(seq.as_bytes());
                                    }
                                }
                            } else if terminal.is_alt_screen {
                                let key_seq = if lines > 0 {
                                    if terminal.cursor_keys_mode { b"\x1bOA" } else { b"\x1b[A" }
                                } else {
                                    if terminal.cursor_keys_mode { b"\x1bOB" } else { b"\x1b[B" }
                                };
                                for _ in 0..lines.abs() {
                                    let _ = pty_master.write(key_seq);
                                }
                            } else {
                                let active_grid = if terminal.is_alt_screen { &mut terminal.alt_grid } else { &mut terminal.grid };
                                let history_len = active_grid.scrollback.lines.len();
                                if lines > 0 {
                                    active_grid.scroll_offset = (active_grid.scroll_offset + lines as usize).min(history_len);
                                } else if lines < 0 {
                                    active_grid.scroll_offset = active_grid.scroll_offset.saturating_sub(lines.abs() as usize);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state.is_pressed() && button == winit::event::MouseButton::Left {
                    if let (Some(renderer), Some(terminal)) = (&self.renderer, &self.terminal) {
                        let cw = renderer.font_loader.cell_width as f64;
                        let ch = renderer.font_loader.cell_height as f64;
                        let col_idx = (self.mouse_x / cw).floor() as usize;
                        let row_idx = (self.mouse_y / ch).floor() as usize;

                        let active_grid = terminal.active_grid();
                        let offset = active_grid.scroll_offset;
                        let history_len = active_grid.scrollback.lines.len();

                        if row_idx < active_grid.height && col_idx < active_grid.width {
                            // Reconstruct the text line that was clicked, accounting for scroll offset
                            let line_text: Option<String> = if offset == 0 {
                                Some((0..active_grid.width)
                                    .map(|x| active_grid.cells[row_idx * active_grid.width + x].character)
                                    .collect())
                            } else {
                                let idx = row_idx + history_len - offset;
                                if idx < history_len {
                                    let line_slice = &active_grid.scrollback.lines[idx];
                                    Some((0..active_grid.width)
                                        .map(|x| line_slice.get(x).map(|c| c.character).unwrap_or(' '))
                                        .collect())
                                } else {
                                    let grid_y = idx - history_len;
                                    Some((0..active_grid.width)
                                        .map(|x| active_grid.cells[grid_y * active_grid.width + x].character)
                                        .collect())
                                }
                            };

                            if let Some(line) = &line_text {
                                let urls = crate::hyperlink::detector::highlight(line);
                                for (start, end, url) in urls {
                                    if col_idx >= start && col_idx < end {
                                        let _ = crate::hyperlink::detector::open(&url);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(renderer), Some(terminal), Some(gl_surface), Some(gl_context)) = 
                   (&self.window, &mut self.renderer, &self.terminal, &self.gl_surface, &self.gl_context) 
                {
                    if let Some(limit) = self.fps_limit {
                        if limit > 0 {
                            let min_duration = std::time::Duration::from_secs_f64(1.0 / limit as f64);
                            let elapsed = self.last_frame_instant.elapsed();
                            if elapsed < min_duration {
                                std::thread::sleep(min_duration - elapsed);
                            }
                        }
                    }
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
                    let history_len = active_grid.scrollback.lines.len();

                    if offset == 0 {
                        self.render_cells_buf.copy_from_slice(&active_grid.cells);
                    } else {
                        for y in 0..height {
                            let idx = y + history_len - offset;
                            let dest_start = y * width;
                            let dest_end = dest_start + width;
                            if idx < history_len {
                                let line_slice = &active_grid.scrollback.lines[idx];
                                let copy_len = line_slice.len().min(width);
                                self.render_cells_buf[dest_start..dest_start + copy_len].copy_from_slice(&line_slice[..copy_len]);
                                if copy_len < width {
                                    let default_cell = Cell {
                                        character: ' ',
                                        foreground: active_grid.default_fg,
                                        background: active_grid.default_bg,
                                        flags: CellFlags::empty(),
                                    };
                                    self.render_cells_buf[dest_start + copy_len..dest_end].fill(default_cell);
                                }
                            } else {
                                let grid_y = idx - history_len;
                                let src_start = grid_y * width;
                                let src_end = src_start + width;
                                self.render_cells_buf[dest_start..dest_end].copy_from_slice(&active_grid.cells[src_start..src_end]);
                            }
                        }
                    }

                    // Auto-underline detected links in the render buffer
                    for y in 0..height {
                        let start_idx = y * width;
                        let line_chars: Vec<char> = (0..width)
                            .map(|x| self.render_cells_buf[start_idx + x].character)
                            .collect();
                        let line_text: String = line_chars.iter().collect();
                        
                        let urls = crate::hyperlink::detector::highlight(&line_text);
                        for (start, end, _) in urls {
                            for x in start..end {
                                if start_idx + x < self.render_cells_buf.len() {
                                    self.render_cells_buf[start_idx + x].flags.insert(CellFlags::UNDERLINE);
                                }
                            }
                        }
                    }

                    let cursor_visible = if offset > 0 { false } else { active_grid.cursor.visible };

                    // Update window title dynamically if it changed
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

                    renderer.draw(
                        &self.render_cells_buf,
                        width,
                        height,
                        active_grid.cursor.x,
                        active_grid.cursor.y,
                        cursor_visible,
                        active_grid.cursor.shape,
                        &terminal.theme,
                        terminal.bold_is_bright,
                    );
                    gl_surface.swap_buffers(gl_context).unwrap();
                    window.request_redraw();
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
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            CustomEvent::PtyExit => {
                event_loop.exit();
            }
        }
    }
}
