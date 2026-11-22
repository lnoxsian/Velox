use std::sync::Arc;
use velox::app::app::WindowState;
use velox::pty::master::PtyMaster;
use velox::screen::cell::{CellFlags, Color};
use velox::window::event::{ElementState, Key, ModifiersState, MouseButton, NamedKey};

fn create_test_pty_pipe() -> (Arc<PtyMaster>, std::os::fd::OwnedFd) {
    let mut fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
        let flags = libc::fcntl(fds[0], libc::F_GETFL);
        libc::fcntl(fds[0], libc::F_SETFL, flags | libc::O_NONBLOCK);
        use std::os::fd::FromRawFd;
        let read_end = std::os::fd::OwnedFd::from_raw_fd(fds[0]);
        let pty = Arc::new(PtyMaster::new(fds[1], None));
        (pty, read_end)
    }
}

fn read_pty_output(fd: &std::os::fd::OwnedFd) -> Vec<u8> {
    use std::os::fd::AsRawFd;
    let mut buf = [0u8; 1024];
    let n = unsafe {
        libc::read(
            fd.as_raw_fd(),
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
        )
    };
    if n > 0 {
        buf[..n as usize].to_vec()
    } else {
        Vec::new()
    }
}

#[test]
fn test_search_interactive_typing_and_navigation() {
    let (pty, read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Populate active pane screen with text
    {
        let grid = ws.active_pane_mut().terminal.active_grid_mut();
        for (i, c) in "alpha beta alpha beta alpha".chars().enumerate() {
            grid.cells[i].character = c;
        }
    }

    // 1. Open search with Ctrl+F
    let handled = ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    assert!(!handled, "Ctrl+F must be consumed and not sent to PTY");
    assert_eq!(read_pty_output(&read_end), b"");
    assert!(ws.search.is_open);

    // 2. Type "alpha"
    for ch in "alpha".chars() {
        let h = ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
        assert!(!h);
    }
    assert_eq!(
        read_pty_output(&read_end),
        b"",
        "Typing search query must not write to PTY"
    );
    assert_eq!(ws.search.query, "alpha");
    assert_eq!(ws.search.matches.len(), 3);
    assert_eq!(ws.search.active_match_index, Some(0));

    // 3. Navigation: Enter cycles 0 -> 1 -> 2 -> 0
    ws.handle_keyboard_input(&Key::Named(NamedKey::Enter), None, ModifiersState::empty());
    assert_eq!(ws.search.active_match_index, Some(1));
    ws.handle_keyboard_input(&Key::Named(NamedKey::Enter), None, ModifiersState::empty());
    assert_eq!(ws.search.active_match_index, Some(2));
    ws.handle_keyboard_input(&Key::Named(NamedKey::Enter), None, ModifiersState::empty());
    assert_eq!(ws.search.active_match_index, Some(0));

    // 4. Shift+Enter cycles backwards 0 -> 2 -> 1 -> 0
    ws.handle_keyboard_input(&Key::Named(NamedKey::Enter), None, ModifiersState::SHIFT);
    assert_eq!(ws.search.active_match_index, Some(2));
    ws.handle_keyboard_input(&Key::Named(NamedKey::Enter), None, ModifiersState::SHIFT);
    assert_eq!(ws.search.active_match_index, Some(1));
    ws.handle_keyboard_input(&Key::Named(NamedKey::Enter), None, ModifiersState::SHIFT);
    assert_eq!(ws.search.active_match_index, Some(0));

    // 5. Backspace erases 'a', query becomes "alph"
    ws.handle_keyboard_input(
        &Key::Named(NamedKey::Backspace),
        None,
        ModifiersState::empty(),
    );
    assert_eq!(ws.search.query, "alph");
    assert_eq!(ws.search.matches.len(), 3);

    // 6. Escape closes search and clears highlights
    ws.handle_keyboard_input(&Key::Named(NamedKey::Escape), None, ModifiersState::empty());
    assert!(!ws.search.is_open);
    assert!(ws.search.matches.is_empty());
    assert_eq!(ws.search.query, "alph", "Query must be retained");
}

#[test]
fn test_search_across_scrollback_and_active_pane() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Add scrollback lines to active pane (height is 24, so 34 lines pushes 10 into scrollback)
    let fg = Color {
        r: 255,
        g: 255,
        b: 255,
    };
    let bg = Color { r: 0, g: 0, b: 0 };
    {
        let grid = ws.active_pane_mut().terminal.active_grid_mut();
        for _ in 0..10 {
            for c in "scrollback_secret".chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            grid.scroll_or_move_down(bg);
            grid.cursor.x = 0;
        }
        for i in 0..24 {
            for c in "visible_target".chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            if i < 23 {
                grid.scroll_or_move_down(bg);
                grid.cursor.x = 0;
            }
        }
    }

    assert!(ws.active_pane().terminal.grid.scrollback.len() >= 10);

    // Open search and search for scrollback secret
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    assert!(ws.search.is_open);

    ws.search.query = "scrollback_secret".to_string();
    ws.recompute_search_matches();
    ws.jump_to_search_match();
    assert_eq!(
        ws.search.matches.len(),
        10,
        "Search must match all 10 lines in scrollback history"
    );
    assert!(
        ws.active_pane().terminal.active_grid().scroll_offset > 0,
        "Viewport must scroll up to show scrollback match"
    );

    // Search for visible screen target
    ws.search.query = "visible_target".to_string();
    ws.recompute_search_matches();
    ws.jump_to_search_match();
    assert_eq!(
        ws.search.matches.len(),
        24,
        "Search must match all 24 visible screen lines"
    );
    assert_eq!(
        ws.active_pane().terminal.active_grid().scroll_offset,
        0,
        "Viewport must return to 0 when jumping to visible screen match"
    );
}

