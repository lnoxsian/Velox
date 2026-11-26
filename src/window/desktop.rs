use crate::window::csd::CsdButtonLayout;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default, Clone)]
pub struct GtkSettings {
    pub theme_name: Option<String>,
    pub prefer_dark: Option<bool>,
    pub cursor_theme: Option<String>,
    pub cursor_size: Option<u32>,
    pub decoration_layout: Option<String>,
}

/// Reads and merges user and system GTK `settings.ini` files in order of priority:
/// 1. `~/.config/gtk-4.0/settings.ini`
/// 2. `~/.config/gtk-3.0/settings.ini`
/// 3. `/etc/gtk-3.0/settings.ini`
pub fn read_gtk_settings() -> GtkSettings {
    let mut merged = GtkSettings::default();

    let mut candidate_paths = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home);
        candidate_paths.push(p.join(".config/gtk-4.0/settings.ini"));
        candidate_paths.push(p.join(".config/gtk-3.0/settings.ini"));
    }
    candidate_paths.push(PathBuf::from("/etc/gtk-3.0/settings.ini"));

    for path in candidate_paths {
        if let Ok(content) = fs::read_to_string(&path) {
            parse_gtk_settings_content(&content, &mut merged);
            if merged.theme_name.is_some()
                && merged.prefer_dark.is_some()
                && merged.cursor_theme.is_some()
                && merged.cursor_size.is_some()
                && merged.decoration_layout.is_some()
            {
                break;
            }
        }
    }

    merged
}

fn parse_gtk_settings_content(content: &str, settings: &mut GtkSettings) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.starts_with('[') {
            continue;
        }

        if let Some((key, val)) = trimmed.split_once('=') {
            let key = key.trim();
            let val = val.trim().trim_matches('\'').trim_matches('"').trim();

            match key {
                "gtk-theme-name" if settings.theme_name.is_none() => {
                    if !val.is_empty() {
                        settings.theme_name = Some(val.to_string());
                    }
                }
                "gtk-application-prefer-dark-theme" if settings.prefer_dark.is_none() => {
                    let v = val.to_lowercase();
                    if v == "1" || v == "true" {
                        settings.prefer_dark = Some(true);
                    } else if v == "0" || v == "false" {
                        settings.prefer_dark = Some(false);
                    }
                }
                "gtk-cursor-theme-name" if settings.cursor_theme.is_none() => {
                    if !val.is_empty() {
                        settings.cursor_theme = Some(val.to_string());
                    }
                }
                "gtk-cursor-theme-size" if settings.cursor_size.is_none() => {
                    if let Ok(sz) = val.parse::<u32>() {
                        if sz > 0 {
                            settings.cursor_size = Some(sz);
                        }
                    }
                }
                "gtk-decoration-layout" if settings.decoration_layout.is_none() => {
                    if !val.is_empty() && val.contains(':') {
                        settings.decoration_layout = Some(val.to_string());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Parses the `COLORFGBG` environment variable (e.g., `"15;0"` or `"default;0"`).
/// Terminal standard: second field is the background color index:
/// 0..=6 and 8 are dark colors, 7 and 15 are light colors.
pub fn parse_colorfgbg_is_dark(val: &str) -> Option<bool> {
    let parts: Vec<&str> = val.split(';').map(|s| s.trim()).collect();
    if parts.len() >= 2 {
        let bg_str = parts[parts.len() - 1];
        if let Ok(bg_num) = bg_str.parse::<u8>() {
            return match bg_num {
                0..=6 | 8 => Some(true),
                7 | 15 => Some(false),
                _ => None,
            };
        }
    }
    None
}

/// Reads KDE Plasma configuration from `~/.config/kdeglobals`.
pub fn read_kde_globals() -> (Option<bool>, Option<String>) {
    if let Ok(home) = std::env::var("HOME") {
        let path = Path::new(&home).join(".config/kdeglobals");
        if let Ok(content) = fs::read_to_string(path) {
            let mut is_dark = None;
            let mut color_scheme = None;

            for line in content.lines() {
                let trimmed = line.trim();
                if let Some((key, val)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let val = val.trim();
                    if key == "ColorScheme" {
                        color_scheme = Some(val.to_string());
                        let lower = val.to_lowercase();
                        if lower.contains("dark") {
                            is_dark = Some(true);
                        } else if lower.contains("light") {
                            is_dark = Some(false);
                        }
                    } else if key == "BackgroundNormal" && is_dark.is_none() {
                        let channels: Vec<u8> =
                            val.split(',').filter_map(|c| c.trim().parse().ok()).collect();
                        if channels.len() >= 3 {
                            let lum = 0.299 * (channels[0] as f32)
                                + 0.587 * (channels[1] as f32)
                                + 0.114 * (channels[2] as f32);
                            is_dark = Some(lum < 128.0);
                        }
                    }
                }
            }
            return (is_dark, color_scheme);
        }
    }
    (None, None)
}

/// Fast, non-blocking detection of system button layout.
/// Checks environment and flat configuration files first (< 0.1ms) before calling `gsettings`.
pub fn detect_button_layout() -> CsdButtonLayout {
    // 1. Explicit VELOX_BUTTON_LAYOUT environment override
    if let Ok(val) = std::env::var("VELOX_BUTTON_LAYOUT") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return CsdButtonLayout::parse(trimmed);
        }
    }

    // 2. Direct GTK settings.ini check (GTK 3/4 standard: `gtk-decoration-layout`)
    let gtk = read_gtk_settings();
    if let Some(ref layout) = gtk.decoration_layout {
        return CsdButtonLayout::parse(layout);
    }

    // 3. XFCE xfwm4.xml flat file check
    if let Ok(home) = std::env::var("HOME") {
        let xfwm_path = Path::new(&home).join(".config/xfce4/xfconf/xfce-perchannel-xml/xfwm4.xml");
        if let Ok(content) = fs::read_to_string(xfwm_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.contains("button_layout") {
                    if let Some(val_start) = trimmed.find("value=\"") {
                        let rest = &trimmed[val_start + 7..];
                        if let Some(val_end) = rest.find('"') {
                            let val = &rest[..val_end];
                            if val.contains('|') {
                                let velox_fmt = val.replace('|', ":");
                                return CsdButtonLayout::parse(&velox_fmt);
                            }
                        }
                    }
                }
            }
        }
    }

    // 4. Slow fallback to gsettings only if no flat files provided a layout
    let is_cinnamon = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_lowercase().contains("cinnamon"))
        .unwrap_or(false);

    let schemas = if is_cinnamon {
        ["org.cinnamon.desktop.wm.preferences", "org.gnome.desktop.wm.preferences"]
    } else {
        ["org.gnome.desktop.wm.preferences", "org.cinnamon.desktop.wm.preferences"]
    };

    for schema in schemas {
        if let Ok(out) = Command::new("gsettings")
            .args(["get", schema, "button-layout"])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if s.contains(':') {
                    return CsdButtonLayout::parse(&s);
                }
            }
        }
    }

    CsdButtonLayout::default()
}

