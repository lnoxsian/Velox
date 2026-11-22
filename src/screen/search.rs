use crate::screen::cell::{Cell, CellFlags};
use crate::screen::grid::Grid;

pub const MAX_QUERY_LEN: usize = 256;
pub const MAX_SEARCH_MATCHES: usize = 10_000;

/// Represents a single search match across the terminal scrollback or active visible screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    /// Absolute row coordinate across scrollback and active grid (0..history_len + grid.height).
    pub abs_row: usize,
    /// Starting column index (inclusive).
    pub start_col: usize,
    /// Ending column index (exclusive).
    pub end_col: usize,
}

/// Reusable scratch buffers for row text extraction and coordinate mapping.
/// Avoids repeated heap allocations during screen search.
#[derive(Debug, Default, Clone)]
pub struct SearchScratch {
    pub lower_text: String,
    pub byte_to_col: Vec<usize>,
    pub byte_to_end_col: Vec<usize>,
}

impl SearchScratch {
    pub fn new() -> Self {
        Self {
            lower_text: String::with_capacity(256),
            byte_to_col: Vec::with_capacity(256),
            byte_to_end_col: Vec::with_capacity(256),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.lower_text.clear();
        self.byte_to_col.clear();
        self.byte_to_end_col.clear();
    }
}

/// Dedicated search state managing query string, cursor position, and cached matches.
#[derive(Debug, Clone)]
pub struct SearchState {
    pub is_open: bool,
    pub query: String,
    pub cursor_pos: usize,
    pub matches: Vec<SearchMatch>,
    pub active_match_index: Option<usize>,
    pub matches_dirty: bool,
    pub target_pane_id: Option<u64>,
    pub dismiss_hovered: bool,
    pub scratch: SearchScratch,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            cursor_pos: 0,
            matches: Vec::new(),
            active_match_index: None,
            matches_dirty: false,
            target_pane_id: None,
            dismiss_hovered: false,
            scratch: SearchScratch::new(),
        }
    }

    /// Opens the search overlay targeting the specified pane.
    pub fn open(&mut self, pane_id: u64) {
        self.is_open = true;
        self.target_pane_id = Some(pane_id);
        self.cursor_pos = self.query.chars().count();
        self.matches_dirty = true;
    }

    /// Closes search overlay, clearing visible matches and highlights.
    /// Retains `query` and `cursor_pos` for subsequent `Ctrl+F` invocations.
    pub fn close(&mut self) {
        self.is_open = false;
        self.matches.clear();
        self.active_match_index = None;
        self.matches_dirty = false;
        self.dismiss_hovered = false;
    }

    pub fn clear_matches(&mut self) {
        self.matches.clear();
        self.active_match_index = None;
        self.matches_dirty = false;
    }

    #[inline]
    pub fn invalidate_matches(&mut self) {
        if self.is_open {
            self.matches_dirty = true;
        }
    }

    pub fn invalidate_for_pane_switch(&mut self, new_pane_id: u64) {
        if self.is_open {
            self.target_pane_id = Some(new_pane_id);
            self.matches_dirty = true;
        }
    }

    /// Inserts a printable character at the current query editing position.
    pub fn insert_char(&mut self, c: char) {
        if self.query.chars().count() >= MAX_QUERY_LEN {
            return;
        }

        let byte_idx = self
            .query
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(idx, _)| idx)
            .unwrap_or(self.query.len());

        self.query.insert(byte_idx, c);
        self.cursor_pos += 1;
        self.matches_dirty = true;
    }

    /// Deletes the character before the current query editing position.
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_idx = self
                .query
                .char_indices()
                .nth(self.cursor_pos)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.query.remove(byte_idx);
            self.matches_dirty = true;
        }
    }

    /// Deletes the character at the current query editing position.
    pub fn delete(&mut self) {
        let total_chars = self.query.chars().count();
        if self.cursor_pos < total_chars {
            let byte_idx = self
                .query
                .char_indices()
                .nth(self.cursor_pos)
                .map(|(idx, _)| idx)
                .unwrap_or(self.query.len());
            self.query.remove(byte_idx);
            self.matches_dirty = true;
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        let total_chars = self.query.chars().count();
        if self.cursor_pos < total_chars {
            self.cursor_pos += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.query.chars().count();
    }

    /// Advance to the next match, wrapping to 0 if at the end.
    /// Returns `Some((old_match_abs_row, new_match_abs_row))` for minimal dirty tracking.
    pub fn next_match(&mut self) -> Option<(usize, usize)> {
        if self.matches.is_empty() {
            return None;
        }
        let old_idx = self.active_match_index.unwrap_or(0);
        let new_idx = (old_idx + 1) % self.matches.len();
        self.active_match_index = Some(new_idx);
        Some((self.matches[old_idx].abs_row, self.matches[new_idx].abs_row))
    }

    /// Retreat to the previous match, wrapping to the last match if at 0.
    /// Returns `Some((old_match_abs_row, new_match_abs_row))` for minimal dirty tracking.
    pub fn prev_match(&mut self) -> Option<(usize, usize)> {
        if self.matches.is_empty() {
            return None;
        }
        let old_idx = self.active_match_index.unwrap_or(0);
        let new_idx = if old_idx == 0 {
            self.matches.len() - 1
        } else {
            old_idx - 1
        };
        self.active_match_index = Some(new_idx);
        Some((self.matches[old_idx].abs_row, self.matches[new_idx].abs_row))
    }

    /// Recomputes matches across both visible screen rows and scrollback history chunks.
    /// Evaluates active screen rows first, then progresses backwards through scrollback chunks.
    /// Match capacity is strictly capped at `MAX_SEARCH_MATCHES` to avoid pathological allocations.
    pub fn recompute_matches(&mut self, grid: &Grid) {
        if self.query.is_empty() || grid.width == 0 || grid.height == 0 {
            self.matches.clear();
            self.active_match_index = None;
            self.matches_dirty = false;
            return;
        }

        let lower_query = self.query.to_lowercase();
        self.matches.clear();

        let history_len = grid.scrollback.len();

        // 1. Scan active visible screen rows first (newest to oldest)
        for grid_y in (0..grid.height).rev() {
            if self.matches.len() >= MAX_SEARCH_MATCHES {
                break;
            }
            let physical_y = grid.physical_row(grid_y);
            let start = physical_y * grid.width;
            let end = (start + grid.width).min(grid.cells.len());
            if start >= grid.cells.len() {
                continue;
            }
            let row_cells = &grid.cells[start..end];
            let is_wrapped = grid.row_wrapped.get(physical_y).copied().unwrap_or(false);
            let abs_row = history_len + grid_y;

            find_matches_in_row(
                row_cells,
                abs_row,
                &lower_query,
                is_wrapped,
                grid.default_bg,
                &mut self.scratch,
                &mut self.matches,
            );
            if self.matches.len() >= MAX_SEARCH_MATCHES {
                self.matches.truncate(MAX_SEARCH_MATCHES);
                break;
            }
        }

        // 2. Scan scrollback history rows backwards (from most recent down to oldest)
        if self.matches.len() < MAX_SEARCH_MATCHES && history_len > 0 {
            for hist_idx in (0..history_len).rev() {
                if self.matches.len() >= MAX_SEARCH_MATCHES {
                    break;
                }
                grid.scrollback
                    .with_row_slice(hist_idx, |row_cells, is_wrapped| {
                        find_matches_in_row(
                            row_cells,
                            hist_idx,
                            &lower_query,
                            is_wrapped,
                            grid.default_bg,
                            &mut self.scratch,
                            &mut self.matches,
                        );
                    });
                if self.matches.len() >= MAX_SEARCH_MATCHES {
                    self.matches.truncate(MAX_SEARCH_MATCHES);
                    break;
                }
            }
        }

        // 3. Sort matches into ascending row and column order
        self.matches.sort_unstable_by(|a, b| {
            a.abs_row
                .cmp(&b.abs_row)
                .then_with(|| a.start_col.cmp(&b.start_col))
        });

        // 4. Update active_match_index (choose match closest to current viewport)
        if self.matches.is_empty() {
            self.active_match_index = None;
        } else {
            let current_top_abs = history_len.saturating_sub(grid.scroll_offset);
            let current_bottom_abs = current_top_abs + grid.height;

            // O(log N) binary search for first visible match in current viewport
            let first_ge = self
                .matches
                .partition_point(|m| m.abs_row < current_top_abs);
            if first_ge < self.matches.len() && self.matches[first_ge].abs_row < current_bottom_abs
            {
                self.active_match_index = Some(first_ge);
            } else {
                let viewport_center_abs =
                    (history_len + grid.height / 2).saturating_sub(grid.scroll_offset);
                // O(log N) binary search for the match closest to viewport center
                let idx = match self
                    .matches
                    .binary_search_by_key(&viewport_center_abs, |m| m.abs_row)
                {
                    Ok(i) => i,
                    Err(i) => {
                        if i == 0 {
                            0
                        } else if i >= self.matches.len() {
                            self.matches.len() - 1
                        } else {
                            let diff_before =
                                viewport_center_abs.saturating_sub(self.matches[i - 1].abs_row);
                            let diff_after =
                                self.matches[i].abs_row.saturating_sub(viewport_center_abs);
                            if diff_before <= diff_after { i - 1 } else { i }
                        }
                    }
                };
                self.active_match_index = Some(idx);
            }
        }

        self.matches_dirty = false;
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

/// Searches row cells for literal occurrences of `lower_query` (case-insensitive).
pub fn find_matches_in_row(
    row_cells: &[Cell],
    abs_row: usize,
    lower_query: &str,
    is_wrapped: bool,
    default_bg: crate::screen::cell::Color,
    scratch: &mut SearchScratch,
    matches: &mut Vec<SearchMatch>,
) {
    if row_cells.is_empty() || lower_query.is_empty() {
        return;
    }
    scratch.clear();

    // Determine the end boundary of searchable text in this row:
    // If wrapped, content extends through the full width.
    // If not wrapped, trim trailing padding spaces with default background.
    let last_content_col = if is_wrapped {
        Some(row_cells.len())
    } else {
        let mut last = None;
        for (i, cell) in row_cells.iter().enumerate().rev() {
            if cell.character != ' '
                || cell.background != default_bg
                || cell.underline_color.is_some()
                || !cell.flags.is_empty()
            {
                last = Some(i + 1);
                break;
            }
        }
        last
    };

    let search_len = match last_content_col {
        Some(len) => len,
        None => return, // Completely blank row
    };

    let mut col = 0;
    while col < search_len {
        let cell = &row_cells[col];
        if cell.flags.contains(CellFlags::WIDE_CONTINUATION) {
            col += 1;
            continue;
        }

        let cell_w = if cell.flags.contains(CellFlags::WIDE) {
            2
        } else {
            1
        };
        let end_col = (col + cell_w).min(row_cells.len());

        // Fast-path: ASCII characters (covers >99% of terminal cell workloads)
        if cell.character.is_ascii() {
            let lch = cell.character.to_ascii_lowercase();
            scratch.lower_text.push(lch);
            scratch.byte_to_col.push(col);
            scratch.byte_to_end_col.push(end_col);
            col += cell_w;
            continue;
        }

        // Handle combining character sequence if codepoint is in PUA registry
        if ('\u{100000}'..='\u{10ffff}').contains(&cell.character) {
            let reg_idx = (cell.character as u32 - 0x100000) as usize;
            if let Ok(reg) = crate::screen::grid::get_combining_registry().lock() {
                if let Some(seq) = reg.get_sequence(reg_idx) {
                    for ch in seq.chars() {
                        for lch in ch.to_lowercase() {
                            let b_start = scratch.lower_text.len();
                            scratch.lower_text.push(lch);
                            let b_end = scratch.lower_text.len();
                            for _ in b_start..b_end {
                                scratch.byte_to_col.push(col);
                                scratch.byte_to_end_col.push(end_col);
                            }
                        }
                    }
                    col += cell_w;
                    continue;
                }
            }
        }

        for lch in cell.character.to_lowercase() {
            let b_start = scratch.lower_text.len();
            scratch.lower_text.push(lch);
            let b_end = scratch.lower_text.len();
            for _ in b_start..b_end {
                scratch.byte_to_col.push(col);
                scratch.byte_to_end_col.push(end_col);
            }
        }

        col += cell_w;
    }

    if scratch.lower_text.is_empty() {
        return;
    }

    let mut search_offset = 0;
    while search_offset < scratch.lower_text.len() {
        if let Some(found_idx) = scratch.lower_text[search_offset..].find(lower_query) {
            let match_start = search_offset + found_idx;
            let match_end = match_start + lower_query.len();
            if match_end <= scratch.byte_to_col.len() && match_start < scratch.byte_to_col.len() {
                let start_col = scratch.byte_to_col[match_start];
                let end_col = scratch.byte_to_end_col[match_end - 1];
                matches.push(SearchMatch {
                    abs_row,
                    start_col,
                    end_col,
                });
            }
            search_offset = match_start + lower_query.len().max(1);
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::cell::Color;

    fn make_test_grid(width: usize, height: usize) -> Grid {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        Grid::new(width, height, fg, bg, 100, false)
    }

    fn write_row_str(grid: &mut Grid, y: usize, text: &str) {
        let phys_y = grid.physical_row(y);
        let start = phys_y * grid.width;
        for (i, ch) in text.chars().enumerate() {
            if i < grid.width {
                grid.cells[start + i].character = ch;
            }
        }
    }

    #[test]
    fn test_empty_query_returns_zero_matches() {
        let mut grid = make_test_grid(80, 24);
        write_row_str(&mut grid, 0, "Hello World");

        let mut search = SearchState::new();
        search.query = String::new();
        search.recompute_matches(&grid);

        assert!(search.matches.is_empty());
        assert_eq!(search.active_match_index, None);
    }

    #[test]
    fn test_single_and_multiple_matches() {
        let mut grid = make_test_grid(80, 24);
        write_row_str(&mut grid, 0, "alpha beta gamma");
        write_row_str(&mut grid, 1, "beta delta beta");

        let mut search = SearchState::new();
        search.query = "beta".to_string();
        search.recompute_matches(&grid);

        assert_eq!(search.matches.len(), 3);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 6,
                end_col: 10
            }
        );
        assert_eq!(
            search.matches[1],
            SearchMatch {
                abs_row: 1,
                start_col: 0,
                end_col: 4
            }
        );
        assert_eq!(
            search.matches[2],
            SearchMatch {
                abs_row: 1,
                start_col: 11,
                end_col: 15
            }
        );
        assert_eq!(search.active_match_index, Some(0));
    }

    #[test]
    fn test_zero_matches_no_match_state() {
        let mut grid = make_test_grid(80, 24);
        write_row_str(&mut grid, 0, "alpha beta gamma");

        let mut search = SearchState::new();
        search.query = "zeta".to_string();
        search.recompute_matches(&grid);

        assert!(search.matches.is_empty());
        assert_eq!(search.active_match_index, None);
    }

    #[test]
    fn test_case_insensitive_matching() {
        let mut grid = make_test_grid(80, 24);
        write_row_str(&mut grid, 0, "Velox VELOX velox VeLoX");

        let mut search = SearchState::new();
        search.query = "velox".to_string();
        search.recompute_matches(&grid);

        assert_eq!(search.matches.len(), 4);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 0,
                end_col: 5
            }
        );
        assert_eq!(
            search.matches[1],
            SearchMatch {
                abs_row: 0,
                start_col: 6,
                end_col: 11
            }
        );
        assert_eq!(
            search.matches[2],
            SearchMatch {
                abs_row: 0,
                start_col: 12,
                end_col: 17
            }
        );
        assert_eq!(
            search.matches[3],
            SearchMatch {
                abs_row: 0,
                start_col: 18,
                end_col: 23
            }
        );
    }

    #[test]
    fn test_next_prev_navigation_wrapping() {
        let mut grid = make_test_grid(80, 24);
        write_row_str(&mut grid, 0, "foo bar foo");

        let mut search = SearchState::new();
        search.query = "foo".to_string();
        search.recompute_matches(&grid);

        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.active_match_index, Some(0));

        // Next wraps forward: 0 -> 1 -> 0
        search.next_match();
        assert_eq!(search.active_match_index, Some(1));
        search.next_match();
        assert_eq!(search.active_match_index, Some(0));

        // Prev wraps backward: 0 -> 1 -> 0
        search.prev_match();
        assert_eq!(search.active_match_index, Some(1));
        search.prev_match();
        assert_eq!(search.active_match_index, Some(0));
    }

    #[test]
    fn test_row_boundaries_first_and_last_columns() {
        let mut grid = make_test_grid(10, 2);
        // "abc" at col 0..3, "xyz" at col 7..10 (last col is 9)
        write_row_str(&mut grid, 0, "abc   xyz");
        // write exact at 7, 8, 9
        let phys_y = grid.physical_row(0);
        let start = phys_y * grid.width;
        grid.cells[start + 7].character = 'x';
        grid.cells[start + 8].character = 'y';
        grid.cells[start + 9].character = 'z';

        let mut search = SearchState::new();
        search.query = "abc".to_string();
        search.recompute_matches(&grid);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 0,
                end_col: 3
            }
        );

        search.query = "xyz".to_string();
        search.recompute_matches(&grid);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 7,
                end_col: 10
            }
        );
    }

    #[test]
    fn test_blank_cells_and_trailing_spaces() {
        let mut grid = make_test_grid(80, 5);
        // Row 0 has content, row 1 is completely empty blank spaces
        write_row_str(&mut grid, 0, "hello");

        let mut search = SearchState::new();
        search.query = "hello".to_string();
        search.recompute_matches(&grid);
        assert_eq!(search.matches.len(), 1);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 0,
                end_col: 5
            }
        );

        // Querying spaces does not explode over blank padding rows
        search.query = "   ".to_string();
        search.recompute_matches(&grid);
        assert_eq!(search.matches.len(), 0);
    }

    #[test]
    fn test_wide_characters() {
        let mut grid = make_test_grid(80, 5);
        let phys_y = grid.physical_row(0);
        let start = phys_y * grid.width;

        // "a🦀b"
        grid.cells[start].character = 'a';
        grid.cells[start + 1].character = '🦀';
        grid.cells[start + 1].flags = CellFlags::WIDE;
        grid.cells[start + 2].character = ' ';
        grid.cells[start + 2].flags = CellFlags::WIDE_CONTINUATION;
        grid.cells[start + 3].character = 'b';

        let mut search = SearchState::new();
        search.query = "🦀".to_string();
        search.recompute_matches(&grid);

        assert_eq!(search.matches.len(), 1);
        // 🦀 occupies columns 1 and 2 (start 1, end 3)
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 1,
                end_col: 3
            }
        );

        search.query = "a🦀b".to_string();
        search.recompute_matches(&grid);
        assert_eq!(search.matches.len(), 1);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 0,
                end_col: 4
            }
        );
    }

    #[test]
    fn test_combining_characters() {
        let mut grid = make_test_grid(80, 5);
        let phys_y = grid.physical_row(0);
        let start = phys_y * grid.width;

        let reg_idx = {
            let mut reg = crate::screen::grid::get_combining_registry()
                .lock()
                .unwrap();
            reg.get_or_insert("e\u{0301}").unwrap()
        };
        let pua_char = char::from_u32(0x100000 + reg_idx).unwrap();
        grid.cells[start].character = pua_char;

        let mut search = SearchState::new();
        search.query = "e\u{0301}".to_string();
        search.recompute_matches(&grid);

        assert_eq!(search.matches.len(), 1);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 0,
                end_col: 1
            }
        );
    }

    #[test]
    fn test_query_editing_and_bounds() {
        let mut search = SearchState::new();
        search.insert_char('a');
        search.insert_char('b');
        search.insert_char('c');
        assert_eq!(search.query, "abc");
        assert_eq!(search.cursor_pos, 3);

        search.move_cursor_left();
        assert_eq!(search.cursor_pos, 2);
        search.insert_char('X');
        assert_eq!(search.query, "abXc");

        search.backspace();
        assert_eq!(search.query, "abc");

        search.move_cursor_home();
        assert_eq!(search.cursor_pos, 0);
        search.delete();
        assert_eq!(search.query, "bc");

        search.move_cursor_end();
        assert_eq!(search.cursor_pos, 2);

        // Bounded query length
        for _ in 0..MAX_QUERY_LEN + 10 {
            search.insert_char('z');
        }
        assert!(search.query.chars().count() <= MAX_QUERY_LEN);
    }

    #[test]
    fn test_search_across_scrollback_and_visible_screen() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        let mut grid = Grid::new(80, 5, fg, bg, 100, false);

        // Fill lines such that 10 lines scroll into scrollback, 5 remain visible
        for i in 0..15 {
            for c in format!("line-{}", i).chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            if i < 14 {
                grid.scroll_or_move_down(bg);
                grid.cursor.x = 0;
            }
        }

        assert_eq!(grid.scrollback.len(), 10);

        let mut search = SearchState::new();
        // "line-0" is deep in scrollback (abs_row: 0)
        search.query = "line-0".to_string();
        search.recompute_matches(&grid);
        assert_eq!(search.matches.len(), 1);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 0,
                start_col: 0,
                end_col: 6,
            }
        );

        // "line-12" is on the visible screen (abs_row: 12)
        search.query = "line-12".to_string();
        search.recompute_matches(&grid);
        assert_eq!(search.matches.len(), 1);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                abs_row: 12,
                start_col: 0,
                end_col: 7,
            }
        );

        // "line-" matches across all 15 lines (10 in scrollback + 5 on visible screen)
        search.query = "line-".to_string();
        search.recompute_matches(&grid);
        assert_eq!(search.matches.len(), 15);
        for (idx, m) in search.matches.iter().enumerate() {
            assert_eq!(m.abs_row, idx);
        }
    }

    #[test]
    fn test_search_match_capacity_cap() {
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        let bg = Color { r: 0, g: 0, b: 0 };
        // Create grid with large scrollback
        let mut grid = Grid::new(80, 24, fg, bg, 20_000, false);

        for _ in 0..12_000 {
            for c in "repeat pattern".chars() {
                grid.put_char(c, fg, bg, None, CellFlags::empty());
            }
            grid.scroll_or_move_down(bg);
            grid.cursor.x = 0;
        }

        let mut search = SearchState::new();
        search.query = "repeat".to_string();
        search.recompute_matches(&grid);

        // Strictly capped at MAX_SEARCH_MATCHES
        assert_eq!(search.matches.len(), MAX_SEARCH_MATCHES);
    }

    #[test]
    fn test_search_performance_large_grid() {
        let mut grid = make_test_grid(200, 100); // 20,000 cells
        for y in 0..100 {
            write_row_str(
                &mut grid,
                y,
                "performance test pattern across visible terminal cells",
            );
        }

        let mut search = SearchState::new();
        search.query = "pattern".to_string();

        let start = std::time::Instant::now();
        search.recompute_matches(&grid);
        let elapsed = start.elapsed();

        assert_eq!(search.matches.len(), 100);
        // 20,000 cells search should easily finish in well under 50 milliseconds
        assert!(
            elapsed.as_millis() < 50,
            "Visible screen search on 20,000 cells took {:?}, exceeding 50ms bound",
            elapsed
        );
    }
}
