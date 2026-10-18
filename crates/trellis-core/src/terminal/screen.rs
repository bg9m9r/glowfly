//! The terminal screen: a VT parser fed with a shell's output, read back as styled rows.

use std::panic::AssertUnwindSafe;

use super::keys::Modes;
use super::{CellStyle, TermColor, TermRow, TermSpan};
use super::{queries, replay};

/// The rows of history a session keeps above its main screen.
pub const HISTORY_ROWS: usize = 5_000;

/// One terminal session's screen and the history that scrolled off its top (terminal/history.rs
/// reads and scrolls it). Lines are numbered from the first row ever pushed into the history:
/// screen row `r` is line `pushed + r`, and the history row `d` rows above the screen is line
/// `pushed - d`. Numbers can be trusted only while `epoch` stays the same.
pub struct TermScreen {
    pub(super) parser: vt100::Parser,
    /// The start of a query split across two chunks of output, held back until the rest comes.
    pending: Vec<u8>,
    /// How many history rows the parser keeps.
    pub(super) history_cap: usize,
    /// How far the view is scrolled back into the history, in rows. Outside of these methods
    /// the parser's scrollback offset always equals it.
    pub(super) view: usize,
    /// The main screen's history length, as last read (it can't be read on the alternate screen).
    pub(super) history_len: usize,
    /// Rows ever pushed into the history.
    pub(super) pushed: u64,
    /// Bumped whenever line numbers stop following the same text (clear, resize, rebuild,
    /// switching screens, or more output at once than the history can count).
    pub(super) epoch: u64,
}

impl TermScreen {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self::with_history(rows, cols, HISTORY_ROWS)
    }

    /// A screen keeping `history` rows of history.
    pub fn with_history(rows: u16, cols: u16, history: usize) -> Self {
        Self {
            parser: vt100::Parser::new(rows.max(1), cols.max(1), history),
            pending: Vec::new(),
            history_cap: history,
            view: 0,
            history_len: 0,
            pushed: 0,
            epoch: 0,
        }
    }

    /// Parses output from the program and returns what the terminal answers to the queries in
    /// it (cursor position and status reports, device attributes). Windows' pseudoconsole asks
    /// for the cursor position at startup and waits for the answer before the shell runs.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut replies = Vec::new();
        let mut data = std::mem::take(&mut self.pending);
        data.extend_from_slice(bytes);
        let mut done = 0;
        for (end, query) in queries::find(&data) {
            self.process(&data[done..end]);
            done = end;
            replies.extend(self.answer(query));
        }
        let keep = queries::partial_tail(&data[done..]);
        self.process(&data[done..data.len() - keep]);
        self.pending = data[data.len() - keep..].to_vec();
        replies
    }

    fn answer(&self, query: queries::Query) -> Vec<u8> {
        match query {
            queries::Query::CursorPosition => {
                let (r, c) = self.parser.screen().cursor_position();
                format!("\x1b[{};{}R", r + 1, c + 1).into_bytes()
            }
            queries::Query::Status => b"\x1b[0n".to_vec(),
            queries::Query::Attributes => b"\x1b[?62;22c".to_vec(),
        }
    }

    /// Hands output to the parser. vt100 panics on some states it gets itself into (see
    /// `resize`); rather than take the app down, the screen starts over from what it showed and
    /// the rest of this output is lost. Programs redraw after a resize anyway.
    ///
    /// vt100 moves a non-zero scrollback offset up by one for each row pushed into the history
    /// (so the same rows stay in view), which counts them. At the bottom the offset is 1 for
    /// the duration, so the count works there too.
    fn process(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let alt = self.parser.screen().alternate_screen();
        let probe = if !alt && self.history_len > 0 { self.view.max(1) } else { 0 };
        self.parser.screen_mut().set_scrollback(probe);
        let parser = &mut self.parser;
        if std::panic::catch_unwind(AssertUnwindSafe(|| parser.process(bytes))).is_err() {
            self.rebuild();
            return;
        }
        if self.parser.screen().alternate_screen() != alt {
            // History isn't readable on the alternate screen: keep the main screen's length.
            self.epoch += 1;
            self.view = 0;
            self.parser.screen_mut().set_scrollback(0);
            if alt {
                self.history_len = self.read_history_len();
                self.pushed = self.pushed.max(self.history_len as u64);
                self.parser.screen_mut().set_scrollback(0);
            }
            return;
        }
        if alt {
            return;
        }
        let after = self.parser.screen().scrollback();
        let len = self.read_history_len();
        let counted = if probe > 0 { after.checked_sub(probe) } else { len.checked_sub(self.history_len) };
        let pushed_now = match counted {
            // A reset cleared the history.
            None => {
                self.epoch += 1;
                self.view = 0;
                0
            }
            Some(n) => {
                // The offset or length stopped at the cap: more may have been pushed.
                let saturated = len == self.history_cap && (probe == 0 || after == len);
                if saturated && n > 0 {
                    self.epoch += 1;
                }
                n
            }
        };
        self.pushed = (self.pushed + pushed_now as u64).max(len as u64);
        self.view = if self.view > 0 { (self.view + pushed_now).min(len) } else { 0 };
        self.parser.screen_mut().set_scrollback(self.view);
        self.history_len = len;
    }

    /// The current grid's history length. Leaves the offset at the top of the history.
    fn read_history_len(&mut self) -> usize {
        self.parser.screen_mut().set_scrollback(usize::MAX);
        self.parser.screen().scrollback()
    }

    /// Replaces the parser with a fresh one showing the same screen. The history is lost.
    pub(super) fn rebuild(&mut self) {
        let cursor = self.parser.screen().cursor_position();
        self.rebuild_with(|rows| rows, cursor);
    }

    /// Replaces the parser with a fresh one of the same size and modes showing `pick` of the
    /// screen's rows from the top, with the cursor at `cursor`. The history is lost.
    pub(super) fn rebuild_with(&mut self, pick: impl FnOnce(&[TermRow]) -> &[TermRow], cursor: (u16, u16)) {
        self.view = 0;
        self.parser.screen_mut().set_scrollback(0);
        let (rows, cols) = self.size();
        let all = self.rows();
        let replay = replay::replay_at(self.parser.screen(), pick(&all), cursor);
        let mut parser = vt100::Parser::new(rows, cols, self.history_cap);
        parser.process(&replay);
        self.parser = parser;
        self.history_len = 0;
        self.epoch += 1;
    }

    /// Resizes the screen; the view goes back to the bottom.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.view = 0;
        self.parser.screen_mut().set_scrollback(0);
        self.epoch += 1;
        let narrower = cols.max(1) < self.size().1;
        self.parser.screen_mut().set_size(rows.max(1), cols.max(1));
        // A wide glyph whose second half the new width cut off stays in vt100's grid, and the
        // next write over it panics. Redraw without it.
        if narrower && self.cut_wide_glyph() {
            self.rebuild();
        }
    }

    fn cut_wide_glyph(&self) -> bool {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        (0..rows).any(|r| screen.cell(r, cols - 1).is_some_and(vt100::Cell::is_wide))
    }

    /// (rows, cols).
    pub fn size(&self) -> (u16, u16) {
        self.parser.screen().size()
    }

    /// Every row in view (history rows while scrolled back) as spans of equally styled cells. A wide glyph is a span of its own,
    /// so a fallback font's glyph can't push the rest of the row out of the grid.
    pub fn rows(&self) -> Vec<TermRow> {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        (0..rows).map(|r| row(screen, r, cols)).collect()
    }

    /// Where the cursor is in view as (row, col), or None while the program hides it or the
    /// view is scrolled back past it.
    pub fn cursor(&self) -> Option<(u16, u16)> {
        let screen = self.parser.screen();
        if screen.hide_cursor() {
            return None;
        }
        let (r, c) = screen.cursor_position();
        let (rows, cols) = screen.size();
        let r = u16::try_from(usize::from(r) + self.view).ok().filter(|r| *r < rows)?;
        Some((r, c.min(cols.saturating_sub(1))))
    }

    /// The input modes the program has set.
    pub fn modes(&self) -> Modes {
        let screen = self.parser.screen();
        Modes { application_cursor: screen.application_cursor(), bracketed_paste: screen.bracketed_paste() }
    }

    /// The text in view, rows joined with newlines, trailing blanks trimmed (for tests).
    pub fn text(&self) -> String {
        let lines: Vec<String> = self
            .rows()
            .iter()
            .map(|r| {
                let mut line = String::new();
                let mut col = 0;
                for sp in &r.spans {
                    line.extend(std::iter::repeat_n(' ', usize::from(sp.col.saturating_sub(col))));
                    line.push_str(&sp.text);
                    col = sp.col + sp.width;
                }
                line.trim_end().to_owned()
            })
            .collect();
        lines.join("\n").trim_end().to_owned()
    }
}

