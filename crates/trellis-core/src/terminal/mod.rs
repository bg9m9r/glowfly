//! Terminal emulation for the built-in terminal panel: the screen model (screen), key and paste
//! encoding (keys), PTY sessions (pty), the queries the terminal answers (queries) and redrawing
//! a screen on a fresh parser (replay).

mod keys;
pub mod pty;
mod queries;
mod replay;
mod screen;

pub use keys::{KeyMods, Modes, TermInput, TermKey, encode};
pub use screen::TermScreen;

/// A cell colour as the program set it. Indexes 0–15 are themed by the UI.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TermColor {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// How a run of cells is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CellStyle {
    pub fg: TermColor,
    pub bg: TermColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

/// Consecutive cells of one row with the same style.
#[derive(Clone, PartialEq, Debug)]
pub struct TermSpan {
    /// The first column.
    pub col: u16,
    /// The number of columns covered (a wide glyph covers two).
    pub width: u16,
    pub text: String,
    pub style: CellStyle,
}

/// One screen row as styled spans; trailing blank cells are left out.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TermRow {
    pub spans: Vec<TermSpan>,
}

/// The RGB value of 256-colour index `idx`: the 6×6×6 cube for 16–231 and the grey ramp for
/// 232–255. None for 0–15, whose colours come from the UI's theme.
pub fn xterm_rgb(idx: u8) -> Option<(u8, u8, u8)> {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match idx {
        0..=15 => None,
        16..=231 => {
            let i = idx - 16;
            Some((LEVELS[usize::from(i / 36)], LEVELS[usize::from(i / 6 % 6)], LEVELS[usize::from(i % 6)]))
        }
        _ => {
            let g = 8 + 10 * (idx - 232);
            Some((g, g, g))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm_cube_and_grey_ramp() {
        for i in 0..=15 {
            assert_eq!(xterm_rgb(i), None);
        }
        assert_eq!(xterm_rgb(16), Some((0, 0, 0)));
        assert_eq!(xterm_rgb(196), Some((255, 0, 0)));
        assert_eq!(xterm_rgb(231), Some((255, 255, 255)));
        assert_eq!(xterm_rgb(232), Some((8, 8, 8)));
        assert_eq!(xterm_rgb(255), Some((238, 238, 238)));
    }
}
