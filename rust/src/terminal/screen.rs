//! Terminal screen buffer management
//!
//! Manages the grid of cells that represents the terminal display.

use super::cell::{Cell, CellAttrs, DEFAULT_BG, DEFAULT_FG};
use crate::session_state::{ScreenLine, StyleSpan, TerminalState};
use serde::{Deserialize, Serialize};

/// The terminal screen buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    /// Grid of cells (row-major order)
    cells: Vec<Vec<Cell>>,
    /// Number of columns
    cols: usize,
    /// Number of rows
    rows: usize,
    /// Cursor row position
    cursor_row: usize,
    /// Cursor column position
    cursor_col: usize,
    /// Scrollback buffer (lines that scrolled off the top)
    scrollback: Vec<Vec<Cell>>,
    /// Maximum scrollback lines
    scrollback_limit: usize,
    /// Current text attributes for new characters
    current_attrs: CellAttrs,
    /// Current foreground color
    current_fg: u32,
    /// Current background color
    current_bg: u32,
    /// Saved cursor (DECSC / ANSI CSI `s`), if any
    saved_cursor: Option<(usize, usize)>,
}

impl Screen {
    /// Create a new screen with given dimensions
    pub fn new(cols: usize, rows: usize) -> Self {
        let cells = vec![vec![Cell::default(); cols]; rows];

        Self {
            cells,
            cols,
            rows,
            cursor_row: 0,
            cursor_col: 0,
            scrollback: Vec::new(),
            scrollback_limit: 10000,
            current_attrs: CellAttrs::default(),
            current_fg: DEFAULT_FG,
            current_bg: DEFAULT_BG,
            saved_cursor: None,
        }
    }

    /// Get screen dimensions
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// Get cursor position
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// Set cursor position (clamped to screen bounds)
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.cursor_row = row.min(self.rows.saturating_sub(1));
        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    /// Move cursor relative to current position
    pub fn move_cursor(&mut self, delta_row: i32, delta_col: i32) {
        let new_row = (self.cursor_row as i32 + delta_row).max(0) as usize;
        let new_col = (self.cursor_col as i32 + delta_col).max(0) as usize;
        self.set_cursor(new_row, new_col);
    }