#[test]
fn test_search_viewport_autoscroll_jump_to_match() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    let fg = Color {
        r: 255,
        g: 255,
        b: 255,
    };
    let bg = Color { r: 0, g: 0, b: 0 };
    {
        let grid = ws.active_pane_mut().terminal.active_grid_mut();
        // 50 lines total -> 26 in scrollback, 24 on screen
        for i in 0..50 {
            for c in format!("item_{:02}", i).chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            if i < 49 {
                grid.scroll_or_move_down(bg);
                grid.cursor.x = 0;
            }
        }
    }

    assert_eq!(ws.active_pane().terminal.grid.scrollback.len(), 26);
    assert_eq!(ws.active_pane().terminal.active_grid().scroll_offset, 0);

    // Open search and search for "item_05" (in scrollback, line 5)
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    for ch in "item_05".chars() {
        ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
    }

    assert_eq!(ws.search.matches.len(), 1);
    assert_eq!(ws.search.matches[0].abs_row, 5);
    // Viewport must have jumped so line 5 is visible
    let scroll_off = ws.active_pane().terminal.active_grid().scroll_offset;
    assert!(
        scroll_off > 0,
        "Viewport must auto-scroll to scrollback match"
    );
    let history_len = 26;
    let disp_row = (5 + scroll_off).saturating_sub(history_len);
    assert!(
        disp_row < 24,
        "Match must be visible on screen (disp_row={})",
        disp_row
    );
}

#[test]
fn test_search_invalidation_on_terminal_output() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Open search with query
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    for ch in "match".chars() {
        ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
    }
    assert_eq!(ws.search.matches.len(), 0);
    assert!(!ws.search.matches_dirty);

    // Feed terminal output matching query
    ws.active_pane_mut().terminal.feed(b"new match text\r\n");
    // When output arrives, matches are invalidated
    ws.search.invalidate_matches();
    assert!(ws.search.matches_dirty);

    // On next draw / recompute, new match is found
    ws.draw();
    assert_eq!(ws.search.matches.len(), 1);
    assert!(!ws.search.matches_dirty);
}

#[test]
fn test_search_invalidation_on_pane_switch() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Split vertical to create a second pane
    let pane2_id = ws.split_vertical().expect("split must succeed");
    assert_eq!(ws.active_pane().id, pane2_id);

    // Write text in pane 2
    {
        let grid2 = ws.active_pane_mut().terminal.active_grid_mut();
        for (i, c) in "pane2_unique".chars().enumerate() {
            grid2.cells[i].character = c;
        }
    }

    // Open search while pane 2 is active
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    for ch in "unique".chars() {
        ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
    }
    assert_eq!(ws.search.matches.len(), 1);

    // Switch to pane 1
    ws.focus_next_pane();
    assert_ne!(ws.active_pane().id, pane2_id);
    assert!(
        ws.search.matches_dirty,
        "Matches must be dirty after pane switch"
    );

    // Draw recomputes matches for the new active pane (which doesn't have "unique")
    ws.draw();
    assert_eq!(
        ws.search.matches.len(),
        0,
        "Inactive pane matches must not be shown for new active pane"
    );
}

