//! The terminal screen: a VT parser fed with a shell's output, read back as styled rows.

use std::panic::AssertUnwindSafe;

use super::keys::Modes;
use super::{CellStyle, TermColor, TermRow, TermSpan};
use super::{queries, replay};

/// The visible screen of one terminal session (no scrollback yet).
pub struct TermScreen {
    parser: vt100::Parser,
    /// The start of a query split across two chunks of output, held back until the rest comes.
    pending: Vec<u8>,
}

impl TermScreen {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { parser: vt100::Parser::new(rows.max(1), cols.max(1), 0), pending: Vec::new() }
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
    fn process(&mut self, bytes: &[u8]) {
        let parser = &mut self.parser;
        if std::panic::catch_unwind(AssertUnwindSafe(|| parser.process(bytes))).is_err() {
            self.rebuild();
        }
    }

    /// Replaces the parser with a fresh one showing the same screen.
    fn rebuild(&mut self) {
        let (rows, cols) = self.size();
        let replay = replay::replay(self.parser.screen(), &self.rows());
        let mut parser = vt100::Parser::new(rows, cols, 0);
        parser.process(&replay);
        self.parser = parser;
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let narrower = cols.max(1) < self.size().1;
        self.parser.set_size(rows.max(1), cols.max(1));
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

    /// Every screen row as spans of equally styled cells. A wide glyph is a span of its own,
    /// so a fallback font's glyph can't push the rest of the row out of the grid.
    pub fn rows(&self) -> Vec<TermRow> {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        (0..rows).map(|r| row(screen, r, cols)).collect()
    }

    /// Where the cursor is as (row, col), or None while the program hides it.
    pub fn cursor(&self) -> Option<(u16, u16)> {
        let screen = self.parser.screen();
        if screen.hide_cursor() {
            return None;
        }
        let (r, c) = screen.cursor_position();
        let (_, cols) = screen.size();
        Some((r, c.min(cols.saturating_sub(1))))
    }

    /// The input modes the program has set.
    pub fn modes(&self) -> Modes {
        let screen = self.parser.screen();
        Modes { application_cursor: screen.application_cursor(), bracketed_paste: screen.bracketed_paste() }
    }

    /// The screen's text, rows joined with newlines, trailing blanks trimmed (for tests).
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
        let text = if contents.is_empty() { " ".to_owned() } else { contents };
        let wide = cell.is_wide();
        match spans.last_mut() {
            Some(last) if joinable && !wide && last.style == style => {
                last.text.push_str(&text);
                last.width += 1;
            }
            _ => spans.push(TermSpan { col: c, width: if wide { 2 } else { 1 }, text, style }),
        }
        joinable = !wide;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(bytes: &str) -> TermScreen {
        let mut s = TermScreen::new(24, 80);
        s.feed(bytes.as_bytes());
        s
    }

    #[test]
    fn rows_group_cells_into_styled_spans() {
        let s = screen("ab\x1b[1;31mcd\x1b[0m  ");
        let rows = s.rows();
        let spans = &rows[0].spans;
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert_eq!((spans[0].col, spans[0].text.as_str(), spans[0].style), (0, "ab", CellStyle::default()));
        assert_eq!((spans[1].col, spans[1].text.as_str()), (2, "cd"));
        assert_eq!(spans[1].style, CellStyle { fg: TermColor::Indexed(1), bold: true, ..Default::default() });
        assert!(rows[1].spans.is_empty());
    }

    #[test]
    fn indexed_and_truecolor_attributes() {
        let s = screen("\x1b[38;5;208mX\x1b[48;2;1;2;3mY\x1b[7mZ");
        let rows = s.rows();
        let spans = &rows[0].spans;
        assert_eq!(spans.len(), 3, "{spans:?}");
        assert_eq!(spans[0].style.fg, TermColor::Indexed(208));
        assert_eq!(spans[1].style.bg, TermColor::Rgb(1, 2, 3));
        assert_eq!(spans[2].text, "Z");
        assert!(spans[2].style.inverse);
    }

    #[test]
    fn wide_characters_get_their_own_two_column_span() {
        let s = screen("a漢b");
        let rows = s.rows();
        let got: Vec<(u16, &str, u16)> = rows[0].spans.iter().map(|sp| (sp.col, sp.text.as_str(), sp.width)).collect();
        assert_eq!(got, vec![(0, "a", 1), (1, "漢", 2), (3, "b", 1)]);
    }

    #[test]
    fn cursor_position_and_hidden_cursor() {
        let mut s = screen("hi");
        assert_eq!(s.cursor(), Some((0, 2)));
        s.feed(b"\x1b[?25l");
        assert_eq!(s.cursor(), None);
        let m = s.modes();
        assert!(!m.application_cursor && !m.bracketed_paste);
        s.feed(b"\x1b[?1h\x1b[?2004h");
        let m = s.modes();
        assert!(m.application_cursor && m.bracketed_paste);
    }

    #[test]
    fn answers_queries_even_when_split() {
        let mut s = TermScreen::new(24, 80);
        assert!(s.feed(b"hi\x1b[").is_empty());
        assert_eq!(s.feed(b"6n"), b"\x1b[1;3R");
        assert_eq!(s.feed(b"\x1b[5n\x1b[c"), b"\x1b[0n\x1b[?62;22c");
        assert_eq!(s.text(), "hi");
    }

    #[test]
    fn resize_changes_size_and_keeps_text() {
        let mut s = screen("hello");
        s.resize(10, 20);
        assert_eq!(s.size(), (10, 20));
        assert!(s.text().starts_with("hello"), "{:?}", s.text());
        s.resize(0, 0);
        assert_eq!(s.size(), (1, 1));
    }

    #[test]
    fn narrowing_through_a_wide_glyph_drops_it_instead_of_panicking() {
        // The glyph sits in the last two columns, and the narrower screen keeps only one of them.
        for (before, cols) in [(77, 78), (78, 79)] {
            let mut s = screen(&format!("{}漢", "a".repeat(before)));
            s.resize(24, cols);
            assert_eq!(s.size(), (24, cols));
            s.feed(b"xy");
            s.feed(b"\x1b[1;1Hz");
            let first = s.text().lines().next().unwrap_or_default().to_owned();
            assert!(first.starts_with(&format!("z{}", "a".repeat(before - 1))), "cols {cols}: {first:?}");
            assert!(!first.contains('漢') && first.chars().count() <= usize::from(cols), "{first:?}");
        }
    }

    #[test]
    fn narrowing_keeps_styles_modes_and_the_cursor() {
        let mut s = screen(&format!("\x1b[?2004h\x1b[31mred\x1b[0m\r\n{}漢\x1b[2;3H", "b".repeat(77)));
        s.resize(24, 78);
        let rows = s.rows();
        assert_eq!(rows[0].spans[0].text, "red");
        assert_eq!(rows[0].spans[0].style.fg, TermColor::Indexed(1));
        assert_eq!(rows[1].spans.iter().map(|sp| sp.text.as_str()).collect::<String>(), "b".repeat(77));
        assert_eq!(s.cursor(), Some((1, 2)));
        assert!(s.modes().bracketed_paste);
        s.feed(b"\x1b[2;78Hz");
        assert_eq!(s.text().lines().nth(1), Some(format!("{}z", "b".repeat(77)).as_str()));
    }

    #[test]
    fn a_parser_panic_resets_the_screen_instead_of_crashing() {
        // A glyph cut by narrowing the main screen while a full-screen program shows the other.
        let mut s = screen(&format!("{}漢\x1b[?1049h", "a".repeat(77)));
        s.resize(24, 78);
        s.feed(b"\x1b[?1049l\x1b[1;78Hxy");
        assert_eq!(s.size(), (24, 78));
        s.feed(b"\x1b[1;1Hok");
        assert!(s.text().starts_with("ok"), "{:?}", s.text());
    }
}
