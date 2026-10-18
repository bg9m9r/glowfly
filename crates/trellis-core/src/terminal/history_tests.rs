//! The history above the main screen: scrolling, line numbers while output arrives, reading
//! text across history and screen, and when line numbers stop being trustworthy.

use super::{TermPoint, TermScreen};

/// Lines "1" to "n" on a `rows` × 20 screen, the cursor after the last.
fn numbered(rows: u16, n: u32) -> TermScreen {
    let mut s = TermScreen::new(rows, 20);
    s.feed(lines(1, n).as_bytes());
    s
}

/// "a\r\na+1\r\n…\r\nb".
fn lines(a: u32, b: u32) -> String {
    (a..=b).map(|i| i.to_string()).collect::<Vec<_>>().join("\r\n")
}

fn shown(s: &TermScreen) -> Vec<String> {
    s.text().lines().map(str::to_owned).collect()
}

fn range(a: u32, b: u32) -> Vec<String> {
    (a..=b).map(|i| i.to_string()).collect()
}

#[test]
fn scrolls_through_the_history() {
    let mut s = numbered(10, 30);
    assert_eq!(s.history_len(), 20);
    assert_eq!(shown(&s), range(21, 30));
    s.scroll_by(5);
    assert_eq!((s.view(), shown(&s)), (5, range(16, 25)));
    s.scroll_by(100);
    assert_eq!(s.view(), 20, "clamped to the history");
    assert_eq!(shown(&s).first().map(String::as_str), Some("1"));
    s.scroll_by(-5);
    // More than one screen back: vt100 0.15 panicked reading these rows.
    assert_eq!((s.view(), shown(&s)), (15, range(6, 15)));
    assert_eq!(s.rows().len(), 10);
    s.scroll_by(-100);
    assert_eq!(s.view(), 0);
    s.scroll_by(3);
    s.scroll_to_bottom();
    assert_eq!((s.view(), shown(&s)), (0, range(21, 30)));
}

#[test]
fn output_while_scrolled_back_keeps_the_view() {
    let mut s = numbered(10, 30);
    s.scroll_by(5);
    let (top, epoch) = (s.top_line(), s.epoch());
    s.feed(format!("\r\n{}", lines(31, 33)).as_bytes());
    assert_eq!(s.view(), 8, "moved up by the three rows pushed");
    assert_eq!(s.top_line(), top);
    assert_eq!(shown(&s).first().map(String::as_str), Some("16"));
    assert_eq!(s.epoch(), epoch);

    s.scroll_to_bottom();
    let (top, first) = (s.top_line(), s.first_line());
    s.feed(b"\r\n34\r\n35");
    assert_eq!(s.view(), 0);
    assert_eq!(s.top_line(), top + 2);
    assert_eq!(s.first_line(), first, "nothing dropped off the front yet");
    assert_eq!(shown(&s).last().map(String::as_str), Some("35"));
    assert_eq!(s.last_line(), s.top_line() + 9);
}

#[test]
fn text_between_history_and_screen() {
    let mut s = TermScreen::with_history(5, 20, 100);
    s.feed("h1\r\nh2\r\nh3\r\nh4\r\ntrail   \r\nabcdefghijklmnopqrstuvwxy\r\n\u{6f22}\u{5b57}x\r\nend".as_bytes());
    assert_eq!(s.history_len(), 4);
    let (start, end) = s.select_all();
    assert_eq!(start, TermPoint { line: s.first_line(), col: 0 });
    assert_eq!(end, TermPoint { line: s.last_line(), col: 20 });
    assert_eq!(s.text_between(start, end), "h1\nh2\nh3\nh4\ntrail\nabcdefghijklmnopqrstuvwxy\n\u{6f22}\u{5b57}x\nend");

    let first = s.first_line();
    let at = |line: u64, col: u16| TermPoint { line: first + line, col };
    assert_eq!(s.text_between(at(5, 3), at(4, 2)), "ail\nabc", "either order");
    assert_eq!(s.text_between(at(1, 0), at(3, 0)), "h2\nh3");
    assert_eq!(s.text_between(at(7, 1), at(7, 5)), "\u{5b57}x", "a wide glyph's second cell is skipped");
    assert_eq!(s.text_between(at(2, 1), at(2, 1)), "");

    // Reading the history doesn't move the view.
    s.scroll_by(2);
    let before = s.text();
    s.text_between(start, end);
    assert_eq!((s.view(), s.text()), (2, before));
}

