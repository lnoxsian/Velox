use crate::app::pane::{Pane, PaneId};
use crate::app::split::{FocusDirection, PaneRect, SeparatorRect, SplitDirection, SplitId};
use crate::app::tab::{Tab, TabBar, TabBarRenderInfo, TabHeaderInfo};
use crate::cli::CliOptions;
use crate::config::config::Config;
use crate::ipc::{IpcListenerHandle, start_ipc_server};
use crate::pty::master::PtyMaster;
use crate::pty::process::spawn_terminal_pty_with_size;
use crate::renderer::{CpuPaneRenderData, CpuRenderer, SeparatorRenderData};
use crate::terminal::terminal::Terminal;
use crate::window::PlatformWindow;
use crate::window::event::{CursorIcon, ModifiersState, PlatformEvent};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

pub type WindowId = u64;

#[derive(Debug, Clone, Copy)]
pub struct DraggingSeparator {
    pub split_id: SplitId,
    pub direction: SplitDirection,
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_w: f32,
    pub bounds_h: f32,
}

/// State tracking for an active mouse tab drag-and-rearrange operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DraggingTab {
    /// The current slot index of the tab being dragged in `ws.tabs`.
    pub current_index: usize,
    /// Horizontal cursor position (pixels) when the left mouse button was pressed.
    pub start_x: f32,
    /// Relative horizontal offset (pixels) within the tab from its left edge (`mouse_x - tab_x`).
    pub offset_in_tab: f32,
    /// Latch indicating if the cursor has exceeded the minimum jitter drag threshold (6px).
    pub has_dragged: bool,
}

