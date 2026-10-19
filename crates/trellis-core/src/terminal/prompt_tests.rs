//! A shell prompt's trailing space: the prompt row as a pseudoconsole sends it (the text as is,
//! trimmed then erased to the end and the cursor placed, or trimmed then erased in place and
//! stepped over), at lengths around and past the right margin, through queries and rebuilds.

use super::{TermColor, TermScreen};

const COLS: usize = 80;

/// A prompt exactly `len` columns wide: "PS C:\", a path, then "> ".
fn prompt(len: usize) -> String {
    let path: String = (0..len - 8).map(|i| if i % 8 == 7 { '\\' } else { char::from(b'a' + (i % 26) as u8) }).collect();
    format!("PS C:\\{path}> ")
}

/// The prompt written as is on row `r0`, the cursor left after it.
fn literal(p: &str, r0: usize) -> String {
    format!("\x1b[?25l\x1b[{};1H{p}\x1b]0;pwsh\x07\x1b[?25h", r0 + 1)
}

/// The prompt without its trailing blank, erased to the end of the line and the cursor placed.
fn trimmed_el(p: &str, r0: usize, cursor: (usize, usize)) -> String {
    let (cr, cc) = cursor;
    format!("\x1b[?25l\x1b[{};1H{}\x1b[K\x1b[{};{}H\x1b[?25h", r0 + 1, p.trim_end(), cr + 1, cc + 1)
}

/// The prompt without its trailing blank, the blank erased in place and stepped over.
fn trimmed_ech(p: &str, r0: usize) -> String {
    format!("\x1b[{};1H{}\x1b[1X\x1b[C", r0 + 1, p.trim_end())
}

/// A cleared screen with two lines of earlier output, so prompts start on row 2.
fn screen_with_output() -> TermScreen {
    let mut s = TermScreen::new(24, 80);
    s.feed(b"\x1b[2J\x1b[m\x1b[H");
    s.feed(b"earlier output\r\nmore output\r\n");
    s
}

/// (row, col) of the prompt's `k`th column.
fn at(r0: usize, k: usize) -> (u16, u16) {
    ((r0 + k / COLS) as u16, (k % COLS) as u16)
}

fn contents(s: &TermScreen, (r, c): (u16, u16)) -> String {
    s.parser.screen().cell(r, c).map(|c| c.contents().to_owned()).unwrap_or_default()
}

/// Where the cursor shows after the prompt: after the space, or on it when the space filled
/// the last column (the next character wraps).
fn cursor_after(r0: usize, len: usize) -> (u16, u16) {
    if len.is_multiple_of(COLS) { ((r0 + len / COLS - 1) as u16, (COLS - 1) as u16) } else { at(r0, len) }
}

/// The `len`-column prompt from row `r0` shows its '>' and a blank after it, nothing later on
/// the row, and the cursor after the blank. With `typed`, a typed 'x' then lands there.
fn assert_prompt(s: &mut TermScreen, r0: usize, len: usize, typed: bool) {
    let gt = at(r0, len - 2);
    let space = at(r0, len - 1);
    assert_eq!(contents(s, gt), ">", "len {len}: {}", s.text());
    assert!(contents(s, space).trim().is_empty(), "len {len}: the space cell holds {:?}", contents(s, space));
    for c in space.1 + 1..COLS as u16 {
        assert_eq!(contents(s, (space.0, c)), "", "len {len}: column {c} after the space");
    }
    let text = s.text();
    let line = text.lines().nth(usize::from(gt.0)).unwrap_or_default();
    assert!(line.ends_with('>'), "len {len}: {line:?}");
    assert_eq!(s.cursor(), Some(cursor_after(r0, len)), "len {len}");
    if typed {
        s.feed(b"x");
        assert_eq!(contents(s, at(r0, len)), "x", "len {len}: {}", s.text());
        assert!(contents(s, space).trim().is_empty(), "len {len}: typing went over the space");
        assert_eq!(contents(s, gt), ">", "len {len}");
    }
}

#[test]
fn pwsh_prompt_keeps_its_space() {
    let r0 = 2;
    for len in [17, 60, 78, 79, 80, 81, 100, 160] {
        let p = prompt(len);
        assert_eq!(p.chars().count(), len);

        let mut s = screen_with_output();
        s.feed(literal(&p, r0).as_bytes());
        assert_prompt(&mut s, r0, len, true);

        // Placing the cursor on the last cell clears the pending wrap, so a typed character
        // would go there; the shell redraws anyway.
        let (cr, cc) = cursor_after(r0, len);
        let mut s = screen_with_output();
        s.feed(trimmed_el(&p, r0, (usize::from(cr), usize::from(cc))).as_bytes());
        assert_prompt(&mut s, r0, len, !len.is_multiple_of(COLS));

        if !len.is_multiple_of(COLS) && !(len - 1).is_multiple_of(COLS) {
            let mut s = screen_with_output();
            s.feed(trimmed_ech(&p, r0).as_bytes());
            assert_prompt(&mut s, r0, len, true);
        }
    }
}

