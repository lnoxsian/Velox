use velox::screen::cell::CellFlags;
use velox::terminal::terminal::Terminal;

#[test]
fn test_decrqm_decrpm_reports() {
    let mut term = Terminal::new(80, 24);

    // 1. Query initial DEC modes
    // Mode 1 (DECCKM): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?1$p");
    assert_eq!(term.outgoing, b"\x1b[?1;2$y");

    // Mode 25 (DECTCEM): initially set (1)
    term.outgoing.clear();
    term.feed(b"\x1b[?25$p");
    assert_eq!(term.outgoing, b"\x1b[?25;1$y");

    // Mode 7 (DECAWM): initially set (1)
    term.outgoing.clear();
    term.feed(b"\x1b[?7$p");
    assert_eq!(term.outgoing, b"\x1b[?7;1$y");

    // Mode 6 (DECOM): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?6$p");
    assert_eq!(term.outgoing, b"\x1b[?6;2$y");

    // Mode 66 (DECNKM): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?66$p");
    assert_eq!(term.outgoing, b"\x1b[?66;2$y");

    // Mode 1000 (Mouse tracking): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?1000$p");
    assert_eq!(term.outgoing, b"\x1b[?1000;2$y");

    // Mode 1006 (SGR mouse): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?1006$p");
    assert_eq!(term.outgoing, b"\x1b[?1006;2$y");

    // Mode 1016 (SGR pixel): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?1016$p");
    assert_eq!(term.outgoing, b"\x1b[?1016;2$y");

    // Mode 2004 (Bracketed paste): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?2004$p");
    assert_eq!(term.outgoing, b"\x1b[?2004;2$y");

    // Mode 2026 (Synchronized output): initially reset (2)
    term.outgoing.clear();
    term.feed(b"\x1b[?2026$p");
    assert_eq!(term.outgoing, b"\x1b[?2026;2$y");

    // 2. Set modes and verify updated reports
    term.feed(b"\x1b[?1h\x1b[?6h\x1b[?66h\x1b[?1000h\x1b[?1006h\x1b[?2004h");

    term.outgoing.clear();
    term.feed(b"\x1b[?1$p");
    assert_eq!(term.outgoing, b"\x1b[?1;1$y");

    term.outgoing.clear();
    term.feed(b"\x1b[?6$p");
    assert_eq!(term.outgoing, b"\x1b[?6;1$y");

    term.outgoing.clear();
    term.feed(b"\x1b[?66$p");
    assert_eq!(term.outgoing, b"\x1b[?66;1$y");

    term.outgoing.clear();
    term.feed(b"\x1b[?1000$p");
    assert_eq!(term.outgoing, b"\x1b[?1000;1$y");

    term.outgoing.clear();
    term.feed(b"\x1b[?1006$p");
    assert_eq!(term.outgoing, b"\x1b[?1006;1$y");

    term.outgoing.clear();
    term.feed(b"\x1b[?2004$p");
    assert_eq!(term.outgoing, b"\x1b[?2004;1$y");

    // 3. ANSI mode 4 (IRM - Insert Mode)
    term.outgoing.clear();
    term.feed(b"\x1b[4$p");
    assert_eq!(term.outgoing, b"\x1b[4;2$y"); // initially reset

    term.feed(b"\x1b[4h"); // enable insert mode
    term.outgoing.clear();
    term.feed(b"\x1b[4$p");
    assert_eq!(term.outgoing, b"\x1b[4;1$y"); // set

    term.feed(b"\x1b[4l"); // reset insert mode
    term.outgoing.clear();
    term.feed(b"\x1b[4$p");
    assert_eq!(term.outgoing, b"\x1b[4;2$y");

    // 4. Unknown mode query
    term.outgoing.clear();
    term.feed(b"\x1b[?9999$p");
    assert_eq!(term.outgoing, b"\x1b[?9999;0$y"); // not recognized
}

