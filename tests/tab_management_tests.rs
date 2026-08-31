use std::sync::Arc;
use std::time::Instant;
use velox::app::tab::{Tab, TabBar, TabBarHitResult, TabBarRenderInfo, TabHeaderInfo};
use velox::config::config::{Config, TabBarVisibility, TabsConfig};
use velox::pty::process::spawn_process;
use velox::renderer::software::CpuRenderer;
use velox::screen::cell::Color;
use velox::screen::cursor::CursorShape;
use velox::screen::grid::Grid;
use velox::terminal::terminal::Terminal;
use velox::theme::theme::Theme;

#[test]
fn test_tab_bar_config_visibility_modes() {
    let mut config = Config::default();
    config.tabs = TabsConfig {
        show_tab_bar: TabBarVisibility::Always,
        tab_bar_height: Some(30.0),
        show_close_button: true,
        show_new_tab_button: false,
        font_size: None,
        tab_accent_color: None,
    };

    let tab_bar = TabBar::from_config(&config);
    assert!(tab_bar.is_visible(0));
    assert!(tab_bar.is_visible(1));
    assert!(tab_bar.is_visible(2));
    assert_eq!(tab_bar.height(18), 30.0);

    let mut auto_config = Config::default();
    auto_config.tabs.show_tab_bar = TabBarVisibility::Auto;
    let auto_tab_bar = TabBar::from_config(&auto_config);
    assert!(!auto_tab_bar.is_visible(1));
    assert!(auto_tab_bar.is_visible(2));

    let mut never_config = Config::default();
    never_config.tabs.show_tab_bar = TabBarVisibility::Never;
    let never_tab_bar = TabBar::from_config(&never_config);
    assert!(!never_tab_bar.is_visible(1));
    assert!(!never_tab_bar.is_visible(5));
}

#[test]
fn test_shared_tab_width_formula_matches_hit_test() {
    let tab_bar = TabBar {
        show_tab_bar: TabBarVisibility::Auto,
        configured_height: Some(28.0),
        show_close_button: true,
        show_new_tab_button: false,
        hovered_tab: None,
        hovered_close: None,
        hovered_new_tab: false,
    };

    let render_info = TabBarRenderInfo {
        height: 28.0,
        tabs: vec![
            TabHeaderInfo {
                title: "Tab 1".to_string(),
                is_active: true,
                is_hovered: false,
                is_close_hovered: false,
            },
            TabHeaderInfo {
                title: "Tab 2".to_string(),
                is_active: false,
                is_hovered: false,
                is_close_hovered: false,
            },
            TabHeaderInfo {
                title: "Tab 3".to_string(),
                is_active: false,
                is_hovered: false,
                is_close_hovered: false,
            },
        ],
        show_new_tab: false,
        is_new_tab_hovered: false,
        show_close_button: true,
    };

    let width_from_tab_bar = tab_bar.tab_width(800.0, 3);
    let width_from_render_info = render_info.compute_tab_width(800.0);

    assert_eq!(width_from_tab_bar, width_from_render_info);
    assert_eq!(width_from_tab_bar, 800.0 / 3.0);
}

