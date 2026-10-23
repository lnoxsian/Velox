use smallvec::SmallVec;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CsdButtonType {
    Minimize,
    Maximize,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsdButtonLayout {
    pub left: Vec<CsdButtonType>,
    pub right: Vec<CsdButtonType>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CsdButtonRect {
    pub button_type: CsdButtonType,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub center_x: f32,
    pub center_y: f32,
}

impl CsdButtonRect {
    #[inline]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

impl Default for CsdButtonLayout {
    fn default() -> Self {
        Self {
            left: Vec::new(),
            right: vec![
                CsdButtonType::Minimize,
                CsdButtonType::Maximize,
                CsdButtonType::Close,
            ],
        }
    }
}

impl CsdButtonLayout {
    /// Parse GTK `button-layout` string format (e.g. `":minimize,maximize,close"` or `"close,minimize,maximize:"`).
    pub fn parse(s: &str) -> Self {
        let cleaned = s.trim().trim_matches('\'').trim_matches('"');
        let parts: Vec<&str> = cleaned.split(':').collect();
        if parts.len() != 2 {
            return Self::default();
        }

        let parse_side = |side: &str| -> Vec<CsdButtonType> {
            side.split(',')
                .filter_map(|token| match token.trim() {
                    "close" => Some(CsdButtonType::Close),
                    "maximize" => Some(CsdButtonType::Maximize),
                    "minimize" => Some(CsdButtonType::Minimize),
                    _ => None,
                })
                .collect()
        };

        let left = parse_side(parts[0]);
        let right = parse_side(parts[1]);

        if left.is_empty() && right.is_empty() {
            Self::default()
        } else {
            Self { left, right }
        }
    }

    /// Query the system desktop settings for GTK button layout.
    /// Checks `VELOX_BUTTON_LAYOUT` env var, then queries desktop gsettings
    /// (Cinnamon first if under Cinnamon desktop, otherwise GNOME).
    pub fn detect_system() -> Self {
        if let Ok(val) = std::env::var("VELOX_BUTTON_LAYOUT") {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Self::parse(trimmed);
            }
        }

        let is_cinnamon = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_lowercase().contains("cinnamon"))
            .unwrap_or(false);

        let query_gnome = || {
            Command::new("gsettings")
                .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout);
                        if s.contains(':') {
                            return Some(Self::parse(&s));
                        }
                    }
                    None
                })
        };

        let query_cinnamon = || {
            Command::new("gsettings")
                .args([
                    "get",
                    "org.cinnamon.desktop.wm.preferences",
                    "button-layout",
                ])
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout);
                        if s.contains(':') {
                            return Some(Self::parse(&s));
                        }
                    }
                    None
                })
        };

        if is_cinnamon {
            if let Some(layout) = query_cinnamon() {
                return layout;
            }
            if let Some(layout) = query_gnome() {
                return layout;
            }
        } else {
            if let Some(layout) = query_gnome() {
                return layout;
            }
            if let Some(layout) = query_cinnamon() {
                return layout;
            }
        }

        Self::default()
    }

#[inline(always)]
fn left_button_slot(i: usize) -> (f32, f32, f32) {
    let slot = i as f32;
    let center_x = 22.0 + slot * 32.0;
    let (hit_x, hit_w) = if i == 0 {
        (0.0, 38.0)
    } else {
        (6.0 + slot * 32.0, 32.0)
    };
    (center_x, hit_x, hit_w)
}

