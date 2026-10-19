//! Redrawing a screen on a fresh VT parser, for when the old parser's state can't be trusted.

use std::fmt::Write;

use super::{CellStyle, TermColor, TermRow, TermSpan};

/// Output that redraws `rows` (the screen's contents, as `TermScreen::rows` reads them) on a fresh
/// parser of the same size from its top row, restores `screen`'s input modes, pen and cursor
/// visibility, and puts the cursor at `cursor` (row, col); a col of `cols` (just written into
/// the last column) is restored by rewriting that cell, so the next character still wraps. A
/// glyph that doesn't fit in the screen's width is left out. The scroll region, saved cursor
/// and the screen a full-screen program hides are not kept; a shell redraws those after a
/// resize.
pub(super) fn replay_at(screen: &vt100::Screen, rows: &[TermRow], cursor: (u16, u16)) -> Vec<u8> {
    let (_, cols) = screen.size();
    let mut out = String::new();
    if screen.alternate_screen() {
        out.push_str("\x1b[?1049h");
    }
    for (r, row) in rows.iter().enumerate() {
        for sp in row.spans.iter().filter(|sp| sp.col + sp.width <= cols) {
            let _ = write!(out, "\x1b[{};{}H", r + 1, sp.col + 1);
            sgr(&mut out, sp.style);
            out.push_str(&sp.text);
        }
    }
    let (r, c) = cursor;
    if c >= cols && cols > 0 {
        pending_wrap(&mut out, rows.get(usize::from(r)), r, cols);
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
