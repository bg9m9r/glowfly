//! Word and line selection on the terminal: the word around a cell (a double-click) and the
//! line around it, following rows the program's text wrapped across (a triple-click).

use super::{TermPoint, TermScreen};

/// Characters that belong to a word besides letters and digits, so one double-click takes a
/// whole path (`~/src/a-b.rs`, `C:\work`), URL, commit hash, `user@host`, `v1.2+build` or `#12`.
pub const WORD_PUNCT: &str = "_-./\\~:@+#%";

/// What a double-click on a cell takes with it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Class {
    /// Letters, digits and `WORD_PUNCT`: a run of them.
    Word,
    /// Empty cells and spaces: a run of them.
    Blank,
    /// Any other character: that cell alone.
    Other,
}

fn class(contents: &str) -> Class {
    match contents.chars().next() {
        None => Class::Blank,
        Some(c) if c.is_whitespace() => Class::Blank,
        Some(c) if c.is_alphanumeric() || WORD_PUNCT.contains(c) => Class::Word,
        Some(_) => Class::Other,
    }
}

/// One cell of a logical line: where it is, how many columns it covers and its class.
struct Cell {
    line: u64,
    col: u16,
    width: u16,
    class: Class,
}

impl TermScreen {
    /// Shows `line` in the parser and returns its row there. Callers put the view back.
    pub(super) fn seek(&mut self, line: u64) -> u16 {
        if line >= self.pushed {
            self.parser.screen_mut().set_scrollback(0);
            (line - self.pushed) as u16
        } else {
            self.parser.screen_mut().set_scrollback((self.pushed - line) as usize);
            0
        }
    }

    /// Whether the program's text wrapped from `line` onto the next.
    fn wraps(&mut self, line: u64) -> bool {
        let row = self.seek(line);
        line < self.last_line() && self.parser.screen().row_wrapped(row)
    }

    /// The first and last lines of the logical line `line` is part of: the rows the program's
    /// text wrapped across, within the history and screen.
    fn logical(&mut self, line: u64) -> (u64, u64) {
        let line = line.clamp(self.first_line(), self.last_line());
        let mut first = line;
        while first > self.first_line() && self.wraps(first - 1) {
            first -= 1;
        }
        let mut last = line;
        while self.wraps(last) {
            last += 1;
        }
        (first, last)
    }

    /// The line `line` is part of, wrapped rows and all, from its first cell to the right edge
    /// of its last row (copying trims what is blank there).
    pub fn line_at(&mut self, line: u64) -> (TermPoint, TermPoint) {
        let (first, last) = self.logical(line);
        self.parser.screen_mut().set_scrollback(self.view);
        (TermPoint { line: first, col: 0 }, TermPoint { line: last, col: self.size().1 })
    }

    /// The word around the cell at column `at.col` of line `at.line`, as cell boundaries: a run
    /// of letters, digits and `WORD_PUNCT`, or of blanks, possibly across wrapped rows; any
    /// other character alone. A wide glyph is one cell, whichever of its columns is under `at`.
    pub fn word_at(&mut self, at: TermPoint) -> (TermPoint, TermPoint) {
        let line = at.line.clamp(self.first_line(), self.last_line());
        let (first, last) = self.logical(line);
        let cells = self.cells(first, last);
        self.parser.screen_mut().set_scrollback(self.view);
        let col = at.col.min(self.size().1.saturating_sub(1));
        let Some(i) = cells.iter().position(|c| c.line == line && c.col <= col && col < c.col + c.width) else {
            let p = TermPoint { line, col };
            return (p, p);
        };
        let kind = cells[i].class;
        let (mut a, mut b) = (i, i);
        if kind != Class::Other {
            while a > 0 && cells[a - 1].class == kind {
                a -= 1;
            }
            while b + 1 < cells.len() && cells[b + 1].class == kind {
                b += 1;
            }
        }
        let (s, e) = (&cells[a], &cells[b]);
        (TermPoint { line: s.line, col: s.col }, TermPoint { line: e.line, col: e.col + e.width })
    }

    /// Every cell of lines `first..=last`, a wide glyph's second column folded into its first.
    fn cells(&mut self, first: u64, last: u64) -> Vec<Cell> {
        let cols = self.size().1;
        let mut out = Vec::new();
        for line in first..=last {
            let row = self.seek(line);
            let screen = self.parser.screen();
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else { break };
                if cell.is_wide_continuation() {
                    continue;
                }
                let width = if cell.is_wide() { 2 } else { 1 };
                out.push(Cell { line, col, width, class: class(cell.contents()) });
            }
        }
        out
    }
}