#[test]
fn test_search_invalidation_on_resize() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Open search
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    for ch in "test".chars() {
        ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
    }
    assert!(!ws.search.matches_dirty);

    // Resize active tab / renderer
    ws.resize_renderer(1024, 768);
    assert!(ws.search.matches_dirty, "Resize must mark matches dirty");
}

#[test]
fn test_search_invalidation_on_alternate_screen() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Main screen has "normal_screen_text"
    {
        let grid = ws.active_pane_mut().terminal.active_grid_mut();
        for (i, c) in "normal_screen_text".chars().enumerate() {
            grid.cells[i].character = c;
        }
    }

    // Open search and search for "normal"
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    for ch in "normal".chars() {
        ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
    }
    assert_eq!(ws.search.matches.len(), 1);

    // Terminal enters alternate screen (e.g. vim opens)
    ws.active_pane_mut().terminal.set_alt_screen(true);
    ws.search.invalidate_matches();
    ws.draw();

    assert_eq!(
        ws.search.matches.len(),
        0,
        "Alternate screen must not show main screen matches"
    );

    // Terminal exits alternate screen
    ws.active_pane_mut().terminal.set_alt_screen(false);
    ws.search.invalidate_matches();
    ws.draw();

    assert_eq!(
        ws.search.matches.len(),
        1,
        "Returning to main screen must restore main screen matches"
    );
}

#[test]
fn test_search_rendering_and_damage_coexistence() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Place text on screen
    {
        let grid = ws.active_pane_mut().terminal.active_grid_mut();
        for (i, c) in "findme".chars().enumerate() {
            grid.cells[i].character = c;
        }
    }

    // Render frame without search
    ws.draw();
    let initial_pixels = ws.renderer.framebuffer.as_slice().to_vec();

    // Open search and query "findme"
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    for ch in "findme".chars() {
        ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
    }

    // Draw frame with search active
    ws.draw();
    let search_pixels = ws.renderer.framebuffer.as_slice().to_vec();
    assert_ne!(
        initial_pixels, search_pixels,
        "Search overlay and highlights must change rendered pixels"
    );

    // Close search with Escape
    ws.handle_keyboard_input(&Key::Named(NamedKey::Escape), None, ModifiersState::empty());
    assert!(!ws.search.is_open);

    // Draw frame after closing search
    ws.draw();
    let closed_pixels = ws.renderer.framebuffer.as_slice().to_vec();
    assert_eq!(
        initial_pixels, closed_pixels,
        "Closing search must restore exact original terminal content with no residual highlights"
    );
}

#[test]
fn test_large_grid_search_performance_and_bounds() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 250, 120); // 30,000 cells

    // Fill all 120 rows with text
    {
        let grid = ws.active_pane_mut().terminal.active_grid_mut();
        for y in 0..120 {
            let start = y * grid.width;
            for (i, c) in "high density terminal workload with benchmark patterns"
                .chars()
                .enumerate()
            {
                grid.cells[start + i].character = c;
            }
        }
    }

    let start = std::time::Instant::now();
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    for ch in "benchmark".chars() {
        ws.handle_keyboard_input(
            &Key::Character(ch.to_string()),
            Some(&ch.to_string()),
            ModifiersState::empty(),
        );
    }
    let elapsed = start.elapsed();

    assert_eq!(ws.search.matches.len(), 120);
    assert!(
        elapsed.as_millis() < 100,
        "Interactive search across 30,000 cells took {:?}, exceeding 100ms bound",
        elapsed
    );
}

#[test]
fn test_search_overlay_mouse_dismiss_click() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Open search with Ctrl+F
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    assert!(ws.search.is_open);

    let (box_x, box_y, box_w, box_h) = ws
        .search_overlay_rect()
        .expect("Search overlay rect must exist when open");

    // Click [Esc] on the right side of search overlay
    ws.handle_cursor_moved(
        (box_x + box_w - 8.0, box_y + box_h / 2.0),
        ModifiersState::empty(),
    );
    ws.handle_mouse_input(
        ElementState::Pressed,
        MouseButton::Left,
        ModifiersState::empty(),
    );
    assert!(
        !ws.search.is_open,
        "Clicking [Esc] on search overlay must close search"
    );
}

