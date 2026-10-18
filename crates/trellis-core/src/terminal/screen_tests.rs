//! The screen model's tests: styled spans, wide glyphs, queries, resizing and panic recovery.

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
fn combining_marks_stay_in_their_cell() {
    let s = screen("e\u{301}x\u{e9}\u{2500}a\u{308}\u{301}b");
    let rows = s.rows();
    let got: Vec<(u16, &str, u16, bool)> = rows[0].spans.iter().map(|sp| (sp.col, sp.text.as_str(), sp.width, sp.one_cell)).collect();
    let want = vec![(0, "e\u{301}", 1, true), (1, "x\u{e9}\u{2500}", 3, false), (4, "a\u{308}\u{301}", 1, true), (5, "b", 1, false)];
    assert_eq!(got, want);
    let cells: Vec<Vec<(u16, &str)>> = rows[0].spans.iter().map(TermSpan::cells).collect();
    assert_eq!(cells[0], [(0, "e\u{301}")]);
    assert_eq!(cells[1], [(0, "x"), (1, "\u{e9}"), (2, "\u{2500}")], "one char per column");
    assert_eq!(cells[3], [(0, "b")]);
    let wide = screen("\u{6f22}\u{301}");
    assert_eq!(wide.rows()[0].spans[0].cells(), [(0, "\u{6f22}\u{301}")], "a wide glyph with its mark");
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
