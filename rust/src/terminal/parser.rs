//! ANSI escape sequence parser
//!
//! Uses the vte crate (from Alacritty) to parse terminal output
//! and update the screen buffer.

use super::cell::{colors, CellAttrs, DEFAULT_BG, DEFAULT_FG};
use super::screen::Screen;
use log::{debug, trace, warn};
use std::sync::{Arc, Mutex};
use vte::{Params, Parser, Perform};

/// Terminal parser that processes PTY output and updates a Screen
pub struct TerminalParser {
    /// VTE parser state machine
    parser: Parser,
    /// Screen to update
    screen: Arc<Mutex<Screen>>,
    /// Current text attributes
    attrs: CellAttrs,
    /// Current foreground color
    fg: u32,
    /// Current background color
    bg: u32,
}

impl TerminalParser {
    /// Create a new parser with a shared screen
    pub fn new(screen: Arc<Mutex<Screen>>) -> Self {
        Self {
            parser: Parser::new(),
            screen,
            attrs: CellAttrs::default(),
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
        }
    }

    /// Process bytes from PTY output
    pub fn process(&mut self, bytes: &[u8]) {
        // Create a temporary performer to handle callbacks
        let mut performer = ParserPerformer {
            screen: self.screen.clone(),
            attrs: self.attrs,
            fg: self.fg,
            bg: self.bg,
        };

        for byte in bytes {
            self.parser.advance(&mut performer, *byte);
        }

        // Save updated state
        self.attrs = performer.attrs;
        self.fg = performer.fg;
        self.bg = performer.bg;
    }

    /// Get reference to the screen
    pub fn screen(&self) -> Arc<Mutex<Screen>> {
        self.screen.clone()
    }
}

/// Performer that handles VTE callbacks
struct ParserPerformer {
    screen: Arc<Mutex<Screen>>,
    attrs: CellAttrs,
    fg: u32,
    bg: u32,
}

impl Perform for ParserPerformer {
    /// Called when the parser encounters a printable character
    fn print(&mut self, c: char) {
        trace!("Print: {:?}", c);
        if let Ok(mut screen) = self.screen.lock() {
            screen.set_fg(self.fg);
            screen.set_bg(self.bg);
            screen.set_attrs(self.attrs);
            screen.write_char(c);
        }
    }

    /// Called when the parser encounters a C0/C1 control character
    fn execute(&mut self, byte: u8) {
        trace!("Execute: 0x{:02x}", byte);
        if let Ok(mut screen) = self.screen.lock() {
            match byte {
                // Backspace
                0x08 => {
                    screen.move_cursor(0, -1);
                }
                // Tab
                0x09 => {
                    let (row, col) = screen.cursor();
                    let next_tab = ((col / 8) + 1) * 8;
                    let (cols, _) = screen.size();
                    screen.set_cursor(row, next_tab.min(cols - 1));
                }
                // Line feed
                0x0a => {
                    screen.newline();
                }
                // Carriage return
                0x0d => {
                    let (row, _) = screen.cursor();
                    screen.set_cursor(row, 0);
                }
                // Bell
                0x07 => {
                    debug!("Bell");
                    // Could trigger a callback for UI to flash/beep
                }
                _ => {
                    trace!("Unhandled control: 0x{:02x}", byte);
                }
            }
        }
    }

    /// Called when the parser encounters a CSI (Control Sequence Introducer) sequence
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        trace!("CSI: {:?} {:?} {}", params, intermediates, action);

        // Handle SGR separately to avoid borrow conflicts
        if action == 'm' {
            self.handle_sgr(params);
            if let Ok(mut screen) = self.screen.lock() {
                screen.set_fg(self.fg);
                screen.set_bg(self.bg);
                screen.set_attrs(self.attrs);
            }
            return;
        }

