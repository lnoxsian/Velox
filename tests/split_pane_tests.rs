use std::sync::Arc;
use velox::app::pane::{Pane, PaneId};
use velox::app::split::{
    FocusDirection, PaneRect, SeparatorRect, SplitDirection, SplitNode, SplitTree,
    find_neighbor_pane,
};
use velox::app::tab::Tab;
use velox::pty::process::spawn_process;
use velox::renderer::renderer::SeparatorRenderData;
use velox::renderer::software::{CpuPaneRenderData, CpuRenderer};
use velox::screen::cell::Color;
use velox::screen::cursor::CursorShape;
use velox::terminal::terminal::Terminal;
use velox::theme::theme::{TabAccentColorConfig, Theme};

fn create_test_pane(id: PaneId, cols: usize, rows: usize) -> Pane {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(cols, rows);
    Pane::new(id, pty, terminal, 14.0, false)
}

#[test]
fn test_pane_creation_and_attributes() {
    let pane = create_test_pane(42, 80, 24);
    assert_eq!(pane.id, 42);
    assert_eq!(pane.font_size, 14.0);
    assert_eq!(pane.current_title, "velox");
    assert_eq!(pane.terminal.grid.width, 80);
    assert_eq!(pane.terminal.grid.height, 24);
    assert!(!pane.hold);
}

#[test]
fn test_single_pane_tree_lifecycle() {
    let mut tree = SplitTree::new(create_test_pane(1, 80, 24));
    assert_eq!(tree.pane_count(), 1);
    assert_eq!(tree.first_pane_id(), Some(1));
    assert_eq!(tree.collect_pane_ids(), vec![1]);

    assert!(tree.find_pane(1).is_some());
    assert!(tree.find_pane(99).is_none());

    // Single pane cannot be removed (preserves terminal invariant)
    let removed = tree.remove_pane(1);
    assert!(removed.is_none());
    assert_eq!(tree.pane_count(), 1);
}

#[test]
fn test_horizontal_and_vertical_split_tree() {
    let mut tree = SplitTree::new(create_test_pane(1, 80, 24));

    // Split pane 1 horizontally (ratio 0.5)
    let p2 = create_test_pane(2, 80, 12);
    let ok = tree.split_pane(1, p2, SplitDirection::Horizontal, 0.5, 100);
    assert!(ok);
    assert_eq!(tree.pane_count(), 2);
    assert_eq!(tree.collect_pane_ids(), vec![1, 2]);

    // Split pane 2 vertically (ratio 0.5)
    let p3 = create_test_pane(3, 40, 12);
    let ok = tree.split_pane(2, p3, SplitDirection::Vertical, 0.5, 101);
    assert!(ok);
    assert_eq!(tree.pane_count(), 3);
    assert_eq!(tree.collect_pane_ids(), vec![1, 2, 3]);

    // Verify all panes exist in tree
    assert_eq!(tree.find_pane(1).unwrap().id, 1);
    assert_eq!(tree.find_pane(2).unwrap().id, 2);
    assert_eq!(tree.find_pane(3).unwrap().id, 3);
}

#[test]
fn test_nested_split_layout_calculation() {
    let mut tree = SplitTree::new(create_test_pane(1, 80, 24));
    tree.split_pane(
        1,
        create_test_pane(2, 80, 12),
        SplitDirection::Horizontal,
        0.5,
        100,
    );
    tree.split_pane(
        2,
        create_test_pane(3, 40, 12),
        SplitDirection::Vertical,
        0.5,
        101,
    );

    let (cw, ch) = (10, 20);
    let sep_size = 4.0;

    let (pane_rects, sep_rects) = tree.calculate_layout(
        0.0, 0.0, 1000.0, 800.0, sep_size, 0.0, 0.0, cw, ch, 14.0, 10, 5,
    );

    assert_eq!(pane_rects.len(), 3);
    assert_eq!(sep_rects.len(), 2);

    // Verify all pane rects are within bounds
    for rect in &pane_rects {
        assert!(rect.x >= 0.0);
        assert!(rect.y >= 0.0);
        assert!(rect.x + rect.width <= 1000.01);
        assert!(rect.y + rect.height <= 800.01);
        assert_eq!(rect.cols, (rect.width / cw as f32).floor() as usize);
        assert_eq!(rect.rows, (rect.height / ch as f32).floor() as usize);
    }

    // Verify separator rects
    let h_sep = sep_rects
        .iter()
        .find(|s| s.direction == SplitDirection::Horizontal)
        .unwrap();
    assert_eq!(h_sep.split_id, 100);
    assert_eq!(h_sep.height, 4.0);

    let v_sep = sep_rects
        .iter()
        .find(|s| s.direction == SplitDirection::Vertical)
        .unwrap();
    assert_eq!(v_sep.split_id, 101);
    assert_eq!(v_sep.width, 4.0);
}

#[test]
fn test_tree_normalization_recursive_cleanup() {
    let mut tree = SplitTree::new(create_test_pane(1, 80, 24));
    // Root split: Left (1), Right (2)
    tree.split_pane(
        1,
        create_test_pane(2, 80, 24),
        SplitDirection::Vertical,
        0.5,
        100,
    );
    // Split Left horizontally: Top-Left (1), Bottom-Left (3)
    tree.split_pane(
        1,
        create_test_pane(3, 80, 24),
        SplitDirection::Horizontal,
        0.5,
        101,
    );
    // Split Right horizontally: Top-Right (2), Bottom-Right (4)
    tree.split_pane(
        2,
        create_test_pane(4, 80, 24),
        SplitDirection::Horizontal,
        0.5,
        102,
    );

    assert_eq!(tree.pane_count(), 4);

    // Remove Bottom-Left (3): Left subtree normalizes to single Pane(1)
    let removed = tree.remove_pane(3);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 3);
    assert_eq!(tree.pane_count(), 3);
    assert_eq!(tree.collect_pane_ids(), vec![1, 2, 4]);

    // Remove Bottom-Right (4): Right subtree normalizes to single Pane(2)
    let removed = tree.remove_pane(4);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 4);
    assert_eq!(tree.pane_count(), 2);
    assert_eq!(tree.collect_pane_ids(), vec![1, 2]);

    // Remove Right (2): Root normalizes from Split to single Pane(1)
    let removed = tree.remove_pane(2);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 2);
    assert_eq!(tree.pane_count(), 1);
    assert_eq!(tree.collect_pane_ids(), vec![1]);
    assert!(matches!(tree.root, SplitNode::Pane(_)));
}