/// Fast, non-blocking dark mode detection.
/// Evaluates environment variables and desktop config files in < 0.1ms before calling `gsettings`.
pub fn detect_is_dark() -> bool {
    // 1. Explicit COLORFGBG environment variable
    if let Ok(val) = std::env::var("COLORFGBG") {
        if let Some(dark) = parse_colorfgbg_is_dark(&val) {
            return dark;
        }
    }

    // 2. Explicit GTK_THEME environment variable
    if let Ok(val) = std::env::var("GTK_THEME") {
        let lower = val.to_lowercase();
        if lower.contains("dark") {
            return true;
        } else if lower.contains("light") {
            return false;
        }
    }

    // 3. KDE Plasma check (~/.config/kdeglobals)
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    if desktop.contains("kde") || desktop.contains("plasma") {
        let (kde_dark, _) = read_kde_globals();
        if let Some(dark) = kde_dark {
            return dark;
        }
    }

    // 4. GTK settings.ini check (~/.config/gtk-3.0/settings.ini)
    let gtk = read_gtk_settings();
    if let Some(prefer_dark) = gtk.prefer_dark {
        return prefer_dark;
    }
    if let Some(ref theme) = gtk.theme_name {
        let lower = theme.to_lowercase();
        if lower.contains("dark") {
            return true;
        } else if lower.contains("light") {
            return false;
        }
    }

    // 5. Fallback to gsettings color-scheme query
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if s.contains("prefer-dark") {
                return true;
            } else if s.contains("prefer-light") {
                return false;
            }
        }
    }

    // Default to dark mode (standard for developer terminals)
    true
}