        if let Ok(mut screen) = self.screen.lock() {
            match action {
                // Cursor Up
                'A' => {
                    let n = get_param(params, 0, 1) as i32;
                    screen.move_cursor(-n, 0);
                }
                // Cursor Down
                'B' => {
                    let n = get_param(params, 0, 1) as i32;
                    screen.move_cursor(n, 0);
                }
                // Cursor Forward
                'C' => {
                    let n = get_param(params, 0, 1) as i32;
                    screen.move_cursor(0, n);
                }
                // Cursor Back
                'D' => {
                    let n = get_param(params, 0, 1) as i32;
                    screen.move_cursor(0, -n);
                }
                // Cursor Position (CUP)
                'H' | 'f' => {
                    let row = get_param(params, 0, 1).saturating_sub(1);
                    let col = get_param(params, 1, 1).saturating_sub(1);
                    screen.set_cursor(row as usize, col as usize);
                }
                // Erase in Display
                'J' => {
                    let mode = get_param(params, 0, 0);
                    match mode {
                        0 => screen.clear_to_end(),
                        1 => {
                            // Clear from beginning to cursor (not commonly used)
                            warn!("Clear to cursor not implemented");
                        }
                        2 | 3 => screen.clear(),
                        _ => {}
                    }
                }
                // Erase in Line
                'K' => {
                    let mode = get_param(params, 0, 0);
                    match mode {
                        0 => screen.clear_line_from_cursor(),
                        1 => {
                            // Clear from beginning to cursor
                            warn!("Clear line to cursor not implemented");
                        }
                        2 => screen.clear_line(),
                        _ => {}
                    }
                }
                // Device Status Report
                'n' => {
                    // Typically asks for cursor position - we'd need to send response
                    debug!("DSR request (ignored)");
                }
                // Save cursor position
                's' => {
                    debug!("Save cursor (not implemented)");
                }
                // Restore cursor position
                'u' => {
                    debug!("Restore cursor (not implemented)");
                }
                _ => {
                    debug!("Unhandled CSI: {} with params {:?}", action, params);
                }
            }
        }
    }

    /// Called for OSC (Operating System Command) sequences
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        trace!("OSC: {:?}", params);
        // OSC sequences often set window title, etc.
        // For now, just log them
        if !params.is_empty() {
            if let Some(&bytes) = params.get(1) {
                if let Ok(s) = std::str::from_utf8(bytes) {
                    debug!("OSC set: {}", s);
                }
            }
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {
        // DCS sequences - not commonly needed
    }

    fn put(&mut self, _byte: u8) {
        // Part of DCS
    }

    fn unhook(&mut self) {
        // End of DCS
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        trace!("ESC: 0x{:02x}", byte);
        // Handle escape sequences like "ESC c" (reset)
        match byte {
            b'c' => {
                // Reset terminal
                if let Ok(mut screen) = self.screen.lock() {
                    screen.clear();
                    screen.reset_attrs();
                }
                self.attrs = CellAttrs::default();
                self.fg = DEFAULT_FG;
                self.bg = DEFAULT_BG;
            }
            _ => {
                debug!("Unhandled ESC: 0x{:02x}", byte);
            }
        }
    }
}