#[test]
fn test_directional_focus_2d_navigation() {
    let rects = vec![
        PaneRect {
            pane_id: 1,
            x: 0.0,
            y: 0.0,
            width: 498.0,
            height: 398.0,
            padding_x: 0.0,
            padding_y: 0.0,
            cols: 49,
            rows: 19,
            cell_width: 10.0,
            cell_height: 20.0,
        },
        PaneRect {
            pane_id: 2,
            x: 502.0,
            y: 0.0,
            width: 498.0,
            height: 398.0,
            padding_x: 0.0,
            padding_y: 0.0,
            cols: 49,
            rows: 19,
            cell_width: 10.0,
            cell_height: 20.0,
        },
        PaneRect {
            pane_id: 3,
            x: 0.0,
            y: 402.0,
            width: 498.0,
            height: 398.0,
            padding_x: 0.0,
            padding_y: 0.0,
            cols: 49,
            rows: 19,
            cell_width: 10.0,
            cell_height: 20.0,
        },
        PaneRect {
            pane_id: 4,
            x: 502.0,
            y: 402.0,
            width: 498.0,
            height: 398.0,
            padding_x: 0.0,
            padding_y: 0.0,
            cols: 49,
            rows: 19,
            cell_width: 10.0,
            cell_height: 20.0,
        },
    ];

    // From Pane 1 (Top-Left)
    assert_eq!(
        find_neighbor_pane(&rects, 1, FocusDirection::Right),
        Some(2)
    );
    assert_eq!(find_neighbor_pane(&rects, 1, FocusDirection::Down), Some(3));
    assert_eq!(find_neighbor_pane(&rects, 1, FocusDirection::Left), None);
    assert_eq!(find_neighbor_pane(&rects, 1, FocusDirection::Up), None);

    // From Pane 2 (Top-Right)
    assert_eq!(find_neighbor_pane(&rects, 2, FocusDirection::Left), Some(1));
    assert_eq!(find_neighbor_pane(&rects, 2, FocusDirection::Down), Some(4));
    assert_eq!(find_neighbor_pane(&rects, 2, FocusDirection::Right), None);
    assert_eq!(find_neighbor_pane(&rects, 2, FocusDirection::Up), None);

    // From Pane 3 (Bottom-Left)
    assert_eq!(find_neighbor_pane(&rects, 3, FocusDirection::Up), Some(1));
    assert_eq!(
        find_neighbor_pane(&rects, 3, FocusDirection::Right),
        Some(4)
    );
    assert_eq!(find_neighbor_pane(&rects, 3, FocusDirection::Down), None);
    assert_eq!(find_neighbor_pane(&rects, 3, FocusDirection::Left), None);

    // From Pane 4 (Bottom-Right)
    assert_eq!(find_neighbor_pane(&rects, 4, FocusDirection::Up), Some(2));
    assert_eq!(find_neighbor_pane(&rects, 4, FocusDirection::Left), Some(3));
    assert_eq!(find_neighbor_pane(&rects, 4, FocusDirection::Down), None);
    assert_eq!(find_neighbor_pane(&rects, 4, FocusDirection::Right), None);
}

#[test]
fn test_split_ratio_resizing_and_clamping() {
    let mut tree = SplitTree::new(create_test_pane(1, 80, 24));
    tree.split_pane(
        1,
        create_test_pane(2, 80, 24),
        SplitDirection::Vertical,
        0.5,
        100,
    );

    // Adjust split ratio by +0.1
    let ok = tree.set_split_ratio(100, 0.6);
    assert!(ok);

    // Attempting extreme ratios clamps to 0.05 .. 0.95
    tree.set_split_ratio(100, 0.999);
    if let SplitNode::Split { ratio, .. } = &tree.root {
        assert_eq!(*ratio, 0.95);
    } else {
        panic!("Expected split node");
    }

    tree.set_split_ratio(100, -0.5);
    if let SplitNode::Split { ratio, .. } = &tree.root {
        assert_eq!(*ratio, 0.05);
    } else {
        panic!("Expected split node");
    }
}

#[test]
fn test_separator_hit_testing() {
    let sep = SeparatorRect {
        split_id: 100,
        direction: SplitDirection::Vertical,
        x: 500.0,
        y: 0.0,
        width: 4.0,
        height: 800.0,
        bounds_x: 0.0,
        bounds_y: 0.0,
        bounds_w: 1000.0,
        bounds_h: 800.0,
    };

    // Direct hit inside the 4px strip
    assert!(sep.hit_test(502.0, 400.0, 3.0));
    // Hit within 3px padding
    assert!(sep.hit_test(498.0, 400.0, 3.0));
    assert!(sep.hit_test(506.0, 400.0, 3.0));
    // Miss outside padding
    assert!(!sep.hit_test(495.0, 400.0, 3.0));
    assert!(!sep.hit_test(510.0, 400.0, 3.0));
    // Miss vertically out of bounds
    assert!(!sep.hit_test(502.0, 805.0, 3.0));
}

#[test]
fn test_tab_pane_management_and_active_pane() {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(80, 24);
    let mut tab = Tab::new(1, pty, terminal, None, "velox".to_string(), false, 14.0);

    assert_eq!(tab.active_pane_id, 1);
    assert_eq!(tab.active_pane().id, 1);
    assert_eq!(tab.active_pane_mut().id, 1);

    // Split pane in tab
    let p2 = create_test_pane(2, 80, 24);
    tab.tree
        .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);
    tab.set_active_pane(2);

    assert_eq!(tab.active_pane().id, 2);
    assert_eq!(tab.tree.pane_count(), 2);

    // If active pane is removed, focus goes to previously active pane
    tab.remove_pane(2);
    assert_eq!(tab.active_pane_id, 1);
    assert_eq!(tab.active_pane().id, 1);
}

#[test]
fn test_closing_last_created_pane_focuses_second_last_created_pane() {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(80, 24);
    let mut tab = Tab::new(1, pty, terminal, None, "velox".to_string(), false, 14.0);

    // Initial state: Pane 1 is active
    assert_eq!(tab.active_pane_id, 1);

    // Split 1 -> Create Pane 2
    let p2 = create_test_pane(2, 80, 24);
    tab.tree
        .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);
    tab.set_active_pane(2);
    assert_eq!(tab.active_pane_id, 2);

    // Split 2 -> Create Pane 3
    let p3 = create_test_pane(3, 80, 24);
    tab.tree
        .split_pane(2, p3, SplitDirection::Horizontal, 0.5, 101);
    tab.set_active_pane(3);
    assert_eq!(tab.active_pane_id, 3);

    // Split 3 -> Create Pane 4
    let p4 = create_test_pane(4, 80, 24);
    tab.tree
        .split_pane(3, p4, SplitDirection::Vertical, 0.5, 102);
    tab.set_active_pane(4);
    assert_eq!(tab.active_pane_id, 4);

    // Close last created pane (Pane 4) -> Focus MUST go to second-last created pane (Pane 3), NOT Pane 1!
    let removed = tab.remove_pane(4);
    assert!(removed.is_some());
    assert_eq!(
        tab.active_pane_id, 3,
        "Closing last created pane 4 should transfer focus to second-last created pane 3"
    );
    assert_eq!(tab.active_pane().id, 3);

    // Close last created remaining pane (Pane 3) -> Focus MUST go to Pane 2, NOT Pane 1!
    let removed = tab.remove_pane(3);
    assert!(removed.is_some());
    assert_eq!(
        tab.active_pane_id, 2,
        "Closing pane 3 should transfer focus to remaining second-last created pane 2"
    );
    assert_eq!(tab.active_pane().id, 2);

    // Close Pane 2 -> Focus MUST go to Pane 1!
    let removed = tab.remove_pane(2);
    assert!(removed.is_some());
    assert_eq!(
        tab.active_pane_id, 1,
        "Closing pane 2 should transfer focus to pane 1"
    );
    assert_eq!(tab.active_pane().id, 1);
}

#[test]
fn test_focus_history_mru_ordering_on_arbitrary_switch() {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(80, 24);
    let mut tab = Tab::new(1, pty, terminal, None, "velox".to_string(), false, 14.0);

    let p2 = create_test_pane(2, 80, 24);
    tab.tree
        .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);
    tab.set_active_pane(2);

    let p3 = create_test_pane(3, 80, 24);
    tab.tree
        .split_pane(2, p3, SplitDirection::Horizontal, 0.5, 101);
    tab.set_active_pane(3);

    // Now focus is on 3, history is [1, 2, 3]
    // User clicks Pane 1
    tab.set_active_pane(1);
    assert_eq!(tab.active_pane_id, 1);

    // Closing Pane 1 should return focus to Pane 3 (most recently active remaining pane)
    tab.remove_pane(1);
    assert_eq!(tab.active_pane_id, 3);

    // Closing Pane 3 should return focus to Pane 2
    tab.remove_pane(3);
    assert_eq!(tab.active_pane_id, 2);
}