#[inline(always)]
fn right_button_slot(i: usize, n: usize, win_w: f32) -> (f32, f32, f32) {
    let slot = (n - 1 - i) as f32;
    let center_x = win_w - 22.0 - slot * 32.0;
    let (hit_x, hit_w) = if i == n - 1 {
        (win_w - 38.0, 38.0)
    } else {
        (win_w - 38.0 - slot * 32.0, 32.0)
    };
    (center_x, hit_x, hit_w)
}

    /// Computes precise visual and hitbox bounding geometry for every button in the layout.
    #[inline]
    pub fn compute_button_rects(
        &self,
        viewport_width: u32,
        csd_height: u32,
    ) -> SmallVec<[CsdButtonRect; 6]> {
        let mut rects = SmallVec::with_capacity(self.left.len() + self.right.len());
        let win_w = viewport_width as f32;
        let win_h = csd_height as f32;
        let center_y = win_h / 2.0;

        // Left buttons
        for (i, &btn_type) in self.left.iter().enumerate() {
            let (center_x, hit_x, hit_w) = Self::left_button_slot(i);
            rects.push(CsdButtonRect {
                button_type: btn_type,
                x: hit_x,
                y: 0.0,
                width: hit_w,
                height: win_h,
                center_x,
                center_y,
            });
        }

        // Right buttons
        let n = self.right.len();
        for (i, &btn_type) in self.right.iter().enumerate() {
            let (center_x, hit_x, hit_w) = Self::right_button_slot(i, n, win_w);
            rects.push(CsdButtonRect {
                button_type: btn_type,
                x: hit_x,
                y: 0.0,
                width: hit_w,
                height: win_h,
                center_x,
                center_y,
            });
        }

        rects
    }

    /// Fast zero-allocation hit-test of a coordinate `(px, py)` against the CSD buttons.
    #[inline]
    pub fn hit_test(
        &self,
        px: f32,
        py: f32,
        viewport_width: u32,
        csd_height: u32,
    ) -> Option<CsdButtonType> {
        if py < 0.0 || py >= csd_height as f32 || px < 0.0 || px >= viewport_width as f32 {
            return None;
        }

        // Left buttons
        for (i, &btn_type) in self.left.iter().enumerate() {
            let (_, hit_x, hit_w) = Self::left_button_slot(i);
            if px >= hit_x && px < hit_x + hit_w {
                return Some(btn_type);
            }
        }

        // Right buttons
        let win_w = viewport_width as f32;
        let n = self.right.len();
        for (i, &btn_type) in self.right.iter().enumerate() {
            let (_, hit_x, hit_w) = Self::right_button_slot(i, n, win_w);
            if px >= hit_x && px < hit_x + hit_w {
                return Some(btn_type);
            }
        }

        None
    }

    /// Returns safe `(min_x, max_x)` bounds for centering title text without overlapping buttons.
    #[inline]
    pub fn safe_title_bounds(&self, viewport_width: u32, _csd_height: u32) -> (f32, f32) {
        let win_w = viewport_width as f32;
        let left_w = if self.left.is_empty() {
            16.0
        } else {
            22.0 + (self.left.len() as f32) * 32.0 + 8.0
        };
        let right_w = if self.right.is_empty() {
            win_w - 16.0
        } else {
            win_w - (22.0 + (self.right.len() as f32) * 32.0 + 8.0)
        };
        (left_w, right_w.max(left_w))
    }
}

/// Detects if `(px, py)` falls within the resize border region of the window (branch-free bitmask).
#[inline]
pub fn detect_resize_edge(
    px: f32,
    py: f32,
    width: u32,
    height: u32,
    border_px: f32,
) -> Option<crate::window::WindowResizeEdge> {
    use crate::window::WindowResizeEdge;
    let w = width as f32;
    let h = height as f32;
    if px < 0.0 || px > w || py < 0.0 || py > h {
        return None;
    }

    let mask = (px <= border_px) as u8
        | (((px >= w - border_px) as u8) << 1)
        | (((py <= border_px) as u8) << 2)
        | (((py >= h - border_px) as u8) << 3);

    match mask {
        0b0101 => Some(WindowResizeEdge::TopLeft),
        0b0110 => Some(WindowResizeEdge::TopRight),
        0b1001 => Some(WindowResizeEdge::BottomLeft),
        0b1010 => Some(WindowResizeEdge::BottomRight),
        0b0100 => Some(WindowResizeEdge::Top),
        0b1000 => Some(WindowResizeEdge::Bottom),
        0b0001 => Some(WindowResizeEdge::Left),
        0b0010 => Some(WindowResizeEdge::Right),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsdThemeMode {
    Auto,
    Light,
    Dark,
    Terminal,
}

impl CsdThemeMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            "terminal" => Self::Terminal,
            _ => Self::Auto,
        }
    }
}

/// Dynamic CSD color palette supporting GNOME/Cinnamon Light & Dark modes,
/// terminal palette blending, and custom config overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsdPalette {
    pub is_light: bool,
    pub bg: u32,
    pub fg: u32,
    pub inactive_bg: u32,
    pub inactive_fg: u32,
    pub border: u32,
    pub button_hover_bg: u32,
    pub button_pressed_bg: u32,
    pub close_hover_bg: u32,
    pub close_pressed_bg: u32,
    pub close_hover_fg: u32,
    pub close_fg: u32,
}

impl Default for CsdPalette {
    fn default() -> Self {
        Self::dark()
    }
}

impl CsdPalette {
    /// Native GNOME Adwaita Dark palette
    pub fn dark() -> Self {
        Self {
            is_light: false,
            bg: 0xFF2E2E2E,
            fg: 0xFFFFFFFF,
            inactive_bg: 0xFF202020,
            inactive_fg: 0xFF808080,
            border: 0xFF1B1B1B,
            button_hover_bg: 0x00FFFFFF,
            button_pressed_bg: 0x00FFFFFF,
            close_hover_bg: 0x00E01B24,
            close_pressed_bg: 0x00B2161E,
            close_hover_fg: 0x00FFFFFF,
            close_fg: 0xFFFFFFFF,
        }
    }

