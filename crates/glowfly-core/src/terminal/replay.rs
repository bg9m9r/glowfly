//! Redrawing a screen and its history on a fresh VT parser, for when the old parser's state
//! can't be trusted or the history is cleared.

use std::fmt::Write;
use std::ops::Range;

use super::screen::row;
use super::{CellStyle, TermColor, TermRow, TermSpan};

/// A row to redraw and whether the program's text wrapped from it onto the next.
pub(super) struct Line {
    pub row: TermRow,
    pub wrapped: bool,
}

/// The last `history` rows of the main screen's history (oldest first), then the screen's rows
/// `pick`. Leaves the scrollback offset at 0.
pub(super) fn lines(screen: &mut vt100::Screen, history: usize, pick: Range<u16>) -> Vec<Line> {
    let (rows, cols) = screen.size();
    let mut out = Vec::with_capacity(history + pick.len());
    let mut back = history;
    while back > 0 {
        // With the view `back` rows up, the screen's top rows are the history from there.
        screen.set_scrollback(back);
        let page = back.min(usize::from(rows));
        out.extend((0..page as u16).map(|r| Line { row: row(screen, r, cols), wrapped: screen.row_wrapped(r) }));
        back -= page;
    }
    screen.set_scrollback(0);
    out.extend(pick.map(|r| Line { row: row(screen, r, cols), wrapped: screen.row_wrapped(r) }));
    out
}

/// Output that redraws `lines` on a fresh parser of `screen`'s size, one after the other from
/// its top row, so all but the last screenful scroll into the history; a line that wrapped onto
/// the next is drawn up to the edge and the next one's first cell wraps it again, so it stays
/// wrapped. Then it restores `screen`'s input modes, pen and cursor visibility, and puts the
/// cursor at `cursor` (row, col, from the top of the screen once drawn); a col of `cols` (just
/// written into the last column) is restored by rewriting that cell, so the next character
/// still wraps. A glyph that doesn't fit in the screen's width is left out. The scroll region,
/// saved cursor and the screen a full-screen program hides are not kept; a shell redraws those
/// after a resize.
pub(super) fn replay_at(screen: &vt100::Screen, lines: &[Line], cursor: (u16, u16)) -> Vec<u8> {
    let (rows, cols) = screen.size();
    let mut out = String::new();
    if screen.alternate_screen() {
        out.push_str("\x1b[?1049h");
    }
    let mut wrapping = false;
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && !wrapping {
            out.push_str("\x1b[0m\r\n");
        }
        let end = draw(&mut out, &line.row, cols, wrapping);
        wrapping = line.wrapped && i + 1 < lines.len();
        if wrapping && end < cols {
            // Trimmed blanks: vt100 wraps only from a last cell with contents.
            let _ = write!(out, "\x1b[{}G\x1b[0m{}", end + 1, " ".repeat(usize::from(cols - end)));
        }
    }
    let top = lines.len().saturating_sub(usize::from(rows));
    let (r, c) = cursor;
    if c >= cols && cols > 0 {
        pending_wrap(&mut out, lines.get(top + usize::from(r)).map(|l| &l.row), r, cols);
    } else {
        let _ = write!(out, "\x1b[{};{}H", r + 1, c.min(cols.saturating_sub(1)) + 1);
    }
    if screen.hide_cursor() {
        out.push_str("\x1b[?25l");
    }
    let mut bytes = out.into_bytes();
    bytes.extend(screen.input_mode_formatted());
    bytes.extend(screen.attributes_formatted());
    bytes
}

/// Draws `row` on the cursor's row and returns the column just past its last drawn cell. With
/// `wrapping` the cursor is past the previous row's edge, so the first cell is written without
/// moving there (a blank when the row starts with none) and wraps.
fn draw(out: &mut String, row: &TermRow, cols: u16, wrapping: bool) -> u16 {
    let mut end = 0;
    let mut first = wrapping;
    for sp in row.spans.iter().filter(|sp| sp.col + sp.width <= cols) {
        if first && sp.col > 0 {
            out.push_str("\x1b[0m ");
        }
        if !(first && sp.col == 0) {
            let _ = write!(out, "\x1b[{}G", sp.col + 1);
        }
        first = false;
        sgr(out, sp.style);
        out.push_str(&sp.text);
        end = sp.col + sp.width;
    }
    if first {
        out.push_str("\x1b[0m ");
        end = 1;
    }
    end
}

/// Puts a fresh parser back in the state vt100 is in right after a write into the last column
/// of row `r` (its column is `cols`, and the next character wraps) by writing that cell again.
fn pending_wrap(out: &mut String, row: Option<&TermRow>, r: u16, cols: u16) {
    let last = cols - 1;
    let covers = |sp: &&TermSpan| sp.col <= last && last < sp.col + sp.width && sp.col + sp.width <= cols;
    let (start, text, style) = match row.and_then(|row| row.spans.iter().find(covers)) {
        Some(sp) if sp.one_cell => (sp.col, sp.text.clone(), sp.style),
        Some(sp) => (last, sp.text.chars().last().map_or_else(|| " ".to_owned(), String::from), sp.style),
        // A trimmed blank, like a prompt's trailing space.
        None => (last, " ".to_owned(), CellStyle::default()),
    };
    let _ = write!(out, "\x1b[{};{}H", r + 1, start + 1);
    sgr(out, style);
    out.push_str(&text);
}

/// The SGR sequence that sets exactly `style`.
fn sgr(out: &mut String, style: CellStyle) {
    out.push_str("\x1b[0");
    color(out, style.fg, 38);
    color(out, style.bg, 48);
    for (on, code) in [(style.bold, 1), (style.italic, 3), (style.underline, 4), (style.inverse, 7)] {
        if on {
            let _ = write!(out, ";{code}");
        }
    }
    out.push('m');
}

fn color(out: &mut String, c: TermColor, base: u8) {
    let _ = match c {
        TermColor::Default => Ok(()),
        TermColor::Indexed(i) => write!(out, ";{base};5;{i}"),
        TermColor::Rgb(r, g, b) => write!(out, ";{base};2;{r};{g};{b}"),
    };
}