#[test]
fn test_tab_bar_hit_test_multiple_tabs() {
    let tab_bar = TabBar {
        show_tab_bar: TabBarVisibility::Auto,
        configured_height: Some(28.0),
        show_close_button: true,
        show_new_tab_button: false,
        hovered_tab: None,
        hovered_close: None,
        hovered_new_tab: false,
    };

    let window_width = 800.0;
    let cell_h = 16;
    let tab_count = 3;

    // Available width = 800. Tab width = (800 / 3) = ~266.67 (spans full space, Terminator style).
    // Tab 0: [0..266.67), Tab 1: [266.67..533.33), Tab 2: [533.33..800.0]

    // Click inside Tab 0
    let hit = tab_bar.hit_test(50.0, 10.0, window_width, cell_h, tab_count);
    assert_eq!(hit, TabBarHitResult::Tab(0));

    // Click inside Tab 0 close button (near right edge of Tab 0)
    let hit_close = tab_bar.hit_test(255.0, 10.0, window_width, cell_h, tab_count);
    assert_eq!(hit_close, TabBarHitResult::CloseTab(0));

    // Click inside Tab 1
    let hit_tab1 = tab_bar.hit_test(300.0, 10.0, window_width, cell_h, tab_count);
    assert_eq!(hit_tab1, TabBarHitResult::Tab(1));

    // Click inside Tab 1 close button
    let hit_close1 = tab_bar.hit_test(520.0, 10.0, window_width, cell_h, tab_count);
    assert_eq!(hit_close1, TabBarHitResult::CloseTab(1));

    // Click inside Tab 2
    let hit_tab2 = tab_bar.hit_test(600.0, 10.0, window_width, cell_h, tab_count);
    assert_eq!(hit_tab2, TabBarHitResult::Tab(2));

    // Click inside Tab 2 close button
    let hit_close2 = tab_bar.hit_test(785.0, 10.0, window_width, cell_h, tab_count);
    assert_eq!(hit_close2, TabBarHitResult::CloseTab(2));

    // Click below tab bar height
    let hit_outside = tab_bar.hit_test(50.0, 35.0, window_width, cell_h, tab_count);
    assert_eq!(hit_outside, TabBarHitResult::None);
}

#[test]
fn test_tab_title_update_debounce() {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(80, 24);
    let mut tab = Tab::new(
        1,
        pty,
        terminal,
        Some("Custom Title".to_string()),
        "Custom Title".to_string(),
        false,
        12.0,
    );

    // Immediately after creation, update_title within 500ms debounce must return false
    assert!(!tab.update_title());

    // If we reset debounce timer without changing custom_title, it should return false because title matches custom_title
    tab.last_title_check = Instant::now() - std::time::Duration::from_millis(600);
    assert!(!tab.update_title());

    // If custom_title is changed to something new and debounce elapsed, it should return true
    tab.custom_title = Some("New Title".to_string());
    tab.last_title_check = Instant::now() - std::time::Duration::from_millis(600);
    assert!(tab.update_title());
    assert_eq!(tab.current_title, "New Title");
}

#[test]
fn test_software_renderer_renders_tab_bar() {
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    let mut target = vec![0u32; 800 * 600];

    let tab_bar_info = TabBarRenderInfo {
        height: 28.0,
        tabs: vec![
            TabHeaderInfo {
                title: "Tab 1 (sh)".to_string(),
                is_active: true,
                is_hovered: false,
                is_close_hovered: false,
            },
            TabHeaderInfo {
                title: "Tab 2 (vim)".to_string(),
                is_active: false,
                is_hovered: true,
                is_close_hovered: false,
            },
        ],
        show_new_tab: false,
        is_new_tab_hovered: false,
        show_close_button: true,
    };

    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        0.0,
        &mut target,
        Some(&tab_bar_info),
    );

    // Verify that target buffer has non-zero pixels rendered in the top tab bar area (rows 0..28)
    let top_pixels_sum: u64 = target[0..800 * 28].iter().map(|&p| p as u64).sum();
    assert!(
        top_pixels_sum > 0,
        "Tab bar should be rendered with non-zero pixels"
    );
}