    /// Native GNOME Adwaita Light palette
    pub fn light() -> Self {
        Self {
            is_light: true,
            bg: 0xFFEBEBEB,
            fg: 0xFF2E3436,
            inactive_bg: 0xFFF6F6F6,
            inactive_fg: 0xFF8F9193,
            border: 0xFFD1D1D1,
            button_hover_bg: 0x00000000,
            button_pressed_bg: 0x00000000,
            close_hover_bg: 0x00E01B24,
            close_pressed_bg: 0x00B2161E,
            close_hover_fg: 0x00FFFFFF,
            close_fg: 0xFF2E3436,
        }
    }

    /// Palette derived from the terminal theme palette (seamless unified terminal look).
    /// Titlebar background matches the terminal background perfectly, eliminating the bottom separator gap.
    pub fn terminal(palette: &crate::renderer::color::PrecomputedPalette) -> Self {
        let r = ((palette.default_bg >> 16) & 0xFF) as f32;
        let g = ((palette.default_bg >> 8) & 0xFF) as f32;
        let b = (palette.default_bg & 0xFF) as f32;
        let lum = 0.299 * r + 0.587 * g + 0.114 * b;
        let is_light = lum > 128.0;

        let red_color = if (palette.ansi_colors[1] & 0x00FFFFFF) != 0 {
            0xFF000000 | (palette.ansi_colors[1] & 0x00FFFFFF)
        } else {
            0xFFE01B24
        };

        Self {
            is_light,
            bg: palette.default_bg,
            fg: palette.default_fg,
            inactive_bg: palette.tab_inactive_bg,
            inactive_fg: palette.tab_inactive_fg,
            border: palette.default_bg,
            button_hover_bg: palette.tab_accent,
            button_pressed_bg: palette.tab_accent,
            close_hover_bg: 0x00E01B24,
            close_pressed_bg: 0x00B2161E,
            close_hover_fg: 0x00FFFFFF,
            close_fg: red_color,
        }
    }

    /// Query the desktop settings for active GTK theme and CSS colors (cached for performance),
    /// falling back to Adwaita dark/light when custom theme CSS is unavailable.
    pub fn detect_system() -> Self {
        if let Ok(val) = std::env::var("VELOX_CSD_THEME") {
            match val.trim().to_lowercase().as_str() {
                "light" => return Self::light(),
                "dark" => return Self::dark(),
                _ => {}
            }
        }

        static CACHED: OnceLock<CsdPalette> = OnceLock::new();
        *CACHED.get_or_init(Self::detect_system_uncached)
    }

    /// Performs fresh, uncached detection of the active GTK theme and CSS colors.
    pub fn detect_system_uncached() -> Self {
        let is_dark = detect_is_dark();

        // 1. Try detecting installed GTK theme and parsing its CSS colors
        if let Some(palette) =
            detect_gtk_theme_name().and_then(|t| load_gtk_theme_palette(&t, is_dark))
        {
            return palette;
        }

        // 2. Fallback to native Adwaita Dark / Light
        if is_dark { Self::dark() } else { Self::light() }
    }

    /// Resolve effective palette based on config and system theme
    pub fn resolve(
        decorations_config: Option<&crate::config::config::WindowDecorationsConfig>,
        terminal_palette: &crate::renderer::color::PrecomputedPalette,
    ) -> Self {
        let env_mode = std::env::var("VELOX_CSD_THEME").ok();
        let mode_str = env_mode
            .as_deref()
            .or_else(|| decorations_config.and_then(|d| d.theme.as_deref()))
            .unwrap_or("auto");
        let mode = CsdThemeMode::parse(mode_str);

        let mut palette = match mode {
            CsdThemeMode::Auto => Self::detect_system(),
            CsdThemeMode::Light => Self::light(),
            CsdThemeMode::Dark => Self::dark(),
            CsdThemeMode::Terminal => Self::terminal(terminal_palette),
        };

        if let Some(cfg) = decorations_config {
            if let Some(c) = cfg
                .bg
                .as_deref()
                .and_then(crate::config::config::parse_hex_color)
            {
                let packed = 0xFF000000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
                palette.bg = packed;
                let lum = 0.299 * (c.r as f32) + 0.587 * (c.g as f32) + 0.114 * (c.b as f32);
                palette.is_light = lum > 128.0;
                palette.button_hover_bg = if palette.is_light {
                    0x00000000
                } else {
                    0x00FFFFFF
                };
                palette.button_pressed_bg = if palette.is_light {
                    0x00000000
                } else {
                    0x00FFFFFF
                };
            }
            if let Some(c) = cfg
                .fg
                .as_deref()
                .and_then(crate::config::config::parse_hex_color)
            {
                palette.fg = 0xFF000000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
            }
            if let Some(c) = cfg
                .inactive_bg
                .as_deref()
                .and_then(crate::config::config::parse_hex_color)
            {
                palette.inactive_bg =
                    0xFF000000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
            }
            if let Some(c) = cfg
                .inactive_fg
                .as_deref()
                .and_then(crate::config::config::parse_hex_color)
            {
                palette.inactive_fg =
                    0xFF000000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
            }
            if let Some(c) = cfg
                .border
                .as_deref()
                .and_then(crate::config::config::parse_hex_color)
            {
                palette.border =
                    0xFF000000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
            }
            if let Some(c) = cfg
                .close_fg
                .as_deref()
                .and_then(crate::config::config::parse_hex_color)
            {
                palette.close_fg =
                    0xFF000000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
            }
        }

        palette
    }
}