#[test]
fn test_origin_mode_scroll_margins_and_dsr() {
    let mut term = Terminal::new(80, 24);

    // Set scroll region from line 5 to 15 (1-based), top=4, bottom=15 (0-based)
    term.feed(b"\x1b[5;15r");

    // Without origin mode: \x1b[H goes to absolute (0, 0)
    term.feed(b"\x1b[H");
    assert_eq!(term.active_grid().cursor.x, 0);
    assert_eq!(term.active_grid().cursor.y, 0);

    term.outgoing.clear();
    term.feed(b"\x1b[6n");
    assert_eq!(term.outgoing, b"\x1b[1;1R");

    // Enable origin mode (DECOM): cursor immediately homes to top-left of scroll region (0, 4)
    term.feed(b"\x1b[?6h");
    assert_eq!(term.active_grid().cursor.x, 0);
    assert_eq!(term.active_grid().cursor.y, 4);

    // DSR in origin mode reports position relative to scroll region top (1, 1)
    term.outgoing.clear();
    term.feed(b"\x1b[6n");
    assert_eq!(term.outgoing, b"\x1b[1;1R");

    // Position cursor at relative line 3, col 4 (\x1b[3;4H)
    term.feed(b"\x1b[3;4H");
    assert_eq!(term.active_grid().cursor.x, 3);
    assert_eq!(term.active_grid().cursor.y, 4 + 2); // row 6

    term.outgoing.clear();
    term.feed(b"\x1b[6n");
    assert_eq!(term.outgoing, b"\x1b[3;4R");

    // Cursor Up cannot escape top margin (row 4)
    term.feed(b"\x1b[20A");
    assert_eq!(term.active_grid().cursor.y, 4);

    // Cursor Down cannot escape bottom margin (row 14)
    term.feed(b"\x1b[20B");
    assert_eq!(term.active_grid().cursor.y, 14);

    // Disable origin mode (DECOM): cursor homes to absolute (0, 0)
    term.feed(b"\x1b[?6l");
    assert_eq!(term.active_grid().cursor.x, 0);
    assert_eq!(term.active_grid().cursor.y, 0);

    term.outgoing.clear();
    term.feed(b"\x1b[6n");
    assert_eq!(term.outgoing, b"\x1b[1;1R");
}

#[test]
fn test_save_and_restore_dec_private_modes() {
    let mut term = Terminal::new(80, 24);

    // Set non-default modes
    term.feed(b"\x1b[?1h"); // DECCKM set
    term.feed(b"\x1b[?7l"); // DECAWM reset
    term.feed(b"\x1b[?25l"); // DECTCEM reset
    assert!(term.cursor_keys_mode);
    assert!(!term.auto_wrap);
    assert!(!term.active_grid().cursor.visible);

    // Save DEC modes: CSI ? s
    term.feed(b"\x1b[?1s\x1b[?7s\x1b[?25s");

    // Toggle all modes to opposite
    term.feed(b"\x1b[?1l\x1b[?7h\x1b[?25h");
    assert!(!term.cursor_keys_mode);
    assert!(term.auto_wrap);
    assert!(term.active_grid().cursor.visible);

    // Restore DEC modes: CSI ? r
    term.feed(b"\x1b[?1r\x1b[?7r\x1b[?25r");
    assert!(term.cursor_keys_mode);
    assert!(!term.auto_wrap);
    assert!(!term.active_grid().cursor.visible);
}

#[test]
fn test_tab_stops_hts_tbc_forward_backward() {
    let mut term = Terminal::new(80, 24);

    // Clear all tab stops (CSI 3 g)
    term.feed(b"\x1b[3g");

    // Set tab stop at col 15 (CSI 1;16H followed by ESC H)
    term.feed(b"\x1b[1;16H\x1bH");

    // Set tab stop at col 30 (CSI 1;31H followed by ESC H)
    term.feed(b"\x1b[1;31H\x1bH");

    // Home cursor (0, 0)
    term.feed(b"\x1b[1;1H");
    assert_eq!(term.active_grid().cursor.x, 0);

    // Forward Tab (0x09) -> col 15
    term.feed(b"\t");
    assert_eq!(term.active_grid().cursor.x, 15);

    // Forward Tab (0x09) -> col 30
    term.feed(b"\t");
    assert_eq!(term.active_grid().cursor.x, 30);

    // Backward Tab / CBT (CSI Z) -> back to col 15
    term.feed(b"\x1b[Z");
    assert_eq!(term.active_grid().cursor.x, 15);

    // Backward Tab (CSI Z) -> back to col 0
    term.feed(b"\x1b[Z");
    assert_eq!(term.active_grid().cursor.x, 0);

    // Clear tab stop at col 15 (move to col 15, CSI 0 g)
    term.feed(b"\x1b[1;16H\x1b[0g");

    // Now forward tab from col 0 jumps directly to col 30
    term.feed(b"\x1b[1;1H\t");
    assert_eq!(term.active_grid().cursor.x, 30);

    // Full reset (RIS: ESC c) restores default 8-column tab stops
    term.feed(b"\x1bc");
    term.feed(b"\x1b[1;1H\t");
    assert_eq!(term.active_grid().cursor.x, 8);
    term.feed(b"\t");
    assert_eq!(term.active_grid().cursor.x, 16);
}

