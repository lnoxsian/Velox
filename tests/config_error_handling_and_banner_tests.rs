use std::sync::Arc;
use velox::app::app::{AlertBanner, WindowState};
use velox::config::loader::{DiagnosticSeverity, load_from_str};
use velox::pty::master::PtyMaster;
use velox::pty::process::resolve_executable_shell;
use velox::window::event::{ElementState, Key, ModifiersState, MouseButton, NamedKey};

#[test]
fn test_config_syntax_error_generates_diagnostic_with_line_and_col() {
    let broken_toml = r##"
[font]
font_family = "Ubuntu
font_size = 14.0
"##;
    let (cfg, diags) = load_from_str(broken_toml);
    assert!(
        !diags.is_empty(),
        "Syntax error must generate at least one diagnostic"
    );
    let err_diag = diags
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .expect("Must have an Error diagnostic for unclosed quote");
    assert!(
        err_diag.line.is_some(),
        "Diagnostic must contain line number"
    );
    assert!(
        err_diag.col.is_some(),
        "Diagnostic must contain column number"
    );

    // Must still produce a usable config (fallback) without panicking
    assert_eq!(cfg.font.font_size, 12.0);
}

#[test]
fn test_config_type_mismatch_retains_valid_sections() {
    let mixed_toml = r##"
[font]
font_size = "not_a_number"

[colors]
default_fg = "#abcdef"
default_bg = "#123456"

[window]
opacity = 0.85
"##;
    let (cfg, diags) = load_from_str(mixed_toml);
    assert!(
        !diags.is_empty(),
        "Type mismatch must generate an Error diagnostic"
    );

    // Broken section [font] falls back safely
    assert_eq!(cfg.font_size(), 12.0);

    // Valid section [colors] must be retained through resilient recovery!
    assert_eq!(cfg.default_fg(), Some("#abcdef"));
    assert_eq!(cfg.default_bg(), Some("#123456"));

    // Valid section [window] must be retained
    assert_eq!(cfg.opacity(), 0.85);
}

#[test]
fn test_config_semantic_validation_warnings() {
    let semantic_toml = r##"
[font]
font_size = -5.0
font_scale_multiplier = 0.0

[window]
opacity = 3.5
window_dim = -0.5
scroll_multiplier = -2.0
cursor_shape = "pyramid"
cursor_color = "not_a_hex_color"

[colors]
default_bg = "superblue"
"##;
    let (_cfg, diags) = load_from_str(semantic_toml);

    let warnings: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .collect();
    assert!(
        warnings.len() >= 5,
        "Expected at least 5 semantic warnings, got {}: {:?}",
        warnings.len(),
        warnings
    );

    let has_font_size_warn = warnings.iter().any(|w| w.message.contains("font_size"));
    let has_opacity_warn = warnings.iter().any(|w| w.message.contains("Opacity"));
    let has_shape_warn = warnings.iter().any(|w| w.message.contains("cursor_shape"));
    let has_cursor_color_warn = warnings.iter().any(|w| w.message.contains("cursor_color"));
    let has_color_warn = warnings
        .iter()
        .any(|w| w.message.contains("colors.default_bg"));

    assert!(has_font_size_warn, "Missing font_size warning");
    assert!(has_opacity_warn, "Missing opacity warning");
    assert!(has_shape_warn, "Missing cursor_shape warning");
    assert!(has_cursor_color_warn, "Missing cursor_color warning");
    assert!(has_color_warn, "Missing colors.default_bg warning");
}

#[test]
fn test_shell_resolution_fallback_on_nonexistent_binary() {
    let resolved = resolve_executable_shell("/usr/bin/this_shell_binary_does_not_exist_xyz123");
    assert!(
        std::path::Path::new(&resolved).exists(),
        "Resolved shell fallback '{}' must exist on disk",
        resolved
    );
}