/// Detects the active desktop GTK theme name from environment, Cinnamon/GNOME gsettings,
/// or user GTK settings.ini.
pub fn detect_gtk_theme_name() -> Option<String> {
    // 1. Explicit GTK_THEME environment variable (e.g. "Adwaita:dark" or "Mint-Y-Dark-Purple")
    if let Ok(val) = std::env::var("GTK_THEME") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            let name = trimmed.split(':').next().unwrap_or(trimmed).trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    let is_cinnamon = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_lowercase().contains("cinnamon"))
        .unwrap_or(false);

    let query_gsettings = |schema: &str| -> Option<String> {
        Command::new("gsettings")
            .args(["get", schema, "gtk-theme"])
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    let trimmed = s.trim().trim_matches('\'').trim_matches('"');
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
                None
            })
    };

    if is_cinnamon {
        if let Some(theme) = query_gsettings("org.cinnamon.desktop.interface") {
            return Some(theme);
        }
        if let Some(theme) = query_gsettings("org.gnome.desktop.interface") {
            return Some(theme);
        }
    } else {
        if let Some(theme) = query_gsettings("org.gnome.desktop.interface") {
            return Some(theme);
        }
        if let Some(theme) = query_gsettings("org.cinnamon.desktop.interface") {
            return Some(theme);
        }
    }

    // Check ~/.config/gtk-4.0/settings.ini or gtk-3.0/settings.ini
    if let Ok(home) = std::env::var("HOME") {
        for sub in &["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
            let path = PathBuf::from(&home).join(".config").join(sub);
            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if let Some((_, val)) = trimmed
                        .strip_prefix("gtk-theme-name")
                        .and_then(|r| r.split_once('='))
                    {
                        let v = val.trim().trim_matches('\'').trim_matches('"');
                        if !v.is_empty() {
                            return Some(v.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

/// Detects whether the user prefers a dark color scheme.
pub fn detect_is_dark() -> bool {
    // 1. Check GTK_THEME env
    if std::env::var("GTK_THEME").is_ok_and(|val| val.to_lowercase().contains("dark")) {
        return true;
    }

    // 2. Check org.gnome.desktop.interface color-scheme
    let query_color_scheme = || {
        Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    if s.contains("prefer-dark") {
                        return Some(true);
                    } else if s.contains("prefer-light") {
                        return Some(false);
                    }
                }
                None
            })
    };

    if let Some(is_dark) = query_color_scheme() {
        return is_dark;
    }

    // 3. Check if detected theme name contains "dark"
    if detect_gtk_theme_name().is_some_and(|theme_name| theme_name.to_lowercase().contains("dark"))
    {
        return true;
    }

    // 4. Check settings.ini for gtk-application-prefer-dark-theme = 1
    if let Ok(home) = std::env::var("HOME") {
        for sub in &["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
            let path = PathBuf::from(&home).join(".config").join(sub);
            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if let Some((_, val)) = trimmed
                        .strip_prefix("gtk-application-prefer-dark-theme")
                        .and_then(|r| r.split_once('='))
                    {
                        let v = val.trim();
                        if v == "1" || v.eq_ignore_ascii_case("true") {
                            return true;
                        } else if v == "0" || v.eq_ignore_ascii_case("false") {
                            return false;
                        }
                    }
                }
            }
        }
    }

    true // default dark for terminal emulator
}