#[test]
fn test_independent_terminal_state_and_pty_isolation() {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(80, 24);
    let mut tab = Tab::new(1, pty, terminal, None, "velox".to_string(), false, 14.0);

    let p2 = create_test_pane(2, 80, 24);
    tab.tree
        .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);

    // Feed text into Pane 1
    {
        let pane1 = tab.tree.find_pane_mut(1).unwrap();
        pane1.terminal.feed(b"Hello Pane 1\r\n");
    }

    // Feed different text into Pane 2 and switch to Alt Screen
    {
        let pane2 = tab.tree.find_pane_mut(2).unwrap();
        pane2.terminal.feed(b"\x1b[?1049h"); // Alt screen
        pane2.terminal.feed(b"Hello Alt Pane 2\r\n");
    }

    // Verify Pane 1 is in Primary screen with "Hello Pane 1"
    let pane1 = tab.tree.find_pane(1).unwrap();
    assert!(!pane1.terminal.is_alt_screen);
    let p1_line: String = pane1.terminal.grid.cells[0..12]
        .iter()
        .map(|c| c.character)
        .collect();
    assert_eq!(p1_line, "Hello Pane 1");

    // Verify Pane 2 is in Alt screen with "Hello Alt Pane 2"
    let pane2 = tab.tree.find_pane(2).unwrap();
    assert!(pane2.terminal.is_alt_screen);
    let p2_line: String = pane2.terminal.alt_grid.cells[0..16]
        .iter()
        .map(|c| c.character)
        .collect();
    assert_eq!(p2_line, "Hello Alt Pane 2");
}

#[test]
fn test_software_renderer_multi_pane_rendering() {
    let theme = Theme::default();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.0, &theme, 1000, 800, true, 1.0);

    let p1 = create_test_pane(1, 49, 39);
    let p2 = create_test_pane(2, 49, 39);

    let rect1 = PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 498.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 49,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };
    let rect2 = PaneRect {
        pane_id: 2,
        x: 502.0,
        y: 0.0,
        width: 498.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 49,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };
    let sep = SeparatorRect {
        split_id: 100,
        direction: SplitDirection::Vertical,
        x: 498.0,
        y: 0.0,
        width: 4.0,
        height: 800.0,
        bounds_x: 0.0,
        bounds_y: 0.0,
        bounds_w: 1000.0,
        bounds_h: 800.0,
    };

    let pane1_data = CpuPaneRenderData {
        pane_id: 1,
        rect: rect1,
        cells: &p1.terminal.grid.cells,
        grid: &p1.terminal.grid,
        font_size: p1.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: true,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };
    let pane2_data = CpuPaneRenderData {
        pane_id: 2,
        rect: rect2,
        cells: &p2.terminal.grid.cells,
        grid: &p2.terminal.grid,
        font_size: p2.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: true,
        cursor_shape: CursorShape::HollowBlock,
        display_cursor_x: 0,
        is_active: false,
    };

    let sep_data = SeparatorRenderData {
        rect: sep,
        is_active: true,
        active_segment: sep.active_segment_for_pane(&rect1),
        is_hovered: false,
        is_dragging: false,
    };

    let mut target_buffer = vec![0u32; 1000 * 800];

    renderer.render_splits(
        &[pane1_data, pane2_data],
        &[sep_data],
        1.0,
        0.0,
        true,
        &mut target_buffer,
        None,
        Some(velox::screen::cell::Color {
            r: 80,
            g: 80,
            b: 80,
        }),
        Some(velox::screen::cell::Color {
            r: 100,
            g: 150,
            b: 250,
        }),
    );

    assert_eq!(renderer.framebuffer.width, 1000);
    assert_eq!(renderer.framebuffer.height, 800);
    assert_eq!(renderer.framebuffer.pixels.len(), 1000 * 800);
    assert_eq!(target_buffer.len(), 1000 * 800);
}

#[test]
fn test_pane_padding_layout_and_geometry() {
    let mut tree = SplitTree::new(create_test_pane(1, 80, 24));
    tree.split_pane(
        1,
        create_test_pane(2, 80, 24),
        SplitDirection::Vertical,
        0.5,
        100,
    );

    let (cw, ch) = (10, 20);
    let sep_size = 4.0;
    let padding_x = 8.0;
    let padding_y = 4.0;

    let (pane_rects, sep_rects) = tree.calculate_layout(
        0.0, 0.0, 1004.0, 800.0, sep_size, padding_x, padding_y, cw, ch, 14.0, 10, 5,
    );

    assert_eq!(pane_rects.len(), 2);
    assert_eq!(sep_rects.len(), 1);

    // Left pane (pane 1): width 500, height 800
    let p1 = &pane_rects[0];
    assert_eq!(p1.pane_id, 1);
    assert_eq!(p1.x, 0.0);
    assert_eq!(p1.width, 500.0);
    assert_eq!(p1.padding_x, 8.0);
    assert_eq!(p1.padding_y, 4.0);
    // Usable text width = 500 - 8*2 = 484 -> floor(484 / 10) = 48 cols
    assert_eq!(p1.cols, 48);
    // Usable text height = 800 - 4*2 = 792 -> floor(792 / 20) = 39 rows
    assert_eq!(p1.rows, 39);
    // Text start positions
    assert_eq!(p1.text_x(), 8.0);
    assert_eq!(p1.text_y(), 4.0);

    // Separator at x = 500, width 4
    let sep = &sep_rects[0];
    assert_eq!(sep.x, 500.0);
    assert_eq!(sep.width, 4.0);

    // Right pane (pane 2): starts at x = 504, width 500
    let p2 = &pane_rects[1];
    assert_eq!(p2.pane_id, 2);
    assert_eq!(p2.x, 504.0);
    assert_eq!(p2.width, 500.0);
    assert_eq!(p2.padding_x, 8.0);
    assert_eq!(p2.padding_y, 4.0);
    assert_eq!(p2.cols, 48);
    assert_eq!(p2.rows, 39);
    // Text start position inside right pane: 504 + 8 = 512
    assert_eq!(p2.text_x(), 512.0);
    assert_eq!(p2.text_y(), 4.0);
}