    /// Get a cell at position
    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.cells.get(row).and_then(|r| r.get(col))
    }

    /// Get a mutable cell at position
    pub fn get_cell_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.cells.get_mut(row).and_then(|r| r.get_mut(col))
    }

    /// Write a character at cursor position and advance cursor
    pub fn write_char(&mut self, c: char) {
        if c == '\n' {
            self.newline();
            return;
        }

        if c == '\r' {
            self.cursor_col = 0;
            return;
        }

        // Copy current style before mutable borrow
        let fg = self.current_fg;
        let bg = self.current_bg;
        let attrs = self.current_attrs;

        // Write character at cursor
        if let Some(cell) = self.get_cell_mut(self.cursor_row, self.cursor_col) {
            cell.c = c;
            cell.fg = fg;
            cell.bg = bg;
            cell.attrs = attrs;
        }

        // Advance cursor
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            self.newline();
        }
    }

    /// Move to next line, scrolling if necessary
    pub fn newline(&mut self) {
        self.cursor_col = 0;
        self.cursor_row += 1;

        if self.cursor_row >= self.rows {
            self.scroll_up();
            self.cursor_row = self.rows - 1;
        }
    }

    /// Scroll the screen up by one line
    pub fn scroll_up(&mut self) {
        if !self.cells.is_empty() {
            // Move top line to scrollback
            let top_line = self.cells.remove(0);
            self.scrollback.push(top_line);

            // Trim scrollback if too large
            while self.scrollback.len() > self.scrollback_limit {
                self.scrollback.remove(0);
            }

            // Add new empty line at bottom
            self.cells.push(vec![Cell::default(); self.cols]);
        }
    }

    /// Clear the entire screen
    pub fn clear(&mut self) {
        for row in &mut self.cells {
            for cell in row {
                cell.reset();
            }
        }
    }

    /// Save cursor position (CSI `s` / DECSC)
    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some((self.cursor_row, self.cursor_col));
    }

    /// Restore cursor position (CSI `u` / DECRC). No-op if nothing was saved.
    pub fn restore_cursor(&mut self) {
        if let Some((row, col)) = self.saved_cursor {
            self.set_cursor(row, col);
        }
    }

    /// Clear from the start of the screen through the cursor (CSI J, mode 1)
    pub fn clear_to_cursor(&mut self) {
        for row in 0..self.cursor_row {
            if let Some(line) = self.cells.get_mut(row) {
                for cell in line {
                    cell.reset();
                }
            }
        }
        self.clear_line_to_cursor();
    }

    /// Clear the current line from column 0 through the cursor (CSI K, mode 1)
    pub fn clear_line_to_cursor(&mut self) {
        if let Some(line) = self.cells.get_mut(self.cursor_row) {
            let end = self.cursor_col.min(self.cols.saturating_sub(1));
            for col in 0..=end {
                if let Some(cell) = line.get_mut(col) {
                    cell.reset();
                }
            }
        }
    }

    /// Clear from cursor to end of screen
    pub fn clear_to_end(&mut self) {
        // Clear rest of current line
        self.clear_line_from_cursor();

        // Clear all lines below
        for row in (self.cursor_row + 1)..self.rows {
            if let Some(line) = self.cells.get_mut(row) {
                for cell in line {
                    cell.reset();
                }
            }
        }
    }

    /// Clear current line from cursor to end
    pub fn clear_line_from_cursor(&mut self) {
        if let Some(line) = self.cells.get_mut(self.cursor_row) {
            for col in self.cursor_col..self.cols {
                if let Some(cell) = line.get_mut(col) {
                    cell.reset();
                }
            }
        }
    }

    /// Clear entire current line
    pub fn clear_line(&mut self) {
        if let Some(line) = self.cells.get_mut(self.cursor_row) {
            for cell in line {
                cell.reset();
            }
        }
    }

    /// Set current text attributes
    pub fn set_attrs(&mut self, attrs: CellAttrs) {
        self.current_attrs = attrs;
    }

    /// Set current foreground color
    pub fn set_fg(&mut self, color: u32) {
        self.current_fg = color;
    }

    /// Set current background color
    pub fn set_bg(&mut self, color: u32) {
        self.current_bg = color;
    }

    /// Reset text attributes to default
    pub fn reset_attrs(&mut self) {
        self.current_attrs = CellAttrs::default();
        self.current_fg = DEFAULT_FG;
        self.current_bg = DEFAULT_BG;
    }

    /// Resize the screen
    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        // Resize existing rows
        for row in &mut self.cells {
            row.resize(new_cols, Cell::default());
        }

        // Add or remove rows
        self.cells.resize(new_rows, vec![Cell::default(); new_cols]);

        self.cols = new_cols;
        self.rows = new_rows;

        // Clamp cursor to new bounds
        self.cursor_row = self.cursor_row.min(new_rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(new_cols.saturating_sub(1));
    }

    /// Get scrollback buffer length
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Get a line from scrollback (0 is most recent)
    pub fn get_scrollback_line(&self, index: usize) -> Option<&Vec<Cell>> {
        self.scrollback
            .get(self.scrollback.len().saturating_sub(1 + index))
    }

    /// Get a row from the screen
    pub fn get_row(&self, row: usize) -> Option<&Vec<Cell>> {
        self.cells.get(row)
    }

    /// Convert screen to text (for testing/debugging)
    pub fn to_text(&self) -> String {
        let mut result = String::new();
        for row in &self.cells {
            for cell in row {
                result.push(cell.c);
            }
            result.push('\n');
        }
        result.trim_end().to_string()
    }

    /// Flatten scrollback + visible rows into checkpoint lines.
    ///
    /// Scrollback comes first (oldest at index 0), then the visible grid.
    /// Trailing empty cells on each row are trimmed so the checkpoint stays
    /// small; restore pads back to the current column width.
    pub fn to_screen_lines(&self) -> Vec<ScreenLine> {
        let mut lines = Vec::with_capacity(self.scrollback.len() + self.cells.len());
        for row in &self.scrollback {
            lines.push(cells_to_screen_line(row));
        }
        for row in &self.cells {
            lines.push(cells_to_screen_line(row));
        }
        lines
    }

    /// Rebuild this screen from a checkpointed [`TerminalState`].
    ///
    /// The last `rows` lines of `screen_buffer` become the visible grid;
    /// anything before that is scrollback. This is the inverse of
    /// [`to_screen_lines`]. A restored session is display state only — the
    /// original shell process is gone (Android will have killed it).
    pub fn restore_from_checkpoint(&mut self, state: &TerminalState) {
        let cols = (state.dimensions.0 as usize).max(1);
        let rows = (state.dimensions.1 as usize).max(1);
        self.resize(cols, rows);

        let lines = &state.screen_buffer;
        let visible_start = lines.len().saturating_sub(rows);

        self.scrollback.clear();
        for line in lines.iter().take(visible_start) {
            self.scrollback.push(screen_line_to_cells(line, cols));
            while self.scrollback.len() > self.scrollback_limit {
                self.scrollback.remove(0);
            }
        }

        self.cells = (0..rows)
            .map(|r| {
                let idx = visible_start + r;
                if idx < lines.len() {
                    screen_line_to_cells(&lines[idx], cols)
                } else {
                    vec![Cell::default(); cols]
                }
            })
            .collect();

        self.set_cursor(
            state.cursor_position.0 as usize,
            state.cursor_position.1 as usize,
        );
    }
}

