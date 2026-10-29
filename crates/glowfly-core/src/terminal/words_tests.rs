//! Word and line selection: word characters, blanks and other characters, wide glyphs, and
//! lines the program's text wrapped across, on the screen and in the history.

use super::{TermPoint, TermScreen};

fn screen(rows: u16, cols: u16, text: &str) -> TermScreen {
    let mut s = TermScreen::new(rows, cols);
    s.feed(text.as_bytes());
    s
}

fn at(line: u64, col: u16) -> TermPoint {
    TermPoint { line, col }
}

/// The text of the word around (line, col).
fn word(s: &mut TermScreen, line: u64, col: u16) -> String {
    let (a, b) = s.word_at(at(line, col));
    s.text_between(a, b)
}

#[test]
fn words_take_paths_hashes_and_addresses() {
    let mut s = screen(4, 80, "$ git show a1b2c3d ~/src/main-file.rs (C:\\work) me@host:x+y #12");
    let top = s.top_line();
    assert_eq!(s.word_at(at(top, 12)), (at(top, 11), at(top, 18)), "the hash");
    assert_eq!(word(&mut s, top, 11), "a1b2c3d", "its first cell");
    assert_eq!(word(&mut s, top, 17), "a1b2c3d", "its last cell");
    assert_eq!(word(&mut s, top, 25), "~/src/main-file.rs");
    assert_eq!(word(&mut s, top, 40), "C:\\work", "brackets are not part of it");
    assert_eq!(word(&mut s, top, 38), "(", "another character is selected alone");
    assert_eq!(word(&mut s, top, 50), "me@host:x+y");
    assert_eq!(word(&mut s, top, 61), "#12");
    assert_eq!(s.word_at(at(top, 200)), (at(top, 63), at(top, 80)), "a column past the edge is the last cell");
    assert_eq!(s.word_at(at(top, 1)), (at(top, 1), at(top, 2)), "one blank between words");
    assert_eq!(s.word_at(at(top + 2, 5)), (at(top + 2, 0), at(top + 2, 80)), "an empty row is one run of blanks");
}

#[test]
fn a_wide_glyph_is_one_cell() {
    // 日 covers columns 3–4, 本 5–6 and 語 7–8.
    let mut s = screen(2, 20, "ab \u{65e5}\u{672c}\u{8a9e} cd");
    let top = s.top_line();
    for col in 3..=8 {
        assert_eq!(s.word_at(at(top, col)), (at(top, 3), at(top, 9)), "column {col}");
    }
    assert_eq!(word(&mut s, top, 6), "\u{65e5}\u{672c}\u{8a9e}");
    let mut s = screen(2, 20, "x \u{2192}\u{2192} y");
    let top = s.top_line();
    assert_eq!(s.word_at(at(top, 2)), (at(top, 2), at(top, 3)), "an arrow is alone");
}

#[test]
fn words_and_lines_follow_wrapped_rows() {
    // 10 columns: "aaaaaaaaaa" wraps onto "bbbb cc", and "next" is its own line.
    let mut s = screen(4, 10, "aaaaaaaaaabbbb cc\r\nnext");
    let top = s.top_line();
    assert_eq!(s.word_at(at(top + 1, 2)), (at(top, 0), at(top + 1, 4)), "the word runs back across the wrap");
    assert_eq!(s.word_at(at(top, 9)), (at(top, 0), at(top + 1, 4)), "and forward");
    assert_eq!(word(&mut s, top + 1, 6), "cc");
    assert_eq!(s.line_at(top + 1), (at(top, 0), at(top + 1, 10)));
    assert_eq!(s.line_at(top), (at(top, 0), at(top + 1, 10)));
    let (a, b) = s.line_at(top);
    assert_eq!(s.text_between(a, b), "aaaaaaaaaabbbb cc");
    assert_eq!(s.line_at(top + 2), (at(top + 2, 0), at(top + 2, 10)), "an unwrapped line is its row");
    assert_eq!(s.line_at(top + 3), (at(top + 3, 0), at(top + 3, 10)), "the empty last row");
    assert_eq!(s.line_at(top + 99), (at(top + 3, 0), at(top + 3, 10)), "a line past the screen is its last");
}

#[test]
fn lines_reach_into_the_history() {
    // 3 rows of 10: the wrapped line's first row scrolls into the history.
    let mut s = screen(3, 10, "1\r\n2\r\nxxxxxxxxxxyyyy\r\n3\r\n4");
    assert_eq!(s.text(), "yyyy\n3\n4");
    let top = s.top_line();
    let (a, b) = s.line_at(top);
    assert_eq!((a, b), (at(top - 1, 0), at(top, 10)));
    assert_eq!(s.text_between(a, b), "xxxxxxxxxxyyyy");
    assert_eq!(word(&mut s, top, 1), "xxxxxxxxxxyyyy");
    assert_eq!(s.text(), "yyyy\n3\n4", "the view stays where it was");
    // Scrolled back, the same lines are found.
    s.scroll_by(2);
    assert_eq!(s.line_at(top - 1), (at(top - 1, 0), at(top, 10)));
    assert_eq!(s.view(), 2);
    assert_eq!(s.text(), "2\nxxxxxxxxxx\nyyyy");
}