#[test]
fn test_unfocused_pane_selection_and_cursor_isolation() {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(80, 24);
    let mut tab = Tab::new(1, pty, terminal, None, "velox".to_string(), false, 14.0);

    let p2 = create_test_pane(2, 80, 24);
    tab.tree
        .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);

    // 1. Create a selection in Pane 1 while Pane 1 is active
    {
        let pane1 = tab.tree.find_pane_mut(1).unwrap();
        pane1.terminal.grid.selection.start_selection(0, 0);
        pane1.terminal.grid.selection.update_selection(10, 0);
        assert!(pane1.terminal.grid.selection.active);
    }

    // 2. Switch focus to Pane 2 and clear unfocused selections
    tab.active_pane_id = 2;
    tab.clear_unfocused_selections();

    // 3. Verify Pane 1 selection is cleared
    let pane1 = tab.tree.find_pane(1).unwrap();
    assert!(!pane1.terminal.grid.selection.active);

    // 4. Verify rendering: Inactive pane 1 does not render cursor or selection
    let theme = Theme::default();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.0, &theme, 1000, 800, true, 1.0);

    let rect1 = PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 500.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 48,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };
    let rect2 = PaneRect {
        pane_id: 2,
        x: 504.0,
        y: 0.0,
        width: 500.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 48,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };

    let p1_ref = tab.tree.find_pane(1).unwrap();
    let p2_ref = tab.tree.find_pane(2).unwrap();

    let pane1_data = CpuPaneRenderData {
        pane_id: 1,
        rect: rect1,
        cells: &p1_ref.terminal.grid.cells,
        grid: &p1_ref.terminal.grid,
        font_size: p1_ref.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false, // Inactive pane
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: false,
    };
    let pane2_data = CpuPaneRenderData {
        pane_id: 2,
        rect: rect2,
        cells: &p2_ref.terminal.grid.cells,
        grid: &p2_ref.terminal.grid,
        font_size: p2_ref.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: true,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    let mut target_buffer = vec![0u32; 1000 * 800];
    renderer.render_splits(
        &[pane1_data, pane2_data],
        &[],
        1.0,
        0.0,
        true,
        &mut target_buffer,
        None,
        None,
        None,
    );

    assert_eq!(target_buffer.len(), 1000 * 800);
}

#[test]
fn test_single_click_vs_drag_selection_empty_check() {
    let pty = Arc::new(spawn_process("/bin/sh", None, None).unwrap());
    let terminal = Terminal::new(80, 24);
    let mut tab = Tab::new(1, pty, terminal, None, "velox".to_string(), false, 14.0);

    let p2 = create_test_pane(2, 80, 24);
    tab.tree
        .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);

    // Single click (start == end): is_empty is true and cleared on release
    {
        let pane = tab.tree.find_pane_mut(1).unwrap();
        pane.terminal.grid.selection.start_selection(4, 4);
        assert!(pane.terminal.grid.selection.is_empty());
        if pane.terminal.grid.selection.is_empty() {
            pane.terminal.grid.selection.clear();
        }
        assert!(!pane.terminal.grid.selection.active);
    }

    // Drag selection (start != end): is_empty is false
    {
        let pane = tab.tree.find_pane_mut(1).unwrap();
        pane.terminal.grid.selection.start_selection(0, 0);
        pane.terminal.grid.selection.update_selection(5, 0);
        assert!(!pane.terminal.grid.selection.is_empty());
        assert!(pane.terminal.grid.selection.active);
    }
}

#[test]
fn test_active_split_separator_adjacency_and_accent() {
    let p1 = create_test_pane(1, 80, 24);
    let mut tree = SplitTree::new(p1);

    // Split 1 vertically: Left (1), Right (2)
    let p2 = create_test_pane(2, 80, 24);
    tree.split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);

    // Split 2 horizontally: Right-Top (2), Right-Bottom (3)
    let p3 = create_test_pane(3, 80, 24);
    tree.split_pane(2, p3, SplitDirection::Horizontal, 0.5, 101);

    let (pane_rects, sep_rects) =
        tree.calculate_layout(0.0, 0.0, 1000.0, 800.0, 2.0, 4.0, 4.0, 10, 20, 14.0, 10, 5);

    let p1_rect = pane_rects.iter().find(|r| r.pane_id == 1).unwrap();
    let p2_rect = pane_rects.iter().find(|r| r.pane_id == 2).unwrap();
    let p3_rect = pane_rects.iter().find(|r| r.pane_id == 3).unwrap();

    let vert_sep = sep_rects.iter().find(|s| s.split_id == 100).unwrap();
    let horiz_sep = sep_rects.iter().find(|s| s.split_id == 101).unwrap();

    // When pane 1 (Left) is active:
    // Vertical separator (100) covers entire height [0.0, 800.0]
    assert_eq!(
        vert_sep.active_segment_for_pane(p1_rect),
        Some((0.0, 800.0))
    );
    // Horizontal separator (101) inside right branch is NOT adjacent to pane 1
    assert_eq!(horiz_sep.active_segment_for_pane(p1_rect), None);

    // When pane 2 (Right-Top) is active:
    // Vertical separator only accents the top segment [0.0, p2_rect.height]
    let p2_vert_segment = vert_sep.active_segment_for_pane(p2_rect).unwrap();
    assert_eq!(p2_vert_segment.0, 0.0);
    assert!((p2_vert_segment.1 - p2_rect.height).abs() < 1.0);
    // Horizontal separator accents the horizontal width of pane 2
    assert!(horiz_sep.active_segment_for_pane(p2_rect).is_some());

    // When pane 3 (Right-Bottom) is active:
    // Vertical separator only accents the bottom segment [p3_rect.y, 800.0]
    let p3_vert_segment = vert_sep.active_segment_for_pane(p3_rect).unwrap();
    assert!((p3_vert_segment.0 - p3_rect.y).abs() < 1.0);
    assert_eq!(p3_vert_segment.1, 800.0);
    assert!(horiz_sep.active_segment_for_pane(p3_rect).is_some());
}

#[test]
fn test_per_pane_and_per_tab_font_size_isolation() {
    let p1 = create_test_pane(1, 80, 24);
    let mut tab1 = Tab::with_pane(1, p1, None, "Tab 1".to_string(), false, 14.0);

    // Split tab 1 into pane 1 and pane 2
    let p2 = create_test_pane(2, 80, 24);
    tab1.tree
        .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);

    // Create a separate tab 2
    let mut p3 = create_test_pane(3, 80, 24);
    p3.font_size = 16.0;
    let tab2 = Tab::with_pane(2, p3, None, "Tab 2".to_string(), false, 16.0);

    // Zoom pane 1 in tab 1
    if let Some(pane1) = tab1.tree.find_pane_mut(1) {
        pane1.font_size = 20.0;
    }
    tab1.active_pane_id = 1;
    tab1.font_size = tab1.active_pane().font_size;

    // Verify pane 1 has 20.0, but pane 2 in tab 1 retains its original 14.0
    assert_eq!(tab1.tree.find_pane(1).unwrap().font_size, 20.0);
    assert_eq!(tab1.tree.find_pane(2).unwrap().font_size, 14.0);
    assert_eq!(tab1.font_size, 20.0);

    // Verify tab 2 retains its independent 16.0
    assert_eq!(tab2.tree.find_pane(3).unwrap().font_size, 16.0);
    assert_eq!(tab2.font_size, 16.0);

    // Switch active pane in tab 1 to pane 2
    tab1.active_pane_id = 2;
    tab1.font_size = tab1.active_pane().font_size;
    assert_eq!(tab1.font_size, 14.0);
    assert_eq!(tab1.tree.find_pane(1).unwrap().font_size, 20.0);

    // Verify calculate_layout gives independent cell dimensions to pane 1 vs pane 2
    let (rects, _) = tab1
        .tree
        .calculate_layout(0.0, 0.0, 800.0, 600.0, 0.0, 0.0, 0.0, 10, 20, 14.0, 5, 5);
    let r1 = rects.iter().find(|r| r.pane_id == 1).unwrap();
    let r2 = rects.iter().find(|r| r.pane_id == 2).unwrap();
    // Pane 1 has font 20.0 (scale 20/14 = 1.428 -> cw=14, ch=29)
    assert!(r1.cell_width > r2.cell_width);
    assert!(r1.cell_height > r2.cell_height);
    // Pane 2 has base font 14.0 (cw=10, ch=20)
    assert_eq!(r2.cell_width, 10.0);
    assert_eq!(r2.cell_height, 20.0);
}