#[test]
fn test_autowrap_disabled_overwrite_behavior() {
    let mut term = Terminal::new(10, 5);

    // Disable auto-wrap (DECAWM: CSI ? 7 l)
    term.feed(b"\x1b[?7l");

    // Write 15 characters on a 10-column screen
    term.feed(b"0123456789ABCDE");

    // Must clamp to last column (9) and not advance line or wrap to row 1
    let grid = term.active_grid();
    assert_eq!(grid.cursor.y, 0);
    assert_eq!(grid.cursor.x, 9);
    assert_eq!(grid.cells[9].character, 'E');

    // Re-enable auto-wrap (DECAWM: CSI ? 7 h)
    term.feed(b"\x1b[?7h");
    term.feed(b"\x1b[1;1H"); // Home
    term.feed(b"0123456789X");

    let grid = term.active_grid();
    assert_eq!(grid.cursor.y, 1);
    assert_eq!(grid.cursor.x, 1);
    assert_eq!(grid.cells[10].character, 'X');
}

#[test]
fn test_insert_mode_irm() {
    let mut term = Terminal::new(80, 24);

    term.feed(b"ABCD");
    assert_eq!(term.active_grid().cells[0].character, 'A');
    assert_eq!(term.active_grid().cells[1].character, 'B');
    assert_eq!(term.active_grid().cells[2].character, 'C');
    assert_eq!(term.active_grid().cells[3].character, 'D');

    // Move to col 1 ('B'), enable Insert Mode (CSI 4 h)
    term.feed(b"\x1b[1;2H\x1b[4h");
    assert!(term.insert_mode);

    // Insert 'X'
    term.feed(b"X");
    let grid = term.active_grid();
    assert_eq!(grid.cells[0].character, 'A');
    assert_eq!(grid.cells[1].character, 'X');
    assert_eq!(grid.cells[2].character, 'B');
    assert_eq!(grid.cells[3].character, 'C');
    assert_eq!(grid.cells[4].character, 'D');

    // Disable Insert Mode (CSI 4 l)
    term.feed(b"\x1b[4l");
    assert!(!term.insert_mode);

    // Overwrite col 2 with 'Y'
    term.feed(b"Y");
    let grid = term.active_grid();
    assert_eq!(grid.cells[2].character, 'Y');
}

#[test]
fn test_ris_and_decstr_resets() {
    let mut term = Terminal::new(80, 24);

    // Modify various state fields
    term.feed(b"\x1b[?1h\x1b[?6h\x1b[?66h\x1b[?1003h\x1b[?1006h\x1b[4h\x1b[?5h");
    assert!(term.cursor_keys_mode);
    assert!(term.origin_mode);
    assert!(term.keypad_application_mode);
    assert_eq!(term.mouse_mode, 1003);
    assert!(term.mouse_sgr);
    assert!(term.insert_mode);
    assert!(term.reverse_video);

    // Soft reset (DECSTR: CSI ! p)
    term.feed(b"\x1b[!p");
    assert!(!term.cursor_keys_mode);
    assert!(!term.origin_mode);
    assert!(!term.keypad_application_mode);
    assert_eq!(term.mouse_mode, 0);
    assert!(!term.mouse_sgr);
    assert!(!term.insert_mode);
    assert!(!term.reverse_video);
    assert!(term.active_grid().cursor.visible);

    // Alter again and test Full Reset (RIS: ESC c)
    term.feed(b"\x1b[?1h\x1b[4h\x1b[?66h");
    term.feed(b"\x1bc");
    assert!(!term.cursor_keys_mode);
    assert!(!term.origin_mode);
    assert!(!term.keypad_application_mode);
    assert_eq!(term.mouse_mode, 0);
    assert!(!term.insert_mode);
}

#[test]
fn test_dynamic_palette_osc4_osc104_osc10_11_12() {
    let mut term = Terminal::new(80, 24);

    // 1. Query index 17
    term.outgoing.clear();
    term.feed(b"\x1b]4;17;?\x1b\\");
    let orig_resp = term.outgoing.clone();
    assert!(orig_resp.starts_with(b"\x1b]4;17;rgb:"));

    // 2. Set index 17 to bright red #FF0000
    term.feed(b"\x1b]4;17;rgb:ff/00/00\x1b\\");

    // 3. Query index 17 again
    term.outgoing.clear();
    term.feed(b"\x1b]4;17;?\x1b\\");
    assert_eq!(term.outgoing, b"\x1b]4;17;rgb:ffff/0000/0000\x1b\\");

    // 4. Reset index 17 back to initial palette via OSC 104
    term.feed(b"\x1b]104;17\x1b\\");
    term.outgoing.clear();
    term.feed(b"\x1b]4;17;?\x1b\\");
    assert_eq!(term.outgoing, orig_resp);

    // 5. Query Dynamic Colors (OSC 10, 11, 12)
    term.outgoing.clear();
    term.feed(b"\x1b]10;?\x1b\\");
    assert!(term.outgoing.starts_with(b"\x1b]10;rgb:"));

    term.outgoing.clear();
    term.feed(b"\x1b]11;?\x1b\\");
    assert!(term.outgoing.starts_with(b"\x1b]11;rgb:"));

    term.outgoing.clear();
    term.feed(b"\x1b]12;?\x1b\\");
    assert!(term.outgoing.starts_with(b"\x1b]12;rgb:"));

    // 6. Reset dynamic colors via OSC 110, 111, 112
    term.feed(b"\x1b]110\x1b\\\x1b]111\x1b\\\x1b]112\x1b\\");
}