/// Fast detection of active GTK theme name from environment or flat files.
pub fn detect_gtk_theme_name() -> Option<String> {
    if let Ok(val) = std::env::var("GTK_THEME") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            let name = trimmed.split(':').next().unwrap_or(trimmed).trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    let gtk = read_gtk_settings();
    if let Some(theme) = gtk.theme_name {
        return Some(theme);
    }

    let is_cinnamon = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_lowercase().contains("cinnamon"))
        .unwrap_or(false);

    let schemas = if is_cinnamon {
        ["org.cinnamon.desktop.interface", "org.gnome.desktop.interface"]
    } else {
        ["org.gnome.desktop.interface", "org.cinnamon.desktop.interface"]
    };

    for schema in schemas {
        if let Ok(out) = Command::new("gsettings")
            .args(["get", schema, "gtk-theme"])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                let trimmed = s.trim().trim_matches('\'').trim_matches('"');
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

/// Reads user-configured cursor theme from `~/.icons/default/index.theme` or `~/.local/share/icons/default/index.theme`.
pub fn read_user_icon_default_theme() -> Option<String> {
    if let Ok(home) = std::env::var("HOME") {
        let p = Path::new(&home);
        let candidates = [
            p.join(".icons/default/index.theme"),
            p.join(".local/share/icons/default/index.theme"),
        ];
        for path in candidates {
            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    let lower = trimmed.to_lowercase();
                    if lower.starts_with("inherits=") {
                        let name = trimmed[9..].trim().trim_matches('\'').trim_matches('"').trim();
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Reads XFCE cursor configuration from `~/.config/xfce4/xfconf/xfce-perchannel-xml/xsettings.xml`.
pub fn read_xfce_cursor_settings() -> (Option<String>, Option<u32>) {
    if let Ok(home) = std::env::var("HOME") {
        let p = Path::new(&home).join(".config/xfce4/xfconf/xfce-perchannel-xml/xsettings.xml");
        if let Ok(content) = fs::read_to_string(p) {
            let mut theme = None;
            let mut size = None;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.contains("CursorThemeName") {
                    if let Some(pos) = trimmed.find("value=\"") {
                        let rest = &trimmed[pos + 7..];
                        if let Some(end) = rest.find('"') {
                            let val = rest[..end].trim();
                            if !val.is_empty() && theme.is_none() {
                                theme = Some(val.to_string());
                            }
                        }
                    }
                } else if trimmed.contains("CursorThemeSize") {
                    if let Some(pos) = trimmed.find("value=\"") {
                        let rest = &trimmed[pos + 7..];
                        if let Some(end) = rest.find('"') {
                            if let Ok(sz) = rest[..end].trim().parse::<u32>() {
                                if sz > 0 && size.is_none() {
                                    size = Some(sz);
                                }
                            }
                        }
                    }
                }
            }
            return (theme, size);
        }
    }
    (None, None)
}

/// Reads system default cursor theme from `/usr/share/icons/default/index.theme` or `/usr/local/share/icons/default/index.theme`.
pub fn read_system_default_icon_theme() -> Option<String> {
    let candidates = [
        "/usr/share/icons/default/index.theme",
        "/usr/local/share/icons/default/index.theme",
    ];
    for path in candidates {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                let lower = trimmed.to_lowercase();
                if lower.starts_with("inherits=") {
                    let name = trimmed[9..].trim().trim_matches('\'').trim_matches('"').trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Fast, comprehensive detection of desktop cursor theme and size.
/// Checks user environment, user desktop files, desktop settings via gsettings/xfconf/kcminput,
/// environment variables, and system fallbacks in prioritized order to match the host desktop accurately.
pub fn detect_cursor_theme_and_size() -> (String, u32) {
    // 1. Explicit VELOX_CURSOR_THEME / VELOX_CURSOR_SIZE environment overrides
    let mut theme = std::env::var("VELOX_CURSOR_THEME")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let mut size = std::env::var("VELOX_CURSOR_SIZE")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&sz| sz > 0);

    if theme.is_some() && size.is_some() {
        return (theme.unwrap(), size.unwrap());
    }

    // 2. User-specific icon theme configuration (~/.icons/default/index.theme, ~/.local/share/icons/default/index.theme)
    if theme.is_none() {
        theme = read_user_icon_default_theme();
    }

    // 3. User GTK settings.ini check (~/.config/gtk-4.0/settings.ini, ~/.config/gtk-3.0/settings.ini)
    if theme.is_none() || size.is_none() {
        let gtk = read_gtk_settings();
        if theme.is_none() {
            theme = gtk.cursor_theme;
        }
        if size.is_none() {
            size = gtk.cursor_size;
        }
    }

    // 4. Desktop environment flat configs (KDE Plasma, XFCE)
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_lowercase();

    if theme.is_none() || size.is_none() {
        if desktop.contains("kde") || desktop.contains("plasma") {
            if let Ok(home) = std::env::var("HOME") {
                let kcminput = Path::new(&home).join(".config/kcminputrc");
                if let Ok(content) = fs::read_to_string(kcminput) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some((k, v)) = trimmed.split_once('=') {
                            let k = k.trim();
                            let v = v.trim();
                            if k == "cursorTheme" && theme.is_none() && !v.is_empty() {
                                theme = Some(v.to_string());
                            } else if k == "cursorSize" && size.is_none() {
                                if let Ok(sz) = v.parse::<u32>() {
                                    if sz > 0 {
                                        size = Some(sz);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if desktop.contains("xfce") {
            let (xfce_theme, xfce_size) = read_xfce_cursor_settings();
            if theme.is_none() {
                theme = xfce_theme;
            }
            if size.is_none() {
                size = xfce_size;
            }
        }
    }

    // 5. Desktop environment gsettings (Cinnamon, GNOME, MATE, etc.)
    if theme.is_none() || size.is_none() {
        let schemas = if desktop.contains("cinnamon") {
            vec![
                "org.cinnamon.desktop.interface",
                "org.gnome.desktop.interface",
            ]
        } else if desktop.contains("mate") {
            vec![
                "org.mate.peripherals-mouse",
                "org.gnome.desktop.interface",
                "org.cinnamon.desktop.interface",
            ]
        } else {
            vec![
                "org.gnome.desktop.interface",
                "org.cinnamon.desktop.interface",
            ]
        };

        for schema in schemas {
            if theme.is_none() {
                if let Ok(out) = Command::new("gsettings")
                    .args(["get", schema, "cursor-theme"])
                    .output()
                {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout);
                        let cleaned = s.trim().trim_matches('\'').trim_matches('"').trim();
                        if !cleaned.is_empty() {
                            theme = Some(cleaned.to_string());
                        }
                    }
                }
            }

            if size.is_none() {
                if let Ok(out) = Command::new("gsettings")
                    .args(["get", schema, "cursor-size"])
                    .output()
                {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout);
                        if let Ok(sz) = s.trim().parse::<u32>() {
                            if sz > 0 {
                                size = Some(sz);
                            }
                        }
                    }
                }
            }

            if theme.is_some() && size.is_some() {
                break;
            }
        }
    }

    // 6. Environment variables (XCURSOR_THEME, XCURSOR_SIZE)
    if theme.is_none() {
        theme = std::env::var("XCURSOR_THEME")
            .ok()
            .filter(|s| !s.trim().is_empty());
    }
    if size.is_none() {
        size = std::env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&sz| sz > 0);
    }

    // 7. System fallback (/usr/share/icons/default/index.theme)
    if theme.is_none() {
        theme = read_system_default_icon_theme();
    }

    (
        theme.unwrap_or_else(|| "default".to_string()),
        size.unwrap_or(24),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_colorfgbg() {
        assert_eq!(parse_colorfgbg_is_dark("15;0"), Some(true)); // Dark bg
        assert_eq!(parse_colorfgbg_is_dark("0;15"), Some(false)); // Light bg
        assert_eq!(parse_colorfgbg_is_dark("7;0;8"), Some(true)); // 8 is dark
        assert_eq!(parse_colorfgbg_is_dark("invalid"), None);
    }

    #[test]
    fn test_parse_gtk_settings_content() {
        let sample = r#"
[Settings]
gtk-theme-name = Adwaita-dark
gtk-application-prefer-dark-theme = 1
gtk-cursor-theme-name = "DMZ-White"
gtk-cursor-theme-size = 24
gtk-decoration-layout = ':minimize,maximize,close'
"#;
        let mut settings = GtkSettings::default();
        parse_gtk_settings_content(sample, &mut settings);

        assert_eq!(settings.theme_name.as_deref(), Some("Adwaita-dark"));
        assert_eq!(settings.prefer_dark, Some(true));
        assert_eq!(settings.cursor_theme.as_deref(), Some("DMZ-White"));
        assert_eq!(settings.cursor_size, Some(24));
        assert_eq!(
            settings.decoration_layout.as_deref(),
            Some(":minimize,maximize,close")
        );
    }

    #[test]
    fn test_detect_cursor_theme_override() {
        unsafe {
            std::env::set_var("VELOX_CURSOR_THEME", "TestCursorTheme");
            std::env::set_var("VELOX_CURSOR_SIZE", "48");
        }
        let (theme, size) = detect_cursor_theme_and_size();
        assert_eq!(theme, "TestCursorTheme");
        assert_eq!(size, 48);
        unsafe {
            std::env::remove_var("VELOX_CURSOR_THEME");
            std::env::remove_var("VELOX_CURSOR_SIZE");
        }
    }

    #[test]
    fn test_detect_cursor_theme_and_size_returns_valid() {
        let (theme, size) = detect_cursor_theme_and_size();
        assert!(!theme.is_empty());
        assert!((12..=128).contains(&size));
    }
}
