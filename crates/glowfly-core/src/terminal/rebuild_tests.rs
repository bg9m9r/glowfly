//! Rebuilding the screen on a fresh parser (Clear history, recovery from a parser panic, a
//! resize that cuts a wide glyph): which rows wrapped is kept, and recovery keeps the history.

use super::{TermColor, TermScreen};

/// Everything the screen and its history hold, as copying all of it reads.
fn all_text(s: &mut TermScreen) -> String {
    let (a, b) = s.select_all();
    s.text_between(a, b)
}

/// Whether the screen's row `r` wrapped onto the next.
fn wrapped(s: &TermScreen, r: u16) -> bool {
    s.parser.screen().row_wrapped(r)
}

/// A 3-row, 10-column screen holding `count` lines "row N" above `bytes`.
fn with_history(count: u32, bytes: &str) -> TermScreen {
    let mut s = TermScreen::new(3, 10);
    let out: String = (1..=count).map(|i| format!("row {i}\r\n")).collect();
    s.feed(out.as_bytes());
    s.feed(bytes.as_bytes());
    s
}

#[test]
fn clearing_the_history_keeps_wrapped_rows_wrapped() {
    let mut s = TermScreen::new(4, 10);
    s.feed(b"abcdefghijKLMNO\r\nnext");
    s.clear_history();
    assert!(wrapped(&s, 0) && !wrapped(&s, 1));
    assert_eq!(all_text(&mut s), "abcdefghijKLMNO\nnext");
    assert_eq!(s.cursor(), Some((2, 4)));
}

#[test]
fn a_wrapped_row_ending_in_blanks_or_a_wide_glyph() {
    // Blanks the program wrote at the end of the row and at the start of the next.
    let mut s = TermScreen::new(4, 10);
    s.feed(b"abcdefg    x");
    s.clear_history();
    assert!(wrapped(&s, 0));
    assert_eq!(all_text(&mut s), "abcdefg    x");

    // The next row holds only a blank.
    let mut s = TermScreen::new(4, 10);
    s.feed(b"abcdefghij \r\nend");
    s.clear_history();
    assert!(wrapped(&s, 0) && !wrapped(&s, 1));
    assert_eq!(all_text(&mut s), "abcdefghij\nend");

    let mut s = TermScreen::new(4, 10);
    s.feed("abcdefgh漢xyz".as_bytes());
    s.clear_history();
    assert!(wrapped(&s, 0));
    assert_eq!(all_text(&mut s), "abcdefgh漢xyz");
}

#[test]
fn clearing_the_screen_keeps_no_wrap_into_rows_it_dropped() {
    let mut s = TermScreen::new(4, 10);
    s.feed(b"abcdefghijKLM");
    s.clear_screen_keep_cursor_line();
    assert_eq!((s.text(), s.cursor()), ("KLM".to_owned(), Some((0, 3))));
    assert!(!wrapped(&s, 0));
}

#[test]
fn a_rebuild_keeps_the_history_and_its_wrapped_rows() {
    let mut s = with_history(2, "abcdefghijKLM\r\n1\r\n2\r\n3");
    let (text, len, cursor) = (all_text(&mut s), s.history_len(), s.cursor());
    assert_eq!(len, 4);
    let epoch = s.epoch();
    s.rebuild();
    assert_eq!((all_text(&mut s), s.history_len(), s.cursor()), (text, len, cursor));
    assert!(s.epoch() > epoch);
    s.scroll_by(2);
    assert!(wrapped(&s, 0), "the history row still wraps");
    assert_eq!(s.text(), "abcdefghij\nKLM\n1");
}

#[test]
fn a_rebuild_keeps_history_styles_within_the_cap() {
    let mut s = TermScreen::with_history(3, 10, 4);
    let out: String = (1..=20).map(|i| format!("\x1b[31mred {i}\x1b[0m\r\n")).collect();
    s.feed(out.as_bytes());
    let text = all_text(&mut s);
    assert_eq!(s.history_len(), 4);
    s.rebuild();
    assert_eq!((all_text(&mut s), s.history_len()), (text, 4));
    s.scroll_by(4);
    let rows = s.rows();
    assert_eq!((rows[0].spans[0].text.as_str(), rows[0].spans[0].style.fg), ("red 15", TermColor::Indexed(1)));
}

#[test]
fn recovering_from_a_parser_panic_keeps_the_history() {
    // A glyph cut by narrowing the main screen while a full-screen program shows the other,
    // then written over once it is back.
    let mut s = TermScreen::new(3, 80);
    let out: String = (1..=10).map(|i| format!("row {i}\r\n")).collect();
    s.feed(out.as_bytes());
    s.feed(format!("{}漢\x1b[?1049h", "a".repeat(77)).as_bytes());
    s.resize(3, 78);
    let epoch = s.epoch();
    s.feed(b"\x1b[?1049l\x1b[3;78Hxy");
    assert!(s.epoch() > epoch, "the screen was rebuilt");
    assert_eq!(s.history_len(), 8);
    let text = all_text(&mut s);
    assert!(text.starts_with("row 1\nrow 2\n"), "{text:?}");
    assert!(text.contains("row 10"), "{text:?}");
    s.feed(b"\r\nok");
    assert_eq!(s.history_len(), 9);
}