#[test]
fn test_multi_pane_independent_font_glyph_cache_stability() {
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.0, &theme, 800, 600, true, 1.0);

    let mut p1 = create_test_pane(1, 40, 24);
    p1.font_size = 24.0;
    p1.terminal.feed(b"Pane 1 Large Text\r\n");

    let mut p2 = create_test_pane(2, 40, 24);
    p2.font_size = 14.0;
    p2.terminal.feed(b"Pane 2 Small Text\r\n");

    let rect1 = PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 600.0,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: 40,
        rows: 24,
        cell_width: 17.0,
        cell_height: 34.0,
    };
    let rect2 = PaneRect {
        pane_id: 2,
        x: 400.0,
        y: 0.0,
        width: 400.0,
        height: 600.0,
        padding_x: 0.0,
        padding_y: 0.0,
        cols: 40,
        rows: 24,
        cell_width: 10.0,
        cell_height: 20.0,
    };

    let pane1_data = CpuPaneRenderData {
        pane_id: 1,
        rect: rect1,
        cells: &p1.terminal.grid.cells,
        grid: &p1.terminal.grid,
        font_size: p1.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: true,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };
    let pane2_data = CpuPaneRenderData {
        pane_id: 2,
        rect: rect2,
        cells: &p2.terminal.grid.cells,
        grid: &p2.terminal.grid,
        font_size: p2.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: false,
    };

    let mut target_buf = vec![0u32; 800 * 600];
    renderer.render_splits(
        &[pane1_data, pane2_data],
        &[],
        1.0,
        0.0,
        true,
        &mut target_buf,
        None,
        None,
        None,
    );

    // Verify pane 1 (24.0) has its own isolated glyph cache in pane_glyph_caches
    let p1_key = (24.0f32 * 100.0).round() as u32;
    assert!(renderer.pane_glyph_caches.contains_key(&p1_key));
    let p1_cache = renderer.pane_glyph_caches.get(&p1_key).unwrap();
    assert_eq!(p1_cache.font_size, 24.0);

    // Verify pane 2 (14.0) used the 14.0 glyph cache
    assert_eq!(renderer.glyph_cache.font_size, 14.0);
    assert!(p1_cache.cell_width > renderer.glyph_cache.cell_width);
    assert!(p1_cache.cell_height > renderer.glyph_cache.cell_height);

    // Capturing snapshot of pane 2 rendered pixels in the framebuffer
    let mut pane2_pixel_snapshot = Vec::new();
    for y in 0..100 {
        for x in 400..500 {
            pane2_pixel_snapshot.push(renderer.framebuffer.pixels[(y * 800 + x) as usize]);
        }
    }

    // Now simulate zooming pane 1 further to 28.0
    p1.font_size = 28.0;
    let pane1_data_zoomed = CpuPaneRenderData {
        pane_id: 1,
        rect: rect1,
        cells: &p1.terminal.grid.cells,
        grid: &p1.terminal.grid,
        font_size: p1.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: true,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };
    let pane2_data_same = CpuPaneRenderData {
        pane_id: 2,
        rect: rect2,
        cells: &p2.terminal.grid.cells,
        grid: &p2.terminal.grid,
        font_size: p2.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: false,
    };

    renderer.render_splits(
        &[pane1_data_zoomed, pane2_data_same],
        &[],
        1.0,
        0.0,
        true,
        &mut target_buf,
        None,
        None,
        None,
    );

    // Verify pane 2 pixel rendered output is 100% stable and unchanged
    let mut idx = 0;
    for y in 0..100 {
        for x in 400..500 {
            assert_eq!(
                renderer.framebuffer.pixels[(y * 800 + x) as usize],
                pane2_pixel_snapshot[idx],
                "Pixel mismatch at ({}, {}) in non-zoomed pane",
                x,
                y
            );
            idx += 1;
        }
    }
}

#[test]
fn test_active_split_separator_accent_color_matches_tab_bar_and_config() {
    let mut theme = Theme::new();
    // 1. Default tab accent is blue (ansi_colors[4])
    let default_tab_accent = theme.resolve_tab_accent_color();
    assert_eq!(default_tab_accent, theme.ansi_colors[4]);

    // When active_separator_color is "tab_accent" or None, parse_color_spec resolves to tab accent
    assert_eq!(
        theme.parse_color_spec("tab_accent"),
        Some(default_tab_accent)
    );
    assert_eq!(theme.parse_color_spec("tab_bar"), Some(default_tab_accent));
    assert_eq!(theme.parse_color_spec("accent"), Some(default_tab_accent));

    // 2. Named color adjustments
    assert_eq!(
        theme.parse_color_spec("magenta"),
        Some(theme.ansi_colors[5])
    );
    assert_eq!(theme.parse_color_spec("red"), Some(theme.ansi_colors[1]));
    assert_eq!(theme.parse_color_spec("green"), Some(theme.ansi_colors[2]));
    assert_eq!(theme.parse_color_spec("cyan"), Some(theme.ansi_colors[6]));
    assert_eq!(
        theme.parse_color_spec("#ff5500"),
        Some(Color {
            r: 255,
            g: 85,
            b: 0
        })
    );

    // 3. When tab bar accent color is customized in theme (e.g. magenta), separator matches it
    theme.tab_accent_color = Some(theme.ansi_colors[5]);
    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(5);
    let updated_tab_accent = theme.resolve_tab_accent_color();
    assert_eq!(updated_tab_accent, theme.ansi_colors[5]);
    assert_eq!(
        theme.parse_color_spec("tab_accent"),
        Some(theme.ansi_colors[5])
    );
}

#[test]
fn test_large_font_split_pane_layout_does_not_overflow() {
    let mut tree = SplitTree::new(create_test_pane(1, 80, 24));
    // Set pane 1 font size to large 48.0 px (base is 14.0)
    tree.find_pane_mut(1).unwrap().font_size = 48.0;

    // Split pane 1 vertically with 0.5 ratio
    let p2 = create_test_pane(2, 80, 24);
    assert!(tree.split_pane(1, p2, SplitDirection::Vertical, 0.5, 100));

    // Layout within an 800x600 window with 4px separator and 4px padding
    let (pane_rects, _) =
        tree.calculate_layout(0.0, 0.0, 800.0, 600.0, 4.0, 4.0, 4.0, 8, 16, 14.0, 20, 10);

    assert_eq!(pane_rects.len(), 2);
    for rect in &pane_rects {
        let text_pixel_width = rect.cols as f32 * rect.cell_width;
        let available_width = rect.width - rect.padding_x * 2.0;
        assert!(
            text_pixel_width <= available_width + 0.01,
            "Pane {} text width ({} cols * {} cell_w = {}) exceeded available width {} (rect width: {})",
            rect.pane_id,
            rect.cols,
            rect.cell_width,
            text_pixel_width,
            available_width,
            rect.width
        );

        let text_pixel_height = rect.rows as f32 * rect.cell_height;
        let available_height = rect.height - rect.padding_y * 2.0;
        assert!(
            text_pixel_height <= available_height + 0.01,
            "Pane {} text height ({} rows * {} cell_h = {}) exceeded available height {} (rect height: {})",
            rect.pane_id,
            rect.rows,
            rect.cell_height,
            text_pixel_height,
            available_height,
            rect.height
        );
    }

    // For the large font pane (rect.cell_width is ~27px), only ~14 cols fit in ~398px.
    // Ensure rect.cols was NOT clamped to min_cols (20), which would overflow by ~150px.
    let large_pane_rect = pane_rects.iter().find(|r| r.pane_id == 1).unwrap();
    assert!(
        large_pane_rect.cols < 20,
        "Large font pane should fit fewer than 20 columns in 398px, got {}",
        large_pane_rect.cols
    );
}