#[test]
fn test_keypad_application_mode_transitions() {
    let mut term = Terminal::new(80, 24);
    assert!(!term.keypad_application_mode);

    // ESC = (DECKPAM)
    term.feed(b"\x1b=");
    assert!(term.keypad_application_mode);

    // ESC > (DECKPNM)
    term.feed(b"\x1b>");
    assert!(!term.keypad_application_mode);

    // CSI ? 66 h
    term.feed(b"\x1b[?66h");
    assert!(term.keypad_application_mode);

    // CSI ? 66 l
    term.feed(b"\x1b[?66l");
    assert!(!term.keypad_application_mode);
}

#[test]
fn test_wide_character_clearing_and_zero_width() {
    let mut term = Terminal::new(80, 24);

    // Put wide character '你' at col 0 (takes col 0 and col 1)
    term.feed("你好".as_bytes());
    assert_eq!(term.active_grid().cells[0].character, '你');
    assert!(term.active_grid().cells[0].flags.contains(CellFlags::WIDE));
    assert_eq!(term.active_grid().cells[1].character, ' ');
    assert!(
        term.active_grid().cells[1]
            .flags
            .contains(CellFlags::WIDE_CONTINUATION)
    );

    // Overwrite continuation cell at col 1 with 'A'
    term.feed(b"\x1b[1;2HA");
    // Column 0 should be cleaned to space, and col 1 is 'A'
    assert_eq!(term.active_grid().cells[0].character, ' ');
    assert!(!term.active_grid().cells[0].flags.contains(CellFlags::WIDE));
    assert_eq!(term.active_grid().cells[1].character, 'A');
}

#[test]
fn test_alternate_screen_isolation_and_mode_leak_prevention() {
    let mut term = Terminal::new(80, 24);

    // Write on primary screen
    term.feed(b"Primary Screen Text");
    assert_eq!(term.active_grid().cells[0].character, 'P');
    assert!(!term.is_alt_screen);
    assert!(!term.cursor_keys_mode);
    assert!(term.active_grid().cursor.visible);

    // Save cursor position at (19, 0)
    assert_eq!(term.active_grid().cursor.x, 19);

    // Switch to alternate screen (CSI ? 1049 h)
    term.feed(b"\x1b[?1049h");
    assert!(term.is_alt_screen);
    // Alternate screen should be cleared and home cursor (0, 0)
    assert_eq!(term.active_grid().cursor.x, 0);
    assert_eq!(term.active_grid().cursor.y, 0);
    assert_eq!(term.active_grid().cells[0].character, ' ');

    // Mutate state while in alternate screen: enable DECCKM, hide cursor, enable mouse 1002
    term.feed(b"\x1b[?1h\x1b[?25l\x1b[?1002h");
    assert!(term.cursor_keys_mode);
    assert!(!term.active_grid().cursor.visible);
    assert_eq!(term.mouse_mode, 1002);

    // Exit alternate screen (CSI ? 1049 l)
    term.feed(b"\x1b[?1049l");
    assert!(!term.is_alt_screen);

    // Primary screen contents and cursor must be intact
    assert_eq!(term.active_grid().cells[0].character, 'P');
    assert_eq!(term.active_grid().cursor.x, 19);
    assert_eq!(term.active_grid().cursor.y, 0);

    // Primary cursor visibility must be restored
    assert!(term.active_grid().cursor.visible);
}

