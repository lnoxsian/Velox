use crate::pty::master::PtyMaster;
use crate::terminal::terminal::Terminal;
use std::sync::Arc;
use std::time::Instant;

pub type PaneId = u64;

pub struct Pane {
    pub id: PaneId,
    pub pty_master: Arc<PtyMaster>,
    pub terminal: Terminal,
    pub font_size: f32,
    pub custom_title: Option<String>,
    pub current_title: String,
    pub last_title_check: Instant,
    pub last_activity: Instant,
    pub last_cleanup: Instant,
    pub hold: bool,
}

impl Pane {
    pub fn new(
        id: PaneId,
        pty_master: Arc<PtyMaster>,
        terminal: Terminal,
        font_size: f32,
        hold: bool,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            pty_master,
            terminal,
            font_size,
            custom_title: None,
            current_title: "velox".to_string(),
            last_title_check: now,
            last_activity: now,
            last_cleanup: now,
            hold,
        }
    }

    pub fn with_title(
        id: PaneId,
        pty_master: Arc<PtyMaster>,
        terminal: Terminal,
        custom_title: Option<String>,
        initial_title: String,
        font_size: f32,
        hold: bool,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            pty_master,
            terminal,
            font_size,
            custom_title,
            current_title: initial_title,
            last_title_check: now,
            last_activity: now,
            last_cleanup: now,
            hold,
        }
    }

    pub fn dummy() -> Self {
        let now = Instant::now();
        Self {
            id: 0,
            pty_master: Arc::new(PtyMaster::dummy()),
            terminal: Terminal::new(1, 1),
            font_size: 14.0,
            custom_title: None,
            current_title: String::new(),
            last_title_check: now,
            last_activity: now,
            last_cleanup: now,
            hold: false,
        }
    }

    /// Refresh foreground process / OSC title for this pane.
    /// Returns `true` if the title actually changed.
    pub fn update_title(&mut self) -> bool {
        if self.last_title_check.elapsed() < std::time::Duration::from_millis(500) {
            return false;
        }
        self.last_title_check = Instant::now();

        // 1. Custom title: compare in-place
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
