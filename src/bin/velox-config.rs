use nix::sys::termios::{LocalFlags, SetArg, tcgetattr, tcsetattr};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write, stdout};
use std::path::PathBuf;
use velox::config::config::{Config, ConfigColors, TabBarVisibility};
use velox::config::defaults::default_config;
use velox::config::loader::{config_path, load_from_str};

// Built-in theme presets embedded directly into the binary
const THEMES: &[(&str, &str)] = &[
    (
        "Ayu Dark",
        include_str!("../../assets/velox_terminal_themes/ayu-dark.toml"),
    ),
    (
        "Ayu Light",
        include_str!("../../assets/velox_terminal_themes/ayu-light.toml"),
    ),
    (
        "Catppuccin Mocha",
        include_str!("../../assets/velox_terminal_themes/catppuccin-mocha.toml"),
    ),
    (
        "Catppuccin Macchiato",
        include_str!("../../assets/velox_terminal_themes/catppuccin-macchiato.toml"),
    ),
    (
        "Catppuccin Frappe",
        include_str!("../../assets/velox_terminal_themes/catppuccin-frappe.toml"),
    ),
    (
        "Catppuccin Latte",
        include_str!("../../assets/velox_terminal_themes/catppuccin-latte.toml"),
    ),
    (
        "Gruvbox Dark",
        include_str!("../../assets/velox_terminal_themes/gruvbox-dark.toml"),
    ),
    (
        "Gruvbox Light",
        include_str!("../../assets/velox_terminal_themes/gruvbox-light.toml"),
    ),
    (
        "Rose Pine",
        include_str!("../../assets/velox_terminal_themes/rose-pine.toml"),
    ),
    (
        "Rose Pine Moon",
        include_str!("../../assets/velox_terminal_themes/rose-pine-moon.toml"),
    ),
    (
        "Rose Pine Dawn",
        include_str!("../../assets/velox_terminal_themes/rose-pine-dawn.toml"),
    ),
    (
        "Solarized Dark",
        include_str!("../../assets/velox_terminal_themes/solarized-dark.toml"),
    ),
    (
        "Solarized Light",
        include_str!("../../assets/velox_terminal_themes/solarized-light.toml"),
    ),
    (
        "GitHub Dark",
        include_str!("../../assets/velox_terminal_themes/github-dark.toml"),
    ),
    (
        "GitHub Light",
        include_str!("../../assets/velox_terminal_themes/github-light.toml"),
    ),
];

// ============================================================================
// Terminal Raw Mode & Keyboard Input Engine (0 external crates)
// ============================================================================

fn get_terminal_dimensions() -> (usize, usize) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0
            && ws.ws_col > 0
            && ws.ws_row > 0
        {
            (ws.ws_col as usize, ws.ws_row as usize)
        } else {
            (80, 24)
        }
    }
}

struct RawModeGuard {
    orig: nix::sys::termios::Termios,
}

impl RawModeGuard {
    fn enter() -> Option<Self> {
        let stdin = std::io::stdin();
        let orig = tcgetattr(&stdin).ok()?;
        let mut raw = orig.clone();
        raw.local_flags
            .remove(LocalFlags::ICANON | LocalFlags::ECHO);
        tcsetattr(&stdin, SetArg::TCSANOW, &raw).ok()?;
        print!("\x1b[?25l"); // Hide cursor during arrow navigation
        let _ = stdout().flush();
        Some(Self { orig })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        print!("\x1b[?25h"); // Restore cursor
        let _ = stdout().flush();
        let stdin = std::io::stdin();
        let _ = tcsetattr(&stdin, SetArg::TCSANOW, &self.orig);
    }
}

enum Key {
    Up,
    Down,
    Enter,
    Escape,
    Char(char),
    Other,
}

fn read_key() -> Key {
    let mut buf = [0u8; 8];
    let mut stdin = std::io::stdin();
    if let Ok(n) = stdin.read(&mut buf) {
        if n == 1 {
            match buf[0] {
                b'\r' | b'\n' => Key::Enter,
                0x1b => Key::Escape,
                b => Key::Char(b as char),
            }
        } else if n >= 3 && buf[0] == 0x1b && buf[1] == b'[' {
            match buf[2] {
                b'A' => Key::Up,
                b'B' => Key::Down,
                _ => Key::Other,
            }
        } else {
            Key::Other
        }
    } else {
        Key::Other
    }
}

