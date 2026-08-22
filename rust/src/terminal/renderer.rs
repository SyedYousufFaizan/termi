//! Terminal renderer - converts screen buffer to displayable format
//!
//! Prepares terminal content for display in the Android UI.

use super::cell::Cell;
use super::screen::Screen;
use serde::{Deserialize, Serialize};

/// A styled span within a line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleSpan {
    /// Start column (inclusive)
    pub start: usize,
    /// End column (exclusive)
    pub end: usize,
    /// Foreground color (ARGB)
    pub fg: u32,
    /// Background color (ARGB)
    pub bg: u32,
    /// Is bold
    pub bold: bool,
    /// Is italic
    pub italic: bool,
    /// Is underlined
    pub underline: bool,
}

/// A single line ready for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderLine {
    /// Text content of the line
    pub text: String,
    /// Style spans for this line
    pub spans: Vec<StyleSpan>,
}

impl RenderLine {
    /// Create from a row of cells
    pub fn from_cells(cells: &[Cell]) -> Self {
        let text: String = cells.iter().map(|c| c.c).collect();
        let spans = Self::extract_spans(cells);
        
        Self { text, spans }
    }

    /// Extract style spans from cells (coalesce adjacent cells with same style)
    fn extract_spans(cells: &[Cell]) -> Vec<StyleSpan> {
        if cells.is_empty() {
            return Vec::new();
        }

        let mut spans = Vec::new();
        let mut current_span = StyleSpan {
            start: 0,
            end: 1,
            fg: cells[0].fg,
            bg: cells[0].bg,
            bold: cells[0].attrs.bold(),
            italic: cells[0].attrs.italic(),
            underline: cells[0].attrs.underline(),
        };

        for (i, cell) in cells.iter().enumerate().skip(1) {
            let same_style = cell.fg == current_span.fg
                && cell.bg == current_span.bg
                && cell.attrs.bold() == current_span.bold
                && cell.attrs.italic() == current_span.italic
                && cell.attrs.underline() == current_span.underline;

            if same_style {
                current_span.end = i + 1;
            } else {
                spans.push(current_span);
                current_span = StyleSpan {
                    start: i,
                    end: i + 1,
                    fg: cell.fg,
                    bg: cell.bg,
                    bold: cell.attrs.bold(),
                    italic: cell.attrs.italic(),
                    underline: cell.attrs.underline(),
                };
            }
        }
        
        spans.push(current_span);
        spans
    }
}

/// Renderer output - what gets sent to the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderOutput {
    /// Lines ready for display (includes scrollback if scrolled)
    pub lines: Vec<RenderLine>,
    /// Cursor position (row, col) relative to visible area
    pub cursor: (usize, usize),
    /// Whether cursor should be visible
    pub cursor_visible: bool,
    /// Total lines including scrollback
    pub total_lines: usize,
    /// Current scroll position (0 = bottom, showing current screen)
    pub scroll_offset: usize,
}

/// Renders the terminal screen to displayable format
pub struct Renderer {
    /// Whether cursor blink is currently "on"
    cursor_blink_on: bool,
    /// Scroll offset from bottom
    scroll_offset: usize,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            cursor_blink_on: true,
            scroll_offset: 0,
        }
    }

    /// Toggle cursor blink state
    pub fn toggle_cursor_blink(&mut self) {
        self.cursor_blink_on = !self.cursor_blink_on;
    }

    /// Set scroll offset
    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Scroll up (show more scrollback)
    pub fn scroll_up(&mut self, lines: usize, max_scrollback: usize) {
        self.scroll_offset = (self.scroll_offset + lines).min(max_scrollback);
    }

    /// Scroll down (show less scrollback)
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Reset scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Render the screen to output format
    pub fn render(&self, screen: &Screen) -> RenderOutput {
        let (_cols, rows) = screen.size();
        let mut lines = Vec::with_capacity(rows);
        let scrollback_len = screen.scrollback_len();
        let total_lines = scrollback_len + rows;

        if self.scroll_offset > 0 {
            // Showing scrollback
            let scroll_start = scrollback_len.saturating_sub(self.scroll_offset);
            
            for i in 0..rows {
                let line_idx = scroll_start + i;
                
                if line_idx < scrollback_len {
                    // From scrollback
                    if let Some(cells) = screen.get_scrollback_line(scrollback_len - 1 - line_idx) {
                        lines.push(RenderLine::from_cells(cells));
                    } else {
                        lines.push(RenderLine { text: String::new(), spans: Vec::new() });
                    }
                } else {
                    // From screen
                    let screen_row = line_idx - scrollback_len;
                    if let Some(cells) = screen.get_row(screen_row) {
                        lines.push(RenderLine::from_cells(cells));
                    } else {
                        lines.push(RenderLine { text: String::new(), spans: Vec::new() });
                    }
                }
            }
        } else {
            // Showing current screen
            for row in 0..rows {
                if let Some(cells) = screen.get_row(row) {
                    lines.push(RenderLine::from_cells(cells));
                } else {
                    lines.push(RenderLine { text: String::new(), spans: Vec::new() });
                }
            }
        }

        let (cursor_row, cursor_col) = screen.cursor();

        RenderOutput {
            lines,
            cursor: (cursor_row, cursor_col),
            cursor_visible: self.cursor_blink_on && self.scroll_offset == 0,
            total_lines,
            scroll_offset: self.scroll_offset,
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::terminal::cell::DEFAULT_FG;

    #[test]
    fn test_render_line() {
        let cells = vec![
            Cell::new('H'),
            Cell::new('i'),
            Cell::new(' '),
        ];
        
        let line = RenderLine::from_cells(&cells);
        assert_eq!(line.text, "Hi ");
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_style_span_coalescing() {
        let cells = vec![
            Cell::styled('A', DEFAULT_FG, 0xFF000000, Default::default()),
            Cell::styled('B', DEFAULT_FG, 0xFF000000, Default::default()),
            Cell::styled('C', 0xFFFF0000, 0xFF000000, Default::default()), // Different fg
        ];
        
        let line = RenderLine::from_cells(&cells);
        // Should have 2 spans: "AB" and "C"
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].end, 2);
        assert_eq!(line.spans[1].start, 2);
    }

    #[test]
    fn test_renderer_scroll() {
        let mut renderer = Renderer::new();
        assert_eq!(renderer.scroll_offset, 0);
        
        renderer.scroll_up(5, 100);
        assert_eq!(renderer.scroll_offset, 5);
        
        renderer.scroll_down(2);
        assert_eq!(renderer.scroll_offset, 3);
        
        renderer.scroll_to_bottom();
        assert_eq!(renderer.scroll_offset, 0);
    }
}
