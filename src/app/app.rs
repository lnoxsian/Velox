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
        let window_attributes = Window::default_attributes()
            .with_title("Velox Terminal")
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

        let config = crate::config::loader::load().unwrap_or_else(|_| {
            crate::config::defaults::default_config()
        });
        let renderer = Renderer::new(gl.clone(), &config.font_family, config.font_size);
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
                    if let Some(pty_master) = &self.pty_master {
                        if let Some(bytes) = crate::input::keyboard::translate_key(&event.logical_key, self.modifiers) {
                            let _ = pty_master.write(&bytes);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(renderer), Some(terminal), Some(gl_surface), Some(gl_context)) = 
                   (&self.window, &mut self.renderer, &self.terminal, &self.gl_surface, &self.gl_context) 
                {
                    let active_grid = terminal.active_grid();
                    renderer.draw(
                        &active_grid.cells,
                        active_grid.width,
                        active_grid.height,
                        active_grid.cursor.x,
                        active_grid.cursor.y,
                        active_grid.cursor.visible,
                        terminal.theme.default_bg,
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
