use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin_winit::GlWindow;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowId};

use crate::app::tab::{Tab, TabBar, TabBarRenderInfo, TabHeaderInfo};
use crate::cli::CliOptions;
use crate::ipc::{IpcListenerHandle, start_ipc_server};
use crate::pty::master::PtyMaster;
use crate::pty::process::spawn_process;

fn spawn_pty_reader(
    pty_reader: Arc<PtyMaster>,
    proxy: EventLoopProxy<CustomEvent>,
    window_id: WindowId,
    tab_id: u64,
) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 65536];
        loop {
            match pty_reader.read(&mut buf) {
                Ok(0) => {
                    let _ = proxy.send_event(CustomEvent::PtyExit { window_id, tab_id });
                    break;
                }
                Ok(n) => {
                    let _ = proxy.send_event(CustomEvent::PtyData {
                        window_id,
                        tab_id,
                        data: buf[..n].to_vec(),
                    });
                }
                Err(_) => {
                    let _ = proxy.send_event(CustomEvent::PtyExit { window_id, tab_id });
                    break;
                }
            }
        }
    });
}

use crate::renderer::renderer::Renderer;
use crate::renderer::software::CpuRenderer;
use crate::screen::cell::{Cell, CellFlags};
use crate::terminal::terminal::Terminal;

pub enum CustomEvent {
    PtyData {
        window_id: WindowId,
        tab_id: u64,
        data: Vec<u8>,
    },
    PtyExit {
        window_id: WindowId,
        tab_id: u64,
    },
    IpcCreateWindow {
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        title: Option<String>,
        hold: Option<bool>,
    },
    IpcCreateTab {
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        title: Option<String>,
        hold: Option<bool>,
    },
}

#[allow(clippy::large_enum_variant)]
pub enum WindowRendererBackend {
    OpenGL {
        renderer: Renderer,
        gl_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
        gl_context: glutin::context::PossiblyCurrentContext,
    },
    Software {
        renderer: CpuRenderer,
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    },
}

impl WindowRendererBackend {
    #[inline(always)]
    pub fn cell_width(&self) -> u32 {
        match self {
            Self::OpenGL { renderer, .. } => renderer.font_loader.cell_width,
            Self::Software { renderer, .. } => renderer.glyph_cache.cell_width,
        }
    }

    #[inline(always)]
    pub fn cell_height(&self) -> u32 {
        match self {
            Self::OpenGL { renderer, .. } => renderer.font_loader.cell_height,
            Self::Software { renderer, .. } => renderer.glyph_cache.cell_height,
        }
    }

    #[inline(always)]
    pub fn base_cell_height(&self) -> u32 {
        match self {
            Self::OpenGL { renderer, .. } => renderer.tab_font_loader.cell_height,
            Self::Software { renderer, .. } => renderer.tab_glyph_cache.cell_height,
        }
    }

    pub fn set_tab_font_size(&mut self, size: f32) {
        match self {
            Self::OpenGL {
                renderer,
                gl_surface,
                gl_context,
            } => {
                let _ = gl_context.make_current(gl_surface);
                renderer.set_tab_font_size(size);
            }
            Self::Software { renderer, .. } => renderer.set_tab_font_size(size),
        }
    }
}

pub struct WindowState {
    pub backend: WindowRendererBackend,
    pub window: Arc<Window>,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub render_cells_buf: Vec<Cell>,
    pub scroll_multiplier: f64,
    pub fps_limit: Option<u32>,
    pub last_frame_instant: std::time::Instant,
    pub current_title: String,
    pub default_font_size: f32,
    pub current_font_size: f32,
    pub base_cell_height: u32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub is_mouse_down: bool,
    pub last_mouse_button: u8,
    pub last_click_instant: Option<std::time::Instant>,
    pub last_click_pos: (usize, usize),
    pub click_count: u8,
    pub last_mouse_cell: (usize, usize),
    pub is_focused: bool,
    pub current_cursor_icon: winit::window::CursorIcon,
    pub needs_redraw: bool,
    pub content_dirty: bool,
    pub cursor_blink_enabled: bool,
    pub cursor_blink_on: bool,
    pub last_cursor_blink: std::time::Instant,
    pub opacity: f32,
    pub window_dim: f32,
    pub shell_path: String,
    pub tabs: Vec<Tab>,
    pub active_tab_index: usize,
    pub next_tab_id: u64,
    pub tab_bar: TabBar,
    pub event_loop_proxy: EventLoopProxy<CustomEvent>,
    /// Set whenever tab bar visual state changes; triggers cache rebuild in draw().
    pub tab_bar_dirty: bool,
    /// Cached render info; rebuilt lazily when tab_bar_dirty is true.
    pub tab_bar_render_cache: Option<TabBarRenderInfo>,
}

