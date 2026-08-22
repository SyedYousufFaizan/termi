//! Terminal cell representation
//!
//! A cell is the fundamental unit of the terminal display - one character with styling.

use serde::{Deserialize, Serialize};

/// A single cell in the terminal grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// The character in this cell (space if empty)
    pub c: char,
    /// Foreground color (ARGB format)
    pub fg: u32,
    /// Background color (ARGB format)
    pub bg: u32,
    /// Cell attributes
    pub attrs: CellAttrs,
}

/// Cell display attributes (packed into a single byte for efficiency)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellAttrs {
    bits: u8,
}

impl CellAttrs {
    const BOLD: u8 = 1 << 0;
    const ITALIC: u8 = 1 << 1;
    const UNDERLINE: u8 = 1 << 2;
    const STRIKETHROUGH: u8 = 1 << 3;
    const INVERSE: u8 = 1 << 4;
    const BLINK: u8 = 1 << 5;
    const DIM: u8 = 1 << 6;
    const HIDDEN: u8 = 1 << 7;

    pub fn new() -> Self {
        Self { bits: 0 }
    }

    pub fn bold(&self) -> bool {
        self.bits & Self::BOLD != 0
    }
    pub fn set_bold(&mut self, v: bool) {
        self.set_bit(Self::BOLD, v);
    }

    pub fn italic(&self) -> bool {
        self.bits & Self::ITALIC != 0
    }
    pub fn set_italic(&mut self, v: bool) {
        self.set_bit(Self::ITALIC, v);
    }

    pub fn underline(&self) -> bool {
        self.bits & Self::UNDERLINE != 0
    }
    pub fn set_underline(&mut self, v: bool) {
        self.set_bit(Self::UNDERLINE, v);
    }

    pub fn strikethrough(&self) -> bool {
        self.bits & Self::STRIKETHROUGH != 0
    }
    pub fn set_strikethrough(&mut self, v: bool) {
        self.set_bit(Self::STRIKETHROUGH, v);
    }

    pub fn inverse(&self) -> bool {
        self.bits & Self::INVERSE != 0
    }
    pub fn set_inverse(&mut self, v: bool) {
        self.set_bit(Self::INVERSE, v);
    }

    pub fn blink(&self) -> bool {
        self.bits & Self::BLINK != 0
    }
    pub fn set_blink(&mut self, v: bool) {
        self.set_bit(Self::BLINK, v);
    }

    pub fn dim(&self) -> bool {
        self.bits & Self::DIM != 0
    }
    pub fn set_dim(&mut self, v: bool) {
        self.set_bit(Self::DIM, v);
    }

    pub fn hidden(&self) -> bool {
        self.bits & Self::HIDDEN != 0
    }
    pub fn set_hidden(&mut self, v: bool) {
        self.set_bit(Self::HIDDEN, v);
    }

    fn set_bit(&mut self, bit: u8, v: bool) {
        if v {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
    }

    pub fn is_default(&self) -> bool {
        self.bits == 0
    }

    pub fn reset(&mut self) {
        self.bits = 0;
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            attrs: CellAttrs::default(),
        }
    }
}

impl Cell {
    /// Create a new cell with a character
    pub fn new(c: char) -> Self {
        Self {
            c,
            ..Default::default()
        }
    }

    /// Create a cell with full styling
    pub fn styled(c: char, fg: u32, bg: u32, attrs: CellAttrs) -> Self {
        Self { c, fg, bg, attrs }
    }

    /// Check if this cell is empty (space with default colors)
    pub fn is_empty(&self) -> bool {
        self.c == ' ' && self.fg == DEFAULT_FG && self.bg == DEFAULT_BG && self.attrs.is_default()
    }

    /// Reset cell to default state
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Default foreground color (white)
pub const DEFAULT_FG: u32 = 0xFFFFFFFF;
/// Default background color (black)
pub const DEFAULT_BG: u32 = 0xFF000000;

/// Standard ANSI color palette (8 basic colors)
pub mod colors {
    pub const BLACK: u32 = 0xFF000000;
    pub const RED: u32 = 0xFFCD0000;
    pub const GREEN: u32 = 0xFF00CD00;
    pub const YELLOW: u32 = 0xFFCDCD00;
    pub const BLUE: u32 = 0xFF0000EE;
    pub const MAGENTA: u32 = 0xFFCD00CD;
    pub const CYAN: u32 = 0xFF00CDCD;
    pub const WHITE: u32 = 0xFFE5E5E5;

    // Bright variants
    pub const BRIGHT_BLACK: u32 = 0xFF7F7F7F;
    pub const BRIGHT_RED: u32 = 0xFFFF0000;
    pub const BRIGHT_GREEN: u32 = 0xFF00FF00;
    pub const BRIGHT_YELLOW: u32 = 0xFFFFFF00;
    pub const BRIGHT_BLUE: u32 = 0xFF5C5CFF;
    pub const BRIGHT_MAGENTA: u32 = 0xFFFF00FF;
    pub const BRIGHT_CYAN: u32 = 0xFF00FFFF;
    pub const BRIGHT_WHITE: u32 = 0xFFFFFFFF;

    /// Get ANSI color by index (0-15)
    pub fn ansi_color(index: u8) -> u32 {
        match index {
            0 => BLACK,
            1 => RED,
            2 => GREEN,
            3 => YELLOW,
            4 => BLUE,
            5 => MAGENTA,
            6 => CYAN,
            7 => WHITE,
            8 => BRIGHT_BLACK,
            9 => BRIGHT_RED,
            10 => BRIGHT_GREEN,
            11 => BRIGHT_YELLOW,
            12 => BRIGHT_BLUE,
            13 => BRIGHT_MAGENTA,
            14 => BRIGHT_CYAN,
            15 => BRIGHT_WHITE,
            // 256-color palette (16-231 are a 6x6x6 color cube)
            16..=231 => {
                let n = index - 16;
                let r = (n / 36) % 6;
                let g = (n / 6) % 6;
                let b = n % 6;
                let r = if r == 0 { 0 } else { r * 40 + 55 };
                let g = if g == 0 { 0 } else { g * 40 + 55 };
                let b = if b == 0 { 0 } else { b * 40 + 55 };
                0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
            }
            // 232-255 are grayscale
            232..=255 => {
                let v = (index - 232) * 10 + 8;
                0xFF000000 | ((v as u32) << 16) | ((v as u32) << 8) | (v as u32)
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_default() {
        let cell = Cell::default();
        assert_eq!(cell.c, ' ');
        assert!(cell.is_empty());
    }

    #[test]
    fn test_cell_attrs() {
        let mut attrs = CellAttrs::new();
        assert!(attrs.is_default());

        attrs.set_bold(true);
        assert!(attrs.bold());
        assert!(!attrs.is_default());

        attrs.set_italic(true);
        assert!(attrs.bold());
        assert!(attrs.italic());

        attrs.reset();
        assert!(attrs.is_default());
    }

    #[test]
    fn test_ansi_colors() {
        assert_eq!(colors::ansi_color(0), colors::BLACK);
        assert_eq!(colors::ansi_color(1), colors::RED);
        assert_eq!(colors::ansi_color(15), colors::BRIGHT_WHITE);
    }
}