/// Discovers candidate GTK CSS files for a given theme name and dark mode preference.
pub fn find_theme_css_files(theme_name: &str, is_dark: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let home = std::env::var("HOME").ok().map(PathBuf::from);

    // 1. User overrides in ~/.config
    if let Some(ref h) = home {
        let gtk4_user = h.join(".config/gtk-4.0/gtk.css");
        if gtk4_user.exists() {
            files.push(gtk4_user);
        }
        let gtk3_user = h.join(".config/gtk-3.0/gtk.css");
        if gtk3_user.exists() {
            files.push(gtk3_user);
        }
    }

    // 2. Search root directories
    let mut base_dirs = Vec::new();
    if let Some(ref h) = home {
        base_dirs.push(h.join(".themes").join(theme_name));
        base_dirs.push(h.join(".local/share/themes").join(theme_name));
    }
    base_dirs.push(PathBuf::from("/usr/local/share/themes").join(theme_name));
    base_dirs.push(PathBuf::from("/usr/share/themes").join(theme_name));

    for base in base_dirs {
        if !base.is_dir() {
            continue;
        }

        if is_dark {
            let candidates = [
                base.join("gtk-4.0/gtk-dark.css"),
                base.join("gtk-4.0/gtk.css"),
                base.join("gtk-3.0/gtk-dark.css"),
                base.join("gtk-3.0/gtk.css"),
                base.join("libadwaita-1.7/defaults-dark.css"),
                base.join("libadwaita-1.5/defaults-dark.css"),
                base.join("libadwaita-1.7/defaults.css"),
            ];
            for c in candidates {
                if c.exists() && !files.contains(&c) {
                    files.push(c);
                }
            }
        } else {
            let candidates = [
                base.join("gtk-4.0/gtk.css"),
                base.join("gtk-3.0/gtk.css"),
                base.join("libadwaita-1.7/defaults-light.css"),
                base.join("libadwaita-1.5/defaults-light.css"),
                base.join("libadwaita-1.7/defaults.css"),
            ];
            for c in candidates {
                if c.exists() && !files.contains(&c) {
                    files.push(c);
                }
            }
        }
    }

    files
}

/// Adjusts brightness of a 32-bit ARGB color by a scaling factor.
pub fn adjust_brightness(c: u32, factor: f32) -> u32 {
    let a = (c >> 24) & 0xFF;
    let r = (((c >> 16) & 0xFF) as f32 * factor).clamp(0.0, 255.0) as u32;
    let g = (((c >> 8) & 0xFF) as f32 * factor).clamp(0.0, 255.0) as u32;
    let b = ((c & 0xFF) as f32 * factor).clamp(0.0, 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

/// Parses a CSS color value string (hex, rgb, rgba, named colors, or `@variable` reference)
/// with zero heap allocations.
pub fn parse_color_value(val: &str, defs: &HashMap<&str, &str>, depth: usize) -> Option<u32> {
    if depth > 5 {
        return None;
    }
    let trimmed = val.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return None;
    }

    // Variable reference @var_name
    if let Some(var_name) = trimmed.strip_prefix('@') {
        if let Some(target) = defs.get(var_name) {
            return parse_color_value(target, defs, depth + 1);
        }
        return None;
    }

    // Named colors
    if trimmed.eq_ignore_ascii_case("white") {
        return Some(0xFFFFFFFF);
    }
    if trimmed.eq_ignore_ascii_case("black") {
        return Some(0xFF000000);
    }
    if trimmed.eq_ignore_ascii_case("transparent") {
        return Some(0x00000000);
    }

    // Hex colors: #RGB, #RGBA, #RRGGBB, #RRGGBBAA
    if let Some(hex) = trimmed.strip_prefix('#') {
        return match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                Some(
                    0xFF000000 | ((r as u32 * 17) << 16) | ((g as u32 * 17) << 8) | (b as u32 * 17),
                )
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()?;
                Some(
                    ((a as u32 * 17) << 24)
                        | ((r as u32 * 17) << 16)
                        | ((g as u32 * 17) << 8)
                        | (b as u32 * 17),
                )
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
            }
            _ => None,
        };
    }

    // rgb(...) or rgba(...) or RGB(...)
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("rgb(") || lower.starts_with("rgba(") {
        let inner = if let Some(s) = lower.strip_prefix("rgba(") {
            s.strip_suffix(')')?
        } else {
            lower.strip_prefix("rgb(")?.strip_suffix(')')?
        };

        // Split by whitespace, comma, or slash without any string allocations
        let mut parts = inner
            .split(|c: char| c == ',' || c == '/' || c.is_ascii_whitespace())
            .filter(|s| !s.is_empty());

        if let (Some(r_str), Some(g_str), Some(b_str)) = (parts.next(), parts.next(), parts.next())
        {
            let r: f32 = r_str.parse().ok()?;
            let g: f32 = g_str.parse().ok()?;
            let b: f32 = b_str.parse().ok()?;
            let a: f32 = if let Some(a_str) = parts.next() {
                let a_trimmed = a_str.trim_end_matches('%');
                if a_str.ends_with('%') {
                    a_trimmed.parse::<f32>().ok()? * 2.55
                } else {
                    let raw = a_trimmed.parse::<f32>().ok()?;
                    if raw <= 1.0 { raw * 255.0 } else { raw }
                }
            } else {
                255.0
            };

            let r_u8 = r.clamp(0.0, 255.0) as u32;
            let g_u8 = g.clamp(0.0, 255.0) as u32;
            let b_u8 = b.clamp(0.0, 255.0) as u32;
            let a_u8 = a.clamp(0.0, 255.0) as u32;

            return Some((a_u8 << 24) | (r_u8 << 16) | (g_u8 << 8) | b_u8);
        }
    }

    None
}

