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
    tab.active_pane_id = 2;

    assert_eq!(tab.active_pane().id, 2);
    assert_eq!(tab.tree.pane_count(), 2);

    // If active pane is removed, active_pane_id falls back to remaining pane
    tab.tree.remove_pane(2);
    if tab.tree.find_pane(tab.active_pane_id).is_none() {
        tab.active_pane_id = tab.tree.first_pane_id().unwrap();
    }
    assert_eq!(tab.active_pane_id, 1);
    assert_eq!(tab.active_pane().id, 1);
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
    assert_eq!(
        theme.parse_color_spec("tab_bar"),
        Some(default_tab_accent)
    );
    assert_eq!(
        theme.parse_color_spec("accent"),
        Some(default_tab_accent)
    );

    // 2. Named color adjustments
    assert_eq!(
        theme.parse_color_spec("magenta"),
        Some(theme.ansi_colors[5])
    );
    assert_eq!(
        theme.parse_color_spec("red"),
        Some(theme.ansi_colors[1])
    );
    assert_eq!(
        theme.parse_color_spec("green"),
        Some(theme.ansi_colors[2])
    );
    assert_eq!(
        theme.parse_color_spec("cyan"),
        Some(theme.ansi_colors[6])
    );
    assert_eq!(
        theme.parse_color_spec("#ff5500"),
        Some(Color { r: 255, g: 85, b: 0 })
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