/// Collapse a row of cells into a checkpoint line, merging consecutive
/// same-style runs into [`StyleSpan`]s.
fn cells_to_screen_line(cells: &[Cell]) -> ScreenLine {
    let last_nonempty = cells
        .iter()
        .rposition(|c| !c.is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);
    let slice = &cells[..last_nonempty];
    if slice.is_empty() {
        return ScreenLine {
            text: String::new(),
            styles: Vec::new(),
        };
    }

    let mut text = String::with_capacity(slice.len());
    let mut styles = Vec::new();
    let mut span_start = 0;
    let mut current = style_key(&slice[0]);

    for (i, cell) in slice.iter().enumerate() {
        text.push(cell.c);
        let key = style_key(cell);
        if key != current {
            styles.push(span_from_cell(span_start, i, &slice[span_start]));
            span_start = i;
            current = key;
        }
    }
    styles.push(span_from_cell(span_start, slice.len(), &slice[span_start]));

    ScreenLine { text, styles }
}

fn style_key(cell: &Cell) -> (u32, u32, bool, bool, bool) {
    (
        cell.fg,
        cell.bg,
        cell.attrs.bold(),
        cell.attrs.italic(),
        cell.attrs.underline(),
    )
}

fn span_from_cell(start: usize, end: usize, cell: &Cell) -> StyleSpan {
    StyleSpan {
        start,
        end,
        foreground: cell.fg,
        background: cell.bg,
        bold: cell.attrs.bold(),
        italic: cell.attrs.italic(),
        underline: cell.attrs.underline(),
    }
}

fn screen_line_to_cells(line: &ScreenLine, cols: usize) -> Vec<Cell> {
    let mut cells = vec![Cell::default(); cols];
    let chars: Vec<char> = line.text.chars().collect();
    for (i, ch) in chars.iter().enumerate().take(cols) {
        cells[i].c = *ch;
    }
    for span in &line.styles {
        let mut attrs = CellAttrs::new();
        attrs.set_bold(span.bold);
        attrs.set_italic(span.italic);
        attrs.set_underline(span.underline);
        let end = span.end.min(cols).min(chars.len());
        let start = span.start.min(end);
        for cell in cells.iter_mut().take(end).skip(start) {
            cell.fg = span.foreground;
            cell.bg = span.background;
            cell.attrs = attrs;
        }
    }
    cells
}

impl Default for Screen {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_creation() {
        let screen = Screen::new(80, 24);
        assert_eq!(screen.size(), (80, 24));
        assert_eq!(screen.cursor(), (0, 0));
    }

    #[test]
    fn test_write_char() {
        let mut screen = Screen::new(10, 5);
        screen.write_char('H');
        screen.write_char('i');

        assert_eq!(screen.cursor(), (0, 2));
        assert_eq!(screen.get_cell(0, 0).unwrap().c, 'H');
        assert_eq!(screen.get_cell(0, 1).unwrap().c, 'i');
    }

    #[test]
    fn test_line_wrap() {
        let mut screen = Screen::new(5, 3);
        for c in "Hello World".chars() {
            screen.write_char(c);
        }

        // "Hello" fills row 0 (5 chars), then wraps
        // " World" starts on row 1: space at col 0, W at col 1
        assert_eq!(screen.get_cell(0, 0).unwrap().c, 'H');
        assert_eq!(screen.get_cell(0, 4).unwrap().c, 'o');
        assert_eq!(screen.get_cell(1, 0).unwrap().c, ' '); // space after Hello
        assert_eq!(screen.get_cell(1, 1).unwrap().c, 'W');
    }

