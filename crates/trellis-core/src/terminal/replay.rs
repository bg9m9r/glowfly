//! Redrawing a screen on a fresh VT parser, for when the old parser's state can't be trusted.

use std::fmt::Write;

use super::{CellStyle, TermColor, TermRow};

/// Output that redraws `rows` (the screen's contents, as `TermScreen::rows` reads them) on a fresh
/// parser of the same size from its top row, restores `screen`'s input modes, pen and cursor
/// visibility, and puts the cursor at `cursor` (row, col). A glyph that doesn't fit in the
/// screen's width is left out. The scroll region, saved cursor and the screen a full-screen
/// program hides are not kept; a shell redraws those after a resize.
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
    let _ = write!(out, "\x1b[{};{}H", r + 1, c.min(cols.saturating_sub(1)) + 1);
    if screen.hide_cursor() {
        out.push_str("\x1b[?25l");
    }
    let mut bytes = out.into_bytes();
    bytes.extend(screen.input_mode_formatted());
    bytes.extend(screen.attributes_formatted());
    bytes
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