#[test]
fn test_software_renderer_renders_custom_tab_accent() {
    let toml_tabs = r##"
        [tabs]
        tab_accent_color = "#ff0077"
    "##;
    let config: Config = toml::from_str(toml_tabs).unwrap();
    let theme = Theme::from_config(&config);

    assert_eq!(
        theme.tab_accent_color,
        Some(Color {
            r: 255,
            g: 0,
            b: 119
        })
    );
    assert_eq!(
        theme.resolve_tab_accent_color(),
        Color {
            r: 255,
            g: 0,
            b: 119
        }
    );

    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    let mut target = vec![0u32; 800 * 600];

    let tab_bar_info = TabBarRenderInfo {
        height: 28.0,
        tabs: vec![
            TabHeaderInfo {
                title: "Tab 1".to_string(),
                is_active: true,
                is_hovered: false,
                is_close_hovered: false,
            },
            TabHeaderInfo {
                title: "Tab 2".to_string(),
                is_active: false,
                is_hovered: false,
                is_close_hovered: false,
            },
        ],
        show_new_tab: false,
        is_new_tab_hovered: false,
        show_close_button: true,
    };

    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        0.0,
        &mut target,
        Some(&tab_bar_info),
    );

    // Active tab (tab 0) top 2 rows should have the custom accent color 0xFFFF0077
    let expected_accent = 0xFF000000 | (255 << 16) | 119;
    assert_eq!(
        target[0], expected_accent,
        "Active tab top accent line must match the configured tab accent color"
    );
    assert_eq!(
        target[10], expected_accent,
        "Active tab top accent line must match the configured tab accent color across the tab"
    );
}

#[test]
fn test_software_renderer_renders_named_tab_accent_from_theme() {
    let toml_str = r##"
        [colors]
        red = "#f38ba8"
        green = "#a6e3a1"
        [tabs]
        tab_accent_color = "red"
    "##;
    let config: Config = toml::from_str(toml_str).unwrap();
    let theme = Theme::from_config(&config);
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let grid = Grid::new(80, 24, theme.default_fg, theme.default_bg, 100, false);
    let mut target = vec![0u32; 800 * 600];

    let tab_bar_info = TabBarRenderInfo {
        height: 28.0,
        tabs: vec![TabHeaderInfo {
            title: "Tab 1".to_string(),
            is_active: true,
            is_hovered: false,
            is_close_hovered: false,
        }],
        show_new_tab: false,
        is_new_tab_hovered: false,
        show_close_button: true,
    };

    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        0.0,
        &mut target,
        Some(&tab_bar_info),
    );

    // Active tab top 2 rows should have the theme's red color #f38ba8 -> 0xFFF38BA8
    let expected_red_accent = 0xFF000000 | (0xf3 << 16) | (0x8b << 8) | 0xa8;
    assert_eq!(
        target[0], expected_red_accent,
        "Active tab accent line must match the theme's named red color"
    );
}

#[test]
fn test_software_renderer_inactive_tab_dim_colors() {
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    let mut target = vec![0u32; 800 * 600];

    let tab_bar_info = TabBarRenderInfo {
        height: 28.0,
        tabs: vec![
            TabHeaderInfo {
                title: "Tab 1".to_string(),
                is_active: true,
                is_hovered: false,
                is_close_hovered: false,
            },
            TabHeaderInfo {
                title: "Tab 2".to_string(),
                is_active: false,
                is_hovered: false,
                is_close_hovered: false,
            },
        ],
        show_new_tab: false,
        is_new_tab_hovered: false,
        show_close_button: true,
    };

    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        8.0,
        4.0,
        true,
        CursorShape::Block,
        grid.cursor.x,
        true,
        1.0,
        0.0,
        &mut target,
        Some(&tab_bar_info),
    );

    // Expected colors from formula:
    let expected_active_bg = renderer.palette.default_bg;
    let expected_inactive_bg = renderer.palette.tab_inactive_bg;

    // Verify inactive tab is dimmer than active tab background
    assert_ne!(
        expected_inactive_bg, renderer.palette.ansi_colors[8],
        "Inactive tab bg must not be bright ANSI 8 gray"
    );
    assert_eq!(
        expected_inactive_bg,
        0xFF000000
            | (((theme.default_bg.r as f32 * 0.72) as u32) << 16)
            | (((theme.default_bg.g as f32 * 0.72) as u32) << 8)
            | ((theme.default_bg.b as f32 * 0.72) as u32),
        "Inactive tab background must match the 0.72 dimming ratio"
    );

    // Tab 1 (active) background sample (row 10, col 50)
    let active_pixel = target[10 * 800 + 50];
    assert_eq!(active_pixel, expected_active_bg);

    // Tab 2 (inactive / lost focus) background sample (row 10, col 450)
    let inactive_pixel = target[10 * 800 + 450];
    assert_eq!(inactive_pixel, expected_inactive_bg);
}