impl Drop for WindowState {
    fn drop(&mut self) {
        if let WindowRendererBackend::OpenGL {
            gl_context,
            gl_surface,
            ..
        } = &self.backend
        {
            let _ = gl_context.make_current(gl_surface);
        }
        crate::memory::trim_allocator_memory();
    }
}

impl WindowState {
    pub fn release_memory(&mut self) {
        match &mut self.backend {
            WindowRendererBackend::OpenGL {
                renderer,
                gl_context,
                gl_surface,
            } => {
                let _ = gl_context.make_current(gl_surface);
                renderer.release_memory();
            }
            WindowRendererBackend::Software { renderer, .. } => {
                renderer.release_memory();
            }
        }
        if self.render_cells_buf.capacity() > 200 * 60 {
            self.render_cells_buf = Vec::new();
        }
        crate::memory::trim_allocator_memory();
    }

    #[inline(always)]
    pub fn cell_width(&self) -> u32 {
        self.backend.cell_width()
    }

    #[inline(always)]
    pub fn cell_height(&self) -> u32 {
        self.backend.cell_height()
    }

    #[inline(always)]
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active_tab_index]
    }

    #[inline(always)]
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab_index]
    }

    #[inline(always)]
    pub fn set_cursor_cached(&mut self, icon: winit::window::CursorIcon) {
        if self.current_cursor_icon != icon {
            self.current_cursor_icon = icon;
            self.window.set_cursor(icon);
        }
    }

    #[inline(always)]
    pub fn tab_bar_height(&self) -> f32 {
        if self.tab_bar.is_visible(self.tabs.len()) {
            self.tab_bar.height(self.base_cell_height)
        } else {
            0.0
        }
    }

    pub fn recalculate_grid_size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        let tab_bar_h = self.tab_bar_height();
        let avail_w = (width as f32 - self.padding_x * 2.0).max(10.0);
        let avail_h = (height as f32 - tab_bar_h - self.padding_y * 2.0).max(10.0);
        let cols = ((avail_w as u32) / self.cell_width()).max(20);
        let rows = ((avail_h as u32) / self.cell_height()).max(10);
        (cols, rows)
    }

    pub fn resize_active_tab(&mut self) {
        let (cols, rows) = self.recalculate_grid_size();
        let tab = self.active_tab_mut();
        tab.terminal.resize(cols, rows);
        let _ = tab.pty_master.resize(cols as u16, rows as u16);
        self.needs_redraw = true;
        self.content_dirty = true;
    }

    pub fn set_renderer_font_size(&mut self, size: f32) {
        match &mut self.backend {
            WindowRendererBackend::OpenGL {
                renderer,
                gl_surface,
                gl_context,
            } => {
                let _ = gl_context.make_current(gl_surface);
                renderer.set_font_size(size);
            }
            WindowRendererBackend::Software { renderer, .. } => {
                renderer.update_font_size(size);
            }
        }
    }

    pub fn set_font_size(&mut self, size: f32) {
        let size = size.max(1.0);
        self.current_font_size = size;
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.font_size = size;
        }
        self.set_renderer_font_size(size);
        self.resize_active_tab();
    }

    pub fn resize_renderer(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        match &mut self.backend {
            WindowRendererBackend::OpenGL {
                renderer,
                gl_surface,
                gl_context,
            } => {
                let _ = gl_context.make_current(gl_surface);
                self.window.resize_surface(gl_surface, gl_context);
                renderer.resize(width, height);
            }
            WindowRendererBackend::Software { renderer, surface } => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
                    let _ = surface.resize(w, h);
                }
                renderer.resize(width, height);
            }
        }
        self.resize_active_tab();
    }

    pub fn create_tab(
        &mut self,
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        custom_title: Option<String>,
        hold: Option<bool>,
    ) -> u64 {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        let was_visible = self.tab_bar.is_visible(self.tabs.len());

        let pty_master = Arc::new(
            spawn_process(
                &self.shell_path,
                command.as_deref(),
                working_directory.as_deref(),
            )
            .unwrap(),
        );

        let tab_font_size = self.default_font_size;
        if (self.current_font_size - tab_font_size).abs() > 0.01 {
            self.current_font_size = tab_font_size;
            self.set_renderer_font_size(tab_font_size);
        }

        let (cols, rows) = self.recalculate_grid_size();
        let _ = pty_master.resize(cols as u16, rows as u16);

        let terminal = Terminal::new(cols as usize, rows as usize);
        let initial_title = custom_title.clone().unwrap_or_else(|| "velox".to_string());

        spawn_pty_reader(
            pty_master.clone(),
            self.event_loop_proxy.clone(),
            self.window.id(),
            tab_id,
        );

        let tab = Tab::new(
            tab_id,
            pty_master,
            terminal,
            custom_title,
            initial_title,
            hold.unwrap_or(false),
            tab_font_size,
        );

        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;

        let is_now_visible = self.tab_bar.is_visible(self.tabs.len());
        if was_visible != is_now_visible {
            self.resize_active_tab();
        }

        self.tab_bar_dirty = true;
        self.needs_redraw = true;
        self.content_dirty = true;
        tab_id
    }

    pub fn close_tab(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        self.tabs.remove(index);

        if self.tabs.is_empty() {
            return true;
        }

        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }

        let active_font_size = self.tabs[self.active_tab_index].font_size;
        if (self.current_font_size - active_font_size).abs() > 0.01 {
            self.current_font_size = active_font_size;
            self.set_renderer_font_size(active_font_size);
        }

        self.resize_active_tab();

        crate::memory::trim_allocator_memory();
        self.tab_bar_dirty = true;
        self.needs_redraw = true;
        self.content_dirty = true;
        self.tabs[self.active_tab_index]
            .terminal
            .active_grid_mut()
            .mark_all_dirty();
        false
    }

    pub fn switch_tab(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active_tab_index {
            self.active_tab_index = index;
            let tab_font_size = self.tabs[index].font_size;
            if (self.current_font_size - tab_font_size).abs() > 0.01 {
                self.current_font_size = tab_font_size;
                self.set_renderer_font_size(tab_font_size);
            }
            let (cols, rows) = self.recalculate_grid_size();
            let tab = &mut self.tabs[index];
            if tab.terminal.grid.width != cols as usize || tab.terminal.grid.height != rows as usize
            {
                tab.terminal.resize(cols, rows);
                let _ = tab.pty_master.resize(cols as u16, rows as u16);
            }
            self.window.set_title(&self.tabs[index].current_title);
            self.current_title = self.tabs[index].current_title.clone();
            self.tab_bar_dirty = true;
            self.needs_redraw = true;
            self.content_dirty = true;
            self.tabs[index].terminal.active_grid_mut().mark_all_dirty();
        }
    }

    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            self.switch_tab(next);
        }
    }

    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            let prev = (self.active_tab_index + self.tabs.len() - 1) % self.tabs.len();
            self.switch_tab(prev);
        }
    }

    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from < self.tabs.len() && to < self.tabs.len() && from != to {
            let tab = self.tabs.remove(from);
            self.tabs.insert(to, tab);
            self.active_tab_index = to;
            self.tab_bar_dirty = true;
            self.needs_redraw = true;
            self.content_dirty = true;
            self.tabs[to].terminal.active_grid_mut().mark_all_dirty();
        }
    }

    pub fn draw(&mut self) {
        self.last_frame_instant = std::time::Instant::now();

        // 1. Ensure renderer font size matches the active tab's isolated font size
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            let tab_font_size = active_tab.font_size;
            if (self.current_font_size - tab_font_size).abs() > 0.01 {
                self.current_font_size = tab_font_size;
                self.set_renderer_font_size(tab_font_size);
            }
        }

        // 2. Update title for active tab (and window title)
        if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index)
            && (active_tab.update_title() || self.current_title != active_tab.current_title)
        {
            self.window.set_title(&active_tab.current_title);
            self.current_title = active_tab.current_title.clone();
            self.tab_bar_dirty = true; // title changed → rebuild cache
        }

        // 3. Prepare tab bar render info (lazily cached; rebuilt only when state changes)
        let is_tab_bar_visible = self.tab_bar.is_visible(self.tabs.len());
        if is_tab_bar_visible && (self.tab_bar_dirty || self.tab_bar_render_cache.is_none()) {
            for tab in &mut self.tabs {
                let _ = tab.update_title();
            }
            let headers: Vec<TabHeaderInfo> = self
                .tabs
                .iter()
                .enumerate()
                .map(|(i, t)| TabHeaderInfo {
                    title: t.current_title.clone(),
                    is_active: i == self.active_tab_index,
                    is_hovered: self.tab_bar.hovered_tab == Some(i),
                    is_close_hovered: self.tab_bar.hovered_close == Some(i),
                })
                .collect();
            self.tab_bar_render_cache = Some(TabBarRenderInfo {
                height: self.tab_bar.height(self.base_cell_height),
                tabs: headers,
                show_new_tab: self.tab_bar.show_new_tab_button,
                is_new_tab_hovered: self.tab_bar.hovered_new_tab,
                show_close_button: self.tab_bar.show_close_button,
            });
            self.tab_bar_dirty = false;
        } else if !is_tab_bar_visible {
            self.tab_bar_render_cache = None;
        }
        let tab_bar_info: Option<&TabBarRenderInfo> = if is_tab_bar_visible {
            self.tab_bar_render_cache.as_ref()
        } else {
            None
        };

        let active_tab = match self.tabs.get(self.active_tab_index) {
            Some(t) => t,
            None => return,
        };

        let active_grid = active_tab.terminal.active_grid();
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
                    if !active_grid.scrollback.copy_row_to_slice(
                        idx,
                        &mut self.render_cells_buf[dest_start..dest_end],
                        default_cell,
                    ) {
                        self.render_cells_buf[dest_start..dest_end].fill(default_cell);
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

        let cursor_visible = if offset > 0 {
            false
        } else if self.cursor_blink_enabled {
            active_grid.cursor.visible && self.cursor_blink_on
        } else {
            active_grid.cursor.visible
        };

        let cursor_shape = if !self.is_focused
            && active_grid.cursor.shape == crate::screen::cursor::CursorShape::Block
        {
            crate::screen::cursor::CursorShape::HollowBlock
        } else {
            active_grid.cursor.shape
        };

        let effective_dim = if !self.is_focused {
            self.window_dim
        } else {
            0.0
        };

        let display_cursor_x = active_grid.cursor.x.min(width.saturating_sub(1));

        match &mut self.backend {
            WindowRendererBackend::OpenGL {
                renderer,
                gl_surface,
                gl_context,
            } => {
                let _ = gl_context.make_current(gl_surface);
                renderer.draw(
                    &self.render_cells_buf,
                    width,
                    height,
                    display_cursor_x,
                    active_grid.cursor.y,
                    cursor_visible,
                    cursor_shape,
                    &active_tab.terminal.theme,
                    active_tab.terminal.bold_is_bright,
                    &active_grid.selection,
                    offset,
                    history_len,
                    self.padding_x,
                    self.padding_y,
                    self.opacity,
                    effective_dim,
                    tab_bar_info,
                );
                let _ = gl_surface.swap_buffers(gl_context);
            }
            WindowRendererBackend::Software { renderer, surface } => {
                if let Ok(mut buffer) = surface.buffer_mut() {
                    renderer.render_with_tab_bar(
                        &self.render_cells_buf,
                        active_grid,
                        &active_tab.terminal.theme,
                        self.padding_x,
                        self.padding_y,
                        cursor_visible,
                        cursor_shape,
                        display_cursor_x,
                        self.is_focused,
                        self.opacity,
                        self.window_dim,
                        &mut buffer,
                        tab_bar_info,
                    );
                    let _ = buffer.present();
                }
                if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index) {
                    active_tab.terminal.active_grid_mut().clear_damage();
                }
            }
        }
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

        let initial_title = match &custom_title {
            Some(t) => t.clone(),
            None => match &config.app_title {
                Some(tpl) => tpl.replace("{program}", "velox"),
                None => "velox".to_string(),
            },
        };

        let icon = load_app_icon();
        let opacity = config.opacity();
        let window_dim = config.window_dim();

        let is_transparent = opacity < 1.0;
        let mut window_attributes = Window::default_attributes()
            .with_title(&initial_title)
            .with_transparent(is_transparent)
            .with_visible(false)
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));

        if let Some(icon) = icon {
            window_attributes = window_attributes.with_window_icon(Some(icon));
        }

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let scroll_multiplier = config.scroll_multiplier().unwrap_or(1.0);
        let cursor_blink_enabled = config.cursor_blink().unwrap_or(true);
        let gpu = config.gpu_acceleration().unwrap_or(true);
        let fps_limit = match config.fps_limit() {
            Some(limit) => Some(limit),
            None => {
                if gpu {
                    Some(120)
                } else {
                    Some(60)
                }
            }
        };

        let font_size = config.font_size();
        let padding_x = config.padding_x().unwrap_or(8.0);
        let padding_y = config.padding_y().unwrap_or(4.0);
        let font_scale_multiplier = config.font_scale_multiplier().unwrap_or(1.5);
        let bold_is_bright = config.bold_is_bright().unwrap_or(true);

        let size = window.inner_size();
        let win_width = size.width.max(1);
        let win_height = size.height.max(1);

        let mut backend = if gpu
            && let (Some(gl_config), Some(gl_display), Some(gl)) = (
                self.gl_config.as_ref(),
                self.gl_display.as_ref(),
                self.gl.as_ref(),
            ) {
            let gl = gl.clone();

            let context_attributes = glutin::context::ContextAttributesBuilder::new()
                .with_context_api(glutin::context::ContextApi::OpenGl(Some(
                    glutin::context::Version::new(3, 3),
                )))
                .build(Some(window.window_handle().unwrap().as_raw()));

            let gl_context = unsafe {
                gl_display
                    .create_context(gl_config, &context_attributes)
                    .unwrap()
            };

            let attrs = window.build_surface_attributes(<_>::default()).unwrap();
            let gl_surface = unsafe {
                gl_config
                    .display()
                    .create_window_surface(gl_config, &attrs)
                    .unwrap()
            };

            let gl_context = gl_context.make_current(&gl_surface).unwrap();

            let renderer = Renderer::new(
                gl,
                config.font_family(),
                font_size,
                font_scale_multiplier,
                win_width,
                win_height,
            );

            WindowRendererBackend::OpenGL {
                renderer,
                gl_surface,
                gl_context,
            }
        } else {
            let context = softbuffer::Context::new(window.clone()).unwrap();
            let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
            if let (Some(w), Some(h)) = (NonZeroU32::new(win_width), NonZeroU32::new(win_height)) {
                let _ = surface.resize(w, h);
            }

            let theme = crate::theme::theme::Theme::from_config(&config);

            let renderer = CpuRenderer::new(
                config.font_family(),
                font_size,
                font_scale_multiplier,
                &theme,
                win_width,
                win_height,
                bold_is_bright,
                opacity,
            );

            WindowRendererBackend::Software { renderer, surface }
        };

        let tab_font_size = config.tab_font_size();
        backend.set_tab_font_size(tab_font_size);
        let base_cell_height = backend.base_cell_height();

        let tab_bar = TabBar::from_config(&config);
        let tab_bar_h = if tab_bar.is_visible(1) {
            tab_bar.height(base_cell_height)
        } else {
            0.0
        };

        let avail_w = (win_width as f32 - padding_x * 2.0).max(10.0);
        let avail_h = (win_height as f32 - tab_bar_h - padding_y * 2.0).max(10.0);
        let cols = ((avail_w as u32) / backend.cell_width()).max(20);
        let rows = ((avail_h as u32) / backend.cell_height()).max(10);

        let terminal = Terminal::new(cols as usize, rows as usize);

        let shell_path = config
            .shell
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());

        let pty_master = Arc::new(
            spawn_process(
                &shell_path,
                command.as_deref(),
                working_directory.as_deref(),
            )
            .unwrap(),
        );
        pty_master.resize(cols as u16, rows as u16).unwrap();

        let window_id = window.id();
        let tab_id = 0u64;
        spawn_pty_reader(
            pty_master.clone(),
            self.event_loop_proxy.clone(),
            window_id,
            tab_id,
        );

        let tab = Tab::new(
            tab_id,
            pty_master,
            terminal,
            custom_title,
            initial_title.clone(),
            hold.unwrap_or(false),
            font_size,
        );

        let mut window_state = WindowState {
            window,
            backend,
            mouse_x: 0.0,
            mouse_y: 0.0,
            render_cells_buf: Vec::new(),
            scroll_multiplier,
            fps_limit,
            last_frame_instant: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(std::time::Instant::now),
            current_title: initial_title,
            default_font_size: font_size,
            current_font_size: font_size,
            base_cell_height,
            padding_x,
            padding_y,
            is_mouse_down: false,
            last_mouse_button: 0,
            last_click_instant: None,
            last_click_pos: (0, 0),
            click_count: 0,
            last_mouse_cell: (0, 0),
            is_focused: true,
            current_cursor_icon: winit::window::CursorIcon::Default,
            needs_redraw: false,
            content_dirty: false,
            cursor_blink_enabled,
            cursor_blink_on: true,
            last_cursor_blink: std::time::Instant::now(),
            opacity,
            window_dim,
            shell_path,
            tabs: vec![tab],
            active_tab_index: 0,
            next_tab_id: 1,
            tab_bar,
            event_loop_proxy: self.event_loop_proxy.clone(),
            tab_bar_dirty: true,
            tab_bar_render_cache: None,
        };

        // Render initial frame synchronously to eliminate any cold-start transparent/blank flicker,
        // then reveal the window.
        window_state.draw();
        window_state.window.set_visible(true);

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

        let gpu = config.gpu_acceleration().unwrap_or(true);

        if gpu && self.gl_display.is_none() {
            let template = glutin::config::ConfigTemplateBuilder::new()
                .with_alpha_size(8)
                .with_transparency(true);

            let dummy_attrs = Window::default_attributes()
                .with_visible(false)
                .with_transparent(true)
                .with_inner_size(winit::dpi::PhysicalSize::new(1, 1));
            let display_builder =
                glutin_winit::DisplayBuilder::new().with_window_attributes(Some(dummy_attrs));

            if let Ok((dummy_window, gl_config)) =
                display_builder.build(event_loop, template, |configs| {
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
                && let Some(dummy_window) = dummy_window
            {
                let gl_display = gl_config.display();

                let context_attributes = glutin::context::ContextAttributesBuilder::new()
                    .with_context_api(glutin::context::ContextApi::OpenGl(Some(
                        glutin::context::Version::new(3, 3),
                    )))
                    .build(Some(dummy_window.window_handle().unwrap().as_raw()));

                if let Ok(dummy_context) =
                    unsafe { gl_display.create_context(&gl_config, &context_attributes) }
                {
                    let attrs = dummy_window
                        .build_surface_attributes(<_>::default())
                        .unwrap();
                    if let Ok(dummy_surface) = unsafe {
                        gl_config
                            .display()
                            .create_window_surface(&gl_config, &attrs)
                    } {
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
                }
            }
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

        if let Some(opts) = self.initial_options.take()
            && !opts.daemon
        {
            self.create_window(
                event_loop,
                opts.working_directory,
                opts.command,
                opts.title,
                Some(opts.hold),
            );
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
                    if !focused {
                        ws.is_mouse_down = false;
                    }
                    let active_tab = ws.active_tab();
                    if active_tab.terminal.focus_tracking {
                        let seq = if focused { b"\x1b[I" } else { b"\x1b[O" };
                        let _ = active_tab.pty_master.write(seq);
                    }
                    ws.content_dirty = true;
                    ws.tab_bar_dirty = true;
                    ws.needs_redraw = true;
                }
                WindowEvent::Resized(size) => {
                    let width = size.width.max(1);
                    let height = size.height.max(1);
                    ws.resize_renderer(width, height);
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
            CustomEvent::PtyData {
                window_id,
                tab_id,
                data,
            } => {
                if let Some(ws) = self.windows.get_mut(&window_id)
                    && let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == tab_id)
                {
                    tab.last_activity = std::time::Instant::now();
                    tab.terminal.feed(&data);
                    if !tab.terminal.outgoing.is_empty() {
                        let _ = tab.pty_master.write(&tab.terminal.outgoing);
                        tab.terminal.outgoing.clear();
                        if tab.terminal.outgoing.capacity() > 4096 {
                            tab.terminal.outgoing.shrink_to_fit();
                        }
                    }
                    if ws.tabs.get(ws.active_tab_index).map(|t| t.id) == Some(tab_id) {
                        ws.needs_redraw = true;
                        ws.content_dirty = true;
                    }
                }
            }
            CustomEvent::PtyExit { window_id, tab_id } => {
                if let Some(ws) = self.windows.get_mut(&window_id)
                    && let Some(tab_idx) = ws.tabs.iter().position(|t| t.id == tab_id)
                {
                    if ws.tabs[tab_idx].hold {
                        ws.tabs[tab_idx].current_title = "[Process exited]".to_string();
                        if ws.active_tab_index == tab_idx {
                            ws.window.set_title("[Process exited]");
                        }
                        ws.tab_bar_dirty = true;
                        ws.needs_redraw = true;
                        return;
                    }
                    let should_close_window = ws.close_tab(tab_idx);
                    if should_close_window {
                        self.windows.remove(&window_id);
                        crate::memory::trim_allocator_memory();
                        if self.windows.is_empty() && !self.daemon_mode {
                            event_loop.exit();
                        }
                    }
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
            CustomEvent::IpcCreateTab {
                working_directory,
                command,
                title,
                hold,
            } => {
                if let Some((_, ws)) = self.windows.iter_mut().next() {
                    ws.create_tab(working_directory, command, title, hold);
                } else {
                    self.create_window(event_loop, working_directory, command, title, hold);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = std::time::Instant::now();
        let mut min_next_wake: Option<std::time::Instant> = None;

        for ws in self.windows.values_mut() {
            // Idle memory trimming (2.5s of PTY inactivity after burst activity)
            let mut should_release = false;
            for tab in &mut ws.tabs {
                if now.duration_since(tab.last_activity) >= std::time::Duration::from_millis(2500)
                    && tab.last_activity > tab.last_cleanup
                {
                    should_release = true;
                    tab.last_cleanup = now;
                }
            }
            if should_release {
                ws.release_memory();
            }

            // Cursor and text blink toggle (500ms cycle)
            if now.duration_since(ws.last_cursor_blink) >= std::time::Duration::from_millis(500) {
                if ws.cursor_blink_enabled {
                    ws.cursor_blink_on = !ws.cursor_blink_on;
                }
                ws.last_cursor_blink = now;
                ws.needs_redraw = true;
            }

            let next_blink = ws.last_cursor_blink + std::time::Duration::from_millis(500);
            min_next_wake = Some(min_next_wake.map_or(next_blink, |t| t.min(next_blink)));

            // Redraw scheduling
            if ws.needs_redraw {
                if let Some(active_tab) = ws.tabs.get_mut(ws.active_tab_index)
                    && active_tab.terminal.is_synchronized_output_active()
                {
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