pub enum CustomEvent {
    PtyData {
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
        data: Vec<u8>,
    },
    PtyExit {
        window_id: WindowId,
        tab_id: u64,
        pane_id: u64,
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

#[derive(Clone)]
pub struct EventLoopProxy {
    sender: Sender<CustomEvent>,
    wake_fd: Option<Arc<OwnedFd>>,
    pub(crate) wake_pending: Arc<std::sync::atomic::AtomicBool>,
}

impl EventLoopProxy {
    pub fn dummy() -> Self {
        let (sender, _) = std::sync::mpsc::channel();
        Self {
            sender,
            wake_fd: None,
            wake_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn new(sender: Sender<CustomEvent>, wake_fd: RawFd) -> Self {
        let wake_fd = if wake_fd >= 0 {
            Some(Arc::new(unsafe { OwnedFd::from_raw_fd(wake_fd) }))
        } else {
            None
        };
        Self {
            sender,
            wake_fd,
            wake_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn from_owned_fd(sender: Sender<CustomEvent>, wake_fd: Arc<OwnedFd>) -> Self {
        Self {
            sender,
            wake_fd: Some(wake_fd),
            wake_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn send(&self, event: CustomEvent) -> Result<(), std::sync::mpsc::SendError<CustomEvent>> {
        self.sender.send(event)?;
        if let Some(wfd) = &self.wake_fd
            && !self
                .wake_pending
                .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            let val = 1u64;
            loop {
                let res = unsafe {
                    libc::write(
                        wfd.as_raw_fd(),
                        &val as *const _ as *const libc::c_void,
                        std::mem::size_of::<u64>(),
                    )
                };
                if res >= 0 {
                    break;
                }
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                if err.raw_os_error() == Some(libc::EAGAIN)
                    || err.raw_os_error() == Some(libc::EWOULDBLOCK)
                {
                    break;
                }
                log::warn!("Failed to write to main loop wake_fd: {}", err);
                break;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResizeState {
    pub pending_width: u32,
    pub pending_height: u32,
    pub last_applied_width: u32,
    pub last_applied_height: u32,
    pub last_cols: usize,
    pub last_rows: usize,
    pub is_pending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlertBanner {
    pub severity: crate::config::loader::DiagnosticSeverity,
    pub message: String,
    pub dismiss_hovered: bool,
}

pub struct WindowState {
    pub window_id: WindowId,
    pub config: Config,
    pub banner: Option<AlertBanner>,
    pub renderer: CpuRenderer,
    pub window: PlatformWindow,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub scroll_multiplier: f64,
    pub scroll_accumulator: f64,
    pub fps_limit: Option<u32>,
    pub last_frame_instant: std::time::Instant,
    pub last_input_instant: std::time::Instant,
    pub last_key_instant: std::time::Instant,
    pub last_resize_instant: std::time::Instant,
    pub current_title: String,
    pub default_font_size: f32,
    pub current_font_size: f32,
    pub base_cell_width: u32,
    pub base_cell_height: u32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub is_mouse_down: bool,
    pub mouse_down_pane_id: Option<PaneId>,
    pub last_mouse_button: u8,
    pub last_click_instant: Option<std::time::Instant>,
    pub last_click_pos: (usize, usize),
    pub last_click_pane_id: Option<PaneId>,
    pub click_count: u8,
    pub last_mouse_cell: (usize, usize),
    pub last_mouse_pane_id: Option<PaneId>,
    pub is_focused: bool,
    pub current_cursor_icon: CursorIcon,
    pub needs_redraw: bool,
    pub content_dirty: bool,
    pub cursor_blink_enabled: bool,
    pub cursor_blink_on: bool,
    pub last_cursor_blink: std::time::Instant,
    pub hide_mouse_on_typing: bool,
    pub mouse_hidden: bool,
    pub opacity: f32,
    pub window_dim: f32,
    pub shell_path: String,
    pub tabs: Vec<Tab>,
    pub active_tab_index: usize,
    pub next_tab_id: u64,
    pub next_pane_id: u64,
    pub next_split_id: u64,
    pub tab_bar: TabBar,
    pub event_loop_proxy: EventLoopProxy,
    pub pty_reactor: crate::pty::PtyReactorHandle,
    pub tab_bar_dirty: bool,
    pub tab_bar_render_cache: Option<TabBarRenderInfo>,
    pub dragging_separator: Option<DraggingSeparator>,
    pub dragging_tab: Option<DraggingTab>,
    pub hovered_separator: Option<SplitId>,
    pub separator_size: f32,
    pub min_cols: usize,
    pub min_rows: usize,
    pub separator_color: Option<String>,
    pub active_separator_color: Option<String>,
    pub last_separator_drag_sync: Option<std::time::Instant>,
    pub csd_button_layout: crate::window::csd::CsdButtonLayout,
    pub csd_palette: crate::window::csd::CsdPalette,
    pub hovered_csd_button: Option<crate::window::csd::CsdButtonType>,
    pub pressed_csd_button: Option<crate::window::csd::CsdButtonType>,
    pub resize_state: ResizeState,
    pub search: crate::screen::search::SearchState,
}

impl Drop for WindowState {
    fn drop(&mut self) {
        crate::memory::trim_allocator_memory();
    }
}

impl WindowState {
    pub fn create_test_window_state(pty_master: Arc<PtyMaster>, cols: usize, rows: usize) -> Self {
        let theme = crate::theme::theme::Theme::default();
        let renderer = CpuRenderer::new("monospace", 14.0, 1.0, &theme, 800, 600, true, 1.0);
        let terminal = Terminal::new(cols, rows);
        let config = crate::config::defaults::default_config();
        let tab_bar = TabBar::from_config(&config);
        let tab = Tab::new(
            0,
            pty_master,
            terminal,
            None,
            "test".to_string(),
            false,
            14.0,
        );
        let csd_palette = crate::window::csd::CsdPalette::resolve(
            config.decorations_config().as_ref(),
            &renderer.palette,
        );
        let now = std::time::Instant::now();
        WindowState {
            window_id: 1,
            config: config.clone(),
            banner: None,
            renderer,
            window: PlatformWindow::dummy(800, 600),
            mouse_x: 0.0,
            mouse_y: 0.0,
            scroll_multiplier: 1.0,
            scroll_accumulator: 0.0,
            fps_limit: Some(60),
            last_frame_instant: now,
            last_input_instant: now,
            last_key_instant: now,
            last_resize_instant: now,
            current_title: "test".to_string(),
            default_font_size: 14.0,
            current_font_size: 14.0,
            base_cell_width: 8,
            base_cell_height: 16,
            padding_x: 4.0,
            padding_y: 4.0,
            is_mouse_down: false,
            mouse_down_pane_id: None,
            last_mouse_button: 0,
            last_click_instant: None,
            last_click_pos: (0, 0),
            last_click_pane_id: None,
            click_count: 0,
            last_mouse_cell: (0, 0),
            last_mouse_pane_id: None,
            is_focused: true,
            current_cursor_icon: CursorIcon::Default,
            needs_redraw: false,
            content_dirty: false,
            cursor_blink_enabled: false,
            cursor_blink_on: true,
            last_cursor_blink: now,
            hide_mouse_on_typing: config.hide_mouse_on_typing().unwrap_or(true),
            mouse_hidden: false,
            opacity: 1.0,
            window_dim: 0.0,
            shell_path: "/bin/sh".to_string(),
            tabs: vec![tab],
            active_tab_index: 0,
            next_tab_id: 1,
            next_pane_id: 1,
            next_split_id: 1,
            tab_bar,
            event_loop_proxy: EventLoopProxy::dummy(),
            pty_reactor: crate::pty::PtyReactorHandle::dummy(),
            tab_bar_dirty: false,
            tab_bar_render_cache: None,
            dragging_separator: None,
            dragging_tab: None,
            hovered_separator: None,
            separator_size: 4.0,
            min_cols: 10,
            min_rows: 5,
            separator_color: None,
            active_separator_color: None,
            last_separator_drag_sync: None,
            csd_button_layout: crate::window::csd::CsdButtonLayout::default(),
            csd_palette,
            hovered_csd_button: None,
            pressed_csd_button: None,
            resize_state: ResizeState {
                last_applied_width: 800,
                last_applied_height: 600,
                last_cols: cols,
                last_rows: rows,
                ..Default::default()
            },
            search: crate::screen::search::SearchState::new(),
        }
    }

    #[inline]
    pub fn mark_interaction(&mut self) {
        let now = std::time::Instant::now();
        self.last_input_instant = now;
        if self.cursor_blink_enabled && !self.cursor_blink_on {
            self.cursor_blink_on = true;
            self.needs_redraw = true;
        }
        self.last_cursor_blink = now;
    }

    #[inline]
    pub fn is_cursor_blink_active(&self) -> bool {
        if !self.cursor_blink_enabled || !self.is_focused {
            return false;
        }
        if let Some(active_tab) = self.tabs.get(self.active_tab_index) {
            active_tab
                .active_pane()
                .terminal
                .active_grid()
                .cursor
                .visible
        } else {
            false
        }
    }

    pub fn release_memory(&mut self) {
        self.renderer.release_memory();
        for tab in &mut self.tabs {
            tab.for_each_pane_mut(|pane| {
                pane.terminal.release_memory();
            });
        }
        self.tab_bar_render_cache = None;
        crate::screen::scrollback::clear_global_scrollback_cache();
        crate::memory::trim_allocator_memory();
    }

    pub fn prune_idle_fallbacks(&mut self, max_idle: std::time::Duration) -> usize {
        self.renderer.prune_idle_fallbacks(max_idle)
    }

    #[inline(always)]
    pub fn cell_width(&self) -> u32 {
        self.renderer.glyph_cache.cell_width
    }

    #[inline(always)]
    pub fn cell_height(&self) -> u32 {
        self.renderer.glyph_cache.cell_height
    }

    #[inline(always)]
    pub fn base_cell_width(&self) -> u32 {
        self.renderer.glyph_cache.cell_width
    }

    #[inline(always)]
    pub fn base_cell_height(&self) -> u32 {
        self.renderer.glyph_cache.cell_height
    }

    pub fn set_tab_font_size(&mut self, font_size: f32) {
        self.renderer.tab_glyph_cache.update_font_size(font_size);
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
    pub fn active_pane(&self) -> &Pane {
        self.active_tab().active_pane()
    }

    #[inline(always)]
    pub fn active_pane_mut(&mut self) -> &mut Pane {
        self.active_tab_mut().active_pane_mut()
    }

    #[inline]
    pub fn mark_search_damage(&mut self, extra_abs_rows: &[usize]) {
        let cell_h = self.base_cell_height.max(1) as usize;
        let matches = &self.search.matches;
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            if let Some(pane) = tab.tree.find_pane_mut(tab.active_pane_id) {
                let grid = pane.terminal.active_grid_mut();
                let grid_h = grid.height;
                let history_len = grid.scrollback.len();
                let scroll_offset = grid.scroll_offset;

                // O(visible): locate matches visible in the current viewport via binary search
                let vis_start_abs = history_len.saturating_sub(scroll_offset);
                let vis_end_abs = (history_len + grid_h).saturating_sub(scroll_offset);

                let start_idx = matches.partition_point(|m| m.abs_row < vis_start_abs);
                let end_idx = matches.partition_point(|m| m.abs_row < vis_end_abs);

                let mut last_marked_row = None;
                for m in &matches[start_idx..end_idx] {
                    let abs_y = m.abs_row + scroll_offset;
                    if abs_y >= history_len {
                        let display_row = abs_y - history_len;
                        if display_row < grid_h && last_marked_row != Some(display_row) {
                            grid.damage.mark_dirty(display_row);
                            last_marked_row = Some(display_row);
                        }
                    }
                }
                for &abs_row in extra_abs_rows {
                    let abs_y = abs_row + scroll_offset;
                    if abs_y >= history_len {
                        let display_row = abs_y - history_len;
                        if display_row < grid_h {
                            grid.damage.mark_dirty(display_row);
                        }
                    }
                }
                if grid_h > 0 {
                    let overlay_rows = (32 / cell_h).max(1) + 1;
                    for r in grid_h.saturating_sub(overlay_rows)..grid_h {
                        grid.damage.mark_dirty(r);
                    }
                }
            }
        }
    }

    /// Auto-scrolls the active pane viewport to center or reveal the active search match.
    pub fn jump_to_search_match(&mut self) {
        if let Some(idx) = self.search.active_match_index {
            if let Some(m) = self.search.matches.get(idx) {
                let match_abs_row = m.abs_row;
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.tree.find_pane_mut(tab.active_pane_id) {
                        let grid = pane.terminal.active_grid_mut();
                        let history_len = grid.scrollback.len();
                        let grid_h = grid.height;

                        if match_abs_row >= history_len {
                            if grid.scroll_offset != 0 {
                                grid.scroll_offset = 0;
                                grid.damage.mark_all();
                                self.content_dirty = true;
                            }
                        } else {
                            let is_visible = if match_abs_row + grid.scroll_offset >= history_len {
                                let display_row =
                                    (match_abs_row + grid.scroll_offset) - history_len;
                                display_row < grid_h
                            } else {
                                false
                            };

                            if !is_visible {
                                let half_h = grid_h / 2;
                                let target_offset = (history_len.saturating_sub(match_abs_row)
                                    + half_h)
                                    .clamp(0, history_len);
                                if grid.scroll_offset != target_offset {
                                    grid.scroll_offset = target_offset;
                                    grid.damage.mark_all();
                                    self.content_dirty = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Advances or retreats the active search match and scrolls the viewport if needed.
    #[inline]
    pub fn navigate_search_match(&mut self, backwards: bool) {
        let dirty = if backwards {
            self.search.prev_match()
        } else {
            self.search.next_match()
        };
        if let Some((old_abs_r, new_abs_r)) = dirty {
            self.jump_to_search_match();
            self.mark_search_damage(&[old_abs_r, new_abs_r]);
        }
        self.needs_redraw = true;
    }

    #[inline]
    pub fn recompute_search_matches(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab_index) {
            if let Some(pane) = tab.tree.find_pane(tab.active_pane_id) {
                let grid = pane.terminal.active_grid();
                self.search.recompute_matches(grid);
            }
        }
    }

    #[inline]
    pub fn close_search(&mut self) {
        if !self.search.is_open {
            return;
        }
        self.mark_search_damage(&[]);
        self.search.close();
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            if let Some(pane) = tab.tree.find_pane_mut(tab.active_pane_id) {
                pane.terminal.active_grid_mut().damage.mark_all();
            }
        }
        self.content_dirty = true;
        self.needs_redraw = true;
    }

    #[inline]
    pub fn search_overlay_rect_with_panes(
        &self,
        pane_rects: &[PaneRect],
    ) -> Option<(f64, f64, f64, f64)> {
        if !self.search.is_open || self.tabs.is_empty() || pane_rects.is_empty() {
            return None;
        }
        let active_pane_id = self.active_pane().id;
        let active_rect = pane_rects
            .iter()
            .find(|r| r.pane_id == active_pane_id)
            .copied()
            .or_else(|| pane_rects.first().copied())?;

        let pane_x = active_rect.x.max(0.0) as f64;
        let pane_w = active_rect.width.max(1.0) as f64;

        let box_w = pane_w;
        if box_w < 100.0 {
            return None;
        }
        let box_h = self.search_height() as f64;
        let banner_h = self.banner_height() as f64;
        let win_h = self.window.height() as f64;
        let box_x = pane_x;
        let box_y = (win_h - banner_h - box_h).max(0.0);

        Some((box_x, box_y, box_w, box_h))
    }

    pub fn search_overlay_rect(&self) -> Option<(f64, f64, f64, f64)> {
        if !self.search.is_open || self.tabs.is_empty() {
            return None;
        }
        let (pane_rects, _) = self.recalculate_panes_layout(self.active_tab_index);
        self.search_overlay_rect_with_panes(&pane_rects)
    }

    pub fn write_to_pane(&self, pane_id: u64, data: Vec<u8>) {
        if data.is_empty() {
            return;
        }
        let tab = self.active_tab();
        if let Some(pane) = tab.tree.find_pane(pane_id) {
            if self.pty_reactor.is_stopped() {
                let _ = pane.pty_master.write(&data);
                return;
            }
            // Fast-path: small input (e.g. single keystrokes <= 4096 bytes) directly calls
            // non-blocking write to avoid reactor wake/channel latency whenever no unwritten bytes
            // are currently buffered in the reactor.
            if !self.pty_reactor.has_buffered_writes() && data.len() <= 4096 {
                match pane.pty_master.write(&data) {
                    Ok(n) if n == data.len() => return,
                    Ok(n) => {
                        let remainder = data[n..].to_vec();
                        let _ = self
                            .pty_reactor
                            .write(self.window_id, tab.id, pane_id, remainder);
                        return;
                    }
                    Err(e)
                        if e.raw_os_error() == Some(libc::EAGAIN)
                            || e.raw_os_error() == Some(libc::EWOULDBLOCK) =>
                    {
                        let _ = self
                            .pty_reactor
                            .write(self.window_id, tab.id, pane_id, data);
                        return;
                    }
                    Err(_) => return,
                }
            }
            let _ = self
                .pty_reactor
                .write(self.window_id, tab.id, pane_id, data);
        }
    }

    #[inline(always)]
    pub fn write_to_active_pane(&self, data: Vec<u8>) {
        let active_id = self.active_pane().id;
        self.write_to_pane(active_id, data);
    }

    /// Paste text from the clipboard into the active pane.
    ///
    /// Retrieves text using `clipboard::paste()`, formats it according to
    /// bracketed paste mode and newline normalization, resets scroll offset if
    /// `scroll_on_keystroke` is set, writes the bytes to the active pane's PTY,
    /// marks interaction, updates `last_key_instant` for zero-latency frame bypass,
    /// and triggers a redraw.
    pub fn paste_clipboard(&mut self) -> bool {
        let text = crate::clipboard::clipboard::paste();
        if !text.is_empty() {
            let formatted = self.active_pane().terminal.format_paste(&text);
            if self.active_pane().terminal.scroll_on_keystroke {
                self.active_pane_mut()
                    .terminal
                    .active_grid_mut()
                    .scroll_offset = 0;
            }
            self.write_to_active_pane(formatted.into_bytes());
            self.last_key_instant = std::time::Instant::now();
            self.mark_interaction();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn paste(&mut self) -> bool {
        self.paste_clipboard()
    }

    #[inline(always)]
    pub fn set_cursor_cached(&mut self, icon: CursorIcon) {
        if self.current_cursor_icon != icon {
            self.current_cursor_icon = icon;
            self.window.set_cursor(icon);
        }
    }

    #[inline]
    pub fn hide_mouse(&mut self) {
        if !self.mouse_hidden {
            self.mouse_hidden = true;
            self.window.set_cursor_visible(false);
        }
    }

    #[inline]
    pub fn unhide_mouse(&mut self) {
        if self.mouse_hidden {
            self.mouse_hidden = false;
            self.window.set_cursor_visible(true);
            self.window.set_cursor(self.current_cursor_icon);
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

    #[inline(always)]
    pub fn banner_height(&self) -> f32 {
        if self.banner.is_some() { 26.0 } else { 0.0 }
    }

    #[inline(always)]
    pub fn search_height(&self) -> f32 {
        if self.search.is_open {
            (self.renderer.tab_glyph_cache.cell_height.max(1) + 8).max(24) as f32
        } else {
            0.0
        }
    }

    pub fn recalculate_panes_layout(
        &self,
        tab_index: usize,
    ) -> (Vec<PaneRect>, Vec<SeparatorRect>) {
        let size = self.window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        let csd_h = self.window.csd_title_bar_height() as f32;
        let tab_bar_h = self.tab_bar_height();
        let banner_h = self.banner_height();
        let search_h = self.search_height();
        let bottom_bars_h = banner_h + search_h;
        let total_x = 0.0;
        let total_y = csd_h + tab_bar_h;
        let total_w = (width as f32).max(10.0);
        let total_h = (height as f32 - csd_h - tab_bar_h - bottom_bars_h).max(10.0);

        if let Some(tab) = self.tabs.get(tab_index) {
            tab.tree.calculate_layout(
                total_x,
                total_y,
                total_w,
                total_h,
                self.separator_size,
                self.padding_x,
                self.padding_y,
                self.base_cell_width,
                self.base_cell_height,
                self.default_font_size,
                self.min_cols,
                self.min_rows,
            )
        } else {
            (Vec::new(), Vec::new())
        }
    }

    pub fn sync_tab_panes_dimensions(&mut self, tab_index: usize) {
        let (pane_rects, _) = self.recalculate_panes_layout(tab_index);

        if let Some(tab) = self.tabs.get_mut(tab_index) {
            for rect in pane_rects {
                if let Some(pane) = tab.tree.find_pane_mut(rect.pane_id) {
                    let cw = rect.cell_width as u32;
                    let ch = rect.cell_height as u32;
                    pane.terminal.set_cell_dimensions(cw, ch);
                    if pane.terminal.grid.width != rect.cols
                        || pane.terminal.grid.height != rect.rows
                    {
                        pane.terminal.resize(rect.cols as u32, rect.rows as u32);
                        crate::diagnostics::ResizeStats::record_pty_resize();
                        let _ = pane.pty_master.resize(rect.cols as u16, rect.rows as u16);
                        pane.terminal.active_grid_mut().mark_all_dirty();
                    }
                }
            }
        }

        if tab_index == self.active_tab_index {
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                let active_pane = tab.active_pane();
                self.resize_state.last_cols = active_pane.terminal.grid.width;
                self.resize_state.last_rows = active_pane.terminal.grid.height;
            }
        }
    }

    pub fn sync_active_tab_layout(&mut self) {
        self.sync_tab_panes_dimensions(self.active_tab_index);
    }

    pub fn recalculate_grid_size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        let csd_h = self.window.csd_title_bar_height() as f32;
        let tab_bar_h = self.tab_bar_height();
        let banner_h = self.banner_height();
        let search_h = self.search_height();
        let bottom_bars_h = banner_h + search_h;
        let avail_w = (width as f32 - self.padding_x * 2.0).max(10.0);
        let avail_h =
            (height as f32 - csd_h - tab_bar_h - bottom_bars_h - self.padding_y * 2.0).max(10.0);
        let cols = ((avail_w as u32) / self.cell_width().max(1)).max(1);
        let rows = ((avail_h as u32) / self.cell_height().max(1)).max(1);
        (cols, rows)
    }

    pub fn resize_active_tab(&mut self) {
        self.sync_active_tab_layout();
        self.search.invalidate_matches();
        self.needs_redraw = true;
        self.content_dirty = true;
    }

    pub fn set_renderer_font_size(&mut self, size: f32) {
        self.renderer.update_font_size(size);
    }

    pub fn max_font_size_for_active_pane(&self) -> f32 {
        let size = self.window.inner_size();
        let csd_h = self.window.csd_title_bar_height() as f32;
        let tab_bar_h = self.tab_bar_height();
        let banner_h = self.banner_height();
        let search_h = self.search_height();
        let total_h = (size.height as f32 - csd_h - tab_bar_h - banner_h - search_h).max(10.0);

        let pane_h = if let Some(tab) = self.tabs.get(self.active_tab_index) {
            tab.tree
                .find_pane_height(
                    tab.active_pane_id,
                    total_h,
                    self.separator_size,
                    self.min_rows,
                    self.base_cell_height as f32,
                    self.padding_y,
                )
                .unwrap_or(total_h)
        } else {
            total_h
        };

        let text_h = (pane_h - self.padding_y * 2.0).max(1.0);

        crate::app::split::max_font_size_for_1_row(
            text_h,
            self.base_cell_height as f32,
            self.default_font_size,
        )
    }

    pub fn sync_active_pane_font_size(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let max_size = self.max_font_size_for_active_pane();
        let active_pane_font_size = crate::app::split::clamp_font_size_with_max(
            self.active_pane().font_size,
            self.default_font_size,
            max_size,
        );
        let active_pane_id = self.active_tab().active_pane_id;
        if let Some(pane) = self.active_tab_mut().tree.find_pane_mut(active_pane_id) {
            pane.font_size = active_pane_font_size;
        }
        self.active_tab_mut().font_size = active_pane_font_size;
        if (self.current_font_size - active_pane_font_size).abs() > 0.01 {
            self.current_font_size = active_pane_font_size;
            self.set_renderer_font_size(active_pane_font_size);
            self.resize_active_tab();
        }
    }

    pub fn set_font_size(&mut self, size: f32) {
        let max_size = self.max_font_size_for_active_pane();
        let size =
            crate::app::split::clamp_font_size_with_max(size, self.default_font_size, max_size);
        if (self.current_font_size - size).abs() < 0.01
            && !self.tabs.is_empty()
            && (self.active_pane().font_size - size).abs() < 0.01
        {
            return;
        }
        self.current_font_size = size;
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.font_size = size;
            let active_pane_id = tab.active_pane_id;
            if let Some(pane) = tab.tree.find_pane_mut(active_pane_id) {
                pane.font_size = size;
            }
        }
        self.set_renderer_font_size(size);
        self.resize_active_tab();
    }

    pub fn request_resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if self.resize_state.last_applied_width == width
            && self.resize_state.last_applied_height == height
            && !self.resize_state.is_pending
        {
            return;
        }

        self.resize_state.pending_width = width;
        self.resize_state.pending_height = height;
        self.resize_state.is_pending = true;
        self.needs_redraw = true;
    }

    pub fn apply_pending_resize(&mut self) {
        if !self.resize_state.is_pending {
            return;
        }
        self.resize_state.is_pending = false;
        let width = self.resize_state.pending_width;
        let height = self.resize_state.pending_height;
        self.apply_viewport_resize(width, height);
    }

    pub fn apply_viewport_resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if self.resize_state.last_applied_width == width
            && self.resize_state.last_applied_height == height
            && self.renderer.viewport_width == width
            && self.renderer.viewport_height == height
        {
            return;
        }

        crate::diagnostics::ResizeStats::record_applied_viewport_resize();

        self.resize_state.last_applied_width = width;
        self.resize_state.last_applied_height = height;
        self.last_resize_instant = std::time::Instant::now();

        self.window.resize_dummy(width, height);
        self.renderer.resize(width, height);
        self.resize_active_tab();
        self.needs_redraw = true;
        self.content_dirty = true;
        self.window.request_redraw();
    }

    pub fn resize_renderer(&mut self, width: u32, height: u32) {
        self.apply_viewport_resize(width, height);
    }

    pub fn split_horizontal(&mut self) -> Option<PaneId> {
        self.split_active_pane(SplitDirection::Horizontal)
    }

    pub fn split_vertical(&mut self) -> Option<PaneId> {
        self.split_active_pane(SplitDirection::Vertical)
    }

    pub fn split_active_pane(&mut self, direction: SplitDirection) -> Option<PaneId> {
        if self.tabs.is_empty() {
            return None;
        }

        let tab_id = self.active_tab().id;
        let active_pane_id = self.active_tab().active_pane_id;
        let new_pane_id = self.next_pane_id;
        self.next_pane_id += 1;
        let split_id = self.next_split_id;
        self.next_split_id += 1;

        let csd_h = self.window.csd_title_bar_height() as f32;
        let banner_h = self.banner_height();
        let search_h = self.search_height();
        let avail_w =
            (self.window.inner_size().width.max(1) as f32 - self.padding_x * 2.0).max(10.0);
        let avail_h = (self.window.inner_size().height.max(1) as f32
            - csd_h
            - self.tab_bar_height()
            - banner_h
            - search_h
            - self.padding_y * 2.0)
            .max(10.0);
        let cols = ((avail_w as u32) / self.base_cell_width.max(1)).max(1);
        let rows = ((avail_h as u32) / self.base_cell_height.max(1)).max(1);

        let cwd = self.active_pane().current_working_directory();
        let pty_spawn = spawn_terminal_pty_with_size(
            &self.shell_path,
            None,
            cwd.as_deref(),
            Some((cols as u16, rows as u16)),
        );
        let pty_master = Arc::new(pty_spawn.ok()?);

        let mut terminal = Terminal::with_config(cols as usize, rows as usize, &self.config);
        terminal.set_cell_dimensions(self.base_cell_width, self.base_cell_height);

        let _ = self
            .pty_reactor
            .register(self.window_id, tab_id, new_pane_id, pty_master.clone());

        let font_size = self.default_font_size;
        let new_pane = Pane::new(new_pane_id, pty_master, terminal, font_size, false);

        let tab = self.active_tab_mut();
        let split_success = tab
            .tree
            .split_pane(active_pane_id, new_pane, direction, 0.5, split_id);
        if !split_success {
            return None;
        }

        tab.set_active_pane(new_pane_id);
        tab.clear_unfocused_selections();
        self.sync_active_pane_font_size();
        self.sync_active_tab_layout();
        self.needs_redraw = true;
        self.content_dirty = true;
        self.tab_bar_dirty = true;

        Some(new_pane_id)
    }

    pub fn close_pane(&mut self) -> bool {
        if self.tabs.is_empty() {
            return true;
        }
        let tab_id = self.active_tab().id;
        let pane_id = self.active_tab().active_pane_id;
        self.close_pane_in_tab(tab_id, pane_id)
    }

    pub fn close_pane_in_tab(&mut self, tab_id: u64, pane_id: u64) -> bool {
        let tab_idx = match self.tabs.iter().position(|t| t.id == tab_id) {
            Some(idx) => idx,
            None => return false,
        };

        let tab = &mut self.tabs[tab_idx];
        if tab.tree.pane_count() <= 1 {
            return self.close_tab(tab_idx);
        }

        let removed = tab.remove_pane(pane_id);
        if removed.is_some() {
            let _ = self.pty_reactor.unregister(self.window_id, tab_id, pane_id);
            tab.clear_unfocused_selections();
            self.sync_active_pane_font_size();
            self.sync_tab_panes_dimensions(tab_idx);
            self.release_memory();
            self.tab_bar_dirty = true;
            self.needs_redraw = true;
            self.content_dirty = true;
        }
        false
    }

    pub fn focus_direction(&mut self, direction: FocusDirection) -> bool {
        if self.tabs.is_empty() {
            return false;
        }

        let (pane_rects, _) = self.recalculate_panes_layout(self.active_tab_index);
        let active_pane_id = self.active_tab().active_pane_id;

        if let Some(target_id) =
            crate::app::split::find_neighbor_pane(&pane_rects, active_pane_id, direction)
        {
            let tab = self.active_tab_mut();
            tab.set_active_pane(target_id);
            tab.clear_unfocused_selections();
            self.search.invalidate_for_pane_switch(target_id);
            self.sync_active_pane_font_size();
            self.needs_redraw = true;
            self.content_dirty = true;
            return true;
        }
        false
    }

    pub fn focus_next_pane(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let tab = self.active_tab_mut();
        let pane_ids = tab.tree.collect_pane_ids();
        if pane_ids.len() > 1
            && let Some(pos) = pane_ids.iter().position(|&id| id == tab.active_pane_id)
        {
            let next_pos = (pos + 1) % pane_ids.len();
            let next_id = pane_ids[next_pos];
            tab.set_active_pane(next_id);
            tab.clear_unfocused_selections();
            self.search.invalidate_for_pane_switch(next_id);
            self.sync_active_pane_font_size();
            self.needs_redraw = true;
            self.content_dirty = true;
        }
    }

    pub fn focus_previous_pane(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let tab = self.active_tab_mut();
        let pane_ids = tab.tree.collect_pane_ids();
        if pane_ids.len() > 1
            && let Some(pos) = pane_ids.iter().position(|&id| id == tab.active_pane_id)
        {
            let prev_pos = (pos + pane_ids.len() - 1) % pane_ids.len();
            let prev_id = pane_ids[prev_pos];
            tab.set_active_pane(prev_id);
            tab.clear_unfocused_selections();
            self.search.invalidate_for_pane_switch(prev_id);
            self.sync_active_pane_font_size();
            self.needs_redraw = true;
            self.content_dirty = true;
        }
    }

    pub fn adjust_active_split_ratio(&mut self, delta: f32) -> bool {
        if self.tabs.is_empty() {
            return false;
        }
        let active_pane_id = self.active_tab().active_pane_id;
        let tab = self.active_tab_mut();
        let adjusted = tab.tree.adjust_ancestor_split_ratio(active_pane_id, delta);
        if adjusted {
            self.sync_active_tab_layout();
            self.needs_redraw = true;
            self.content_dirty = true;
            return true;
        }
        false
    }

    pub fn create_tab(
        &mut self,
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        custom_title: Option<String>,
        hold: Option<bool>,
    ) -> u64 {
        let working_directory = working_directory.or_else(|| {
            if !self.tabs.is_empty() {
                self.active_pane().current_working_directory()
            } else {
                None
            }
        });

        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let was_visible = self.tab_bar.is_visible(self.tabs.len());

        let tab_font_size = self.default_font_size;
        if (self.current_font_size - tab_font_size).abs() > 0.01 {
            self.current_font_size = tab_font_size;
            self.set_renderer_font_size(tab_font_size);
        }

        let (cols, rows) = self.recalculate_grid_size();

        let pty_spawn = spawn_terminal_pty_with_size(
            &self.shell_path,
            command.as_deref(),
            working_directory.as_deref(),
            Some((cols as u16, rows as u16)),
        );

        let pty_master = match pty_spawn {
            Ok(pty) => Arc::new(pty),
            Err(e) => {
                log::error!("Critical failure spawning terminal shell process: {}", e);
                return self
                    .tabs
                    .get(self.active_tab_index)
                    .map(|t| t.id)
                    .unwrap_or(0);
            }
        };

        let mut terminal = Terminal::with_config(cols as usize, rows as usize, &self.config);
        terminal.set_cell_dimensions(self.cell_width(), self.cell_height());
        let initial_title = custom_title.clone().unwrap_or_else(|| "velox".to_string());

        let _ = self
            .pty_reactor
            .register(self.window_id, tab_id, pane_id, pty_master.clone());

        let pane = Pane::with_title(
            pane_id,
            pty_master,
            terminal,
            custom_title.clone(),
            initial_title.clone(),
            tab_font_size,
            hold.unwrap_or(false),
        );

        let tab = Tab::with_pane(
            tab_id,
            pane,
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
        } else {
            self.sync_active_tab_layout();
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

        let removed_tab = self.tabs.remove(index);
        for pane in removed_tab.tree.panes() {
            let _ = self
                .pty_reactor
                .unregister(self.window_id, removed_tab.id, pane.id);
        }

        if self.tabs.is_empty() {
            return true;
        }

        if index < self.active_tab_index {
            self.active_tab_index -= 1;
        } else if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len().saturating_sub(1);
        }

        let active_font_size = self.tabs[self.active_tab_index].active_pane().font_size;
        self.tabs[self.active_tab_index].font_size = active_font_size;
        if (self.current_font_size - active_font_size).abs() > 0.01 {
            self.current_font_size = active_font_size;
            self.set_renderer_font_size(active_font_size);
        }

        self.resize_active_tab();

        self.release_memory();
        self.tab_bar_dirty = true;
        self.needs_redraw = true;
        self.content_dirty = true;
        for pane in self.tabs[self.active_tab_index].tree.panes_mut() {
            pane.terminal.active_grid_mut().mark_all_dirty();
        }
        false
    }

    pub fn switch_tab(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active_tab_index {
            self.active_tab_index = index;
            let active_pane_font_size = self.active_pane().font_size;
            self.tabs[index].font_size = active_pane_font_size;
            if (self.current_font_size - active_pane_font_size).abs() > 0.01 {
                self.current_font_size = active_pane_font_size;
                self.set_renderer_font_size(active_pane_font_size);
            }
            self.sync_tab_panes_dimensions(index);
            let new_active_id = self.active_pane().id;
            self.search.invalidate_for_pane_switch(new_active_id);
            self.window.set_title(&self.tabs[index].current_title);
            self.current_title = self.tabs[index].current_title.clone();
            self.tab_bar_dirty = true;
            self.needs_redraw = true;
            self.content_dirty = true;
            for pane in self.tabs[index].tree.panes_mut() {
                pane.terminal.active_grid_mut().mark_all_dirty();
            }
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

    /// Reorders a tab from slot `from` to slot `to`.
    ///
    /// Updates the active tab index, marks the tab bar cache dirty, resets the cached render
    /// info, updates `last_resize_instant` for zero-latency frame timing, marks all renderer damage,
    /// and flags the active panes as dirty for immediate presentation across all display backends.
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from < self.tabs.len() && to < self.tabs.len() && from != to {
            let tab = self.tabs.remove(from);
            self.tabs.insert(to, tab);
            self.active_tab_index = to;
            self.tab_bar_dirty = true;
            self.tab_bar_render_cache = None;
            self.needs_redraw = true;
            self.content_dirty = true;
            self.last_resize_instant = std::time::Instant::now();
            self.renderer.damage.mark_all();
            for pane in self.tabs[to].tree.panes_mut() {
                pane.terminal.active_grid_mut().mark_all_dirty();
            }
        }
    }

    fn update_active_tab_title(&mut self) {
        if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index)
            && (active_tab.update_title() || self.current_title != active_tab.current_title)
        {
            self.window.set_title(&active_tab.current_title);
            self.current_title = active_tab.current_title.clone();
            self.tab_bar_dirty = true;
            self.renderer.damage.mark_all();
        }
    }

    fn update_tab_bar_cache_if_dirty(&mut self) {
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
    }

    fn prepare_separator_render_data(
        sep_rects: Vec<crate::app::split::SeparatorRect>,
        active_pane_rect: Option<crate::app::split::PaneRect>,
        dragging_separator: Option<DraggingSeparator>,
        hovered_separator: Option<SplitId>,
    ) -> Vec<SeparatorRenderData> {
        let dragging_split_id = dragging_separator.map(|d| d.split_id);
        let hovered_split_id = hovered_separator;

        sep_rects
            .into_iter()
            .map(|r| {
                let active_segment = active_pane_rect.and_then(|ap| r.active_segment_for_pane(&ap));
                let is_active = active_segment.is_some();
                SeparatorRenderData {
                    rect: r,
                    is_active,
                    active_segment,
                    is_hovered: hovered_split_id == Some(r.split_id),
                    is_dragging: dragging_split_id == Some(r.split_id),
                }
            })
            .collect()
    }

    #[inline(always)]
    pub(crate) fn compute_cursor_visibility(
        is_active: bool,
        scroll_offset: usize,
        grid_cursor_visible: bool,
        cursor_blink_enabled: bool,
        cursor_blink_on: bool,
    ) -> bool {
        is_active
            && scroll_offset == 0
            && grid_cursor_visible
            && (!cursor_blink_enabled || cursor_blink_on)
    }

    fn prepare_pane_render_data<'a>(
        pane_rects: &[crate::app::split::PaneRect],
        active_tab: &'a Tab,
        active_pane_id: PaneId,
        cursor_blink_enabled: bool,
        cursor_blink_on: bool,
        is_focused: bool,
    ) -> Vec<CpuPaneRenderData<'a>> {
        let mut pane_render_datas = Vec::with_capacity(pane_rects.len());

        for rect in pane_rects {
            if let Some(pane) = active_tab.tree.find_pane(rect.pane_id) {
                let active_grid = pane.terminal.active_grid();
                let width = active_grid.width;
                let offset = active_grid.scroll_offset;
                let is_active = pane.id == active_pane_id;

                let cursor_visible = Self::compute_cursor_visibility(
                    is_active,
                    offset,
                    active_grid.cursor.visible,
                    cursor_blink_enabled,
                    cursor_blink_on,
                );

                let cursor_shape = if !is_focused
                    && active_grid.cursor.shape == crate::screen::cursor::CursorShape::Block
                {
                    crate::screen::cursor::CursorShape::HollowBlock
                } else {
                    active_grid.cursor.shape
                };

                let display_cursor_x = active_grid.cursor.x.min(width.saturating_sub(1));

                pane_render_datas.push(CpuPaneRenderData {
                    pane_id: pane.id,
                    rect: *rect,
                    cells: &active_grid.cells,
                    grid: active_grid,
                    font_size: pane.font_size,
                    theme: &pane.terminal.theme,
                    bold_is_bright: pane.terminal.bold_is_bright,
                    cursor_visible,
                    cursor_shape,
                    display_cursor_x,
                    is_active,
                });
            }
        }

        pane_render_datas
    }

    pub fn draw(&mut self) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 || self.tabs.is_empty() {
            return;
        }

        // If a resize is pending or window inner size does not match renderer viewport size,
        // apply the resize FIRST so renderer dimensions, terminal grids, and window buffer
        // are perfectly synchronized before rendering any pixels.
        if self.resize_state.is_pending
            || self.renderer.viewport_width != size.width
            || self.renderer.viewport_height != size.height
        {
            self.apply_viewport_resize(size.width, size.height);
        }

        crate::diagnostics::ResizeStats::record_frame_rendered();
        if self.renderer.damage.full_redraw {
            crate::diagnostics::ResizeStats::record_full_redraw();
        }

        self.last_frame_instant = std::time::Instant::now();

        // 1. Ensure renderer font size matches the active pane's isolated font size
        self.sync_active_pane_font_size();

        // 2. Update title for active tab (and window title)
        self.update_active_tab_title();

        // 3. Update tab bar render cache if dirty
        self.update_tab_bar_cache_if_dirty();

        // 4. Compute layouts & active pane
        let (mut pane_rects, mut sep_rects) = self.recalculate_panes_layout(self.active_tab_index);

        // Ensure all pane terminals are synchronized with the computed layout before rendering.
        // If split ratio or window dimensions changed without a layout sync, sync now so that
        // the grid cells and reflow are always 100% consistent with the pane bounds.
        let mut needs_layout_sync = false;
        if let Some(tab) = self.tabs.get(self.active_tab_index) {
            for rect in &pane_rects {
                if let Some(pane) = tab.tree.find_pane(rect.pane_id) {
                    if pane.terminal.grid.width != rect.cols
                        || pane.terminal.grid.height != rect.rows
                        || pane.terminal.cell_width_px != rect.cell_width as u32
                        || pane.terminal.cell_height_px != rect.cell_height as u32
                    {
                        needs_layout_sync = true;
                        break;
                    }
                }
            }
        }
        if needs_layout_sync {
            self.sync_active_tab_layout();
            self.search.matches_dirty = true;
            let (new_pane_rects, new_sep_rects) =
                self.recalculate_panes_layout(self.active_tab_index);
            pane_rects = new_pane_rects;
            sep_rects = new_sep_rects;
        }

        let active_tab = &self.tabs[self.active_tab_index];
        let active_pane_id = active_tab.active_pane_id;
        let active_pane_rect = pane_rects
            .iter()
            .find(|r| r.pane_id == active_pane_id)
            .copied();

        // 5. Prepare separator & pane render data
        let separator_render_datas = Self::prepare_separator_render_data(
            sep_rects,
            active_pane_rect,
            self.dragging_separator,
            self.hovered_separator,
        );

        let fallback_theme;
        let active_theme = if let Some(active_pane) = active_tab.tree.find_pane(active_pane_id) {
            &active_pane.terminal.theme
        } else if let Some(first_pane) = active_tab.tree.panes().first() {
            &first_pane.terminal.theme
        } else {
            fallback_theme = crate::theme::theme::Theme::default();
            &fallback_theme
        };

        let effective_separator_color = self
            .separator_color
            .as_deref()
            .and_then(|spec| active_theme.parse_color_spec(spec));
        let effective_active_separator_color = self
            .active_separator_color
            .as_deref()
            .and_then(|spec| active_theme.parse_color_spec(spec))
            .or_else(|| Some(active_theme.resolve_tab_accent_color()));

        let cpu_pane_render_datas = Self::prepare_pane_render_data(
            &pane_rects,
            active_tab,
            active_pane_id,
            self.cursor_blink_enabled,
            self.cursor_blink_on,
            self.is_focused,
        );

        let effective_dim = if !self.is_focused {
            self.window_dim
        } else {
            0.0
        };

        let is_tab_bar_visible = self.tab_bar.is_visible(self.tabs.len());
        let tab_bar_info = if is_tab_bar_visible {
            self.tab_bar_render_cache.as_ref()
        } else {
            None
        };

        let csd_height = self.window.csd_title_bar_height();
        self.renderer.csd_height = csd_height;

        if self.search.is_open {
            if self.search.matches_dirty {
                if let Some(tab) = self.tabs.get(self.active_tab_index) {
                    if let Some(pane) = tab.tree.find_pane(tab.active_pane_id) {
                        self.search.recompute_matches(pane.terminal.active_grid());
                    }
                }
            }
            self.renderer
                .set_search_matches(&self.search.matches, self.search.active_match_index);
        } else {
            self.renderer.clear_search_matches();
        }

        // 6. Render & Present
        let damage_rect = self.renderer.render_splits(
            &cpu_pane_render_datas,
            &separator_render_datas,
            self.opacity,
            effective_dim,
            self.is_focused,
            None,
            tab_bar_info,
            effective_separator_color,
            effective_active_separator_color,
        );

        let mut final_damage = damage_rect;

        if self.search.is_open && !pane_rects.is_empty() {
            let active_rect = pane_rects
                .iter()
                .find(|r| r.pane_id == active_pane_id)
                .copied()
                .unwrap_or(pane_rects[0]);
            let banner_h = self.banner_height() as u32;
            if let Some(search_damage) =
                self.renderer
                    .render_search_overlay(&self.search, &active_rect, banner_h)
            {
                final_damage = Some(match final_damage {
                    Some(dmg) => dmg.union(search_damage),
                    None => search_damage,
                });
            }
        }

        if csd_height > 0 {
            let is_max = self.window.is_maximized();
            let csd_redrawn = self.renderer.render_csd_title_bar(
                &self.current_title,
                csd_height,
                self.is_focused,
                is_max,
                self.hovered_csd_button,
                self.pressed_csd_button,
                &self.csd_button_layout,
                &self.csd_palette,
            );
            if csd_redrawn {
                final_damage = Some(crate::window::DamageRect {
                    x: 0,
                    y: 0,
                    width: self.renderer.viewport_width,
                    height: self.renderer.viewport_height,
                });
            }

            // Continuously maintain 1px window border for unmaximized windows so pane damage
            // (e.g. typing at shell prompt or scrolling) never cuts out or makes borders transparent.
            if !is_max {
                let border_color = 0xFF000000 | (self.csd_palette.border & 0x00FFFFFF);
                self.renderer.render_csd_window_borders(border_color);
            }
        }

        if let Some(ref banner) = self.banner {
            let banner_y = self.renderer.viewport_height.saturating_sub(26);
            self.renderer.render_alert_banner(banner, banner_y, 26);
            let banner_damage = crate::window::DamageRect {
                x: 0,
                y: banner_y,
                width: self.renderer.viewport_width,
                height: 26,
            };
            final_damage = Some(match final_damage {
                Some(dmg) => dmg.union(banner_damage),
                None => banner_damage,
            });
        }

        self.window.present_frame(
            self.renderer.framebuffer.as_slice(),
            self.renderer.viewport_width as usize,
            final_damage,
        );

        if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index) {
            for pane in active_tab.tree.panes_mut() {
                pane.terminal.active_grid_mut().clear_damage();
            }
        }
    }

    /// Checks if the desktop theme (dark/light mode) or button layout changed,
    /// updating `csd_palette` and marking damage for an immediate redraw if so.
    pub fn update_csd_theme_if_changed(&mut self) -> bool {
        let new_palette = crate::window::csd::CsdPalette::resolve(
            self.config.decorations_config().as_ref(),
            &self.renderer.palette,
        );
        let new_layout = crate::window::csd::CsdButtonLayout::detect_system();

        let mut changed = false;
        if self.csd_palette != new_palette {
            self.csd_palette = new_palette;
            changed = true;
        }
        if self.csd_button_layout != new_layout {
            self.csd_button_layout = new_layout;
            changed = true;
        }

        if changed {
            self.renderer.prev_csd_hash = 0;
            self.renderer.damage.mark_all();
            self.needs_redraw = true;
        }

        changed
    }
}

pub struct App {
    pub(crate) event_loop_proxy: EventLoopProxy,
    pub(crate) event_receiver: Receiver<CustomEvent>,
    pub(crate) wake_fd: Option<Arc<OwnedFd>>,
    pub(crate) modifiers: ModifiersState,
    pub(crate) windows: HashMap<WindowId, WindowState>,
    pub(crate) next_window_id: u64,
    pub(crate) daemon_mode: bool,
    pub(crate) single_instance_mode: bool,
    pub(crate) ipc_listener: Option<IpcListenerHandle>,
    pub(crate) initial_options: Option<CliOptions>,
    pub(crate) pty_reactor: Option<crate::pty::PtyReactor>,
    pub(crate) config: Config,
    pub(crate) config_diagnostics: Vec<crate::config::loader::ConfigDiagnostic>,
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(mut reactor) = self.pty_reactor.take() {
            reactor.shutdown();
        }
        if let Some(mut ipc) = self.ipc_listener.take() {
            ipc.shutdown();
        }
        self.wake_fd = None;
    }
}

impl App {
    pub fn new_with_config(
        options: CliOptions,
        config: Config,
        diagnostics: Vec<crate::config::loader::ConfigDiagnostic>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let wake_raw = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if wake_raw < 0 {
            return Err("Failed to create eventfd for main loop wakeup".into());
        }
        let wake_fd = Arc::new(unsafe { OwnedFd::from_raw_fd(wake_raw) });

        let proxy = EventLoopProxy::from_owned_fd(sender, wake_fd.clone());
        let pty_reactor = crate::pty::PtyReactor::new(proxy.clone())?;
        let daemon_mode = options.daemon;
        let single_instance_mode = options.single_instance;

        Ok(Self {
            event_loop_proxy: proxy,
            event_receiver: receiver,
            wake_fd: Some(wake_fd),
            modifiers: ModifiersState::empty(),
            windows: HashMap::new(),
            next_window_id: 1,
            daemon_mode,
            single_instance_mode,
            ipc_listener: None,
            initial_options: Some(options),
            pty_reactor: Some(pty_reactor),
            config,
            config_diagnostics: diagnostics,
        })
    }

    pub fn new(options: CliOptions) -> Result<Self, Box<dyn std::error::Error>> {
        let (config, diagnostics, _) = crate::config::loader::load_with_diagnostics();
        Self::new_with_config(options, config, diagnostics)
    }

    pub fn create_window(
        &mut self,
        working_directory: Option<String>,
        command: Option<Vec<String>>,
        custom_title: Option<String>,
        hold: Option<bool>,
    ) -> Result<WindowId, Box<dyn std::error::Error>> {
        let config = self.config.clone();

        let initial_title = match &custom_title {
            Some(t) => t.clone(),
            None => match &config.app_title {
                Some(tpl) => tpl.replace("{program}", "velox"),
                None => "velox".to_string(),
            },
        };

        let (mut window, mut renderer) =
            crate::renderer::create_window_and_renderer(&initial_title, 800, 600, &config)?;

        if config.repeat_delay().is_some() || config.repeat_rate().is_some() {
            window.set_key_repeat(config.repeat_delay(), config.repeat_rate());
        }

        let size = window.inner_size();
        let win_width = size.width.max(1);
        let win_height = size.height.max(1);

        let scroll_multiplier = config.scroll_multiplier().unwrap_or(1.0);
        let cursor_blink_enabled = config.cursor_blink().unwrap_or(true);
        let hide_mouse_on_typing = config.hide_mouse_on_typing().unwrap_or(true);
        let detected_refresh_rate = window.detect_refresh_rate();
        let fps_limit = config.fps_limit().or(detected_refresh_rate).or(Some(120));

        let font_size = config.font_size();
        let padding_x = config.pane_padding_x().unwrap_or(8.0);
        let padding_y = config.pane_padding_y().unwrap_or(4.0);

        let tab_font_size = config.tab_font_size();
        renderer.tab_glyph_cache.update_font_size(tab_font_size);
        let base_cell_width = renderer.glyph_cache.cell_width;
        let base_cell_height = renderer.glyph_cache.cell_height;

        let csd_h = window.csd_title_bar_height() as f32;
        let tab_bar = TabBar::from_config(&config);
        let tab_bar_h = if tab_bar.is_visible(1) {
            tab_bar.height(base_cell_height)
        } else {
            0.0
        };

        let banner = if let Some(diag) = self.config_diagnostics.first() {
            Some(AlertBanner {
                severity: diag.severity,
                message: format!("{diag}"),
                dismiss_hovered: false,
            })
        } else {
            None
        };
        let banner_h = if banner.is_some() { 26.0 } else { 0.0 };

        let avail_w = (win_width as f32 - padding_x * 2.0).max(10.0);
        let avail_h =
            (win_height as f32 - csd_h - tab_bar_h - banner_h - padding_y * 2.0).max(10.0);
        let cols = ((avail_w as u32) / base_cell_width.max(1)).max(1);
        let rows = ((avail_h as u32) / base_cell_height.max(1)).max(1);

        let terminal = Terminal::with_config(cols as usize, rows as usize, &config);

        let shell_path = config
            .shell
            .clone()
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());

        let working_directory = working_directory.or_else(|| config.working_directory());

        let pty_spawn = spawn_terminal_pty_with_size(
            &shell_path,
            command.as_deref(),
            working_directory.as_deref(),
            Some((cols as u16, rows as u16)),
        );

        let pty_master = Arc::new(
            pty_spawn
                .map_err(|e| format!("Failed to spawn shell process after fallbacks: {}", e))?,
        );

        let window_id = self.next_window_id;
        self.next_window_id += 1;
        let tab_id = 0u64;
        let pane_id = 0u64;
        if let Some(ref reactor) = self.pty_reactor {
            reactor.register(window_id, tab_id, pane_id, pty_master.clone())?;
        }

        let tab = Tab::new(
            tab_id,
            pty_master,
            terminal,
            custom_title,
            initial_title.clone(),
            hold.unwrap_or(false),
            font_size,
        );

        let separator_size = config.pane_separator_size();
        let min_cols = config.pane_minimum_columns();
        let min_rows = config.pane_minimum_rows();
        let separator_color = config.pane_separator_color().map(String::from);
        let active_separator_color = config.pane_active_separator_color().map(String::from);
        let csd_palette = crate::window::csd::CsdPalette::resolve(
            config.decorations_config().as_ref(),
            &renderer.palette,
        );

        let mut window_state = WindowState {
            window_id,
            config: config.clone(),
            banner,
            renderer,
            window,
            mouse_x: 0.0,
            mouse_y: 0.0,
            scroll_multiplier,
            scroll_accumulator: 0.0,
            fps_limit,
            last_frame_instant: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(std::time::Instant::now),
            last_input_instant: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(10))
                .unwrap_or_else(std::time::Instant::now),
            last_key_instant: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(10))
                .unwrap_or_else(std::time::Instant::now),
            last_resize_instant: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(10))
                .unwrap_or_else(std::time::Instant::now),
            current_title: initial_title,
            default_font_size: font_size,
            current_font_size: font_size,
            base_cell_width,
            base_cell_height,
            padding_x,
            padding_y,
            is_mouse_down: false,
            mouse_down_pane_id: None,
            last_mouse_button: 0,
            last_click_instant: None,
            last_click_pos: (0, 0),
            last_click_pane_id: None,
            click_count: 0,
            last_mouse_cell: (0, 0),
            last_mouse_pane_id: None,
            is_focused: true,
            current_cursor_icon: CursorIcon::Default,
            needs_redraw: false,
            content_dirty: false,
            cursor_blink_enabled,
            cursor_blink_on: true,
            last_cursor_blink: std::time::Instant::now(),
            hide_mouse_on_typing,
            mouse_hidden: false,
            opacity: config.opacity(),
            window_dim: config.window_dim(),
            shell_path,
            tabs: vec![tab],
            active_tab_index: 0,
            next_tab_id: 1,
            next_pane_id: 1,
            next_split_id: 1,
            tab_bar,
            event_loop_proxy: self.event_loop_proxy.clone(),
            pty_reactor: self
                .pty_reactor
                .as_ref()
                .map(|r| r.handle())
                .unwrap_or_else(crate::pty::PtyReactorHandle::dummy),
            tab_bar_dirty: true,
            tab_bar_render_cache: None,
            dragging_separator: None,
            dragging_tab: None,
            hovered_separator: None,
            separator_size,
            min_cols,
            min_rows,
            separator_color,
            active_separator_color,
            last_separator_drag_sync: None,
            csd_button_layout: crate::window::csd::CsdButtonLayout::detect_system(),
            csd_palette,
            hovered_csd_button: None,
            pressed_csd_button: None,
            resize_state: ResizeState {
                last_applied_width: win_width,
                last_applied_height: win_height,
                last_cols: cols as usize,
                last_rows: rows as usize,
                ..Default::default()
            },
            search: crate::screen::search::SearchState::new(),
        };

        window_state.window.set_opacity(window_state.opacity);
        window_state.draw();
        window_state.window.set_visible(true);

        self.windows.insert(window_id, window_state);
        crate::memory::trim_allocator_memory();
        Ok(window_id)
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Velox starting (pure Rust zero-C-libraries windowing)");

        let config = &self.config;

        if config.single_instance.unwrap_or(true) {
            self.single_instance_mode = true;
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
            let working_dir = opts
                .working_directory
                .or_else(|| config.working_directory());
            self.create_window(working_dir, opts.command, opts.title, Some(opts.hold))?;
        }

        Ok(())
    }

    /// Drain pending custom events with bounded coalescing for PTY data batches.
    /// Reduces terminal state machine re-entry, avoids intermediate redraws, and bounds latency to 256 KiB.
    pub fn drain_custom_events(&mut self) -> bool {
        self.drain_custom_events_with_first(None)
    }

    pub fn drain_custom_events_with_first(&mut self, mut first_event: Option<CustomEvent>) -> bool {
        let mut last_affected_pane: Option<(WindowId, u64, PaneId)> = None;
        let mut coalesced_bytes = 0usize;
        let mut had_pty_data = false;

        loop {
            let event = match first_event.take() {
                Some(ev) => ev,
                None => match self.event_receiver.try_recv() {
                    Ok(ev) => ev,
                    Err(_) => break,
                },
            };

            match event {
                CustomEvent::PtyData {
                    window_id,
                    tab_id,
                    pane_id,
                    data,
                } => {
                    had_pty_data = true;
                    let key = (window_id, tab_id, pane_id);
                    if let Some(prev) = last_affected_pane {
                        if prev != key || coalesced_bytes >= 256 * 1024 {
                            self.flush_pane_output_and_redraw(prev.0, prev.1, prev.2);
                            last_affected_pane = Some(key);
                            coalesced_bytes = 0;
                        }
                    } else {
                        last_affected_pane = Some(key);
                    }

                    if let Some(ws) = self.windows.get_mut(&window_id)
                        && let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == tab_id)
                        && let Some(pane) = tab.tree.find_pane_mut(pane_id)
                    {
                        pane.terminal.feed(&data);
                        coalesced_bytes += data.len();
                    }
                    crate::pty::recycle_pty_buffer(data);
                }
                other => {
                    if let Some(prev) = last_affected_pane.take() {
                        self.flush_pane_output_and_redraw(prev.0, prev.1, prev.2);
                        coalesced_bytes = 0;
                    }
                    self.handle_custom_event(other);
                }
            }
        }

        if let Some(prev) = last_affected_pane {
            self.flush_pane_output_and_redraw(prev.0, prev.1, prev.2);
        }

        had_pty_data
    }

    fn flush_pane_output_and_redraw(&mut self, window_id: WindowId, tab_id: u64, pane_id: PaneId) {
        if let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == tab_id)
            && let Some(pane) = tab.tree.find_pane_mut(pane_id)
        {
            let now = std::time::Instant::now();
            pane.last_activity = now;
            tab.last_activity = now;
            let _ = pane.flush_outgoing();
            if !pane.terminal.outgoing.is_empty() {
                let unwritten = std::mem::take(&mut pane.terminal.outgoing);
                let _ = ws.pty_reactor.write(window_id, tab_id, pane_id, unwritten);
            }
            if ws.tabs.get(ws.active_tab_index).map(|t| t.id) == Some(tab_id) {
                ws.needs_redraw = true;
                ws.content_dirty = true;
            }
        }
    }

    pub fn handle_custom_event(&mut self, event: CustomEvent) {
        match event {
            CustomEvent::PtyData {
                window_id,
                tab_id,
                pane_id,
                data,
            } => {
                if let Some(ws) = self.windows.get_mut(&window_id)
                    && let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == tab_id)
                    && let Some(pane) = tab.tree.find_pane_mut(pane_id)
                {
                    pane.last_activity = std::time::Instant::now();
                    tab.last_activity = pane.last_activity;
                    pane.terminal.feed(&data);
                    let _ = pane.flush_outgoing();
                    if !pane.terminal.outgoing.is_empty() {
                        let unwritten = std::mem::take(&mut pane.terminal.outgoing);
                        let _ = ws.pty_reactor.write(window_id, tab_id, pane_id, unwritten);
                    }
                    if ws.tabs.get(ws.active_tab_index).map(|t| t.id) == Some(tab_id) {
                        if ws.active_pane().id == pane_id {
                            ws.search.invalidate_matches();
                        }
                        ws.needs_redraw = true;
                        ws.content_dirty = true;
                    }
                }
                crate::pty::recycle_pty_buffer(data);
            }
            CustomEvent::PtyExit {
                window_id,
                tab_id,
                pane_id,
            } => {
                let mut should_remove_window = false;
                if let Some(ws) = self.windows.get_mut(&window_id)
                    && let Some(tab_idx) = ws.tabs.iter().position(|t| t.id == tab_id)
                {
                    let tab = &mut ws.tabs[tab_idx];
                    if let Some(pane) = tab.tree.find_pane_mut(pane_id) {
                        if pane.hold {
                            let title = "[Process exited]";
                            pane.current_title = title.to_string();
                            if tab.active_pane_id == pane_id {
                                tab.current_title = title.to_string();
                                if ws.active_tab_index == tab_idx {
                                    ws.window.set_title(title);
                                }
                            }
                            ws.tab_bar_dirty = true;
                            ws.needs_redraw = true;
                            return;
                        }
                    }
                    should_remove_window = ws.close_pane_in_tab(tab_id, pane_id);
                }
                if should_remove_window {
                    if let Some(ws) = self.windows.remove(&window_id) {
                        for tab in &ws.tabs {
                            for pane in tab.tree.panes() {
                                let _ = ws.pty_reactor.unregister(window_id, tab.id, pane.id);
                            }
                        }
                    }
                    crate::memory::trim_allocator_memory();
                }
            }
            CustomEvent::IpcCreateWindow {
                working_directory,
                command,
                title,
                hold,
            } => {
                let _ = self.create_window(working_directory, command, title, hold);
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
                    let _ = self.create_window(working_directory, command, title, hold);
                }
            }
        }
    }

    #[inline]
    pub(crate) fn compute_ppoll_timeout(
        min_next_wake: Option<std::time::Instant>,
        has_dirty_windows: bool,
    ) -> (libc::timespec, bool) {
        match min_next_wake {
            Some(wake_time) => {
                let cur = std::time::Instant::now();
                if wake_time > cur {
                    let diff = wake_time.duration_since(cur);
                    // If the deadline is within 250µs, don't sleep in the kernel - do an immediate poll
                    // to prevent kernel timer slack overshooting and causing micro-stutter.
                    if diff < std::time::Duration::from_micros(250) {
                        (
                            libc::timespec {
                                tv_sec: 0,
                                tv_nsec: 0,
                            },
                            true,
                        )
                    } else {
                        (
                            libc::timespec {
                                tv_sec: diff.as_secs() as libc::time_t,
                                tv_nsec: diff.subsec_nanos() as libc::c_long,
                            },
                            true,
                        )
                    }
                } else {
                    (
                        libc::timespec {
                            tv_sec: 0,
                            tv_nsec: 0,
                        },
                        true,
                    )
                }
            }
            None => {
                if has_dirty_windows {
                    (
                        libc::timespec {
                            tv_sec: 0,
                            tv_nsec: 0,
                        },
                        true,
                    )
                } else {
                    (
                        libc::timespec {
                            tv_sec: 0,
                            tv_nsec: 0,
                        },
                        false,
                    )
                }
            }
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.init()?;

        // Optimize Linux process timer slack to 50 microseconds (50,000 ns) so that
        // kernel wakeups and sub-millisecond ppoll deadlines aren't coalesced into coarse
        // 1-2ms buckets.
        #[cfg(target_os = "linux")]
        unsafe {
            let _ = libc::prctl(libc::PR_SET_TIMERSLACK, 50_000usize, 0, 0, 0);
        }

        let mut last_idle_prune = std::time::Instant::now();
        let mut poll_fds: smallvec::SmallVec<[libc::pollfd; 8]> = smallvec::SmallVec::new();

        while !self.windows.is_empty() || self.daemon_mode {
            let mut closed_windows = Vec::new();
            let mut had_keyboard_input = false;

            for (&window_id, ws) in &mut self.windows {
                let mut pending_resize: Option<(u32, u32)> = None;
                while let Some(event) = ws.window.poll_event() {
                    match event {
                        PlatformEvent::CloseRequested => {
                            closed_windows.push(window_id);
                            break;
                        }
                        PlatformEvent::ModifiersChanged(mods) => {
                            let old_mods = self.modifiers;
                            self.modifiers = mods;
                            if old_mods.control_key() != mods.control_key()
                                || old_mods.shift_key() != mods.shift_key()
                            {
                                ws.last_mouse_cell = (usize::MAX, usize::MAX);
                                ws.handle_cursor_moved((ws.mouse_x, ws.mouse_y), self.modifiers);
                            }
                        }
                        PlatformEvent::Focused(focused) => {
                            ws.is_focused = focused;
                            if !focused {
                                ws.unhide_mouse();
                                self.modifiers = ModifiersState::empty();
                                ws.is_mouse_down = false;
                                ws.mouse_down_pane_id = None;
                                ws.dragging_separator = None;
                                ws.dragging_tab = None;
                                ws.cursor_blink_on = true;
                                ws.release_memory();
                            } else {
                                crate::window::csd::CsdPalette::invalidate_system_cache();
                                ws.update_csd_theme_if_changed();
                                ws.mark_interaction();
                            }
                            let active_pane = ws.active_pane();
                            if active_pane.terminal.focus_tracking {
                                let seq = if focused { b"\x1b[I" } else { b"\x1b[O" };
                                ws.write_to_active_pane(seq.to_vec());
                            }
                            ws.content_dirty = true;
                            ws.tab_bar_dirty = true;
                            ws.needs_redraw = true;
                        }
                        PlatformEvent::Resized { width, height } => {
                            crate::diagnostics::ResizeStats::record_pixel_resize();
                            if width > 0 && height > 0 {
                                ws.request_resize(width, height);
                                pending_resize = Some((width, height));
                            }
                        }
                        PlatformEvent::KeyPressed {
                            key,
                            text,
                            modifiers,
                        } => {
                            self.modifiers = modifiers;
                            if ws.handle_keyboard_input(&key, text.as_deref(), modifiers) {
                                had_keyboard_input = true;
                            }
                        }
                        PlatformEvent::KeyReleased { modifiers, .. } => {
                            self.modifiers = modifiers;
                        }
                        PlatformEvent::CursorMoved { x, y } => {
                            ws.handle_cursor_moved((x, y), self.modifiers);
                        }
                        PlatformEvent::MouseWheel { delta_y } => {
                            ws.handle_mouse_wheel(delta_y, self.modifiers);
                        }
                        PlatformEvent::MouseInput { state, button } => {
                            ws.handle_mouse_input(state, button, self.modifiers);
                        }
                        PlatformEvent::RedrawRequested => {
                            ws.renderer.damage.mark_all();
                            ws.needs_redraw = true;
                        }
                    }
                }

                if pending_resize.is_some() {
                    ws.apply_pending_resize();
                }
            }

            for wid in closed_windows {
                if let Some(ws) = self.windows.remove(&wid) {
                    for tab in &ws.tabs {
                        for pane in tab.tree.panes() {
                            let _ = ws.pty_reactor.unregister(wid, tab.id, pane.id);
                        }
                    }
                }
                crate::memory::trim_allocator_memory();
            }

            if self.windows.is_empty() && !self.daemon_mode {
                break;
            }

            let _had_pty_data = self.drain_custom_events();

            let now = std::time::Instant::now();
            let is_any_resizing = self.windows.values().any(|ws| {
                now.duration_since(ws.last_resize_instant) < std::time::Duration::from_millis(150)
            });

            if had_keyboard_input && !is_any_resizing {
                // If keyboard input was just dispatched to PTY, give the shell up to 2.5ms to return its echo.
                // Typical shell latency is 50-300µs; as soon as the echo arrives, recv_timeout returns
                // immediately and renders in the exact same frame with zero delay!
                if let Ok(event) = self
                    .event_receiver
                    .recv_timeout(std::time::Duration::from_micros(2500))
                {
                    self.drain_custom_events_with_first(Some(event));
                }
            }

            if self.windows.is_empty() && !self.daemon_mode {
                break;
            }

            // Re-sample wall-clock after draining shell echo so all subsequent frame pacing,
            // typing checks, and poll deadline calculations are strictly accurate without timing skew.
            let now = std::time::Instant::now();

            let mut min_next_wake: Option<std::time::Instant> = None;

            for ws in self.windows.values_mut() {
                if ws.window.csd_title_bar_height() > 0 {
                    ws.update_csd_theme_if_changed();
                    let theme_wake = now + std::time::Duration::from_millis(500);
                    min_next_wake = Some(min_next_wake.map_or(theme_wake, |t| t.min(theme_wake)));
                }

                let mut should_prune = false;
                for tab in &mut ws.tabs {
                    if now.duration_since(tab.last_activity) >= std::time::Duration::from_secs(60)
                        && tab.last_activity >= tab.last_cleanup
                    {
                        tab.last_cleanup = now;
                        should_prune = true;
                    }
                }
                if should_prune {
                    ws.prune_idle_fallbacks(std::time::Duration::from_secs(60));
                }

                let cursor_blink_active = ws.is_cursor_blink_active();
                let is_typing_blink =
                    now.duration_since(ws.last_key_instant) < std::time::Duration::from_millis(500);

                if is_typing_blink {
                    // While actively typing, freeze cursor blink to solid ON so the cursor
                    // never blinks off or flickers during typing bursts.
                    if !ws.cursor_blink_on {
                        ws.cursor_blink_on = true;
                        ws.needs_redraw = true;
                    }
                    ws.last_cursor_blink = now;
                } else if cursor_blink_active
                    && now.duration_since(ws.last_cursor_blink)
                        >= std::time::Duration::from_millis(500)
                {
                    ws.cursor_blink_on = !ws.cursor_blink_on;
                    ws.last_cursor_blink = now;
                    ws.needs_redraw = true;
                }

                if cursor_blink_active && !is_typing_blink {
                    let next_blink = ws.last_cursor_blink + std::time::Duration::from_millis(500);
                    min_next_wake = Some(min_next_wake.map_or(next_blink, |t| t.min(next_blink)));
                }

                if let Some(repeat_wake) = ws.window.next_wake_time() {
                    min_next_wake = Some(min_next_wake.map_or(repeat_wake, |t| t.min(repeat_wake)));
                }

                if ws.needs_redraw {
                    if let Some(active_tab) = ws.tabs.get_mut(ws.active_tab_index)
                        && active_tab
                            .active_pane_mut()
                            .terminal
                            .is_synchronized_output_active()
                    {
                        let sync_deadline = now + std::time::Duration::from_millis(8);
                        min_next_wake =
                            Some(min_next_wake.map_or(sync_deadline, |t| t.min(sync_deadline)));
                        continue;
                    }

                    let frame_duration = ws
                        .fps_limit
                        .filter(|&l| l > 0)
                        .map(|l| std::time::Duration::from_secs_f64(1.0 / l as f64))
                        .unwrap_or(std::time::Duration::from_millis(8));

                    // Zero-latency rendering for keyboard typing and active window resizing:
                    // bypass frame-rate throttling and redraw immediately!
                    let is_typing = now.duration_since(ws.last_key_instant)
                        < std::time::Duration::from_millis(150);
                    let is_resizing = now.duration_since(ws.last_resize_instant)
                        < std::time::Duration::from_millis(150);

                    let next_frame = ws.last_frame_instant + frame_duration;

                    if is_typing || is_resizing || now >= next_frame {
                        ws.draw();
                        ws.needs_redraw = false;
                        if is_typing || is_resizing {
                            ws.last_frame_instant = now;
                        } else {
                            ws.last_frame_instant += frame_duration;
                            // Clamp to prevent burst frames if render loop fell behind
                            if now.duration_since(ws.last_frame_instant) > frame_duration * 2 {
                                ws.last_frame_instant = now;
                            }
                        }
                    } else {
                        min_next_wake =
                            Some(min_next_wake.map_or(next_frame, |t| t.min(next_frame)));
                    }
                }
            }

            // Periodic idle prune of fallback fonts unused for >= 10 seconds
            if now.duration_since(last_idle_prune) >= std::time::Duration::from_secs(10) {
                last_idle_prune = now;
                let mut total_evicted = 0;
                for ws in self.windows.values_mut() {
                    total_evicted += ws.prune_idle_fallbacks(std::time::Duration::from_secs(10));
                }
                if total_evicted > 0 {
                    crate::memory::trim_allocator_memory();
                }
            }

            let (timeout_ts, has_timeout) = Self::compute_ppoll_timeout(
                min_next_wake,
                self.windows.values().any(|w| w.needs_redraw),
            );

            // If new custom events arrived while drawing or updating, drain immediately without sleeping
            if let Ok(event) = self.event_receiver.try_recv() {
                self.drain_custom_events_with_first(Some(event));
                continue;
            }

            // Flush all window connections (X11 / Wayland) before polling so that no rendering
            // commands or property updates remain in userspace client buffers.
            for ws in self.windows.values() {
                ws.window.flush();
            }

            let raw_wake_fd = self.wake_fd.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1);

            // Drain any pending wake_fd signal before sleeping to prevent immediate spurious wakeup
            if raw_wake_fd >= 0 {
                let mut wake_buf = [0u8; 8];
                loop {
                    let r = unsafe {
                        libc::read(raw_wake_fd, wake_buf.as_mut_ptr() as *mut libc::c_void, 8)
                    };
                    if r >= 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR)
                    {
                        break;
                    }
                }
            }
            self.event_loop_proxy
                .wake_pending
                .store(false, std::sync::atomic::Ordering::Release);

            // Double check if an event arrived concurrently with draining wake_fd
            if let Ok(event) = self.event_receiver.try_recv() {
                self.drain_custom_events_with_first(Some(event));
                continue;
            }

            poll_fds.clear();
            if raw_wake_fd >= 0 {
                poll_fds.push(libc::pollfd {
                    fd: raw_wake_fd,
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
            for ws in self.windows.values() {
                poll_fds.push(libc::pollfd {
                    fd: ws.window.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                });
            }

            let poll_ret = unsafe {
                libc::ppoll(
                    poll_fds.as_mut_ptr(),
                    poll_fds.len() as libc::nfds_t,
                    if has_timeout {
                        &timeout_ts as *const libc::timespec
                    } else {
                        std::ptr::null()
                    },
                    std::ptr::null(),
                )
            };

            if poll_ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                log::error!("Main event loop libc::ppoll error: {}", err);
                continue;
            }

            if !poll_fds.is_empty() && raw_wake_fd >= 0 && (poll_fds[0].revents & libc::POLLIN) != 0
            {
                let mut buf = [0u8; 8];
                loop {
                    let r = unsafe {
                        libc::read(raw_wake_fd, buf.as_mut_ptr() as *mut libc::c_void, 8)
                    };
                    if r >= 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR)
                    {
                        break;
                    }
                }
                self.event_loop_proxy
                    .wake_pending
                    .store(false, std::sync::atomic::Ordering::Release);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_cursor_visibility() {
        // Active pane, at screen bottom (offset 0), cursor visible, blinking disabled -> true
        assert!(WindowState::compute_cursor_visibility(
            true, 0, true, false, false
        ));
        assert!(WindowState::compute_cursor_visibility(
            true, 0, true, false, true
        ));

        // Active pane, at screen bottom, cursor visible, blinking enabled: follows blink phase
        assert!(WindowState::compute_cursor_visibility(
            true, 0, true, true, true
        ));
        assert!(!WindowState::compute_cursor_visibility(
            true, 0, true, true, false
        ));

        // Explicit DECTCEM hidden: blinking must NEVER override explicit hidden state
        assert!(!WindowState::compute_cursor_visibility(
            true, 0, false, false, true
        ));
        assert!(!WindowState::compute_cursor_visibility(
            true, 0, false, false, false
        ));
        assert!(!WindowState::compute_cursor_visibility(
            true, 0, false, true, true
        ));
        assert!(!WindowState::compute_cursor_visibility(
            true, 0, false, true, false
        ));

        // Inactive pane: always hidden
        assert!(!WindowState::compute_cursor_visibility(
            false, 0, true, false, true
        ));
        assert!(!WindowState::compute_cursor_visibility(
            false, 0, true, true, true
        ));

        // Scrolled back into history (offset > 0): always hidden
        assert!(!WindowState::compute_cursor_visibility(
            true, 5, true, false, true
        ));
        assert!(!WindowState::compute_cursor_visibility(
            true, 1, true, true, true
        ));
    }

    #[test]
    fn test_pty_exit_without_hold_closes_window_cleanly() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let mut app = App {
            event_loop_proxy: proxy,
            event_receiver: receiver,
            wake_fd: None,
            modifiers: ModifiersState::empty(),
            windows: HashMap::new(),
            next_window_id: 2,
            daemon_mode: false,
            single_instance_mode: false,
            ipc_listener: None,
            initial_options: None,
            pty_reactor: None,
            config: Config::default(),
            config_diagnostics: Vec::new(),
        };

        let ws = WindowState::create_test_window_state(Arc::new(PtyMaster::dummy()), 80, 24);
        let tab_id = ws.tabs[0].id;
        let pane_id = ws.tabs[0].active_pane_id;
        app.windows.insert(ws.window_id, ws);

        assert_eq!(app.windows.len(), 1);

        // When a shell exits (e.g. user typed 'exit'), PtyExit arrives
        app.handle_custom_event(CustomEvent::PtyExit {
            window_id: 1,
            tab_id,
            pane_id,
        });

        // The window should close cleanly without being held or falsely labeled as failed to start
        assert!(
            app.windows.is_empty(),
            "Window must close upon child process exit when hold is false"
        );
    }

    #[test]
    fn test_pty_exit_with_hold_retains_window_and_updates_title() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let mut app = App {
            event_loop_proxy: proxy,
            event_receiver: receiver,
            wake_fd: None,
            modifiers: ModifiersState::empty(),
            windows: HashMap::new(),
            next_window_id: 2,
            daemon_mode: false,
            single_instance_mode: false,
            ipc_listener: None,
            initial_options: None,
            pty_reactor: None,
            config: Config::default(),
            config_diagnostics: Vec::new(),
        };

        let mut ws = WindowState::create_test_window_state(Arc::new(PtyMaster::dummy()), 80, 24);
        ws.tabs[0].active_pane_mut().hold = true;
        let tab_id = ws.tabs[0].id;
        let pane_id = ws.tabs[0].active_pane_id;
        app.windows.insert(ws.window_id, ws);

        assert_eq!(app.windows.len(), 1);

        app.handle_custom_event(CustomEvent::PtyExit {
            window_id: 1,
            tab_id,
            pane_id,
        });

        // Window remains open and title updates to "[Process exited]"
        assert_eq!(app.windows.len(), 1);
        let ws = app.windows.get(&1).unwrap();
        assert_eq!(ws.tabs[0].current_title, "[Process exited]");
    }

    #[test]
    fn test_rapid_window_open_and_immediate_exit() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let mut app = App {
            event_loop_proxy: proxy,
            event_receiver: receiver,
            wake_fd: None,
            modifiers: ModifiersState::empty(),
            windows: HashMap::new(),
            next_window_id: 2,
            daemon_mode: false,
            single_instance_mode: false,
            ipc_listener: None,
            initial_options: None,
            pty_reactor: None,
            config: Config::default(),
            config_diagnostics: Vec::new(),
        };

        // Rapidly create and exit 5 windows in succession
        for wid in 1..=5 {
            let mut ws =
                WindowState::create_test_window_state(Arc::new(PtyMaster::dummy()), 80, 24);
            ws.window_id = wid;
            let tab_id = ws.tabs[0].id;
            let pane_id = ws.tabs[0].active_pane_id;
            app.windows.insert(wid, ws);

            assert_eq!(app.windows.len(), 1);

            // Immediate exit simulation
            app.handle_custom_event(CustomEvent::PtyExit {
                window_id: wid,
                tab_id,
                pane_id,
            });

            assert!(
                app.windows.is_empty(),
                "Rapid exit cycle {} must close window cleanly without hang",
                wid
            );
        }
    }

    #[test]
    fn test_compute_ppoll_timeout() {
        let now = std::time::Instant::now();

        // 1. Deadline in the past -> timeout is 0, has_timeout is true
        let past = now
            .checked_sub(std::time::Duration::from_millis(10))
            .unwrap();
        let (ts, has_timeout) = App::compute_ppoll_timeout(Some(past), false);
        assert!(has_timeout);
        assert_eq!(ts.tv_sec, 0);
        assert_eq!(ts.tv_nsec, 0);

        // 2. Deadline < 250µs away -> timeout is 0 (poll without sleeping to avoid overshoot)
        let sub_250us = std::time::Instant::now() + std::time::Duration::from_micros(100);
        let (ts, has_timeout) = App::compute_ppoll_timeout(Some(sub_250us), false);
        assert!(has_timeout);
        assert_eq!(ts.tv_sec, 0);
        assert_eq!(ts.tv_nsec, 0);

        // 3. Deadline 50ms away -> exact conversion to seconds and nanoseconds
        let future_50ms = std::time::Instant::now() + std::time::Duration::from_millis(50);
        let (ts, has_timeout) = App::compute_ppoll_timeout(Some(future_50ms), false);
        assert!(has_timeout);
        assert_eq!(ts.tv_sec, 0);
        assert!(ts.tv_nsec > 40_000_000 && ts.tv_nsec <= 50_000_000);

        // 4. No deadline, but windows need redraw -> timeout is 0 (immediate yield/drain)
        let (ts, has_timeout) = App::compute_ppoll_timeout(None, true);
        assert!(has_timeout);
        assert_eq!(ts.tv_sec, 0);
        assert_eq!(ts.tv_nsec, 0);

        // 5. No deadline, no redraw -> indefinite poll (null timespec)
        let (_, has_timeout) = App::compute_ppoll_timeout(None, false);
        assert!(!has_timeout);
    }

    #[test]
    fn test_cursor_blink_typing_suppression() {
        let mut ws = WindowState::create_test_window_state(Arc::new(PtyMaster::dummy()), 80, 24);
        ws.cursor_blink_enabled = true;
        ws.is_focused = true;
        ws.cursor_blink_on = false;

        let now = std::time::Instant::now();
        // User typed 100ms ago (<500ms threshold)
        ws.last_key_instant = now - std::time::Duration::from_millis(100);

        let is_typing_blink =
            now.duration_since(ws.last_key_instant) < std::time::Duration::from_millis(500);
        assert!(is_typing_blink);

        // While typing is active, cursor is forced to solid ON
        if is_typing_blink && !ws.cursor_blink_on {
            ws.cursor_blink_on = true;
            ws.needs_redraw = true;
        }
        assert!(ws.cursor_blink_on);
        assert!(ws.needs_redraw);

        // When typing has ceased (> 500ms ago), blinking resumes
        ws.last_key_instant = now - std::time::Duration::from_millis(600);
        let is_typing_blink_after =
            now.duration_since(ws.last_key_instant) < std::time::Duration::from_millis(500);
        assert!(!is_typing_blink_after);
    }

    #[test]
    fn test_phase_locked_frame_pacing() {
        let frame_duration = std::time::Duration::from_micros(8333); // ~120 FPS
        let mut last_frame_instant = std::time::Instant::now();
        let now = last_frame_instant + frame_duration;

        // Steady state pacing: advance anchor by frame_duration
        last_frame_instant += frame_duration;
        assert_eq!(last_frame_instant, now);

        // If window stalled for 3 frames, anchor clamps to prevent burst frames
        let stalled_now = last_frame_instant + frame_duration * 3;
        if stalled_now.duration_since(last_frame_instant) > frame_duration * 2 {
            last_frame_instant = stalled_now;
        }
        assert_eq!(last_frame_instant, stalled_now);
    }

    #[test]
    fn test_update_csd_theme_if_changed() {
        let mut ws = WindowState::create_test_window_state(Arc::new(PtyMaster::dummy()), 80, 24);
        ws.renderer.prev_csd_hash = 12345;
        ws.renderer.damage.clear();
        ws.needs_redraw = false;

        // Force a palette change by altering current palette
        ws.csd_palette = crate::window::csd::CsdPalette::light();
        // Set env to dark
        unsafe { std::env::set_var("VELOX_CSD_THEME", "dark") };
        crate::window::csd::CsdPalette::invalidate_system_cache();

        let changed = ws.update_csd_theme_if_changed();
        assert!(changed);
        assert_eq!(ws.csd_palette, crate::window::csd::CsdPalette::dark());
        assert_eq!(ws.renderer.prev_csd_hash, 0);
        assert!(ws.renderer.damage.full_redraw);
        assert!(ws.needs_redraw);

        // When nothing changes, returns false and doesn't re-mark damage
        ws.renderer.damage.clear();
        ws.needs_redraw = false;
        let changed_again = ws.update_csd_theme_if_changed();
        assert!(!changed_again);
        assert!(!ws.needs_redraw);

        // Clean up env
        unsafe { std::env::remove_var("VELOX_CSD_THEME") };
        crate::window::csd::CsdPalette::invalidate_system_cache();
    }
}