/// Interactive selection menu with scrolling viewport for large lists (e.g. 900+ fonts)
fn select_menu(prompt: &str, items: &[String], default_idx: usize) -> usize {
    if items.is_empty() {
        return 0;
    }
    let (term_cols, term_rows) = get_terminal_dimensions();
    let max_visible = term_rows.saturating_sub(8).clamp(4, 14).min(items.len());
    let mut selected = default_idx.min(items.len().saturating_sub(1));

    print!(
        "\x1b[2K\r\x1b[1;36m?\x1b[0m \x1b[1m{}\x1b[0m \x1b[2m(Arrows to navigate, Enter to confirm)\x1b[0m\n",
        prompt
    );

    let render_items = |sel: usize| -> usize {
        let mut lines = 0;
        let start = if items.len() <= max_visible {
            0
        } else if sel >= items.len().saturating_sub(max_visible / 2) {
            items.len().saturating_sub(max_visible)
        } else {
            sel.saturating_sub(max_visible / 2)
        };
        let end = (start + max_visible).min(items.len());
        let max_item_w = term_cols.saturating_sub(14).max(20);

        if items.len() > max_visible {
            if start > 0 {
                print!("\x1b[2K\r    \x1b[2m^ ... ({} more above)\x1b[0m\n", start);
            } else {
                print!("\x1b[2K\r    \x1b[2m-- Top of list --\x1b[0m\n");
            }
            lines += 1;
        }

        for (i, item) in items.iter().enumerate().take(end).skip(start) {
            let trimmed_item = if item.len() > max_item_w {
                format!("{}...", &item[..max_item_w.saturating_sub(3)])
            } else {
                item.clone()
            };

            if i == sel {
                print!("\x1b[2K\r  \x1b[1;36m> [~] {}\x1b[0m\n", trimmed_item);
            } else {
                print!("\x1b[2K\r    \x1b[2m[~]\x1b[0m {}\n", trimmed_item);
            }
            lines += 1;
        }

        if items.len() > max_visible {
            if end < items.len() {
                print!(
                    "\x1b[2K\r    \x1b[2mv ... ({} more below, {} total)\x1b[0m\n",
                    items.len() - end,
                    items.len()
                );
            } else {
                print!("\x1b[2K\r    \x1b[2m-- End of list --\x1b[0m\n");
            }
            lines += 1;
        }

        let _ = stdout().flush();
        lines
    };

    let mut lines_drawn = render_items(selected);
    let _guard = RawModeGuard::enter();

    loop {
        match read_key() {
            Key::Up => {
                if selected > 0 {
                    selected -= 1;
                    print!("\x1b[{}A\r", lines_drawn);
                    lines_drawn = render_items(selected);
                }
            }
            Key::Down => {
                if selected + 1 < items.len() {
                    selected += 1;
                    print!("\x1b[{}A\r", lines_drawn);
                    lines_drawn = render_items(selected);
                }
            }
            Key::Char(c) if c.is_ascii_digit() && c != '0' => {
                let digit_idx = (c as usize) - ('1' as usize);
                if digit_idx < items.len() {
                    selected = digit_idx;
                    break;
                }
            }
            Key::Enter => break,
            Key::Escape | Key::Char('q') | Key::Char('Q') => {
                selected = items.len().saturating_sub(1);
                break;
            }
            _ => {}
        }
    }

    drop(_guard);
    // Erase prompt and all rendered items completely so menus update in-place without repeating
    print!("\x1b[{}A\r\x1b[J", lines_drawn + 1);
    let _ = stdout().flush();
    selected
}

// ============================================================================
// Text, Hex, Stepper & Boolean Input Helpers
// ============================================================================

fn prompt_text(prompt: &str, default: Option<&str>) -> String {
    print!("\x1b[2K\r\x1b[1;32m?\x1b[0m {prompt}");
    if let Some(def) = default
        && !def.is_empty()
    {
        print!(" \x1b[2m[{def}]\x1b[0m");
    }
    print!(": ");
    let _ = stdout().flush();

    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let trimmed = line.trim();
    let result = if trimmed.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        trimmed.to_string()
    };
    // Clear input prompt line so it never leaves a trail
    print!("\x1b[1A\x1b[2K\r");
    let _ = stdout().flush();
    result
}

fn prompt_bool(prompt: &str, default: bool) -> bool {
    let def_str = if default { "Y/n" } else { "y/N" };
    loop {
        let input = prompt_text(&format!("{prompt} ({def_str})"), None);
        let trimmed = input.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return default;
        }
        if trimmed == "y" || trimmed == "yes" || trimmed == "true" || trimmed == "1" {
            return true;
        }
        if trimmed == "n" || trimmed == "no" || trimmed == "false" || trimmed == "0" {
            return false;
        }
        println!("\x1b[31m  [!] Please type 'y' for yes or 'n' for no.\x1b[0m");
    }
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let clean = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        Some((r, g, b))
    } else if clean.len() == 3 {
        let r = u8::from_str_radix(&clean[0..1], 16).ok()?;
        let g = u8::from_str_radix(&clean[1..2], 16).ok()?;
        let b = u8::from_str_radix(&clean[2..3], 16).ok()?;
        Some((r * 17, g * 17, b * 17))
    } else {
        None
    }
}

fn render_swatch(hex: &str) -> String {
    if let Some((r, g, b)) = parse_hex_rgb(hex) {
        format!("\x1b[48;2;{r};{g};{b}m  \x1b[0m {hex}")
    } else {
        format!("[--] {hex}")
    }
}

fn prompt_hex(prompt: &str, current: Option<&str>, allow_blank: bool) -> Option<String> {
    loop {
        let input = prompt_text(prompt, current);
        let trimmed = input.trim();
        if allow_blank && (trimmed.is_empty() || trimmed == "default" || trimmed == "none") {
            return None;
        }
        let clean = trimmed.strip_prefix('#').unwrap_or(trimmed);
        if (clean.len() == 6 || clean.len() == 3) && clean.chars().all(|c| c.is_ascii_hexdigit()) {
            let formatted = if trimmed.starts_with('#') {
                trimmed.to_string()
            } else {
                format!("#{trimmed}")
            };
            return Some(formatted);
        }
        println!(
            "\x1b[31m  [!] Invalid hex color! Please enter format #RRGGBB (e.g. #1e1e2e) or leave empty.\x1b[0m"
        );
    }
}

fn prompt_stepper_f32(label: &str, mut current: f32, step: f32, min: f32, max: f32) -> f32 {
    loop {
        let options = vec![
            format!(
                "[-] Step down  ({:.2} -> {:.2})",
                current,
                (current - step).max(min)
            ),
            format!(
                "[+] Step up    ({:.2} -> {:.2})",
                current,
                (current + step).min(max)
            ),
            "[Input] Enter exact number directly".to_string(),
            "[Done]  Accept current value".to_string(),
        ];
        let sel = select_menu(&format!("{label} [Current: {:.2}]", current), &options, 3);
        match sel {
            0 => current = (current - step).max(min),
            1 => current = (current + step).min(max),
            2 => {
                let s = prompt_text(
                    &format!("Enter number between {min} and {max}"),
                    Some(&format!("{current}")),
                );
                if let Ok(val) = s.parse::<f32>() {
                    if val >= min && val <= max {
                        current = val;
                    } else {
                        println!("\x1b[31m  Value must be between {min} and {max}\x1b[0m");
                    }
                }
            }
            _ => break,
        }
    }
    current
}

