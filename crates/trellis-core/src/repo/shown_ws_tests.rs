//! The pieces of whitespace-hidden hunk operations that need no repository.

use super::*;

fn line(origin: char, text: &str, old: Option<u32>, new: Option<u32>) -> RawLine {
    RawLine { origin, bytes: text.as_bytes().to_vec(), old, new }
}

fn set(i: &[usize]) -> BTreeSet<usize> {
    i.iter().copied().collect()
}

fn side(lines: &[(u32, &str)]) -> HashMap<u32, Vec<u8>> {
    lines.iter().map(|(n, t)| (*n, t.as_bytes().to_vec())).collect()
}

fn texts(content: &[(char, Vec<u8>)]) -> Vec<String> {
    content.iter().map(|(o, b)| format!("{o}{}", String::from_utf8_lossy(b))).collect()
}

#[test]
fn whitespace_amounts_and_ends_are_the_same() {
    assert!(same_but_whitespace(b"    x\n", b"        x\n"), "4 vs 8 spaces");
    assert!(same_but_whitespace(b"\tx  y\n", b" x y\n"), "tabs and runs collapse");
    assert!(same_but_whitespace(b"x\n", b"x  \n"), "trailing spaces");
    assert!(same_but_whitespace(b"x\t\n", b"x\n"), "trailing tab");
    assert!(same_but_whitespace(b"x\r\n", b"x\n"), "CRLF");
    assert!(same_but_whitespace(b"x", b"x\n"), "no newline at the end");
}

#[test]
fn whitespace_added_or_removed_inside_differs() {
    assert!(!same_but_whitespace(b"x\n", b" x\n"), "an indent where there was none");
    assert!(!same_but_whitespace(b"a b\n", b"ab\n"));
    assert!(!same_but_whitespace(b"x\n", b"y\n"));
}

/// The re-indent example: old `a, ····x, ····y, b`, new `a, if {, ········x, ········y, }, b`.
fn reindent() -> Vec<RawLine> {
    vec![
        line(' ', "a\n", Some(1), Some(1)),
        line('+', "if {\n", None, Some(2)),
        line(' ', "    x\n", Some(2), Some(3)),
        line(' ', "    y\n", Some(3), Some(4)),
        line('+', "}\n", None, Some(5)),
        line(' ', "b\n", Some(4), Some(6)),
    ]
}

#[test]
fn reverse_takes_new_side_bytes_by_new_number() {
    let new_side = side(&[(3, "        x\n"), (4, "        y\n")]);
    let (content, sel) = exact_context(&reindent(), &new_side, true, &set(&[1, 4])).unwrap();
    assert_eq!(texts(&content), [" a\n", "+if {\n", "         x\n", "         y\n", "+}\n", " b\n"]);
    assert_eq!(sel, set(&[1, 4]));
}

#[test]
fn forward_takes_old_side_bytes_by_old_number() {
    // Old line 2 really ends in two spaces; new line 2 is unrelated.
    let old_side = side(&[(2, "    x  \n"), (3, "nope\n")]);
    let shown = vec![line(' ', "    x\n", Some(2), Some(3)), line('-', "gone\n", Some(3), None)];
    let (content, _) = exact_context(&shown, &old_side, false, &set(&[1])).unwrap();
    assert_eq!(texts(&content), ["     x  \n", "-gone\n"], "changed lines keep their own bytes");
}

#[test]
fn unmapped_numbers_keep_the_shown_bytes() {
    let (content, _) = exact_context(&reindent(), &HashMap::new(), true, &set(&[1])).unwrap();
    assert_eq!(texts(&content)[2], "     x\n");
}

#[test]
fn context_eof_markers_are_dropped_and_the_selection_renumbered() {
    let shown = vec![
        line('-', "a\n", Some(1), None),
        line('+', "A\n", None, Some(1)),
        line(' ', "b", Some(2), Some(2)),
        line('=', "\n\\ No newline at end of file\n", None, None),
        line('+', "c", None, Some(3)),
        line('<', "\n\\ No newline at end of file\n", None, None),
    ];
    let (content, sel) = exact_context(&shown, &side(&[(2, "b\n")]), true, &set(&[1, 4])).unwrap();
    let origins: Vec<char> = content.iter().map(|(o, _)| *o).collect();
    assert_eq!(origins, ['-', '+', ' ', '+', '<'], "only the context marker goes");
    assert_eq!(content[2].1, b"b\n");
    assert_eq!(sel, set(&[1, 3]));
}

#[test]
fn a_line_that_differs_beyond_whitespace_is_refused() {
    let r = exact_context(&reindent(), &side(&[(3, "        z\n")]), true, &set(&[1]));
    match r {
        Err(Error::Unsupported(m)) => assert_eq!(m, no_counterpart(3)),
        other => panic!("{other:?}"),
    }
    assert!(no_counterpart(3).starts_with("with whitespace changes hidden, line 3 doesn't line up with the file;"));
}

#[test]
fn changed_lists_plus_and_minus() {
    assert_eq!(changed(&reindent()), set(&[1, 4]));
}