#[test]
fn test_software_renderer_large_font_split_does_not_spill_pixels() {
    let theme = Theme::new();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.0, &theme, 800, 600, true, 1.0);
    let mut target_buf = vec![0u32; 800 * 600];

    // Left pane with large font (36px) spanning x: 0..398
    let mut p1 = create_test_pane(1, 10, 10);
    p1.font_size = 36.0;
    p1.terminal.feed(b"\x1b[31mXXXXXXXXXX\x1b[0m\r\n");

    let left_rect = PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 398.0,
        height: 600.0,
        padding_x: 2.0,
        padding_y: 2.0,
        cols: 10,
        rows: 10,
        cell_width: 25.0,
        cell_height: 45.0,
    };
    let left_pane_data = CpuPaneRenderData {
        pane_id: 1,
        rect: left_rect,
        cells: &p1.terminal.grid.cells,
        grid: &p1.terminal.grid,
        font_size: 36.0,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    // Right pane with normal font (14px) spanning x: 402..800
    let mut p2 = create_test_pane(2, 30, 20);
    p2.font_size = 14.0;
    p2.terminal.feed(b"\x1b[32mOOOOOOOOOO\x1b[0m\r\n");

    let right_rect = PaneRect {
        pane_id: 2,
        x: 402.0,
        y: 0.0,
        width: 398.0,
        height: 600.0,
        padding_x: 2.0,
        padding_y: 2.0,
        cols: 30,
        rows: 20,
        cell_width: 8.0,
        cell_height: 16.0,
    };
    let right_pane_data = CpuPaneRenderData {
        pane_id: 2,
        rect: right_rect,
        cells: &p2.terminal.grid.cells,
        grid: &p2.terminal.grid,
        font_size: 14.0,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: false,
    };

    let sep = SeparatorRect {
        split_id: 1,
        direction: SplitDirection::Vertical,
        x: 398.0,
        y: 0.0,
        width: 4.0,
        height: 600.0,
        bounds_x: 0.0,
        bounds_y: 0.0,
        bounds_w: 800.0,
        bounds_h: 600.0,
    };
    let sep_data = SeparatorRenderData {
        rect: sep,
        is_active: false,
        active_segment: None,
        is_hovered: false,
        is_dragging: false,
    };

    renderer.render_splits(
        &[left_pane_data, right_pane_data],
        &[sep_data],
        1.0,
        0.0,
        true,
        &mut target_buf,
        None,
        None,
        None,
    );

    // Verify: Separator region (x: 398..402) has no red pixels from left pane
    let red_ansi = theme.ansi_colors[1];
    let red_packed = ((red_ansi.r as u32) << 16) | ((red_ansi.g as u32) << 8) | (red_ansi.b as u32);
    for y in 0..600 {
        for x in 398..402 {
            let p = renderer.framebuffer.pixels[(y * 800 + x) as usize];
            assert_ne!(
                p & 0x00FFFFFF,
                red_packed,
                "Separator region ({}, {}) was overwritten by red pixel from left pane",
                x,
                y
            );
        }
    }
}

#[test]
fn test_font_loader_initial_atlas_dim_scales_for_large_fonts() {
    use velox::font::loader::compute_initial_atlas_dim;

    // Small/normal fonts: cell dims around 8x16 -> 512
    assert_eq!(compute_initial_atlas_dim(8, 16), 512);
    assert_eq!(compute_initial_atlas_dim(10, 20), 512);

    // Medium-large fonts (e.g. 24px - 32px): cell dims around 18x36
    let dim_medium = compute_initial_atlas_dim(18, 36);
    assert!(
        dim_medium >= 1024,
        "Atlas dim should be >= 1024 for 18x36, got {}",
        dim_medium
    );

    // Very large fonts (e.g. 48px - 72px): cell dims around 35x70 or 50x100
    let dim_large = compute_initial_atlas_dim(35, 70);
    assert!(
        dim_large >= 2048,
        "Atlas dim should be >= 2048 for 35x70, got {}",
        dim_large
    );
}

#[test]
fn test_pane_render_state_atlas_tracking() {
    use velox::renderer::state::PaneRenderState;

    let mut state = PaneRenderState::default();
    assert_eq!(state.last_atlas_texture, None);
    assert_eq!(state.last_atlas_generation, 0);

    // Simulate texture and generation updates
    state.last_atlas_generation = 1;
    state.ensure_rows(24);
    assert_eq!(state.row_cache.len(), 24);

    // Marking full redraw clears row cache validity
    state.row_cache[0].valid = true;
    state.mark_full_redraw();
    assert!(!state.row_cache[0].valid);
    assert!(state.full_redraw);
}

#[test]
fn test_new_split_and_tab_font_size_defaults_to_config_not_zoomed() {
    let config_default_font_size = 14.0;

    // Create a tab with pane 1 at config default font size
    let mut tab = Tab::new(
        1,
        Arc::new(spawn_process("/bin/sh", None, None).unwrap()),
        Terminal::new(80, 24),
        None,
        "tab 1".to_string(),
        false,
        config_default_font_size,
    );

    // Simulate zooming in pane 1 to 32.0 px
    let zoomed_font_size = 32.0;
    tab.tree.find_pane_mut(1).unwrap().font_size = zoomed_font_size;
    tab.font_size = zoomed_font_size;
    assert_eq!(tab.active_pane().font_size, zoomed_font_size);

    // Now spawn a new split pane (pane 2) using config_default_font_size
    let mut p2_terminal = Terminal::new(80, 24);
    p2_terminal.set_cell_dimensions(8, 16);
    let p2 = Pane::new(
        2,
        Arc::new(spawn_process("/bin/sh", None, None).unwrap()),
        p2_terminal,
        config_default_font_size,
        false,
    );

    assert!(
        tab.tree
            .split_pane(1, p2, SplitDirection::Vertical, 0.5, 100)
    );
    tab.set_active_pane(2);

    // Verify pane 1 retains its zoomed font size
    assert_eq!(tab.tree.find_pane(1).unwrap().font_size, zoomed_font_size);

    // Verify newly spawned pane 2 starts at config default font size, NOT the zoomed font size
    assert_eq!(
        tab.tree.find_pane(2).unwrap().font_size,
        config_default_font_size
    );
    assert_eq!(tab.active_pane().font_size, config_default_font_size);

    // Recalculate layout and verify cell scaling respects each pane's respective font size
    let (pane_rects, _) = tab.tree.calculate_layout(
        0.0,
        0.0,
        800.0,
        600.0,
        4.0,
        4.0,
        4.0,
        8,
        16,
        config_default_font_size,
        20,
        10,
    );

    let r1 = pane_rects.iter().find(|r| r.pane_id == 1).unwrap();
    let r2 = pane_rects.iter().find(|r| r.pane_id == 2).unwrap();

    // Pane 1 (zoomed 32.0) has larger cell dimensions than pane 2 (default 14.0)
    assert!(r1.cell_width > r2.cell_width);
    assert!(r1.cell_height > r2.cell_height);
    // Pane 2 (default 14.0) should have base cell dimensions (8x16)
    assert_eq!(r2.cell_width, 8.0);
    assert_eq!(r2.cell_height, 16.0);

    // Also verify new tab creation always uses config default font size
    let new_tab = Tab::new(
        2,
        Arc::new(spawn_process("/bin/sh", None, None).unwrap()),
        Terminal::new(80, 24),
        None,
        "tab 2".to_string(),
        false,
        config_default_font_size,
    );
    assert_eq!(new_tab.font_size, config_default_font_size);
    assert_eq!(new_tab.active_pane().font_size, config_default_font_size);
}

