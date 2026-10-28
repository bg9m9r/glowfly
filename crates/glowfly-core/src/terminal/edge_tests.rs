//! The cursor at the right edge: after a write into the last column, controls that move from or
//! edit at the cursor start from the last cell, as in xterm, and the rest keep the wrap pending.

use super::TermScreen;

/// A 4-row, 10-column screen fed `bytes` in one chunk.
fn screen(bytes: &str) -> TermScreen {
    let mut s = TermScreen::new(4, 10);
    s.feed(bytes.as_bytes());
    s
}

/// Ten letters filling row 0, the cursor left past them with the wrap pending.
const FULL: &str = "abcdefghij";

fn lines(s: &TermScreen) -> Vec<String> {
    s.text().lines().map(str::to_owned).collect()
}

#[test]
fn backspace_and_cub_start_from_the_last_cell() {
    assert_eq!(screen(&format!("{FULL}\x08X")).text(), "abcdefghXj");
    assert_eq!(screen(&format!("{FULL}\x08\x08X")).text(), "abcdefgXij");
    assert_eq!(screen(&format!("{FULL}\x1b[3DX")).text(), "abcdefXhij");
    assert_eq!(screen(&format!("{FULL}\x1b[DX")).text(), "abcdefghXj");
    assert_eq!(screen(&format!("{FULL}\x1b[20DX")).text(), "Xbcdefghij");
    // Away from the edge they move as before.
    assert_eq!(screen("abc\x08X").text(), "abX");
    assert_eq!(screen("abcdefghi\x08X").text(), "abcdefghX");
}

#[test]
fn erasing_reaches_the_last_cell() {
    for el in ["\x1b[K", "\x1b[0K", "\x1b[?K"] {
        let mut s = screen(&format!("{FULL}{el}"));
        assert_eq!(s.text(), "abcdefghi", "{el:?}");
        assert_eq!(s.cursor(), Some((0, 9)));
        // The wrap is no longer pending: the next character goes in the last cell.
        s.feed(b"X");
        assert_eq!(s.text(), "abcdefghiX", "{el:?}");
    }
    assert_eq!(screen(&format!("{FULL}\x1b[1K")).text(), "");
    assert_eq!(screen(&format!("{FULL}\x1b[X")).text(), "abcdefghi");
    assert_eq!(screen(&format!("{FULL}\x1b[5X")).text(), "abcdefghi");
    assert_eq!(screen(&format!("{FULL}\r\n12345\x1b[1;1H{FULL}\x1b[J")).text(), "abcdefghi");
}

#[test]
fn deleting_and_inserting_cells_at_the_last_cell() {
    assert_eq!(screen(&format!("{FULL}\x1b[P")).text(), "abcdefghi");
    let mut s = screen(&format!("{FULL}\x1b[@"));
    assert_eq!(s.text(), "abcdefghi");
    s.feed(b"X");
    assert_eq!(s.text(), "abcdefghiX");
}

#[test]
fn vertical_moves_drop_the_pending_wrap() {
    // Down a row with LF: the next character goes in that row's last cell, not a row further.
    assert_eq!(lines(&screen(&format!("{FULL}\nX"))), [FULL, "         X"]);
    assert_eq!(lines(&screen(&format!("\r\n{FULL}\x1b[AX"))), ["         X", FULL]);
    assert_eq!(lines(&screen(&format!("{FULL}\x1b[BX"))), [FULL, "         X"]);
    assert_eq!(lines(&screen(&format!("{FULL}\x1b[3dX"))), [FULL, "", "         X"]);
    assert_eq!(lines(&screen(&format!("\r\n{FULL}\x1bMX"))), ["         X", FULL]);
}

#[test]
fn other_controls_keep_the_wrap_pending() {
    for between in ["\x1b[1m", "\x1b[0m", "\x1b]0;title\x07", "\x1b]0;a\x08b\x1b\\", "\x1b[?25l", "\x1b[?2004h", "\x1b7\x1b8"] {
        let s = screen(&format!("{FULL}{between}X"));
        assert_eq!(lines(&s), [FULL, "X"], "{between:?}");
    }
    // A carriage return starts the row over either way.
    assert_eq!(screen(&format!("{FULL}\rX")).text(), "Xbcdefghij");
    // Sequences vt100 doesn't dispatch as these controls leave the cursor alone.
    assert_eq!(lines(&screen(&format!("{FULL}\x1b[?DX"))), [FULL, "X"]);
    assert_eq!(lines(&screen(&format!("{FULL}\x1b[1?KX"))), [FULL, "X"]);
}

#[test]
fn split_sequences_and_controls_inside_them() {
    let mut s = screen(FULL);
    s.feed(b"\x1b[");
    s.feed(b"K");
    assert_eq!(s.text(), "abcdefghi");

    let mut s = screen(FULL);
    s.feed(b"\x1b[3");
    s.feed(b"DX");
    assert_eq!(s.text(), "abcdefXhij");

    // A BS inside a control sequence runs at once, as vte runs it.
    assert_eq!(screen(&format!("{FULL}\x1b[\x08mX")).text(), "abcdefghXj");
    // CAN cancels the sequence.
    assert_eq!(lines(&screen(&format!("{FULL}\x1b[\x18KX"))), [FULL, "KX"]);
}

#[test]
fn a_wide_glyph_in_the_last_two_columns() {
    let s = screen("abcdefgh漢\x08\x08X");
    assert_eq!(s.text(), "abcdefgX漢");
    let mut s = screen("abcdefgh漢\x1b[K");
    assert_eq!(s.text(), "abcdefgh");
    s.feed(b"X");
    assert_eq!(s.text(), "abcdefgh X");
}

#[test]
fn line_editing_a_prompt_that_fills_the_row() {
    // A shell changing the typed word "fix" to "fig" at the end of a full row: back onto the
    // 'x', erase to the end, write the new letter and the space again.
    let mut s = screen("$ git fix ");
    assert_eq!(s.cursor(), Some((0, 9)));
    s.feed(b"\x08\x1b[Kg ");
    assert_eq!(s.text(), "$ git fig");
    assert_eq!(s.cursor(), Some((0, 9)));
    s.feed(b"y");
    assert_eq!(lines(&s), ["$ git fig", "y"]);
}