fn color(c: vt100::Color) -> TermColor {
    match c {
        vt100::Color::Default => TermColor::Default,
        vt100::Color::Idx(i) => TermColor::Indexed(i),
        vt100::Color::Rgb(r, g, b) => TermColor::Rgb(r, g, b),
    }
}

fn row(screen: &vt100::Screen, r: u16, cols: u16) -> TermRow {
    let mut spans: Vec<TermSpan> = Vec::new();
    let mut joinable = false;
    for c in 0..cols {
        let Some(cell) = screen.cell(r, c) else { continue };
        if cell.is_wide_continuation() {
            continue;
        }
        let style = CellStyle {
            fg: color(cell.fgcolor()),
            bg: color(cell.bgcolor()),
            bold: cell.bold(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        };
        let contents = cell.contents();
        let text = if contents.is_empty() { " ".to_owned() } else { contents.to_owned() };
        let wide = cell.is_wide();
        // A cell can hold a character and its combining marks: it gets a span of its own, so
        // every other span has one char per column.
        let one_cell = wide || text.chars().nth(1).is_some();
        match spans.last_mut() {
            Some(last) if joinable && !one_cell && last.style == style => {
                last.text.push_str(&text);
                last.width += 1;
            }
            _ => spans.push(TermSpan { col: c, width: if wide { 2 } else { 1 }, text, style, one_cell }),
        }
        joinable = !one_cell;
    }
    // Trailing blanks that draw nothing.
    while let Some(last) = spans.last_mut() {
        let s = last.style;
        if s.bg != TermColor::Default || s.inverse || s.underline {
            break;
        }
        let kept = last.text.trim_end_matches(' ').len();
        let blanks = (last.text.len() - kept) as u16;
        if kept > 0 {
            last.text.truncate(kept);
            last.width -= blanks;
            break;
        }
        spans.pop();
    }
    TermRow { spans }
}