#[test]
fn select_all_ends_at_the_last_row_with_text() {
    let mut s = TermScreen::new(10, 20);
    s.feed(b"one\r\ntwo");
    let (start, end) = s.select_all();
    assert_eq!((start, end), (TermPoint { line: 0, col: 0 }, TermPoint { line: 1, col: 20 }));
    assert_eq!(s.text_between(start, end), "one\ntwo");
    let mut empty = TermScreen::new(10, 20);
    let (a, b) = empty.select_all();
    assert_eq!(empty.text_between(a, b), "");
}

#[test]
fn select_all_on_the_alternate_screen_copies_only_that_screen() {
    let mut s = TermScreen::new(5, 20);
    s.feed(lines(1, 12).as_bytes());
    assert_eq!(s.history_len(), 7);
    s.feed(b"\x1b[?1049h\x1b[H\x1b[2Jtop\r\nmiddle\r\nbottom");
    assert!(s.alternate());
    assert_eq!(s.first_line(), s.top_line(), "no history on the alternate screen");
    let (start, end) = s.select_all();
    assert_eq!(start, TermPoint { line: s.top_line(), col: 0 });
    assert_eq!(s.text_between(start, end), "top\nmiddle\nbottom");
    // A range that reaches back past the screen is cut at its top row.
    let from_history = TermPoint { line: 0, col: 0 };
    assert_eq!(s.text_between(from_history, end), "top\nmiddle\nbottom");

    s.feed(b"\x1b[?1049l");
    assert_eq!(s.first_line(), s.top_line() - 7, "the main screen's history is back");
    let (start, end) = s.select_all();
    assert_eq!(s.text_between(start, end), lines(1, 12).replace("\r\n", "\n"));
}

#[test]
fn line_numbers_are_dropped_when_they_can_no_longer_be_followed() {
    // More output in one chunk than the history can count.
    let mut s = TermScreen::with_history(10, 20, 5);
    s.feed(lines(1, 15).as_bytes());
    assert_eq!(s.history_len(), 5);
    s.scroll_by(2);
    let epoch = s.epoch();
    s.feed(format!("\r\n{}", lines(16, 45)).as_bytes());
    assert!(s.epoch() > epoch, "saturated");
    assert_eq!(s.view(), 5);

    let epoch = s.epoch();
    s.resize(8, 30);
    assert!(s.epoch() > epoch);
    assert_eq!(s.view(), 0);

    s.scroll_by(2);
    let epoch = s.epoch();
    s.feed(b"\x1b[?1049h");
    assert!(s.alternate() && s.epoch() > epoch && s.view() == 0);
    s.scroll_by(3);
    assert_eq!(s.view(), 0, "no history on the alternate screen");
    let epoch = s.epoch();
    s.feed(b"\x1b[?1049l");
    assert!(!s.alternate() && s.epoch() > epoch);
    assert_eq!(s.history_len(), 5);
    s.scroll_by(1);
    assert_eq!(s.view(), 1);
}

#[test]
fn clearing_the_history_keeps_the_screen() {
    let mut s = numbered(10, 30);
    s.feed(b"\x1b[?2004h\x1b[3;4H");
    let (text, cursor, epoch) = (s.text(), s.cursor(), s.epoch());
    s.scroll_by(4);
    s.clear_history();
    assert_eq!((s.history_len(), s.view()), (0, 0));
    assert_eq!(s.text(), text);
    assert_eq!(s.cursor(), cursor);
    assert!(s.modes().bracketed_paste);
    assert!(s.epoch() > epoch);
    s.scroll_by(3);
    assert_eq!(s.view(), 0);
}

#[test]
fn clearing_the_screen_keeps_the_cursor_row() {
    let mut s = numbered(10, 30);
    s.feed(b"\r\nPS> dir");
    s.clear_screen_keep_cursor_line();
    assert_eq!((s.text(), s.history_len()), ("PS> dir".to_owned(), 0));
    assert_eq!(s.cursor(), Some((0, 7)));
    s.feed(b"x");
    assert_eq!(s.text(), "PS> dirx");
}

#[test]
fn the_cursor_follows_the_view() {
    let mut s = numbered(10, 30);
    s.feed(b"\x1b[6;3H");
    assert_eq!(s.cursor(), Some((5, 2)));
    s.scroll_by(1);
    assert_eq!(s.cursor(), Some((6, 2)));
    s.scroll_by(4);
    assert_eq!(s.cursor(), None, "scrolled out of sight");
}