fn prompt_stepper_usize(
    label: &str,
    mut current: usize,
    step: usize,
    min: usize,
    max: usize,
) -> usize {
    loop {
        let options = vec![
            format!(
                "[-] Step down  ({} -> {})",
                current,
                current.saturating_sub(step).max(min)
            ),
            format!(
                "[+] Step up    ({} -> {})",
                current,
                (current + step).min(max)
            ),
            "[Input] Enter exact integer directly".to_string(),
            "[Done]  Accept current value".to_string(),
        ];
        let sel = select_menu(&format!("{label} [Current: {}]", current), &options, 3);
        match sel {
            0 => current = current.saturating_sub(step).max(min),
            1 => current = (current + step).min(max),
            2 => {
                let s = prompt_text(
                    &format!("Enter integer between {min} and {max}"),
                    Some(&format!("{current}")),
                );
                if let Ok(val) = s.parse::<usize>() {
                    if val >= min && val <= max {
                        current = val;
                    } else {
                        println!("\x1b[31m  Value must be between {min} and {max}\x1b[0m");
                    }
                }
            }
            _ => break,
        }
    }
    current
}

// ============================================================================
// State & Persistence
// ============================================================================

fn load_initial_config() -> (Config, bool) {
    if let Some(path) = config_path()
        && path.exists()
        && let Ok(content) = fs::read_to_string(&path)
    {
        let (cfg, _diags) = load_from_str(&content);
        return (cfg, true);
    }
    (default_config(), false)
}

fn save_and_exit(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let target_path = config_path().unwrap_or_else(|| {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|_| PathBuf::from("."));
        base.join("velox").join("config.toml")
    });

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if target_path.exists() {
        let backup_path = target_path.with_extension("toml.bak");
        if let Err(e) = fs::copy(&target_path, &backup_path) {
            println!(
                "\x1b[33mWarning: Failed to create backup file: {}\x1b[0m",
                e
            );
        } else {
            println!(
                "\x1b[32m[OK] Created backup at {}\x1b[0m",
                backup_path.display()
            );
        }
    }

    let toml_str = toml::to_string_pretty(config)?;
    fs::write(&target_path, toml_str)?;

    println!(
        "\x1b[1;32m[OK] Configuration saved successfully to {}\x1b[0m\n",
        target_path.display()
    );
    Ok(())
}

fn discover_fonts(mono_only: bool) -> Vec<String> {
    let mut fonts = vec![
        "Fira Code".to_string(),
        "JetBrains Mono".to_string(),
        "Hack".to_string(),
        "Cascadia Code".to_string(),
        "Source Code Pro".to_string(),
        "Inconsolata".to_string(),
        "Ubuntu Mono".to_string(),
        "monospace".to_string(),
    ];

    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut detected: HashSet<String> = HashSet::new();

    for face in db.faces() {
        if !mono_only
            || face.monospaced
            || face
                .families
                .iter()
                .any(|(f, _)| f.to_lowercase().contains("mono"))
        {
            for (family, _) in &face.families {
                detected.insert(family.clone());
            }
        }
    }

    let mut sorted_detected: Vec<String> = detected.into_iter().collect();
    sorted_detected.sort();

    for f in sorted_detected {
        if !fonts.contains(&f) {
            fonts.push(f);
        }
    }

    fonts
}

fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

