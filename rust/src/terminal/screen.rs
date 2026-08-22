//! Terminal screen buffer management
//!
//! Manages the grid of cells that represents the terminal display.

use super::cell::{Cell, CellAttrs, DEFAULT_BG, DEFAULT_FG};
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
}