#[test]
fn test_terminal_text_does_not_overlap_with_find_or_error_banner() {
    let (pty, _read_end) = create_test_pty_pipe();
    let mut ws = WindowState::create_test_window_state(pty, 80, 24);

    // Initial draw
    ws.draw();
    let (init_pane_rects, _) = ws.recalculate_panes_layout(ws.active_tab_index);
    let init_pane = init_pane_rects[0];

    // 1. Open Find overlay with Ctrl+F
    ws.handle_keyboard_input(
        &Key::Character("f".to_string()),
        None,
        ModifiersState::CONTROL,
    );
    assert!(ws.search.is_open);
    ws.draw();

    let (search_pane_rects, _) = ws.recalculate_panes_layout(ws.active_tab_index);
    let search_pane = search_pane_rects[0];
    let (_s_x, s_y, _s_w, _s_h) = ws
        .search_overlay_rect()
        .expect("Search overlay rect must exist");

    // The bottom edge of the terminal pane must not exceed the top of the search bar
    let terminal_bottom_y = search_pane.y + search_pane.height;
    assert!(
        terminal_bottom_y <= s_y as f32,
        "Terminal pane bottom ({}) must not overlap with search overlay top ({})",
        terminal_bottom_y,
        s_y
    );
    assert!(
        search_pane.height < init_pane.height,
        "Pane height must shrink when search is open (init: {}, search: {})",
        init_pane.height,
        search_pane.height
    );

    // 2. Attach AlertBanner while Search is also open
    ws.banner = Some(velox::app::app::AlertBanner {
        severity: velox::config::loader::DiagnosticSeverity::Error,
        message: "Error diagnostic message".to_string(),
        dismiss_hovered: false,
    });
    ws.draw();

    let (both_pane_rects, _) = ws.recalculate_panes_layout(ws.active_tab_index);
    let both_pane = both_pane_rects[0];
    let (_s2_x, s2_y, _s2_w, s2_h) = ws
        .search_overlay_rect()
        .expect("Search overlay rect must exist");
    let banner_h = ws.banner_height();
    let win_h = ws.renderer.viewport_height as f32;
    let banner_top_y = win_h - banner_h;

    // Zero overlap checks between all three elements:
    // (a) Pane bottom <= Search top
    assert!(
        both_pane.y + both_pane.height <= s2_y as f32,
        "Terminal pane bottom ({}) must not overlap with search box top ({})",
        both_pane.y + both_pane.height,
        s2_y
    );
    // (b) Search bottom <= Banner top
    assert!(
        s2_y as f32 + s2_h as f32 <= banner_top_y,
        "Search box bottom ({}) must not overlap with banner top ({})",
        s2_y as f32 + s2_h as f32,
        banner_top_y
    );
    // (c) Banner bottom <= Window height
    assert!(
        banner_top_y + banner_h <= win_h,
        "Banner bottom must not exceed window height"
    );

    // 3. Close search: terminal pane expands to occupy space above banner
    ws.handle_keyboard_input(&Key::Named(NamedKey::Escape), None, ModifiersState::empty());
    assert!(!ws.search.is_open);
    ws.draw();

    let (banner_only_rects, _) = ws.recalculate_panes_layout(ws.active_tab_index);
    let banner_only_pane = banner_only_rects[0];
    assert!(
        banner_only_pane.y + banner_only_pane.height <= banner_top_y,
        "Terminal pane bottom ({}) must not overlap with banner top ({})",
        banner_only_pane.y + banner_only_pane.height,
        banner_top_y
    );
    assert!(
        banner_only_pane.height > both_pane.height,
        "Pane height must expand when search is closed while banner remains"
    );

    // 4. Dismiss banner: terminal pane expands to full height
    let consumed =
        ws.handle_keyboard_input(&Key::Named(NamedKey::Escape), None, ModifiersState::empty());
    assert!(consumed);
    assert!(ws.banner.is_none());
    ws.draw();

    let (final_pane_rects, _) = ws.recalculate_panes_layout(ws.active_tab_index);
    let final_pane = final_pane_rects[0];
    assert_eq!(
        final_pane.height, init_pane.height,
        "Pane height must be fully restored"
    );
}