#[test]
fn test_drag_selection_across_split_clamps_to_originating_pane() {
    let mut tree = SplitTree::new(create_test_pane(1, 40, 24));
    let p2 = create_test_pane(2, 40, 24);
    tree.split_pane(1, p2, SplitDirection::Vertical, 0.5, 100);

    let (pane_rects, _) = tree.calculate_layout(
        0.0, 0.0, 800.0, 600.0, 4.0, 0.0, 0.0, 10, 20, 14.0, 10, 5,
    );
    let r1 = pane_rects.iter().find(|r| r.pane_id == 1).unwrap();
    let r2 = pane_rects.iter().find(|r| r.pane_id == 2).unwrap();

    // Verify layout: Pane 1 is on the left, Pane 2 on the right
    assert_eq!(r1.x, 0.0);
    assert!(r2.x >= 400.0);

    // Compute col_idx and row_idx helper using the pane capture clamping formula
    let calc_cell = |rect: &PaneRect, mouse_x: f64, mouse_y: f64| -> (usize, usize) {
        let px = (rect.x + rect.padding_x) as f64;
        let py = (rect.y + rect.padding_y) as f64;
        let cw = rect.cell_width as f64;
        let ch = rect.cell_height as f64;
        let grid_width = rect.cols;
        let grid_height = rect.rows;

        let col_idx = if cw > 0.0 {
            let raw = ((mouse_x - px) / cw).floor() as i64;
            raw.clamp(0, grid_width.saturating_sub(1) as i64) as usize
        } else {
            0
        };
        let row_idx = if ch > 0.0 {
            let raw = ((mouse_y - py) / ch).floor() as i64;
            raw.clamp(0, grid_height.saturating_sub(1) as i64) as usize
        } else {
            0
        };
        (col_idx, row_idx)
    };

    // User starts drag at cell (5, 5) in Pane 1
    let (c_start, r_start) = calc_cell(r1, 55.0, 105.0);
    assert_eq!((c_start, r_start), (5, 5));

    // Mouse moves deep into Pane 2 (e.g. x = 600.0, inside Pane 2)
    // Relative to Pane 1, this clamps to the rightmost column of Pane 1
    let (c_in_pane2, r_in_pane2) = calc_cell(r1, 600.0, 105.0);
    assert_eq!(c_in_pane2, r1.cols - 1);
    assert_eq!(r_in_pane2, 5);

    // Mouse moves above Pane 1 (e.g. into tab bar at y = -10.0)
    let (c_above, r_above) = calc_cell(r1, 55.0, -10.0);
    assert_eq!(c_above, 5);
    assert_eq!(r_above, 0);

    // Mouse moves below Pane 1 (y = 700.0)
    let (c_below, r_below) = calc_cell(r1, 55.0, 700.0);
    assert_eq!(c_below, 5);
    assert_eq!(r_below, r1.rows - 1);
}

#[test]
fn test_software_renderer_multi_pane_focus_switching() {
    let mut p1 = create_test_pane(1, 49, 39);
    let mut p2 = create_test_pane(2, 49, 39);
    let theme = p1.terminal.theme.clone();
    p2.terminal.theme = theme.clone();
    let mut renderer = CpuRenderer::new("monospace", 14.0, 1.0, &theme, 1000, 800, false, 1.0);

    // Feed bold red text at column 1, leaving cursor at (0,0) and cell 0 blank
    p1.terminal.feed(b" \x1b[1;31mA\x1b[0m\x1b[1;1H");
    p2.terminal.feed(b" \x1b[1;31mA\x1b[0m\x1b[1;1H");

    let rect1 = PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 498.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 49,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };
    let rect2 = PaneRect {
        pane_id: 2,
        x: 502.0,
        y: 0.0,
        width: 498.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 49,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };

    let mut target_buffer = vec![0u32; 1000 * 800];

    // Frame 1: Pane 1 is active (cursor visible, bold_is_bright: true), Pane 2 is inactive (cursor hidden, bold_is_bright: false)
    let pane1_frame1 = CpuPaneRenderData {
        pane_id: 1,
        rect: rect1,
        cells: &p1.terminal.grid.cells,
        grid: &p1.terminal.grid,
        font_size: p1.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: true,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };
    let pane2_frame1 = CpuPaneRenderData {
        pane_id: 2,
        rect: rect2,
        cells: &p2.terminal.grid.cells,
        grid: &p2.terminal.grid,
        font_size: p2.font_size,
        theme: &theme,
        bold_is_bright: false,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: false,
    };

    renderer.render_splits(
        &[pane1_frame1, pane2_frame1],
        &[],
        1.0,
        0.0,
        true,
        &mut target_buffer,
        None,
        None,
        None,
    );

    let cursor_pixel_idx = (4 + 2) * 1000 + (8 + 2);
    let cursor_color = velox::renderer::software::color::PackedColor::from_color(
        theme.resolve_cursor_color(theme.default_fg),
    )
    .to_u32();
    let bg_color = velox::renderer::software::color::PackedColor::from_color(
        theme.default_bg,
    )
    .to_u32();

    // Verify cursor on active pane
    assert_eq!(target_buffer[cursor_pixel_idx], cursor_color, "Pane 1 cursor must be drawn in Frame 1");

    // Verify per-pane bold_is_bright mapping on cell 1
    let cell1 = &p1.terminal.grid.cells[1];
    let (fg1, _) = renderer.palette.resolve_cell_colors_pane(cell1, false, true, 0.0, &theme, 0);
    let cell2 = &p2.terminal.grid.cells[1];
    let (fg2, _) = renderer.palette.resolve_cell_colors_pane(cell2, false, false, 0.0, &theme, 0);
    let bright_red = velox::renderer::software::color::PackedColor::from_color(theme.ansi_colors[9]).to_u32();
    let regular_red = velox::renderer::software::color::PackedColor::from_color(theme.ansi_colors[1]).to_u32();
    assert_eq!(fg1, bright_red, "Pane 1 with bold_is_bright: true must use bright ANSI color 9");
    assert_eq!(fg2, regular_red, "Pane 2 with bold_is_bright: false must use regular ANSI color 1");

    // Verify focus dimming in Frame 1 (Pane 1 active dim 0.0, Pane 2 inactive dim 0.15)
    let (p1_f1_fg, _) = renderer.palette.resolve_cell_colors_pane(cell1, false, true, 0.0, &theme, 0);
    let (p2_f1_fg, _) = renderer.palette.resolve_cell_colors_pane(cell2, false, false, 0.15, &theme, 0);
    let undimmed_fg = velox::renderer::software::color::PackedColor::from_color(theme.ansi_colors[9]).to_u32();
    let dimmed_fg = velox::renderer::software::color::PackedColor::from_color(theme.ansi_colors[1].dim(0.15)).to_u32();
    assert_eq!(p1_f1_fg, undimmed_fg, "Pane 1 should be undimmed in Frame 1");
    assert_eq!(p2_f1_fg, dimmed_fg, "Pane 2 should be dimmed in Frame 1");

    // Frame 2: Focus moves to Pane 2! Grids have clear_damage() called between frames
    p1.terminal.active_grid_mut().clear_damage();
    p2.terminal.active_grid_mut().clear_damage();

    let pane1_frame2 = CpuPaneRenderData {
        pane_id: 1,
        rect: rect1,
        cells: &p1.terminal.grid.cells,
        grid: &p1.terminal.grid,
        font_size: p1.font_size,
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: false,
    };
    let pane2_frame2 = CpuPaneRenderData {
        pane_id: 2,
        rect: rect2,
        cells: &p2.terminal.grid.cells,
        grid: &p2.terminal.grid,
        font_size: p2.font_size,
        theme: &theme,
        bold_is_bright: false,
        cursor_visible: true,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };

    renderer.render_splits(
        &[pane1_frame2, pane2_frame2],
        &[],
        1.0,
        0.0,
        true,
        &mut target_buffer,
        None,
        None,
        None,
    );

    // Verify Pane 1 cursor was erased despite clean damage
    assert_eq!(
        target_buffer[cursor_pixel_idx],
        bg_color,
        "Pane 1 cursor must be erased after losing focus even with clean grid damage"
    );

    // Verify updated dimming states tracked in renderer
    assert_eq!(renderer.pane_states.get(&1).unwrap().last_dim, 0.15);
    assert_eq!(renderer.pane_states.get(&2).unwrap().last_dim, 0.0);
}

