//! `copy` takes exactly the selected code: no line numbers or signs (unless asked for), no
//! newlines from wrapping, tabs kept, across hunks and the gaps between them, and in split view
//! one side only; words and lines grow from a double- or triple-click.

use super::*;

fn key() -> (DiffTarget, u64) {
    (DiffTarget::Unstaged("app.rs".into()), 3)
}

fn hunk(hunk: usize, line: usize) -> LineRef {
    LineRef::Hunk { hunk, line }
}

fn pos(line: LineRef, col: usize) -> TextPos {
    TextPos { line, col }
}

fn sel(anchor: TextPos, head: TextPos, unit: Unit) -> TextSel {
    TextSel { anchor, head, unit, side: TextSide::Inline, key: key() }
}

/// Two hunks as the inline view shows them, with the unchanged lines between them filled in:
/// hunk 0 is ` fn main() {`, `-\tlet a = 1;`, `+\tlet a = 2;`; the gap above hunk 1 is one
/// line; hunk 1 is ` }` and `+// a long line that wraps on screen but is one line`.
fn lines() -> Vec<ShownLine<'static>> {
    let l = |line, origin, text| ShownLine { line, origin, text };
    vec![
        l(hunk(0, 0), ' ', "fn main() {"),
        l(hunk(0, 1), '-', "\tlet a = 1;"),
        l(hunk(0, 2), '+', "\tlet a = 2;"),
        l(LineRef::Gap { hunk: 1, line: 0 }, ' ', "\tprintln!(\"{a}\");"),
        l(hunk(1, 0), ' ', "}"),
        l(hunk(1, 1), '+', "// a long line that wraps on screen but is one line"),
    ]
}

#[test]
fn copy_takes_only_the_code_with_tabs_and_no_signs() {
    let s = sel(pos(hunk(0, 0), 3), pos(hunk(0, 2), 4), Unit::Char);
    assert_eq!(copy(&lines(), &s, false), "main() {\n\tlet a = 1;\n\tlet");
    // The other way round copies the same.
    let back = sel(s.head, s.anchor, Unit::Char);
    assert_eq!(copy(&lines(), &back, false), "main() {\n\tlet a = 1;\n\tlet");
}

#[test]
fn copy_crosses_hunks_and_gaps_without_their_headers() {
    let s = sel(pos(hunk(0, 2), 1), pos(hunk(1, 1), 4), Unit::Char);
    assert_eq!(copy(&lines(), &s, false), "let a = 2;\n\tprintln!(\"{a}\");\n}\n// a");
}

#[test]
fn a_wrapped_line_copies_as_one_line() {
    let s = sel(pos(hunk(1, 1), 0), pos(hunk(1, 1), 99), Unit::Char);
    assert_eq!(copy(&lines(), &s, false), "// a long line that wraps on screen but is one line");
}

#[test]
fn a_selection_past_a_line_end_takes_its_newline() {
    // To the start of the next line: what the nub shows is copied as the line break.
    let s = sel(pos(hunk(0, 0), 0), pos(hunk(0, 1), 0), Unit::Char);
    assert_eq!(copy(&lines(), &s, false), "fn main() {\n");
}

#[test]
fn split_copies_one_side_only() {
    // The new side of hunk 0 shows the context line and the added one, never the removed.
    let new: Vec<ShownLine> = lines().into_iter().filter(|l| l.origin != '-').collect();
    let s = TextSel { side: TextSide::New, ..sel(pos(hunk(0, 0), 0), pos(hunk(0, 2), 99), Unit::Char) };
    assert_eq!(copy(&new, &s, false), "fn main() {\n\tlet a = 2;");
    let old: Vec<ShownLine> = lines().into_iter().filter(|l| l.origin != '+').collect();
    let s = TextSel { side: TextSide::Old, ..sel(pos(hunk(0, 0), 0), pos(hunk(0, 1), 99), Unit::Char) };
    assert_eq!(copy(&old, &s, false), "fn main() {\n\tlet a = 1;");
    // A line the side doesn't show selects nothing.
    let s = TextSel { side: TextSide::Old, ..sel(pos(hunk(0, 0), 0), pos(hunk(0, 2), 2), Unit::Char) };
    assert_eq!(copy(&old, &s, false), "");
}

#[test]
fn signs_come_first_when_asked_for() {
    let s = sel(pos(hunk(0, 0), 0), pos(hunk(0, 2), 99), Unit::Char);
    assert_eq!(copy(&lines(), &s, true), " fn main() {\n-\tlet a = 1;\n+\tlet a = 2;");
}

#[test]
fn nothing_selected_copies_nothing() {
    let at = pos(hunk(0, 1), 2);
    assert_eq!(copy(&lines(), &sel(at, at, Unit::Char), false), "");
    assert_eq!(copy(&[], &sel(at, pos(hunk(0, 2), 1), Unit::Char), false), "", "lines not shown");
}

#[test]
fn words_and_lines_grow_from_the_press() {
    let at = pos(hunk(0, 1), 2); // inside `let`
    assert_eq!(copy(&lines(), &sel(at, at, Unit::Word), false), "let");
    assert_eq!(copy(&lines(), &sel(at, at, Unit::Line), false), "\tlet a = 1;");
    // Dragging on after a double-click grows by words, either way.
    assert_eq!(copy(&lines(), &sel(at, pos(hunk(0, 2), 5), Unit::Word), false), "let a = 1;\n\tlet a");
    assert_eq!(copy(&lines(), &sel(at, pos(hunk(0, 0), 4), Unit::Word), false), "main() {\n\tlet");
    assert_eq!(copy(&lines(), &sel(at, pos(hunk(0, 2), 0), Unit::Line), false), "\tlet a = 1;\n\tlet a = 2;");
}

#[test]
fn word_at_follows_the_kind_of_character() {
    let t = "foo_bar1 == x;;  y";
    assert_eq!(word_at(t, 2), (0, 8), "letters, digits and _");
    assert_eq!(word_at(t, 9), (9, 11), "a run of one punctuation character");
    assert_eq!(word_at(t, 13), (13, 15), "`;;`, not the space after it");
    assert_eq!(word_at(t, 15), (15, 17), "whitespace");
    assert_eq!(word_at("a=-b", 1), (1, 2), "different punctuation stays apart");
    assert_eq!(word_at(t, 99), (17, 18), "past the end: the last word");
    assert_eq!(word_at("", 0), (0, 0));
    assert_eq!(word_at("été", 1), (0, 3), "letters beyond ASCII");
}

#[test]
fn a_word_drag_ending_at_a_word_end_stops_there() {
    // `	let a = 2;`: col 4 is the end of `let`, before the space.
    let at = pos(hunk(0, 1), 2);
    assert_eq!(
        copy(&lines(), &sel(at, pos(hunk(0, 2), 4), Unit::Word), false),
        "let a = 1;
	let"
    );
    // Back toward the start, an end at a word's end is the anchor's own word.
    assert_eq!(copy(&lines(), &sel(pos(hunk(0, 2), 4), pos(hunk(0, 2), 4), Unit::Word), false), " ");
}