#[test]
fn space_in_the_last_column_then_a_new_line() {
    let (r0, len) = (2, 80);
    let mut s = screen_with_output();
    let frame = format!("\x1b[{};1H{}\x1b[K\r\n", r0 + 1, prompt(len).trim_end());
    s.feed(frame.as_bytes());
    assert_eq!(contents(&s, at(r0, len - 2)), ">");
    assert_eq!(s.cursor(), Some(at(r0, len)));
    s.feed(b"x");
    assert_eq!(contents(&s, at(r0, len)), "x");
}

#[test]
fn startup_query_then_prompt() {
    let mut s = TermScreen::new(24, 80);
    assert_eq!(s.feed(b"\x1b[6n"), b"\x1b[1;1R");
    s.feed(literal(&prompt(17), 0).as_bytes());
    assert_prompt(&mut s, 0, 17, true);
}

#[test]
fn wrapping_prompt_on_the_bottom_row_scrolls() {
    let mut s = TermScreen::new(24, 80);
    let lines: String = (1..=23).map(|i| format!("line {i}\r\n")).collect();
    s.feed(lines.as_bytes());
    s.feed(literal(&prompt(100), 23).as_bytes());
    assert_eq!(s.history_len(), 1);
    assert_eq!(s.cursor(), Some((23, 20)));
    assert_eq!(contents(&s, (23, 18)), ">");
    assert!(contents(&s, (23, 19)).trim().is_empty());
}

#[test]
fn cursor_report_after_writing_the_last_column() {
    let mut s = TermScreen::new(24, 80);
    s.feed(literal(&prompt(80), 0).as_bytes());
    assert_eq!(String::from_utf8_lossy(&s.feed(b"\x1b[6n")), "\x1b[1;80R");

    let mut s = TermScreen::new(24, 80);
    s.feed(format!("\x1b[4;1H{}", "a".repeat(80)).as_bytes());
    assert_eq!(String::from_utf8_lossy(&s.feed(b"\x1b[6n")), "\x1b[4;80R");
}

#[test]
fn rebuilt_screen_keeps_the_prompt_and_its_wrap() {
    let r0 = 2;
    for len in [17, 79, 80, 100, 160] {
        let mut s = screen_with_output();
        s.feed(literal(&prompt(len), r0).as_bytes());
        s.clear_history();
        assert_prompt(&mut s, r0, len, true);
    }

    // A styled last cell is written again as it was.
    let mut s = screen_with_output();
    let p = prompt(80);
    s.feed(format!("\x1b[{};1H{}\x1b[44mZ\x1b[0m", r0 + 1, &p[..79]).as_bytes());
    s.clear_history();
    let last = at(r0, 79);
    assert_eq!(contents(&s, last), "Z");
    let rows = s.rows();
    let span = rows[usize::from(last.0)].spans.last().expect("a span");
    assert_eq!((span.col + span.width, span.style.bg), (80, TermColor::Indexed(4)));
    s.feed(b"x");
    assert_eq!(contents(&s, at(r0, 80)), "x", "{}", s.text());
    assert_eq!(contents(&s, last), "Z");
}

#[test]
fn clear_keeping_the_cursor_row_keeps_a_last_column_prompt() {
    let mut s = screen_with_output();
    s.feed(literal(&prompt(80), 5).as_bytes());
    s.clear_screen_keep_cursor_line();
    assert_eq!(contents(&s, (0, 78)), ">");
    assert_eq!(s.cursor(), Some((0, 79)));
    s.feed(b"x");
    assert_eq!(contents(&s, (1, 0)), "x", "{}", s.text());
    assert!(contents(&s, (0, 79)).trim().is_empty());
}

#[test]
fn trailing_blank_is_trimmed_but_the_cursor_is_not() {
    let mut s = TermScreen::new(24, 80);
    s.feed(literal(&prompt(17), 0).as_bytes());
    let rows = s.rows();
    let last = rows[0].spans.last().expect("a span");
    assert_eq!(last.col + last.width, 16, "the space is not a span");
    assert_eq!(s.cursor(), Some((0, 17)));

    // A blank that draws a background stays.
    let mut s = TermScreen::new(24, 80);
    let p = prompt(17);
    s.feed(format!("{}\x1b[44m> \x1b[0m", &p[..15]).as_bytes());
    let rows = s.rows();
    let last = rows[0].spans.last().expect("a span");
    assert_eq!((last.text.as_str(), last.col + last.width, last.style.bg), ("> ", 17, TermColor::Indexed(4)));
    assert_eq!(s.cursor(), Some((0, 17)));
}