impl ParserPerformer {
    /// Handle SGR (Select Graphic Rendition) parameters
    fn handle_sgr(&mut self, params: &Params) {
        let mut iter = params.iter();

        while let Some(param) = iter.next() {
            let code = param.first().copied().unwrap_or(0);

            match code {
                // Reset all attributes
                0 => {
                    self.attrs = CellAttrs::default();
                    self.fg = DEFAULT_FG;
                    self.bg = DEFAULT_BG;
                }
                // Bold
                1 => self.attrs.set_bold(true),
                // Dim
                2 => self.attrs.set_dim(true),
                // Italic
                3 => self.attrs.set_italic(true),
                // Underline
                4 => self.attrs.set_underline(true),
                // Blink (slow)
                5 => self.attrs.set_blink(true),
                // Inverse/reverse video
                7 => self.attrs.set_inverse(true),
                // Hidden
                8 => self.attrs.set_hidden(true),
                // Strikethrough
                9 => self.attrs.set_strikethrough(true),
                // Normal intensity (not bold, not dim)
                22 => {
                    self.attrs.set_bold(false);
                    self.attrs.set_dim(false);
                }
                // Not italic
                23 => self.attrs.set_italic(false),
                // Not underlined
                24 => self.attrs.set_underline(false),
                // Not blinking
                25 => self.attrs.set_blink(false),
                // Not inverse
                27 => self.attrs.set_inverse(false),
                // Reveal (not hidden)
                28 => self.attrs.set_hidden(false),
                // Not strikethrough
                29 => self.attrs.set_strikethrough(false),
                // Foreground colors (30-37)
                30..=37 => {
                    self.fg = colors::ansi_color((code - 30) as u8);
                }
                // Default foreground
                39 => {
                    self.fg = DEFAULT_FG;
                }
                // Background colors (40-47)
                40..=47 => {
                    self.bg = colors::ansi_color((code - 40) as u8);
                }
                // Default background
                49 => {
                    self.bg = DEFAULT_BG;
                }
                // Bright foreground colors (90-97)
                90..=97 => {
                    self.fg = colors::ansi_color((code - 90 + 8) as u8);
                }
                // Bright background colors (100-107)
                100..=107 => {
                    self.bg = colors::ansi_color((code - 100 + 8) as u8);
                }
                // 256-color foreground: 38;5;n
                38 => {
                    if let Some(next) = iter.next() {
                        if next.first() == Some(&5) {
                            if let Some(color_param) = iter.next() {
                                if let Some(&n) = color_param.first() {
                                    self.fg = colors::ansi_color(n as u8);
                                }
                            }
                        } else if next.first() == Some(&2) {
                            // True color: 38;2;r;g;b
                            let r = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                            let g = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                            let b = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                            self.fg =
                                0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                        }
                    }
                }
                // 256-color background: 48;5;n
                48 => {
                    if let Some(next) = iter.next() {
                        if next.first() == Some(&5) {
                            if let Some(color_param) = iter.next() {
                                if let Some(&n) = color_param.first() {
                                    self.bg = colors::ansi_color(n as u8);
                                }
                            }
                        } else if next.first() == Some(&2) {
                            // True color: 48;2;r;g;b
                            let r = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                            let g = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                            let b = iter.next().and_then(|p| p.first().copied()).unwrap_or(0);
                            self.bg =
                                0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                        }
                    }
                }
                _ => {
                    trace!("Unknown SGR code: {}", code);
                }
            }
        }
    }
}

/// Helper to get a parameter with default value
fn get_param(params: &Params, index: usize, default: u16) -> u16 {
    params
        .iter()
        .nth(index)
        .and_then(|p| p.first().copied())
        .unwrap_or(default)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_simple_text() {
        let screen = Arc::new(Mutex::new(Screen::new(80, 24)));
        let mut parser = TerminalParser::new(screen.clone());

        parser.process(b"Hello, World!");

        let screen = screen.lock().unwrap();
        assert_eq!(screen.get_cell(0, 0).unwrap().c, 'H');
        assert_eq!(screen.get_cell(0, 12).unwrap().c, '!');
    }

    #[test]
    fn test_parser_newline() {
        let screen = Arc::new(Mutex::new(Screen::new(80, 24)));
        let mut parser = TerminalParser::new(screen.clone());

        parser.process(b"Line1\r\nLine2");

        let screen = screen.lock().unwrap();
        assert_eq!(screen.get_cell(0, 0).unwrap().c, 'L');
        assert_eq!(screen.get_cell(1, 0).unwrap().c, 'L');
    }

    #[test]
    fn test_parser_color() {
        let screen = Arc::new(Mutex::new(Screen::new(80, 24)));
        let mut parser = TerminalParser::new(screen.clone());

        // Red foreground: ESC[31m
        parser.process(b"\x1b[31mRed");

        let screen = screen.lock().unwrap();
        let cell = screen.get_cell(0, 0).unwrap();
        assert_eq!(cell.c, 'R');
        assert_eq!(cell.fg, colors::RED);
    }

    #[test]
    fn test_parser_cursor_movement() {
        let screen = Arc::new(Mutex::new(Screen::new(80, 24)));
        let mut parser = TerminalParser::new(screen.clone());

        // Move cursor to row 5, col 10: ESC[5;10H
        parser.process(b"\x1b[5;10HX");

        let screen = screen.lock().unwrap();
        // Row and col are 1-indexed in ANSI, so 5,10 becomes 4,9
        assert_eq!(screen.get_cell(4, 9).unwrap().c, 'X');
    }

    #[test]
    fn test_parser_clear() {
        let screen = Arc::new(Mutex::new(Screen::new(80, 24)));
        let mut parser = TerminalParser::new(screen.clone());

        parser.process(b"ABC");
        parser.process(b"\x1b[2J"); // Clear screen

        let screen = screen.lock().unwrap();
        assert_eq!(screen.get_cell(0, 0).unwrap().c, ' ');
    }
}