#[test]
fn test_zoom_limits_and_clamping_parity() {
    use velox::app::split::{clamp_font_size, MAX_FONT_SIZE_SCALE, MIN_FONT_SIZE_SCALE};

    let base = 14.0;
    let min_expected = (base * MIN_FONT_SIZE_SCALE).max(1.0); // 2.8
    let max_expected = base * MAX_FONT_SIZE_SCALE; // 70.0

    // 1. clamp_font_size function bounds
    assert_eq!(clamp_font_size(14.0, base), 14.0);
    assert_eq!(clamp_font_size(28.0, base), 28.0);
    assert_eq!(clamp_font_size(min_expected, base), min_expected);
    assert_eq!(clamp_font_size(max_expected, base), max_expected);
    assert_eq!(clamp_font_size(1.0, base), min_expected);
    assert_eq!(clamp_font_size(0.0, base), min_expected);
    assert_eq!(clamp_font_size(-10.0, base), min_expected);
    assert_eq!(clamp_font_size(75.0, base), max_expected);
    assert_eq!(clamp_font_size(150.0, base), max_expected);
    assert_eq!(clamp_font_size(500.0, base), max_expected);
    assert_eq!(clamp_font_size(0.5, 3.0), 1.0); // small base floor
    assert_eq!(clamp_font_size(20.0, 3.0), 15.0);

    // 2. CpuRenderer::update_font_size clamping
    let theme = Theme::default();
    let mut renderer = CpuRenderer::new("monospace", base, 1.0, &theme, 1000, 800, true, 1.0);
    assert_eq!(renderer.default_font_size, base);
    assert_eq!(renderer.glyph_cache.font_size, base);

    for zoom_in in [18.0, 30.0, 50.0, 70.0, 80.0, 100.0, 200.0, 500.0] {
        renderer.update_font_size(zoom_in);
    }
    assert_eq!(renderer.glyph_cache.font_size, max_expected);

    for zoom_out in [60.0, 40.0, 14.0, 5.0, 2.8, 1.5, 0.5, -10.0] {
        renderer.update_font_size(zoom_out);
    }
    assert_eq!(renderer.glyph_cache.font_size, min_expected);

    // Reset base font size so glyph_cache is at base (14.0) before testing per-pane caches
    renderer.update_font_size(base);
    assert_eq!(renderer.glyph_cache.font_size, base);

    // 3. CpuRenderer::render_splits clamps per-pane font sizes
    let p1 = create_test_pane(1, 49, 39);
    let p2 = create_test_pane(2, 49, 39);
    let rect1 = PaneRect {
        pane_id: 1,
        x: 0.0,
        y: 0.0,
        width: 498.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 49,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };
    let rect2 = PaneRect {
        pane_id: 2,
        x: 502.0,
        y: 0.0,
        width: 498.0,
        height: 800.0,
        padding_x: 8.0,
        padding_y: 4.0,
        cols: 49,
        rows: 39,
        cell_width: 10.0,
        cell_height: 20.0,
    };

    let pane1_data = CpuPaneRenderData {
        pane_id: 1,
        rect: rect1,
        cells: &p1.terminal.grid.cells,
        grid: &p1.terminal.grid,
        font_size: 200.0, // extreme zoom in
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: true,
    };
    let pane2_data = CpuPaneRenderData {
        pane_id: 2,
        rect: rect2,
        cells: &p2.terminal.grid.cells,
        grid: &p2.terminal.grid,
        font_size: 0.1, // extreme zoom out
        theme: &theme,
        bold_is_bright: true,
        cursor_visible: false,
        cursor_shape: CursorShape::Block,
        display_cursor_x: 0,
        is_active: false,
    };

    let mut target_buffer = vec![0u32; 1000 * 800];
    renderer.render_splits(
        &[pane1_data, pane2_data],
        &[],
        1.0,
        0.0,
        true,
        &mut target_buffer,
        None,
        None,
        None,
    );

    assert!(renderer.pane_glyph_caches.contains_key(&7000));
    assert!(!renderer.pane_glyph_caches.contains_key(&20000));
    assert_eq!(renderer.pane_glyph_caches.get(&7000).unwrap().font_size, 70.0);
    assert!(renderer.pane_glyph_caches.contains_key(&280));
    assert!(!renderer.pane_glyph_caches.contains_key(&10));
    assert_eq!(renderer.pane_glyph_caches.get(&280).unwrap().font_size, 2.8);
    assert_eq!(renderer.pane_states.get(&1).unwrap().last_font_size, 70.0);
    assert_eq!(renderer.pane_states.get(&2).unwrap().last_font_size, 2.8);

    // 4. SplitTree::calculate_layout clamps dimensions identically
    let mut tab = Tab::new(
        1,
        Arc::new(spawn_process("/bin/sh", None, None).unwrap()),
        Terminal::new(80, 24),
        None,
        "tab 1".to_string(),
        false,
        base,
    );
    tab.tree.find_pane_mut(1).unwrap().font_size = 70.0;
    let (rects_70, _) = tab.tree.calculate_layout(0.0, 0.0, 800.0, 600.0, 4.0, 4.0, 4.0, 8, 16, base, 20, 10);
    tab.tree.find_pane_mut(1).unwrap().font_size = 200.0;
    let (rects_200, _) = tab.tree.calculate_layout(0.0, 0.0, 800.0, 600.0, 4.0, 4.0, 4.0, 8, 16, base, 20, 10);
    assert_eq!(rects_70[0].cell_width, rects_200[0].cell_width);
    assert_eq!(rects_70[0].cell_height, rects_200[0].cell_height);
    assert_eq!(rects_70[0].cols, rects_200[0].cols);
    assert_eq!(rects_70[0].rows, rects_200[0].rows);

    tab.tree.find_pane_mut(1).unwrap().font_size = 2.8;
    let (rects_2_8, _) = tab.tree.calculate_layout(0.0, 0.0, 800.0, 600.0, 4.0, 4.0, 4.0, 8, 16, base, 20, 10);
    tab.tree.find_pane_mut(1).unwrap().font_size = 0.1;
    let (rects_0_1, _) = tab.tree.calculate_layout(0.0, 0.0, 800.0, 600.0, 4.0, 4.0, 4.0, 8, 16, base, 20, 10);
    assert_eq!(rects_2_8[0].cell_width, rects_0_1[0].cell_width);
    assert_eq!(rects_2_8[0].cell_height, rects_0_1[0].cell_height);
    assert_eq!(rects_2_8[0].cols, rects_0_1[0].cols);
    assert_eq!(rects_2_8[0].rows, rects_0_1[0].rows);
}