#[inline]
fn trim_comments(mut s: &str) -> &str {
    s = s.trim();
    while s.starts_with("/*") {
        if let Some(end) = s.find("*/") {
            s = s[end + 2..].trim();
        } else {
            break;
        }
    }
    while s.ends_with("*/") {
        if let Some(start) = s.rfind("/*") {
            s = s[..start].trim();
        } else {
            break;
        }
    }
    s
}

pub type GtkCssColors = (
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
);

/// Parses GTK CSS text content in-place without heap-copying comments,
/// extracting headerbar background, foreground, border, and backdrop colors.
pub fn parse_gtk_css_content(content: &str, _is_dark: bool) -> GtkCssColors {
    let mut defs: HashMap<&str, &str> = HashMap::new();

    // 1. In-place collection of @define-color declarations
    for line in content.lines() {
        let trimmed = trim_comments(line);
        if let Some(rest) = trimmed.strip_prefix("@define-color") {
            let rest = rest.trim();
            if let Some(pos) = rest.find(|c: char| c.is_ascii_whitespace()) {
                let name = rest[..pos].trim();
                let mut val = trim_comments(rest[pos..].trim().trim_end_matches(';'));
                if let Some(c_idx) = val.find("/*") {
                    val = val[..c_idx].trim();
                }
                defs.insert(name, val);
            }
        }
    }

    // Check predefined named variables in themes
    let def_bg = [
        "headerbar_bg_color",
        "theme_bg_color",
        "window_bg_color",
        "mint_header",
    ]
    .iter()
    .find_map(|k| defs.get(*k).and_then(|v| parse_color_value(v, &defs, 0)));

    let def_fg = ["headerbar_fg_color", "theme_fg_color", "window_fg_color"]
        .iter()
        .find_map(|k| defs.get(*k).and_then(|v| parse_color_value(v, &defs, 0)));

    let def_border = ["headerbar_border_color", "borders", "theme_border_color"]
        .iter()
        .find_map(|k| defs.get(*k).and_then(|v| parse_color_value(v, &defs, 0)));

    let def_inactive_bg = ["headerbar_backdrop_color", "theme_backdrop_bg_color"]
        .iter()
        .find_map(|k| defs.get(*k).and_then(|v| parse_color_value(v, &defs, 0)));

    // 2. Scan CSS rule blocks targeting headerbar or .titlebar directly from content
    let mut rule_bg = None;
    let mut rule_fg = None;
    let mut rule_border = None;
    let mut rule_inactive_bg = None;
    let mut rule_inactive_fg = None;

    let mut cursor = 0;
    while let Some(open_rel) = content[cursor..].find('{') {
        let open = cursor + open_rel;
        let selector = trim_comments(&content[cursor..open]);
        if let Some(close_rel) = content[open + 1..].find('}') {
            let close = open + 1 + close_rel;
            let body = &content[open + 1..close];
            cursor = close + 1;

            let is_backdrop = selector.contains(":backdrop");
            let targets_headerbar = selector.split(',').any(|part| {
                let p = trim_comments(part);
                (p.starts_with("headerbar")
                    || p.starts_with(".titlebar")
                    || p.starts_with(".csd headerbar")
                    || p.starts_with(".csd .titlebar"))
                    && !p.contains(" button")
                    && !p.contains(" switch")
                    && !p.contains(" entry")
                    && !p.contains(" separator")
                    && !p.contains(" label")
                    && !p.contains(" iconview")
            });

            if targets_headerbar {
                for decl in body.split(';') {
                    let d = trim_comments(decl);
                    if let Some((k, v)) = d.split_once(':') {
                        let key = trim_comments(k);
                        let mut val = trim_comments(v);
                        if let Some(c_idx) = val.find("/*") {
                            val = val[..c_idx].trim();
                        }
                        if let Some(c) = parse_color_value(val, &defs, 0) {
                            if is_backdrop {
                                if (key == "background-color" || key == "background")
                                    && rule_inactive_bg.is_none()
                                {
                                    rule_inactive_bg = Some(c);
                                } else if key == "color" && rule_inactive_fg.is_none() {
                                    rule_inactive_fg = Some(c);
                                }
                            } else {
                                if (key == "background-color" || key == "background")
                                    && rule_bg.is_none()
                                {
                                    rule_bg = Some(c);
                                } else if key == "color" && rule_fg.is_none() {
                                    rule_fg = Some(c);
                                } else if (key == "border-color" || key == "border-bottom-color")
                                    && rule_border.is_none()
                                {
                                    rule_border = Some(c);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            break;
        }
    }

    let bg = rule_bg.or(def_bg);
    let fg = rule_fg.or(def_fg);
    let border = rule_border.or(def_border);
    let inactive_bg = rule_inactive_bg.or(def_inactive_bg);
    let inactive_fg = rule_inactive_fg;

    (bg, fg, border, inactive_bg, inactive_fg)
}

/// Loads palette colors from installed GTK theme CSS files.
pub fn load_gtk_theme_palette(theme_name: &str, is_dark: bool) -> Option<CsdPalette> {
    let css_files = find_theme_css_files(theme_name, is_dark);
    if css_files.is_empty() {
        return None;
    }

    let mut final_bg = None;
    let mut final_fg = None;
    let mut final_border = None;
    let mut final_inactive_bg = None;
    let mut final_inactive_fg = None;

    for path in css_files {
        if let Ok(content) = fs::read_to_string(&path) {
            let (bg, fg, border, inact_bg, inact_fg) = parse_gtk_css_content(&content, is_dark);
            if final_bg.is_none() && bg.is_some() {
                final_bg = bg;
            }
            if final_fg.is_none() && fg.is_some() {
                final_fg = fg;
            }
            if final_border.is_none() && border.is_some() {
                final_border = border;
            }
            if final_inactive_bg.is_none() && inact_bg.is_some() {
                final_inactive_bg = inact_bg;
            }
            if final_inactive_fg.is_none() && inact_fg.is_some() {
                final_inactive_fg = inact_fg;
            }
            if final_bg.is_some() {
                break;
            }
        }
    }

    let raw_bg = final_bg?;
    // Ensure opaque 0xFF alpha for titlebar background surface
    let bg = 0xFF000000 | (raw_bg & 0x00FFFFFF);

    let r = ((bg >> 16) & 0xFF) as f32;
    let g = ((bg >> 8) & 0xFF) as f32;
    let b = (bg & 0xFF) as f32;
    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
    let is_light = lum > 128.0;

    let fg = final_fg.unwrap_or(if is_light { 0xFF2E3436 } else { 0xFFFFFFFF });
    let border = final_border.unwrap_or_else(|| {
        if is_light {
            adjust_brightness(bg, 0.90)
        } else {
            adjust_brightness(bg, 0.80)
        }
    });

    let inactive_bg = final_inactive_bg.unwrap_or_else(|| {
        if is_light {
            adjust_brightness(bg, 1.04)
        } else {
            adjust_brightness(bg, 0.85)
        }
    });

    let inactive_fg = final_inactive_fg.unwrap_or(if is_light { 0xFF8F9193 } else { 0xFF808080 });

    Some(CsdPalette {
        is_light,
        bg,
        fg,
        inactive_bg,
        inactive_fg,
        border,
        button_hover_bg: if is_light { 0x00000000 } else { 0x00FFFFFF },
        button_pressed_bg: if is_light { 0x00000000 } else { 0x00FFFFFF },
        close_hover_bg: 0x00E01B24,
        close_pressed_bg: 0x00B2161E,
        close_hover_fg: 0x00FFFFFF,
        close_fg: if is_light { 0xFF2E3436 } else { 0xFFFFFFFF },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csd_button_layout_parsing() {
        let layout1 = CsdButtonLayout::parse("':minimize,maximize,close'");
        assert_eq!(layout1.left, vec![]);
        assert_eq!(
            layout1.right,
            vec![
                CsdButtonType::Minimize,
                CsdButtonType::Maximize,
                CsdButtonType::Close
            ]
        );

        let layout2 = CsdButtonLayout::parse("'close,minimize,maximize:'");
        assert_eq!(
            layout2.left,
            vec![
                CsdButtonType::Close,
                CsdButtonType::Minimize,
                CsdButtonType::Maximize
            ]
        );
        assert_eq!(layout2.right, vec![]);

        let layout3 = CsdButtonLayout::parse("':close'");
        assert_eq!(layout3.left, vec![]);
        assert_eq!(layout3.right, vec![CsdButtonType::Close]);
    }

    #[test]
    fn test_csd_hit_test_right_aligned() {
        let layout = CsdButtonLayout::default();
        let width = 800;
        let height = 38;

        // Top right corner (800 - 10 = 790, y = 19) should hit Close
        assert_eq!(
            layout.hit_test(790.0, 19.0, width, height),
            Some(CsdButtonType::Close)
        );

        // Middle button (750.0, 19.0) should hit Maximize
        assert_eq!(
            layout.hit_test(750.0, 19.0, width, height),
            Some(CsdButtonType::Maximize)
        );

        // Leftmost right button (720.0, 19.0) should hit Minimize
        assert_eq!(
            layout.hit_test(720.0, 19.0, width, height),
            Some(CsdButtonType::Minimize)
        );

        // Center of window should hit None
        assert_eq!(layout.hit_test(400.0, 19.0, width, height), None);

        // Below titlebar should hit None
        assert_eq!(layout.hit_test(790.0, 42.0, width, height), None);
    }

    #[test]
    fn test_detect_resize_edge() {
        let (w, h) = (800, 600);
        let border = 6.0;

        assert_eq!(
            detect_resize_edge(2.0, 2.0, w, h, border),
            Some(crate::window::WindowResizeEdge::TopLeft)
        );
        assert_eq!(
            detect_resize_edge(798.0, 2.0, w, h, border),
            Some(crate::window::WindowResizeEdge::TopRight)
        );
        assert_eq!(
            detect_resize_edge(2.0, 598.0, w, h, border),
            Some(crate::window::WindowResizeEdge::BottomLeft)
        );
        assert_eq!(
            detect_resize_edge(798.0, 598.0, w, h, border),
            Some(crate::window::WindowResizeEdge::BottomRight)
        );
        assert_eq!(
            detect_resize_edge(400.0, 2.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Top)
        );
        assert_eq!(
            detect_resize_edge(400.0, 598.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Bottom)
        );
        assert_eq!(
            detect_resize_edge(2.0, 300.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Left)
        );
        assert_eq!(
            detect_resize_edge(798.0, 300.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Right)
        );
        assert_eq!(detect_resize_edge(400.0, 300.0, w, h, border), None);
    }

    #[test]
    fn test_csd_palette_defaults_and_resolution() {
        let dark = CsdPalette::dark();
        assert!(!dark.is_light);
        assert_eq!(dark.bg, 0xFF2E2E2E);
        assert_eq!(dark.fg, 0xFFFFFFFF);

        let light = CsdPalette::light();
        assert!(light.is_light);
        assert_eq!(light.bg, 0xFFEBEBEB);
        assert_eq!(light.fg, 0xFF2E3436);

        assert_eq!(CsdThemeMode::parse("light"), CsdThemeMode::Light);
        assert_eq!(CsdThemeMode::parse("dark"), CsdThemeMode::Dark);
        assert_eq!(CsdThemeMode::parse("terminal"), CsdThemeMode::Terminal);
        assert_eq!(CsdThemeMode::parse("anything_else"), CsdThemeMode::Auto);
    }

    #[test]
    fn test_parse_color_value_formats() {
        let defs = HashMap::new();
        // 6-digit hex
        assert_eq!(parse_color_value("#222226", &defs, 0), Some(0xFF222226));
        // 3-digit hex
        assert_eq!(parse_color_value("#fff", &defs, 0), Some(0xFFFFFFFF));
        // named color
        assert_eq!(parse_color_value("white", &defs, 0), Some(0xFFFFFFFF));
        assert_eq!(parse_color_value("transparent", &defs, 0), Some(0x00000000));
        // rgb(...)
        assert_eq!(
            parse_color_value("rgb(46, 46, 51)", &defs, 0),
            Some(0xFF2E2E33)
        );
        // rgba(...) with fractional alpha
        assert_eq!(
            parse_color_value("rgba(255, 255, 255, 0.87)", &defs, 0),
            Some(0xDDFFFFFF)
        );
        // CSS Color Level 4 RGB(r g b / a%)
        assert_eq!(
            parse_color_value("RGB(0 0 6/36%)", &defs, 0),
            Some(0x5B000006)
        );
    }

    #[test]
    fn test_parse_gtk_css_content_variables_and_rules() {
        let css = r#"
            @define-color mint_header #222226;
            @define-color window_bg_color @mint_header;
            @define-color headerbar_bg_color @window_bg_color;

            headerbar,
            .titlebar:not(headerbar) {
                border-color: #18181b;
                color: rgba(255, 255, 255, 0.87);
                background-color: @headerbar_bg_color;
            }

            headerbar:backdrop,
            .titlebar:backdrop:not(headerbar) {
                color: rgba(255, 255, 255, 0.47);
            }
        "#;

        let (bg, fg, border, inact_bg, inact_fg) = parse_gtk_css_content(css, true);
        assert_eq!(
            bg,
            Some(0xFF222226),
            "headerbar background must be extracted via variable chain"
        );
        assert_eq!(
            fg,
            Some(0xDDFFFFFF),
            "headerbar foreground color must be extracted"
        );
        assert_eq!(border, Some(0xFF18181B), "border color must be extracted");
        assert_eq!(
            inact_fg,
            Some(0x77FFFFFF),
            "backdrop inactive foreground must be extracted"
        );
        assert!(inact_bg.is_none());
    }

    #[test]
    fn test_parse_gtk_css_content_with_comments() {
        let css = "headerbar { /* inline comment */ background-color: #123456; /* multiline\n comment */ color: white; }";
        let (bg, fg, ..) = parse_gtk_css_content(css, true);
        assert_eq!(bg, Some(0xFF123456));
        assert_eq!(fg, Some(0xFFFFFFFF));
    }

    #[test]
    fn test_adjust_brightness() {
        let color = 0xFF808080;
        let darker = adjust_brightness(color, 0.5);
        assert_eq!(darker, 0xFF404040);
        let brighter = adjust_brightness(color, 1.5);
        assert_eq!(brighter, 0xFFC0C0C0);
    }
}
