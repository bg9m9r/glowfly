use super::*;
use crate::conflict_pick::ChunkPick;
use crate::repo::{Choice, Chunk, ConflictContent, ConflictIds, Operation, Segment, SideLabels};

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| format!("{s}\n")).collect()
}

/// "top", chunk 0 (ours "a1 é", theirs "b1"), "mid", chunk 1 (ours "x", theirs "y"), "end".
fn file() -> ConflictFile {
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() },
        content: ConflictContent::Text(vec![
            Segment::Clean(lines(&["top"])),
            Segment::Conflict(Chunk { base: Vec::new(), ours: lines(&["a1 é"]), theirs: lines(&["b1"]) }),
            Segment::Clean(lines(&["mid"])),
            Segment::Conflict(Chunk { base: Vec::new(), ours: lines(&["x"]), theirs: lines(&["y"]) }),
            Segment::Clean(lines(&["end"])),
        ]),
        no_ancestor: false,
        base_markers: false,
    }
}

/// Chunk 0 takes ours; chunk 1 is open (markers).
fn before() -> EditText {
    edit_text(&file(), &[ChunkPick::Sides(Choice::Ours), ChunkPick::default()]).unwrap()
}

fn edit(before: &EditText, at: usize, remove: usize, insert: &str) -> String {
    let mut s = before.text.clone();
    s.replace_range(at..at + remove, insert);
    s
}

fn chunk(k: usize, v: &[&str]) -> EditResult {
    EditResult::Chunk { chunk: k, lines: lines(v) }
}

#[test]
fn the_text_has_markers_for_open_chunks() {
    let b = before();
    assert_eq!(b.text, "top\na1 é\nmid\n<<<<<<< main\nx\n=======\ny\n>>>>>>> other\nend\n");
    assert_eq!(&b.text[b.chunks[0].clone()], "a1 é\n");
    assert_eq!(edit_text(&file(), &[ChunkPick::default()]), None);
}

#[test]
fn edits_inside_a_chunk() {
    let b = before();
    let c0 = b.chunks[0].start;
    assert_eq!(locate_edit(&b, &edit(&b, c0 + 1, 0, "Z"), "\n"), chunk(0, &["aZ1 é"]));
    assert_eq!(locate_edit(&b, &edit(&b, c0, 0, "Z"), "\n"), chunk(0, &["Za1 é"]), "at its first character");
    let end = b.chunks[0].end;
    assert_eq!(locate_edit(&b, &edit(&b, end - 1, 0, "!"), "\n"), chunk(0, &["a1 é!"]), "at its last character");
    assert_eq!(locate_edit(&b, &edit(&b, end, 0, "more\n"), "\n"), chunk(0, &["a1 é", "more"]), "appended at its end");
    assert_eq!(locate_edit(&b, &edit(&b, c0, end - c0, ""), "\n"), EditResult::Chunk { chunk: 0, lines: Vec::new() });
    let r1 = b.chunks[1].clone();
    assert_eq!(locate_edit(&b, &edit(&b, r1.start, r1.len(), "mine\n"), "\n"), chunk(1, &["mine"]), "markers replaced");
    assert_eq!(locate_edit(&b, &b.text, "\n"), EditResult::Unchanged);
}

#[test]
fn edits_outside_the_chunks() {
    let b = before();
    assert_eq!(locate_edit(&b, &edit(&b, 1, 0, "X"), "\n"), EditResult::Outside, "in the top line");
    // Typed at the start of the clean line after a chunk, without a line break: that line.
    assert_eq!(locate_edit(&b, &edit(&b, b.chunks[0].end, 0, "X"), "\n"), EditResult::Outside);
    let (a, z) = (b.chunks[0].start + 1, b.chunks[1].start + 2);
    assert_eq!(locate_edit(&b, &edit(&b, a, z - a, "q"), "\n"), EditResult::Outside, "across two chunks");
}

#[test]
fn line_endings_and_wide_characters() {
    let b = before();
    let at = b.text.find('é').unwrap();
    // é replaced by è: the bytes share a lead byte, the change starts on the character.
    assert_eq!(locate_edit(&b, &edit(&b, at, 'é'.len_utf8(), "è"), "\r\n"), EditResult::Chunk { chunk: 0, lines: vec!["a1 è\r\n".into()] });
    let after = edit(&b, at + 'é'.len_utf8(), 0, "ü\r\nnew");
    assert_eq!(locate_edit(&b, &after, "\r\n"), EditResult::Chunk { chunk: 0, lines: vec!["a1 éü\r\n".into(), "new\r\n".into()] });
}

#[test]
fn repeated_text_is_placed_in_a_chunk_when_it_can_be() {
    // Chunk 1 takes "end", as the clean line after it reads: a copy of that line typed at the
    // chunk's start reads, by the common start alone, as typed after the clean line.
    let mut f = file();
    if let ConflictContent::Text(segs) = &mut f.content
        && let Segment::Conflict(c) = &mut segs[3]
    {
        c.ours = lines(&["end"]);
    }
    let b = edit_text(&f, &[ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Ours)]).unwrap();
    assert_eq!(
        b.text,
        "top
a1 é
mid
end
end
"
    );
    let start = b.chunks[1].start;
    assert_eq!(
        locate_edit(
            &b,
            &edit(
                &b, start, 0, "end
"
            ),
            "
"
        ),
        chunk(1, &["end", "end"])
    );
    // Deleting the clean copy leaves the same text as deleting the chunk's: the chunk's it is.
    let clean = b.chunks[1].end;
    assert_eq!(
        locate_edit(
            &b,
            &edit(&b, clean, 4, ""),
            "
"
        ),
        EditResult::Chunk { chunk: 1, lines: Vec::new() }
    );
}
