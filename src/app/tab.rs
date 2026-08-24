use crate::config::config::{Config, TabBarVisibility};
use crate::pty::master::PtyMaster;
use crate::terminal::terminal::Terminal;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBarHitResult {
    None,
    Tab(usize),
    CloseTab(usize),
    NewTab,
    EmptyArea,
}

#[derive(Debug, Clone)]
pub struct TabHeaderInfo {
    pub title: String,
    pub is_active: bool,
    pub is_hovered: bool,
    pub is_close_hovered: bool,
}

#[derive(Debug, Clone)]
pub struct TabBarRenderInfo {
    pub height: f32,
    pub tabs: Vec<TabHeaderInfo>,
    pub show_new_tab: bool,
    pub is_new_tab_hovered: bool,
    pub show_close_button: bool,
}

impl TabBarRenderInfo {
    /// Single authoritative tab-width formula shared by both renderers and hit testing.
    #[inline]
    pub fn compute_tab_width(&self, viewport_width: f32) -> f32 {
        tab_width_formula(viewport_width, self.tabs.len(), self.show_new_tab)
    }
}

/// Shared tab-width formula — the single source of truth for tab geometry.
/// Tabs span the available horizontal space equally across the window (Terminator style).
#[inline]
fn tab_width_formula(viewport_width: f32, tab_count: usize, show_new_tab: bool) -> f32 {
    if tab_count == 0 {
        return viewport_width.max(40.0);
    }
    let new_tab_btn_w = if show_new_tab { 32.0 } else { 0.0 };
    let avail_w = (viewport_width - new_tab_btn_w).max(40.0);
    (avail_w / tab_count as f32).max(40.0)
}

pub struct Tab {
    pub id: u64,
    pub pty_master: Arc<PtyMaster>,
    pub terminal: Terminal,
    pub custom_title: Option<String>,
    pub current_title: String,
    pub last_title_check: Instant,
    pub last_activity: Instant,
    pub last_cleanup: Instant,
    pub hold: bool,
    pub font_size: f32,
}

impl Tab {
    pub fn new(
        id: u64,
        pty_master: Arc<PtyMaster>,
        terminal: Terminal,
        custom_title: Option<String>,
        initial_title: String,
        hold: bool,
        font_size: f32,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            pty_master,
            terminal,
            custom_title,
            current_title: initial_title,
            last_title_check: now,
            last_activity: now,
            last_cleanup: now,
            hold,
            font_size,
        }
    }

    /// Refresh foreground process / OSC title for this tab.
    /// Returns `true` if the title actually changed.
    pub fn update_title(&mut self) -> bool {
        if self.last_title_check.elapsed() < std::time::Duration::from_millis(500) {
            return false;
        }
        self.last_title_check = Instant::now();

        // 1. Custom title: no process polling needed — compare in-place.
        if let Some(ref t) = self.custom_title {
            if self.current_title == *t {
                return false;
            }
            self.current_title = t.clone();
            return true;
        }

        // 2. OSC title explicitly set by shell/program (OSC 0 / OSC 2)
        if let Some(ref osc) = self.terminal.osc_title {
            if self.current_title == *osc {
                return false;
            }
            self.current_title = osc.clone();
            return true;
        }

        // 3. Foreground process name polling
        if let Some(program) = self.pty_master.get_foreground_process_name() {
            let new_title = match &self.terminal.app_title {
                Some(tpl) => tpl.replace("{program}", &program),
                None => program,
            };

            if self.current_title != new_title {
                self.current_title = new_title;
                return true;
            }
        }

        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabBar {
    pub show_tab_bar: TabBarVisibility,
    pub configured_height: Option<f32>,
    pub show_close_button: bool,
    pub show_new_tab_button: bool,
    pub hovered_tab: Option<usize>,
    pub hovered_close: Option<usize>,
    pub hovered_new_tab: bool,
}

impl Default for TabBar {
    fn default() -> Self {
        Self {
            show_tab_bar: TabBarVisibility::Auto,
            configured_height: None,
            show_close_button: true,
            show_new_tab_button: false,
            hovered_tab: None,
            hovered_close: None,
            hovered_new_tab: false,
        }
    }
}

impl TabBar {
    pub fn from_config(config: &Config) -> Self {
        Self {
            show_tab_bar: config.show_tab_bar(),
            configured_height: config.tab_bar_height(),
            show_close_button: config.show_close_button(),
            show_new_tab_button: config.show_new_tab_button(),
            hovered_tab: None,
            hovered_close: None,
            hovered_new_tab: false,
        }
    }

    #[inline]
    pub fn is_visible(&self, tab_count: usize) -> bool {
        match self.show_tab_bar {
            TabBarVisibility::Never => false,
            TabBarVisibility::Always => true,
            TabBarVisibility::Auto => tab_count > 1,
        }
    }

    #[inline]
    pub fn height(&self, cell_height: u32) -> f32 {
        self.configured_height
            .unwrap_or_else(|| ((cell_height as f32) + 10.0).max(28.0))
    }

    /// Calculate tab width for a given window width and tab count.
    /// Delegates to the shared `tab_width_formula` so hit testing and rendering always agree.
    pub fn tab_width(&self, win_width: f32, tab_count: usize) -> f32 {
        tab_width_formula(win_width, tab_count, self.show_new_tab_button)
    }

    /// Hit-test the tab bar geometry
    pub fn hit_test(
        &self,
        x: f32,
        y: f32,
        win_width: f32,
        cell_height: u32,
        tab_count: usize,
    ) -> TabBarHitResult {
        if !self.is_visible(tab_count) {
            return TabBarHitResult::None;
        }

        let bar_h = self.height(cell_height);
        if y < 0.0 || y > bar_h || x < 0.0 || x > win_width {
            return TabBarHitResult::None;
        }

        let tab_w = self.tab_width(win_width, tab_count);

        for i in 0..tab_count {
            let tab_x = (i as f32) * tab_w;
            let tab_x_end = if i == tab_count - 1 && !self.show_new_tab_button {
                win_width
            } else {
                tab_x + tab_w
            };

            if x >= tab_x && (x < tab_x_end || (i == tab_count - 1 && x <= tab_x_end)) {
                // Check if close button is clicked
                if self.show_close_button {
                    let close_w = 20.0;
                    let close_x = tab_x_end - close_w - 4.0;
                    let close_x_end = tab_x_end - 4.0;
                    let close_y = (bar_h - close_w) / 2.0;
                    let close_y_end = close_y + close_w;

                    if x >= close_x && x <= close_x_end && y >= close_y && y <= close_y_end {
                        return TabBarHitResult::CloseTab(i);
                    }
                }
                return TabBarHitResult::Tab(i);
            }
        }

        if self.show_new_tab_button {
            let btn_x = (tab_count as f32) * tab_w + 4.0;
            let btn_w = 24.0;
            let btn_y = (bar_h - 24.0) / 2.0;
            if x >= btn_x && x <= (btn_x + btn_w) && y >= btn_y && y <= (btn_y + 24.0) {
                return TabBarHitResult::NewTab;
            }
        }

        TabBarHitResult::EmptyArea
    }
}
