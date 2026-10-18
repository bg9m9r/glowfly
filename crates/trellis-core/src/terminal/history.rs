//! The history above a terminal's main screen: scrolling the view into it, absolute line
//! numbers for selections, reading the text between two points, and clearing it.

use super::{TermPoint, TermScreen};

impl TermScreen {
    /// Scrolls the view `lines` rows back into the history (forward when negative), within the
    /// history. Nothing happens on the alternate screen, which has none.
    pub fn scroll_by(&mut self, lines: i32) {
        if self.alternate() {
            return;
        }
        let to = (self.view as i64 + i64::from(lines)).clamp(0, self.history_len as i64);
        self.view = to as usize;
        self.parser.screen_mut().set_scrollback(self.view);
    }

    /// Puts the main screen back in view.
    pub fn scroll_to_bottom(&mut self) {
        self.view = 0;
        self.parser.screen_mut().set_scrollback(0);
    }

    /// How many rows the view is scrolled back; 0 shows the screen.
    pub fn view(&self) -> usize {
        self.view
    }

    /// The rows of history kept above the main screen.
    pub fn history_len(&self) -> usize {
        self.history_len
    }

    /// Changes whenever line numbers stop following the same text, so a selection made under
    /// an older epoch no longer applies.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The line shown in the view's top row.
    pub fn top_line(&self) -> u64 {
        self.pushed - self.view as u64
    }

    /// The oldest line in the history (the screen's top row when there is none).
    pub fn first_line(&self) -> u64 {
        self.pushed - self.history_len as u64
    }

    /// The screen's bottom row.
    pub fn last_line(&self) -> u64 {
        self.pushed + u64::from(self.size().0) - 1
    }

    /// Whether a full-screen program has the alternate screen up.
    pub fn alternate(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    /// The text from `start` up to `end` (cell boundaries, end exclusive), in reading order.
    /// Empty cells are spaces, trailing blanks are trimmed, rows end with a newline unless the
    /// program's text wrapped onto the next, and trailing empty lines are left out.
    pub fn text_between(&mut self, start: TermPoint, end: TermPoint) -> String {
        let (start, end) = super::ordered(start, end);
        let start = start.max(TermPoint { line: self.first_line(), col: 0 });
        let end = end.min(TermPoint { line: self.last_line() + 1, col: 0 });
        let cols = self.size().1;
        let mut out = String::new();
        let mut line = start.line;
        while line <= end.line && (TermPoint { line, col: 0 }) < end {
            let from = if line == start.line { start.col } else { 0 };
            let to = if line == end.line { end.col } else { cols };
            let (text, wrapped) = self.line_text(line, from, to);
            out.push_str(&text);
            let last = line == end.line || TermPoint { line: line + 1, col: 0 } >= end;
            if !last && !(wrapped && to == cols) {
                out.push('\n');
            }
            line += 1;
        }
        self.parser.screen_mut().set_scrollback(self.view);
        out.trim_end_matches('\n').to_owned()
    }

    /// Columns `from..to` of `line` and whether it wraps onto the next; trimmed unless it does.
    /// Leaves the parser's offset where reading put it.
    fn line_text(&mut self, line: u64, from: u16, to: u16) -> (String, bool) {
        let row = if line >= self.pushed {
            self.parser.screen_mut().set_scrollback(0);
            (line - self.pushed) as u16
        } else {
            self.parser.screen_mut().set_scrollback((self.pushed - line) as usize);
            0
        };
        let screen = self.parser.screen();
        let mut text = String::new();
        for c in from..to {
            let Some(cell) = screen.cell(row, c) else { break };
            if cell.is_wide_continuation() {
                continue;
            }
            let contents = cell.contents();
            text.push_str(if contents.is_empty() { " " } else { contents });
        }
        let wrapped = screen.row_wrapped(row);
        if !(wrapped && to == self.size().1) {
            text.truncate(text.trim_end().len());
        }
        (text, wrapped)
    }

    /// From the oldest history row to the end of the screen's last row with text.
    pub fn select_all(&mut self) -> (TermPoint, TermPoint) {
        self.parser.screen_mut().set_scrollback(0);
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let filled = |r: u16| (0..cols).any(|c| screen.cell(r, c).is_some_and(|cell| !cell.contents().trim().is_empty()));
        let end = match (0..rows).rev().find(|r| filled(*r)) {
            Some(r) => TermPoint { line: self.pushed + u64::from(r), col: cols },
            None => TermPoint { line: self.pushed, col: 0 },
        };
        self.parser.screen_mut().set_scrollback(self.view);
        (TermPoint { line: self.first_line(), col: 0 }, end)
    }

    /// Forgets the history, keeping the screen, its modes, pen and cursor.
    pub fn clear_history(&mut self) {
        self.scroll_to_bottom();
        self.rebuild();
    }

    /// Clears the screen and the history except for the cursor's row, which moves to the top
    /// with the cursor (for shells that don't redraw on Ctrl+L).
    pub fn clear_screen_keep_cursor_line(&mut self) {
        self.scroll_to_bottom();
        let (r, c) = self.parser.screen().cursor_position();
        let r = usize::from(r);
        self.rebuild_with(|rows| rows.get(r..=r).unwrap_or_default(), (0, c));
    }
}