#[test]
fn test_repeated_alternate_screen_switching() {
    let mut term = Terminal::new(80, 24);

    for cycle in 0..20 {
        // Primary text
        let primary_str = format!("Cycle {}", cycle);
        term.feed(b"\x1b[1;1H");
        term.feed(primary_str.as_bytes());
        assert_eq!(term.active_grid().cells[0].character, 'C');

        // Enter alt screen
        term.feed(b"\x1b[?1049h");
        assert!(term.is_alt_screen);
        assert_eq!(term.active_grid().cursor.x, 0);
        assert_eq!(term.active_grid().cursor.y, 0);

        // Alt text
        term.feed(b"In Alt");
        assert_eq!(term.active_grid().cells[0].character, 'I');

        // Leave alt screen
        term.feed(b"\x1b[?1049l");
        assert!(!term.is_alt_screen);
        assert_eq!(term.active_grid().cells[0].character, 'C');
    }
}

#[test]
fn test_osc52_payload_size_bounding() {
    let mut term = Terminal::new(80, 24);

    // Payload under 10MB limit (normal base64 copy)
    let normal_payload = format!("\x1b]52;c;{}\x1b\\", "SGVsbG8gV29ybGQ=");
    term.feed(normal_payload.as_bytes());

    // Payload exceeding 10MB limit: safely ignored without OOM
    let mut huge_osc = Vec::with_capacity(10_000_100 + 32);
    huge_osc.extend_from_slice(b"\x1b]52;c;");
    huge_osc.resize(10_000_100 + 7, b'A');
    huge_osc.extend_from_slice(b"\x1b\\");

    term.feed(&huge_osc);
    // Terminal must continue operating normally
    term.feed(b"\x1b[1;1HOK");
    assert_eq!(term.active_grid().cells[0].character, 'O');
    assert_eq!(term.active_grid().cells[1].character, 'K');
}

#[test]
fn test_osc777_sanitization_and_bounded() {
    let mut term = Terminal::new(80, 24);

    // OSC 777 notification with control characters and newlines
    let evil_notification =
        b"\x1b]777;notify;Alert\x07Evil\r\nTitle;Malicious\x1b[31mMessage\x1b\\";
    term.feed(evil_notification);

    // Terminal must continue operating safely without corruption
    term.feed(b"\x1b[1;1HHealthy");
    assert_eq!(term.active_grid().cells[0].character, 'H');
}

#[test]
fn test_detected_urls_and_osc8_have_no_automatic_underline() {
    let mut term = Terminal::new(80, 24);

    // 1. Output a plain detected URL
    term.feed(b"Visit https://example.com/velox for details\r\n");

    // Verify URL detection works for mouse hover & Ctrl-click
    let grid = term.active_grid();
    assert!(grid.has_url_at(6, 0));
    assert!(grid.has_url_at(20, 0));
    assert_eq!(
        grid.hyperlink_url_at(6, 0).as_deref(),
        Some("https://example.com/velox")
    );

    // Verify NO cells in row 0 have automatic underline flags
    for col in 0..80 {
        let cell = &grid.cells[col];
        assert!(
            !cell.flags.intersects(
                CellFlags::UNDERLINE | CellFlags::DOUBLE_UNDERLINE | CellFlags::CURLY_UNDERLINE
            ),
            "Detected URL cell at col {} must NOT have automatic underline",
            col
        );
    }

    // 2. Output an explicit OSC 8 hyperlink
    term.feed(b"\x1b]8;;https://example.com/osc8\x1b\\Click Here\x1b]8;;\x1b\\\r\n");

    let grid = term.active_grid();
    let row1_start = 80;
    assert!(grid.has_url_at(0, 1));
    assert_eq!(
        grid.hyperlink_url_at(0, 1).as_deref(),
        Some("https://example.com/osc8")
    );

    // Verify explicit OSC 8 hyperlink cells also have NO automatic underline
    for col in 0..10 {
        let cell = &grid.cells[row1_start + col];
        assert!(
            !cell.flags.intersects(
                CellFlags::UNDERLINE | CellFlags::DOUBLE_UNDERLINE | CellFlags::CURLY_UNDERLINE
            ),
            "OSC 8 hyperlink cell at col {} must NOT have automatic underline",
            col
        );
    }

    // 3. Verify explicit SGR 4m underline STILL works for intentional underlines
    term.feed(b"\x1b[4mUnderlined Text\x1b[24m Normal Text\r\n");
    let grid = term.active_grid();
    let row2_start = 2 * 80;
    for col in 0..15 {
        let cell = &grid.cells[row2_start + col];
        assert!(
            cell.flags.contains(CellFlags::UNDERLINE),
            "Explicit SGR 4m cell at col {} must have CellFlags::UNDERLINE",
            col
        );
    }
    for col in 16..27 {
        let cell = &grid.cells[row2_start + col];
        assert!(
            !cell.flags.contains(CellFlags::UNDERLINE),
            "Reset SGR 24m cell at col {} must NOT have CellFlags::UNDERLINE",
            col
        );
    }
}

