//! `OldText`'s line count and the lines after the last hunk (the run of unchanged lines below it).

use super::context_tests::hunk_at;
use super::*;

#[test]
fn the_total_counts_lines_like_the_diff_does() {
    let h = [hunk_at(0, &[(' ', "a")])];
    assert_eq!(OldText::new(b"a\nb\nc\n", &h).unwrap().total(), 3);
    assert_eq!(OldText::new(b"a\nb\nc", &h).unwrap().total(), 3, "a last line without a newline counts");
    assert_eq!(OldText::new(b"a\n\n", &h).unwrap().total(), 2, "an empty line counts");
    assert_eq!(OldText::new(b"", &[hunk_at(0, &[('+', "a")])]).unwrap().total(), 0);
}

#[test]
fn the_tail_is_what_follows_the_last_hunk() {
    let hunks = [hunk_at(0, &[('-', "a"), ('+', "A")]), hunk_at(3, &[(' ', "d"), ('-', "e"), ('+', "E")])];
    let old = OldText::whole(b"a\nb\nc\nd\ne\r\nf\ng", &hunks).unwrap();
    assert_eq!(old.total(), 7);
    assert_eq!(old.tail(&hunks), Some(&["f".to_owned(), "g".to_owned()][..]), "the last line has no newline");
    assert_eq!(old.rest().len(), 4);
    // `\r` is trimmed like the diff's lines.
    let crlf = OldText::whole(b"a\r\nb\r\nc\r\nd\r\ne\r\nf\r\n", &hunks).unwrap();
    assert_eq!(crlf.tail(&hunks), Some(&["f".to_owned()][..]));
    // The last hunk reaches the end of the file.
    let end = OldText::whole(b"a\nb\nc\nd\ne\n", &hunks).unwrap();
    assert_eq!(end.tail(&hunks), Some(&[][..]));
    // Not kept by `new`, and not for hunks that aren't the ones it was read for.
    assert_eq!(OldText::new(b"a\nb\nc\nd\ne\nf\n", &hunks).unwrap().tail(&hunks), None);
    assert_eq!(old.tail(&hunks[..1]), None);
}

#[test]
fn texts_with_the_same_lines_are_equal() {
    let hunks = [hunk_at(1, &[(' ', "b"), ('-', "c"), ('+', "C")])];
    let (a, b) = (OldText::new(b"a\nb\nc\nd\n", &hunks).unwrap(), OldText::whole(b"a\nb\nc\nd\ne\n", &hunks).unwrap());
    assert_eq!(a, b);
    assert_ne!(a, OldText::new(b"x\nb\nc\nd\n", &hunks).unwrap());
}