    #[test]
    fn test_scroll() {
        let mut screen = Screen::new(5, 2);
        screen.write_char('A');
        screen.newline();
        screen.write_char('B');
        screen.newline();
        screen.write_char('C');

        // Screen should have scrolled, first line goes to scrollback
        assert_eq!(screen.scrollback_len(), 1);
    }

    #[test]
    fn test_resize() {
        let mut screen = Screen::new(10, 5);
        screen.set_cursor(4, 9);
        screen.resize(5, 3);

        assert_eq!(screen.size(), (5, 3));
        assert_eq!(screen.cursor(), (2, 4)); // Clamped to new bounds
    }

    #[test]
    fn test_save_restore_cursor() {
        let mut screen = Screen::new(10, 5);
        screen.set_cursor(2, 4);
        screen.save_cursor();
        screen.set_cursor(0, 0);
        screen.restore_cursor();
        assert_eq!(screen.cursor(), (2, 4));
    }

    #[test]
    fn test_clear_to_cursor_and_line() {
        let mut screen = Screen::new(8, 3);
        for c in "AAAAAAAA".chars() {
            screen.write_char(c);
        }
        screen.newline();
        for c in "BBBBBBBB".chars() {
            screen.write_char(c);
        }
        screen.set_cursor(1, 3);
        screen.clear_line_to_cursor();
        assert_eq!(screen.get_cell(1, 0).unwrap().c, ' ');
        assert_eq!(screen.get_cell(1, 3).unwrap().c, ' ');
        assert_eq!(screen.get_cell(1, 4).unwrap().c, 'B');

        screen.set_cursor(1, 2);
        screen.clear_to_cursor();
        assert_eq!(screen.get_cell(0, 0).unwrap().c, ' ');
        assert_eq!(screen.get_cell(1, 2).unwrap().c, ' ');
        assert_eq!(screen.get_cell(1, 4).unwrap().c, 'B');
    }

    #[test]
    fn test_checkpoint_lines_roundtrip_preserves_text_and_style() {
        let mut screen = Screen::new(10, 3);
        screen.set_fg(0xFFCD0000);
        let mut attrs = CellAttrs::new();
        attrs.set_bold(true);
        screen.set_attrs(attrs);
        for c in "Hi".chars() {
            screen.write_char(c);
        }
        screen.reset_attrs();
        screen.write_char('!');

        let mut state = TerminalState::new("snap");
        state.dimensions = (10, 3);
        state.cursor_position = {
            let (r, c) = screen.cursor();
            (r as u32, c as u32)
        };
        state.screen_buffer = screen.to_screen_lines();

        let mut restored = Screen::new(10, 3);
        restored.restore_from_checkpoint(&state);

        assert_eq!(restored.get_cell(0, 0).unwrap().c, 'H');
        assert_eq!(restored.get_cell(0, 1).unwrap().c, 'i');
        assert_eq!(restored.get_cell(0, 2).unwrap().c, '!');
        assert_eq!(restored.get_cell(0, 0).unwrap().fg, 0xFFCD0000);
        assert!(restored.get_cell(0, 0).unwrap().attrs.bold());
        assert!(!restored.get_cell(0, 2).unwrap().attrs.bold());
        assert_eq!(restored.cursor(), screen.cursor());
    }

    #[test]
    fn test_checkpoint_preserves_scrollback() {
        let mut screen = Screen::new(8, 2);
        for c in "AAAA".chars() {
            screen.write_char(c);
        }
        // Push the current top row into scrollback without depending on wrap.
        screen.scroll_up();
        screen.set_cursor(0, 0);
        for c in "BBBB".chars() {
            screen.write_char(c);
        }
        screen.newline();
        for c in "CCCC".chars() {
            screen.write_char(c);
        }
        assert_eq!(screen.scrollback_len(), 1);

        let mut state = TerminalState::new("scroll");
        state.dimensions = (8, 2);
        state.screen_buffer = screen.to_screen_lines();

        let mut restored = Screen::new(8, 2);
        restored.restore_from_checkpoint(&state);
        assert_eq!(restored.scrollback_len(), 1);
        let scrolled = restored.get_scrollback_line(0).unwrap();
        assert_eq!(scrolled[0].c, 'A');
        assert_eq!(restored.get_cell(1, 0).unwrap().c, 'C');
    }
}