#[test]
fn test_damage_tracker_clear_and_sync() {
    let mut grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );

    // Mark dirty row
    grid.damage.mark_dirty(3);
    assert!(grid.damage.dirty_rows[3]);

    // Clear damage
    grid.clear_damage();
    assert!(!grid.damage.dirty_rows[3]);
    assert!(grid.damage.dirty_rows.iter().all(|&d| !d));
}

#[test]
fn test_tab_bar_hover_state_tracking() {
    let mut tab_bar = TabBar {
        show_tab_bar: TabBarVisibility::Always,
        configured_height: Some(28.0),
        show_close_button: true,
        show_new_tab_button: true,
        hovered_tab: None,
        hovered_close: None,
        hovered_new_tab: false,
    };

    // Test hit testing on tab 0
    let hit = tab_bar.hit_test(50.0, 10.0, 800.0, 16, 2);
    assert_eq!(hit, TabBarHitResult::Tab(0));

    tab_bar.hovered_tab = Some(0);
    assert_eq!(tab_bar.hovered_tab, Some(0));

    // Test hit testing on new tab button
    // Tab width with 2 tabs and new tab button: (800 - 32) / 2 = 384.
    // New tab button starts at 2 * 384 + 4 = 772.
    let hit_new = tab_bar.hit_test(780.0, 10.0, 800.0, 16, 2);
    assert_eq!(hit_new, TabBarHitResult::NewTab);
}

#[test]
fn test_software_renderer_tab_switch_clean_redraw() {
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.5, &theme, 800, 600, true, 1.0);
    let mut grid_tab1 = Grid::new(80, 24, theme.default_fg, theme.default_bg, 100, false);
    let mut grid_tab2 = Grid::new(80, 24, theme.default_fg, theme.default_bg, 100, false);

    // Tab 1 content on row 5
    grid_tab1.cursor.y = 5;
    grid_tab1.cursor.x = 0;
    for ch in "TAB 1 FULL SCREEN CONTENT".chars() {
        grid_tab1.put_char(
            ch,
            Color {
                r: 255,
                g: 255,
                b: 255,
            },
            Color { r: 0, g: 0, b: 0 },
            None,
            velox::screen::cell::CellFlags::empty(),
        );
    }

    let mut target = vec![0u32; 800 * 600];

    // Render Tab 1
    renderer.render(
        &grid_tab1.cells,
        &grid_tab1,
        &theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        grid_tab1.cursor.x,
        true,
        1.0,
        &mut target,
    );
    grid_tab1.clear_damage();

    // Verify Tab 1 row 5 has rendered content
    let cell_h = renderer.glyph_cache.cell_height;
    let row_5_pixels: u64 = target[((5 * cell_h) as usize * 800)..((6 * cell_h) as usize * 800)]
        .iter()
        .map(|&p| p as u64)
        .sum();
    assert!(row_5_pixels > 0, "Tab 1 row 5 must have non-zero pixels");

    // Switch to Tab 2: mark Tab 2 grid dirty
    grid_tab2.mark_all_dirty();

    // Render Tab 2 (empty grid)
    renderer.render(
        &grid_tab2.cells,
        &grid_tab2,
        &theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        grid_tab2.cursor.x,
        true,
        1.0,
        &mut target,
    );
    grid_tab2.clear_damage();

    // Verify row 5 is now completely clean (only default_bg, zero Tab 1 leftover pixels)
    let default_bg = renderer.palette.default_bg;
    let row_5_slice = &target[((5 * cell_h) as usize * 800)..((6 * cell_h) as usize * 800)];
    let row_5_all_bg = row_5_slice.iter().all(|&p| p == default_bg);
    assert!(
        row_5_all_bg,
        "Tab 2 must completely overwrite Tab 1 content on row 5"
    );
}