#[test]
fn test_alert_banner_layout_adjustment_and_dismiss_via_escape() {
    let pty = Arc::new(PtyMaster::dummy());
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    let (initial_cols, initial_rows) = ws.recalculate_grid_size();

    // Attach an alert banner
    ws.banner = Some(AlertBanner {
        severity: DiagnosticSeverity::Error,
        message: "Config error in ~/.config/velox/config.toml: line 5: syntax error".to_string(),
        dismiss_hovered: false,
    });

    assert_eq!(ws.banner_height(), 26.0);
    let (banner_cols, banner_rows) = ws.recalculate_grid_size();
    assert_eq!(
        banner_cols, initial_cols,
        "Columns must not change with banner"
    );
    assert!(
        banner_rows <= initial_rows,
        "Rows must accommodate banner height (initial: {}, with banner: {})",
        initial_rows,
        banner_rows
    );

    // Pressing Escape must dismiss the banner and restore height
    let consumed =
        ws.handle_keyboard_input(&Key::Named(NamedKey::Escape), None, ModifiersState::empty());
    assert!(consumed, "Escape key must be consumed to dismiss banner");
    assert!(ws.banner.is_none(), "Banner must be cleared after Escape");
    assert_eq!(ws.banner_height(), 0.0);
    let (restored_cols, restored_rows) = ws.recalculate_grid_size();
    assert_eq!(restored_cols, initial_cols);
    assert_eq!(restored_rows, initial_rows);
}

#[test]
fn test_alert_banner_dismiss_via_mouse_click() {
    let pty = Arc::new(PtyMaster::dummy());
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    ws.banner = Some(AlertBanner {
        severity: DiagnosticSeverity::Warning,
        message: "Warning in config".to_string(),
        dismiss_hovered: false,
    });

    let csd_h = ws.window.csd_title_bar_height() as f64;
    let tab_bar_h = ws.tab_bar_height() as f64;
    let banner_click_y = csd_h + tab_bar_h + 10.0;

    // Set mouse position and send left click on banner
    ws.mouse_x = ws.window.width() as f64 - 15.0;
    ws.mouse_y = banner_click_y;
    ws.handle_mouse_input(
        ElementState::Pressed,
        MouseButton::Left,
        ModifiersState::empty(),
    );

    assert!(
        ws.banner.is_none(),
        "Banner must be cleared after mouse click"
    );
    assert_eq!(ws.banner_height(), 0.0);
}

#[test]
fn test_default_config_produces_zero_diagnostics() {
    let default_cfg = velox::config::defaults::default_config();
    let diags = default_cfg.validate();
    assert!(
        diags.is_empty(),
        "default_config() must produce 0 validation diagnostics, got: {:?}",
        diags
    );
}

#[test]
fn test_default_config_toml_produces_zero_diagnostics() {
    let default_toml = r##"
single_instance = true

[font]
font_family = "Nerd Font"
font_scale_multiplier = 1.5
bold_is_bright = true

[window]
scrollback_limit = 2000
infinite_scrollback = true
scroll_multiplier = 2.5
padding_x = 4.0
padding_y = 4.0
cursor_shape = "block"
cursor_blink = false
cursor_color = "default"
cursor_text_color = "default"
scroll_on_output = false
scroll_on_keystroke = true
opacity = 1.0
window_dim = 0.2
hide_mouse_on_typing = true

[tabs]
show_tab_bar = "Auto"
show_close_button = true
show_new_tab_button = false
tab_accent_color = "blue"

[panes]
enabled = true
separator_size = 2.0
minimum_columns = 20
minimum_rows = 5

[colors]
default_fg = "#ebdbb2"
default_bg = "#282828"
black = "#282828"
red = "#cc241d"
green = "#98971a"
yellow = "#d79921"
blue = "#458588"
magenta = "#b16286"
cyan = "#689d6a"
white = "#a89984"
bright_black = "#928374"
bright_red = "#fb4934"
"##;
    let (_cfg, diags) = load_from_str(default_toml);
    assert!(
        diags.is_empty(),
        "Default-style configuration must produce 0 diagnostics, got: {:?}",
        diags
    );
}