// ============================================================================
// Main Menu Loop & Category Submenus
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\x1b[1;36m===================================================\x1b[0m");
    println!("\x1b[1;36m       Velox Terminal Configuration Utility        \x1b[0m");
    println!("\x1b[1;36m===================================================\x1b[0m");

    let (mut config, loaded_from_disk) = load_initial_config();
    let original_config = config.clone();

    if loaded_from_disk {
        println!("\x1b[32m[OK] Loaded existing configuration from disk.\x1b[0m\n");
    } else {
        println!("\x1b[33m[Info] No existing configuration found. Using defaults.\x1b[0m\n");
    }

    let menu_options = vec![
        "Colors & Theme".to_string(),
        "Font & Typography".to_string(),
        "Window & Cursor".to_string(),
        "Tabs & Panes".to_string(),
        "General & Shell".to_string(),
        "Reset to Factory Defaults".to_string(),
        "Save & Exit".to_string(),
        "Discard & Exit".to_string(),
    ];

    loop {
        let choice = select_menu("Main Menu -- Select a category:", &menu_options, 0);

        match choice {
            0 => menu_colors(&mut config)?,
            1 => menu_font(&mut config)?,
            2 => menu_window(&mut config)?,
            3 => menu_tabs_panes(&mut config)?,
            4 => menu_general(&mut config)?,
            5 => {
                if prompt_bool("Reset all settings to factory defaults?", false) {
                    config = default_config();
                }
            }
            6 => {
                save_and_exit(&config)?;
                break;
            }
            7 => {
                if config != original_config {
                    if prompt_bool("You have unsaved changes. Discard and exit?", false) {
                        break;
                    }
                } else {
                    break;
                }
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

fn menu_colors(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.colors.is_none() {
        config.colors = default_config().colors;
    }
    let colors = config.colors.as_mut().unwrap();

    loop {
        let color_items = [
            (
                "Foreground",
                colors.default_fg.as_deref().unwrap_or("#d4d4d4"),
            ),
            (
                "Background",
                colors.default_bg.as_deref().unwrap_or("#181818"),
            ),
            ("Black", colors.black.as_deref().unwrap_or("#000000")),
            ("Red", colors.red.as_deref().unwrap_or("#cd3131")),
            ("Green", colors.green.as_deref().unwrap_or("#0dbc79")),
            ("Yellow", colors.yellow.as_deref().unwrap_or("#e5e510")),
            ("Blue", colors.blue.as_deref().unwrap_or("#2472c8")),
            ("Magenta", colors.magenta.as_deref().unwrap_or("#bc3fbc")),
            ("Cyan", colors.cyan.as_deref().unwrap_or("#11a8cd")),
            ("White", colors.white.as_deref().unwrap_or("#e5e5e5")),
            (
                "Bright Black",
                colors.bright_black.as_deref().unwrap_or("#666666"),
            ),
            (
                "Bright Red",
                colors.bright_red.as_deref().unwrap_or("#f14c4c"),
            ),
            (
                "Bright Green",
                colors.bright_green.as_deref().unwrap_or("#23d18b"),
            ),
            (
                "Bright Yellow",
                colors.bright_yellow.as_deref().unwrap_or("#f5f543"),
            ),
            (
                "Bright Blue",
                colors.bright_blue.as_deref().unwrap_or("#3b8eea"),
            ),
            (
                "Bright Magenta",
                colors.bright_magenta.as_deref().unwrap_or("#d670d6"),
            ),
            (
                "Bright Cyan",
                colors.bright_cyan.as_deref().unwrap_or("#29b8db"),
            ),
            (
                "Bright White",
                colors.bright_white.as_deref().unwrap_or("#ffffff"),
            ),
        ];

        let mut options: Vec<String> = vec![
            "-> Sequential Walkthrough (Edit all 18 colors)".to_string(),
            "Load Preset Theme (Ayu, Catppuccin, Gruvbox, etc.)".to_string(),
        ];

        for (name, hex) in color_items {
            options.push(format!("{:<14} {}", name, render_swatch(hex)));
        }
        options.push("<- Return to Main Menu".to_string());

        let sel = select_menu("Colors & Theme Customizer:", &options, 0);

        if sel == options.len() - 1 {
            break;
        } else if sel == 1 {
            // Load preset theme
            let mut theme_options: Vec<String> =
                THEMES.iter().map(|(n, _)| n.to_string()).collect();
            theme_options.push("<- Cancel".to_string());
            let theme_sel = select_menu("Choose a Theme Preset to load:", &theme_options, 0);
            if theme_sel < THEMES.len() {
                let (_name, toml_str) = THEMES[theme_sel];
                if let Ok(table) = toml::from_str::<toml::Table>(toml_str)
                    && let Some(col_table) = table.get("colors")
                    && let Ok(loaded_colors) = col_table.clone().try_into::<ConfigColors>()
                {
                    *colors = loaded_colors;
                }
            }
        } else if sel == 0 {
            // Sequential Walkthrough
            colors.default_fg = prompt_hex("1. Foreground", colors.default_fg.as_deref(), false);
            colors.default_bg = prompt_hex("2. Background", colors.default_bg.as_deref(), false);
            colors.black = prompt_hex("3. ANSI Black (0)", colors.black.as_deref(), false);
            colors.red = prompt_hex("4. ANSI Red (1)", colors.red.as_deref(), false);
            colors.green = prompt_hex("5. ANSI Green (2)", colors.green.as_deref(), false);
            colors.yellow = prompt_hex("6. ANSI Yellow (3)", colors.yellow.as_deref(), false);
            colors.blue = prompt_hex("7. ANSI Blue (4)", colors.blue.as_deref(), false);
            colors.magenta = prompt_hex("8. ANSI Magenta (5)", colors.magenta.as_deref(), false);
            colors.cyan = prompt_hex("9. ANSI Cyan (6)", colors.cyan.as_deref(), false);
            colors.white = prompt_hex("10. ANSI White (7)", colors.white.as_deref(), false);
            colors.bright_black = prompt_hex(
                "11. Bright Black (8)",
                colors.bright_black.as_deref(),
                false,
            );
            colors.bright_red =
                prompt_hex("12. Bright Red (9)", colors.bright_red.as_deref(), false);
            colors.bright_green = prompt_hex(
                "13. Bright Green (10)",
                colors.bright_green.as_deref(),
                false,
            );
            colors.bright_yellow = prompt_hex(
                "14. Bright Yellow (11)",
                colors.bright_yellow.as_deref(),
                false,
            );
            colors.bright_blue =
                prompt_hex("15. Bright Blue (12)", colors.bright_blue.as_deref(), false);
            colors.bright_magenta = prompt_hex(
                "16. Bright Magenta (13)",
                colors.bright_magenta.as_deref(),
                false,
            );
            colors.bright_cyan =
                prompt_hex("17. Bright Cyan (14)", colors.bright_cyan.as_deref(), false);
            colors.bright_white = prompt_hex(
                "18. Bright White (15)",
                colors.bright_white.as_deref(),
                false,
            );
        } else {
            // Edit specific color
            let color_idx = sel - 2;
            let (name, target) = match color_idx {
                0 => ("Foreground", &mut colors.default_fg),
                1 => ("Background", &mut colors.default_bg),
                2 => ("ANSI Black (0)", &mut colors.black),
                3 => ("ANSI Red (1)", &mut colors.red),
                4 => ("ANSI Green (2)", &mut colors.green),
                5 => ("ANSI Yellow (3)", &mut colors.yellow),
                6 => ("ANSI Blue (4)", &mut colors.blue),
                7 => ("ANSI Magenta (5)", &mut colors.magenta),
                8 => ("ANSI Cyan (6)", &mut colors.cyan),
                9 => ("ANSI White (7)", &mut colors.white),
                10 => ("Bright Black (8)", &mut colors.bright_black),
                11 => ("Bright Red (9)", &mut colors.bright_red),
                12 => ("Bright Green (10)", &mut colors.bright_green),
                13 => ("Bright Yellow (11)", &mut colors.bright_yellow),
                14 => ("Bright Blue (12)", &mut colors.bright_blue),
                15 => ("Bright Magenta (13)", &mut colors.bright_magenta),
                16 => ("Bright Cyan (14)", &mut colors.bright_cyan),
                17 => ("Bright White (15)", &mut colors.bright_white),
                _ => ("Color", &mut colors.default_fg),
            };
            *target = prompt_hex(name, target.as_deref(), false);
        }
    }
    Ok(())
}

fn choose_font_family(current_font: &mut String, filter_mono_only: &mut bool) {
    loop {
        let all_fonts = discover_fonts(*filter_mono_only);
        let count = all_fonts.len();

        let mut font_list = Vec::new();
        font_list.push(format!(
            "[Toggle] Filter: Currently [{}] (Click to switch)",
            if *filter_mono_only {
                "Monospace Only"
            } else {
                "All System Fonts"
            }
        ));
        font_list.push("[Search] Filter fonts by name...".to_string());
        font_list.push("[Custom] Type custom font name manually".to_string());

        for font in &all_fonts {
            font_list.push(font.clone());
        }
        font_list.push("<- Cancel".to_string());

        let prompt = format!("Select Font Family ({} fonts available):", count);
        let font_sel = select_menu(&prompt, &font_list, 3);

        if font_sel == 0 {
            *filter_mono_only = !*filter_mono_only;
            continue;
        } else if font_sel == 1 {
            let query = prompt_text(
                "Enter search keyword (e.g. 'code', 'mono', 'sans', 'nerd')",
                None,
            );
            if !query.is_empty() {
                let filtered: Vec<String> = all_fonts
                    .into_iter()
                    .filter(|f| f.to_lowercase().contains(&query.to_lowercase()))
                    .collect();

                if filtered.is_empty() {
                    continue;
                }

                let mut search_options = filtered;
                search_options.push("<- Cancel".to_string());
                let s_sel = select_menu(
                    &format!("Search results for '{query}':"),
                    &search_options,
                    0,
                );
                if s_sel < search_options.len() - 1 {
                    *current_font = search_options[s_sel].clone();
                    break;
                }
            }
        } else if font_sel == 2 {
            let manual = prompt_text("Enter font family name", Some(current_font));
            if !manual.is_empty() {
                *current_font = manual;
                break;
            }
        } else if font_sel == font_list.len() - 1 {
            break;
        } else {
            *current_font = font_list[font_sel].clone();
            break;
        }
    }
}

fn menu_font(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut filter_mono_only = false; // Default to showing all fonts, with a toggle

    loop {
        let f = &mut config.font;
        let options = vec![
            format!("Font Family:                  {}", f.font_family),
            format!(
                "Filter Monospace Fonts Only:  {}",
                if filter_mono_only {
                    "[ON] (Shows monospace only)"
                } else {
                    "[OFF] (Shows all system fonts)"
                }
            ),
            format!("Font Size:                    {:.1} pt", f.font_size),
            format!(
                "Scale Multiplier:             {:.2}x",
                f.font_scale_multiplier.unwrap_or(1.5)
            ),
            format!(
                "Bold is Bright:               {}",
                if f.bold_is_bright.unwrap_or(true) {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            format!(
                "Line Height:                  {}",
                f.line_height
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "Default (Auto)".to_string())
            ),
            format!(
                "Nerd Font Glyph Size:         {}",
                f.nerd_font_scale
                    .map(|v| format!("{v:.2}x"))
                    .unwrap_or_else(|| "Default (1.00x)".to_string())
            ),
            "<- Return to Main Menu".to_string(),
        ];

        let sel = select_menu("Font & Typography Settings:", &options, 0);

        match sel {
            0 => {
                choose_font_family(&mut f.font_family, &mut filter_mono_only);
            }
            1 => {
                filter_mono_only = !filter_mono_only;
            }
            2 => {
                f.font_size = prompt_stepper_f32("Font Size (pt)", f.font_size, 0.5, 6.0, 72.0);
            }
            3 => {
                let cur = f.font_scale_multiplier.unwrap_or(1.5);
                f.font_scale_multiplier = Some(prompt_stepper_f32(
                    "Font Scale Multiplier",
                    cur,
                    0.1,
                    1.0,
                    3.0,
                ));
            }
            4 => {
                let cur = f.bold_is_bright.unwrap_or(true);
                f.bold_is_bright = Some(prompt_bool(
                    "Render bold text with bright ANSI colors?",
                    cur,
                ));
            }
            5 => {
                if prompt_bool(
                    "Configure custom line height multiplier?",
                    f.line_height.is_some(),
                ) {
                    let cur = f.line_height.unwrap_or(1.0);
                    f.line_height = Some(prompt_stepper_f32(
                        "Line Height Multiplier",
                        cur,
                        0.05,
                        0.5,
                        3.0,
                    ));
                } else {
                    f.line_height = None;
                }
            }
            6 => {
                if prompt_bool(
                    "Configure custom Nerd Font glyph size multiplier?",
                    f.nerd_font_scale.is_some(),
                ) {
                    let cur = f.nerd_font_scale.unwrap_or(1.0);
                    f.nerd_font_scale = Some(prompt_stepper_f32(
                        "Nerd Font Glyph Size",
                        cur,
                        0.05,
                        0.2,
                        3.0,
                    ));
                } else {
                    f.nerd_font_scale = None;
                }
            }
            _ => break,
        }
    }
    Ok(())
}

fn menu_window(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    let is_wayland = is_wayland_session();

    loop {
        let w = &mut config.window;
        let mut options = vec![
            format!(
                "Cursor Shape:          {}",
                w.cursor_shape.as_deref().unwrap_or("block")
            ),
            format!(
                "Cursor Blink:          {}",
                if w.cursor_blink.unwrap_or(false) {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            format!(
                "Cursor Color:          {}",
                w.cursor_color.as_deref().unwrap_or("Default / Inverted")
            ),
            format!(
                "Cursor Text Color:     {}",
                w.cursor_text_color.as_deref().unwrap_or("Default")
            ),
            format!(
                "Scroll Multiplier:     {:.1}x",
                w.scroll_multiplier.unwrap_or(5.0)
            ),
            format!(
                "FPS Limit:             {}",
                w.fps_limit
                    .map(|v| format!("{v} FPS"))
                    .unwrap_or_else(|| "Uncapped (None)".to_string())
            ),
            format!("Window Opacity:        {:.2}", w.opacity.unwrap_or(1.0)),
            format!("Window Dimming:        {:.2}", w.window_dim.unwrap_or(0.0)),
            format!(
                "Padding X (Horiz):     {:.1} px",
                w.padding_x.unwrap_or(4.0)
            ),
            format!(
                "Padding Y (Vert):      {:.1} px",
                w.padding_y.unwrap_or(4.0)
            ),
            format!(
                "Scrollback Limit:      {}",
                if w.infinite_scrollback.unwrap_or(false) {
                    "Infinite".to_string()
                } else {
                    format!("{} lines", w.scrollback_limit.unwrap_or(10000))
                }
            ),
            format!(
                "Scroll on Output:      {}",
                if w.scroll_on_output.unwrap_or(true) {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            format!(
                "Scroll on Keystroke:   {}",
                if w.scroll_on_keystroke.unwrap_or(true) {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            format!(
                "Hide Mouse on Typing:  {}",
                if w.hide_mouse_on_typing.unwrap_or(false) {
                    "ON"
                } else {
                    "OFF"
                }
            ),
        ];

        if is_wayland {
            options.push(format!(
                "CSD Window Decoration: {}",
                w.csd_theme.as_deref().unwrap_or("Default")
            ));
        } else {
            options.push("CSD Window Decoration: [N/A on X11 / Server-side WM]".to_string());
        }
        options.push("<- Return to Main Menu".to_string());

        let sel = select_menu("Window & Cursor Settings:", &options, 0);

        match sel {
            0 => {
                let shapes = vec![
                    "Block".to_string(),
                    "Beam".to_string(),
                    "Underline".to_string(),
                    "Hollow".to_string(),
                ];
                let shape_sel = select_menu("Choose Cursor Shape:", &shapes, 0);
                w.cursor_shape = Some(shapes[shape_sel].to_lowercase());
            }
            1 => {
                w.cursor_blink = Some(prompt_bool(
                    "Enable blinking cursor?",
                    w.cursor_blink.unwrap_or(false),
                ));
            }
            2 => {
                w.cursor_color = prompt_hex(
                    "Cursor color (hex or leave blank for default)",
                    w.cursor_color.as_deref(),
                    true,
                );
            }
            3 => {
                w.cursor_text_color = prompt_hex(
                    "Cursor text color (hex or leave blank for default)",
                    w.cursor_text_color.as_deref(),
                    true,
                );
            }
            4 => {
                let cur = w.scroll_multiplier.unwrap_or(5.0) as f32;
                w.scroll_multiplier =
                    Some(prompt_stepper_f32("Mouse Scroll Multiplier", cur, 1.0, 1.0, 20.0) as f64);
            }
            5 => {
                let fps_opts = vec![
                    "Uncapped (None)".to_string(),
                    "60 FPS".to_string(),
                    "120 FPS".to_string(),
                    "144 FPS".to_string(),
                    "165 FPS".to_string(),
                    "240 FPS".to_string(),
                    "Custom Number".to_string(),
                ];
                let fps_sel = select_menu("Select Display FPS Limit:", &fps_opts, 0);
                w.fps_limit = match fps_sel {
                    1 => Some(60),
                    2 => Some(120),
                    3 => Some(144),
                    4 => Some(165),
                    5 => Some(240),
                    6 => {
                        let custom = prompt_text("Enter max FPS (30 to 360)", Some("60"));
                        custom.parse::<u32>().ok()
                    }
                    _ => None,
                };
            }
            6 => {
                let cur = w.opacity.unwrap_or(1.0);
                w.opacity = Some(prompt_stepper_f32("Window Opacity", cur, 0.05, 0.10, 1.0));
            }
            7 => {
                let cur = w.window_dim.unwrap_or(0.0);
                w.window_dim = Some(prompt_stepper_f32(
                    "Unfocused Window Dimming",
                    cur,
                    0.05,
                    0.0,
                    1.0,
                ));
            }
            8 => {
                let cur = w.padding_x.unwrap_or(4.0);
                w.padding_x = Some(prompt_stepper_f32(
                    "Horizontal Padding (px)",
                    cur,
                    2.0,
                    0.0,
                    64.0,
                ));
            }
            9 => {
                let cur = w.padding_y.unwrap_or(4.0);
                w.padding_y = Some(prompt_stepper_f32(
                    "Vertical Padding (px)",
                    cur,
                    2.0,
                    0.0,
                    64.0,
                ));
            }
            10 => {
                let infinite = prompt_bool(
                    "Enable infinite scrollback history?",
                    w.infinite_scrollback.unwrap_or(false),
                );
                w.infinite_scrollback = Some(infinite);
                if !infinite {
                    let cur = w.scrollback_limit.unwrap_or(10000);
                    w.scrollback_limit = Some(prompt_stepper_usize(
                        "Max Scrollback Lines",
                        cur,
                        1000,
                        100,
                        1000000,
                    ));
                }
            }
            11 => {
                w.scroll_on_output = Some(prompt_bool(
                    "Scroll to bottom on new output?",
                    w.scroll_on_output.unwrap_or(true),
                ));
            }
            12 => {
                w.scroll_on_keystroke = Some(prompt_bool(
                    "Scroll to bottom on key press?",
                    w.scroll_on_keystroke.unwrap_or(true),
                ));
            }
            13 => {
                w.hide_mouse_on_typing = Some(prompt_bool(
                    "Hide mouse cursor while typing?",
                    w.hide_mouse_on_typing.unwrap_or(false),
                ));
            }
            14 if is_wayland => {
                let csd_opts = vec![
                    "Dark Preset".to_string(),
                    "Light Preset".to_string(),
                    "Adwaita".to_string(),
                    "Breeze".to_string(),
                    "Custom Hex Colors".to_string(),
                    "Disable Custom CSD".to_string(),
                ];
                let csd_sel = select_menu("Wayland Client-Side Decorations (CSD):", &csd_opts, 0);
                match csd_sel {
                    0 => {
                        w.csd_theme = Some("dark".into());
                        w.decorations = None;
                    }
                    1 => {
                        w.csd_theme = Some("light".into());
                        w.decorations = None;
                    }
                    2 => {
                        w.csd_theme = Some("Adwaita".into());
                        w.decorations = None;
                    }
                    3 => {
                        w.csd_theme = Some("Breeze".into());
                        w.decorations = None;
                    }
                    4 => {
                        let mut dec = w.decorations.clone().unwrap_or_default();
                        dec.bg = prompt_hex("Titlebar Background", dec.bg.as_deref(), true);
                        dec.fg = prompt_hex("Titlebar Text Color", dec.fg.as_deref(), true);
                        dec.inactive_bg = prompt_hex(
                            "Inactive Titlebar Background",
                            dec.inactive_bg.as_deref(),
                            true,
                        );
                        dec.inactive_fg = prompt_hex(
                            "Inactive Titlebar Text Color",
                            dec.inactive_fg.as_deref(),
                            true,
                        );
                        dec.border = prompt_hex("Border Color", dec.border.as_deref(), true);
                        dec.close_fg =
                            prompt_hex("Close Button Color", dec.close_fg.as_deref(), true);
                        w.decorations = Some(dec);
                        w.csd_theme = Some("custom".into());
                    }
                    _ => {
                        w.csd_theme = None;
                        w.decorations = None;
                    }
                }
            }
            _ => break,
        }
    }
    Ok(())
}

fn menu_tabs_panes(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let t = &mut config.tabs;
        let p = &mut config.panes;

        let options = vec![
            format!("Tab Bar Visibility:       {:?}", t.show_tab_bar),
            format!(
                "Tab Bar Height:           {}",
                t.tab_bar_height
                    .map(|v| format!("{v:.1} px"))
                    .unwrap_or_else(|| "Default (Auto)".to_string())
            ),
            format!(
                "Tab Font Size:            {}",
                t.font_size
                    .map(|v| format!("{v:.1} pt"))
                    .unwrap_or_else(|| "Default (Inherit terminal font)".to_string())
            ),
            format!(
                "Tab Close Button:         {}",
                if t.show_close_button { "ON" } else { "OFF" }
            ),
            format!(
                "New Tab Button:           {}",
                if t.show_new_tab_button { "ON" } else { "OFF" }
            ),
            format!(
                "Tab Accent Color:         {}",
                t.tab_accent_color.as_deref().unwrap_or("Default")
            ),
            format!(
                "Panes / Splits Enabled:   {}",
                if p.enabled { "ON" } else { "OFF" }
            ),
            format!("Pane Separator Size:      {:.1} px", p.separator_size),
            format!("Pane Min Columns:         {}", p.minimum_columns),
            format!("Pane Min Rows:            {}", p.minimum_rows),
            format!(
                "Pane Separator Color:     {}",
                p.separator_color.as_deref().unwrap_or("Default")
            ),
            format!(
                "Pane Active Sep Color:    {}",
                p.active_separator_color.as_deref().unwrap_or("Default")
            ),
            format!(
                "Pane Horizontal Padding:  {}",
                p.padding_x
                    .map(|v| format!("{v:.1} px"))
                    .unwrap_or_else(|| "Default (Inherit window)".to_string())
            ),
            format!(
                "Pane Vertical Padding:    {}",
                p.padding_y
                    .map(|v| format!("{v:.1} px"))
                    .unwrap_or_else(|| "Default (Inherit window)".to_string())
            ),
            "<- Return to Main Menu".to_string(),
        ];

        let sel = select_menu("Tabs & Panes Configuration:", &options, 0);

        match sel {
            0 => {
                let vis = vec![
                    "Auto".to_string(),
                    "Always".to_string(),
                    "Never".to_string(),
                ];
                let vis_sel = select_menu("Choose Tab Bar Visibility:", &vis, 0);
                t.show_tab_bar = match vis_sel {
                    1 => TabBarVisibility::Always,
                    2 => TabBarVisibility::Never,
                    _ => TabBarVisibility::Auto,
                };
            }
            1 => {
                if prompt_bool("Set custom tab bar height?", t.tab_bar_height.is_some()) {
                    let cur = t.tab_bar_height.unwrap_or(28.0);
                    t.tab_bar_height = Some(prompt_stepper_f32(
                        "Tab Bar Height (px)",
                        cur,
                        1.0,
                        16.0,
                        64.0,
                    ));
                } else {
                    t.tab_bar_height = None;
                }
            }
            2 => {
                if prompt_bool(
                    "Configure custom tab font size (independent from terminal)?",
                    t.font_size.is_some(),
                ) {
                    let cur = t.font_size.unwrap_or(config.font.font_size);
                    t.font_size = Some(prompt_stepper_f32(
                        "Tab Font Size (pt)",
                        cur,
                        0.5,
                        6.0,
                        48.0,
                    ));
                } else {
                    t.font_size = None;
                }
            }
            3 => {
                t.show_close_button =
                    prompt_bool("Show close button on tabs?", t.show_close_button);
            }
            4 => {
                t.show_new_tab_button =
                    prompt_bool("Show '+' button to spawn new tabs?", t.show_new_tab_button);
            }
            5 => {
                t.tab_accent_color = prompt_hex(
                    "Tab accent color (hex or leave blank for default)",
                    t.tab_accent_color.as_deref(),
                    true,
                );
            }
            6 => {
                p.enabled = prompt_bool("Enable multi-pane window splits?", p.enabled);
            }
            7 => {
                p.separator_size = prompt_stepper_f32(
                    "Split Separator Thickness (px)",
                    p.separator_size,
                    1.0,
                    1.0,
                    10.0,
                );
            }
            8 => {
                p.minimum_columns =
                    prompt_stepper_usize("Minimum columns per pane", p.minimum_columns, 5, 10, 80);
            }
            9 => {
                p.minimum_rows =
                    prompt_stepper_usize("Minimum rows per pane", p.minimum_rows, 1, 2, 40);
            }
            10 => {
                p.separator_color = prompt_hex(
                    "Inactive separator color (hex or blank)",
                    p.separator_color.as_deref(),
                    true,
                );
            }
            11 => {
                p.active_separator_color = prompt_hex(
                    "Active separator color (hex or blank)",
                    p.active_separator_color.as_deref(),
                    true,
                );
            }
            12 => {
                if prompt_bool(
                    "Set custom per-pane horizontal padding?",
                    p.padding_x.is_some(),
                ) {
                    let cur = p
                        .padding_x
                        .unwrap_or(config.window.padding_x.unwrap_or(4.0));
                    p.padding_x = Some(prompt_stepper_f32(
                        "Pane Horizontal Padding (px)",
                        cur,
                        2.0,
                        0.0,
                        64.0,
                    ));
                } else {
                    p.padding_x = None;
                }
            }
            13 => {
                if prompt_bool(
                    "Set custom per-pane vertical padding?",
                    p.padding_y.is_some(),
                ) {
                    let cur = p
                        .padding_y
                        .unwrap_or(config.window.padding_y.unwrap_or(4.0));
                    p.padding_y = Some(prompt_stepper_f32(
                        "Pane Vertical Padding (px)",
                        cur,
                        2.0,
                        0.0,
                        64.0,
                    ));
                } else {
                    p.padding_y = None;
                }
            }
            _ => break,
        }
    }
    Ok(())
}

fn menu_general(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    let default_shell_env = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    loop {
        let options = vec![
            format!(
                "Default Shell:       {}",
                config.shell.as_deref().unwrap_or(&default_shell_env)
            ),
            format!(
                "Working Directory:   {}",
                config
                    .working_directory
                    .as_deref()
                    .unwrap_or("Inherited (Current Working Directory)")
            ),
            format!(
                "Application Title:   {}",
                config.app_title.as_deref().unwrap_or("Velox")
            ),
            format!(
                "Single Instance IPC: {}",
                if config.single_instance.unwrap_or(true) {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            "<- Return to Main Menu".to_string(),
        ];

        let sel = select_menu("General & System Settings:", &options, 0);

        match sel {
            0 => {
                let current = config.shell.as_deref().unwrap_or(&default_shell_env);
                let input = prompt_text("Default startup shell command/path", Some(current));
                config.shell = if input.is_empty() { None } else { Some(input) };
            }
            1 => {
                let current = config.working_directory.as_deref().unwrap_or("");
                let input = prompt_text(
                    "Startup working directory path (leave empty for inherited cwd)",
                    Some(current),
                );
                config.working_directory = if input.is_empty() { None } else { Some(input) };
            }
            2 => {
                let current = config.app_title.as_deref().unwrap_or("Velox");
                let input = prompt_text("Window & Application Title", Some(current));
                config.app_title = if input.is_empty() { None } else { Some(input) };
            }
            3 => {
                let cur = config.single_instance.unwrap_or(true);
                config.single_instance = Some(prompt_bool(
                    "Route new window/tab requests through running single instance?",
                    cur,
                ));
            }
            _ => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_rgb() {
        assert_eq!(parse_hex_rgb("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_rgb("000000"), Some((0, 0, 0)));
        assert_eq!(parse_hex_rgb("#ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex_rgb("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_rgb("#000"), Some((0, 0, 0)));
        assert_eq!(parse_hex_rgb("invalid"), None);
        assert_eq!(parse_hex_rgb("#12345"), None);
    }

    #[test]
    fn test_render_swatch() {
        let valid = render_swatch("#ffffff");
        assert!(valid.contains("255;255;255"));
        let invalid = render_swatch("invalid");
        assert!(invalid.contains("[--]"));
    }

    #[test]
    fn test_system_fonts_discovery() {
        let mono_fonts = discover_fonts(true);
        assert!(!mono_fonts.is_empty());
        let all_fonts = discover_fonts(false);
        assert!(all_fonts.len() >= mono_fonts.len());
    }

    #[test]
    fn test_nerd_font_scale_config() {
        let mut cfg = default_config();
        assert_eq!(cfg.font.nerd_font_scale, None);
        assert_eq!(cfg.nerd_font_scale(), None);
        cfg.font.nerd_font_scale = Some(1.25);
        assert_eq!(cfg.nerd_font_scale(), Some(1.25));
    }
}