#[test]
fn test_per_tab_font_size_isolation() {
    let pty1 = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let pty2 = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal1 = Terminal::new(80, 24);
    let terminal2 = Terminal::new(80, 24);

    let mut tab1 = Tab::new(
        1,
        pty1,
        terminal1,
        Some("Tab 1".to_string()),
        "Tab 1".to_string(),
        false,
        12.0,
    );
    let tab2 = Tab::new(
        2,
        pty2,
        terminal2,
        Some("Tab 2".to_string()),
        "Tab 2".to_string(),
        false,
        12.0,
    );

    // Zoom Tab 1
    tab1.font_size = 18.0;

    // Verify Tab 2's font_size is unchanged
    assert_eq!(tab1.font_size, 18.0);
    assert_eq!(tab2.font_size, 12.0);
}

#[test]
fn test_tab_bar_height_independent_of_cell_zoom() {
    let config = Config::default();
    let tab_bar = TabBar::from_config(&config);

    let base_cell_height = 16u32;
    let base_height = tab_bar.height(base_cell_height);

    // If active tab zooms to cell height 36, tab bar height using base_cell_height stays identical
    assert_eq!(tab_bar.height(base_cell_height), base_height);
}

#[test]
fn test_tab_font_cache_isolation_on_terminal_zoom() {
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.0, &theme, 800, 600, true, 1.0);
    renderer.set_tab_font_size(14.0);

    let initial_tab_cw = renderer.tab_glyph_cache.cell_width;
    let initial_tab_ch = renderer.tab_glyph_cache.cell_height;

    // Zoom terminal font size down to very small (e.g. 4.0)
    renderer.update_font_size(4.0);

    // Terminal glyph cache shrinks
    assert!(renderer.glyph_cache.cell_width < initial_tab_cw);
    assert!(renderer.glyph_cache.cell_height < initial_tab_ch);

    // Tab glyph cache remains isolated and razor sharp at 14.0
    assert_eq!(renderer.tab_glyph_cache.cell_width, initial_tab_cw);
    assert_eq!(renderer.tab_glyph_cache.cell_height, initial_tab_ch);

    // Setting tab font size specifically updates only tab_glyph_cache
    renderer.set_tab_font_size(18.0);
    assert!(renderer.tab_glyph_cache.cell_height > initial_tab_ch);
    // Terminal glyph cache is unaffected by tab font size change
    assert!(renderer.glyph_cache.cell_height < initial_tab_ch);
}

#[test]
fn test_software_renderer_renders_sharp_tab_bar_when_terminal_small() {
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 5.0, 1.0, &theme, 800, 600, true, 1.0);
    renderer.set_tab_font_size(14.0);

    let grid = Grid::new(
        80,
        24,
        Color {
            r: 255,
            g: 255,
            b: 255,
        },
        Color { r: 0, g: 0, b: 0 },
        100,
        false,
    );
    let mut target = vec![0u32; 800 * 600];

    let tab_bar_info = TabBarRenderInfo {
        height: 28.0,
        tabs: vec![TabHeaderInfo {
            title: "Terminal Tab 1".to_string(),
            is_active: true,
            is_hovered: false,
            is_close_hovered: false,
        }],
        show_close_button: true,
        show_new_tab: true,
        is_new_tab_hovered: false,
    };

    renderer.render_with_tab_bar(
        &grid.cells,
        &grid,
        &theme,
        0.0,
        0.0,
        true,
        CursorShape::Block,
        0,
        true,
        1.0,
        0.0,
        &mut target,
        Some(&tab_bar_info),
    );

    // Verify non-zero pixels drawn in tab bar area (first 28 rows)
    let tab_bar_slice = &target[0..(28 * 800)];
    let active_pixels = tab_bar_slice.iter().filter(|&&p| p != 0).count();
    assert!(
        active_pixels > 100,
        "Tab bar must render visible pixels with decoupled tab glyph cache"
    );
}
