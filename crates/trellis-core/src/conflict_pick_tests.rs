use super::*;
use crate::repo::{ConflictIds, Operation, SideLabels, render_output};
use Side::{Ours, Theirs};

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| format!("{s}\n")).collect()
}

/// Ours [o0, o1], theirs [t0, t1, t2].
fn chunk() -> Chunk {
    Chunk { base: lines(&["b0"]), ours: lines(&["o0", "o1"]), theirs: lines(&["t0", "t1", "t2"]) }
}

fn r(side: Side, line: usize) -> LineRef {
    LineRef { side, line }
}

fn ticks(c: &Chunk, from: ChunkPick, seq: &[(Side, usize)]) -> ChunkPick {
    seq.iter().fold(from, |p, &(s, l)| p.toggle_line(c, s, l))
}

#[test]
fn toggle_line_follows_tick_order() {
    let c = chunk();
    let p = ticks(&c, ChunkPick::default(), &[(Theirs, 1), (Ours, 0)]);
    assert_eq!(p, ChunkPick::Lines(vec![r(Theirs, 1), r(Ours, 0)]));
    assert_eq!(p.out_lines(&c), ["t1\n", "o0\n"]);
    assert!(p.takes_line(&c, Theirs, 1) && !p.takes_line(&c, Theirs, 0));
    assert!(p.partly_takes_side(&c, Ours) && !p.takes_side(&c, Ours));
    assert!(!p.is_open());
}

#[test]
fn normalize_back_to_sides() {
    let c = chunk();
    assert_eq!(ticks(&c, ChunkPick::default(), &[(Ours, 0), (Ours, 1)]), ChunkPick::Sides(Choice::Ours));
    assert_eq!(ticks(&c, ChunkPick::default(), &[(Ours, 1), (Ours, 0)]), ChunkPick::Lines(vec![r(Ours, 1), r(Ours, 0)]));
    let back = ticks(&c, ChunkPick::default(), &[(Theirs, 2), (Theirs, 2)]);
    assert!(back == ChunkPick::Sides(Choice::Unresolved) && back.is_open(), "unticking the only line");
    assert_eq!(
        ChunkPick::Sides(Choice::OursThenTheirs).toggle_line(&c, Ours, 1),
        ChunkPick::Lines(vec![r(Ours, 0), r(Theirs, 0), r(Theirs, 1), r(Theirs, 2)])
    );
    let all = ticks(&c, ChunkPick::default(), &[(Ours, 0), (Ours, 1), (Theirs, 0), (Theirs, 1), (Theirs, 2)]);
    assert_eq!(all, ChunkPick::Sides(Choice::OursThenTheirs));
    let all = ticks(&c, ChunkPick::default(), &[(Theirs, 0), (Theirs, 1), (Theirs, 2), (Ours, 0), (Ours, 1)]);
    assert_eq!(all, ChunkPick::Sides(Choice::TheirsThenOurs));
    // A line past the side's end changes nothing.
    assert_eq!(ChunkPick::default().toggle_line(&c, Ours, 5), ChunkPick::default());
}

#[test]
fn toggle_side_on_lines() {
    let c = chunk();
    let p = ChunkPick::Lines(vec![r(Theirs, 1), r(Ours, 0)]);
    let taken = p.toggle_side(&c, Ours);
    assert_eq!(taken, ChunkPick::Lines(vec![r(Theirs, 1), r(Ours, 0), r(Ours, 1)]));
    assert!(taken.takes_side(&c, Ours));
    // Theirs is only partly taken: ticking it takes all of it, at the end.
    let both = taken.toggle_side(&c, Theirs);
    assert_eq!(both, ChunkPick::Sides(Choice::OursThenTheirs));
    assert_eq!(both.toggle_side(&c, Theirs), ChunkPick::Sides(Choice::Ours), "untaking theirs leaves all of ours");
    assert_eq!(taken.toggle_side(&c, Ours), ChunkPick::Lines(vec![r(Theirs, 1)]), "a whole side is left out");
}

#[test]
fn toggle_side_on_an_empty_side() {
    let c = Chunk { base: Vec::new(), ours: Vec::new(), theirs: lines(&["t0", "t1"]) };
    let p = ChunkPick::Lines(vec![r(Theirs, 1)]);
    assert_eq!(p.toggle_side(&c, Ours), p, "no lines to take");
    assert!(!p.takes_side(&c, Ours));
    for from in [Choice::Unresolved, Choice::Ours, Choice::Theirs, Choice::OursThenTheirs, Choice::TheirsThenOurs] {
        for side in [Side::Ours, Side::Theirs] {
            assert_eq!(ChunkPick::Sides(from).toggle_side(&c, side), ChunkPick::Sides(from.toggle(side)));
        }
    }
}

#[test]
fn edited_is_frozen() {
    let c = chunk();
    let e = ChunkPick::Edited { lines: lines(&["x"]), before: Box::new(ChunkPick::Sides(Choice::Ours)) };
    assert_eq!(e.toggle_line(&c, Ours, 0), e);
    assert_eq!(e.toggle_side(&c, Theirs), e);
    assert!(e.is_edited() && !e.is_open() && !e.takes_side(&c, Ours) && e.refs(&c).is_empty());
    assert_eq!(e.out_lines(&c), ["x\n"]);
    let marked =
        ChunkPick::Edited { lines: lines(&["<<<<<<< main", "a", "=======", "b", ">>>>>>> other"]), before: Box::new(ChunkPick::default()) };
    assert!(marked.is_open());
    assert!(!ChunkPick::Edited { lines: Vec::new(), before: Box::new(ChunkPick::default()) }.is_open());
}

fn file(segs: Vec<Segment>) -> ConflictFile {
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other\nbranch".into() },
        content: ConflictContent::Text(segs),
        no_ancestor: false,
        base_markers: false,
    }
}

fn two_chunks() -> ConflictFile {
    file(vec![
        Segment::Clean(lines(&["top"])),
        Segment::Conflict(chunk()),
        Segment::Clean(lines(&["mid"])),
        Segment::Conflict(Chunk { base: lines(&["b"]), ours: lines(&["x"]), theirs: lines(&["y"]) }),
        Segment::Clean(lines(&["end"])),
    ])
}

#[test]
fn render_without_markers_needs_every_pick() {
    let f = two_chunks();
    let open = [ChunkPick::Sides(Choice::Ours), ChunkPick::default()];
    assert_eq!(render_picks(&f, &open, false, false, None), None);
    assert_eq!(render_picks(&f, &open[..1], true, false, None), None, "one pick per chunk");
    for (a, b) in [(Choice::Ours, Choice::Theirs), (Choice::TheirsThenOurs, Choice::OursThenTheirs)] {
        assert_eq!(render_picks(&f, &[a.into(), b.into()], false, false, None), render_output(&f, &[a, b]));
    }
    let picks = [ChunkPick::Lines(vec![r(Theirs, 1), r(Ours, 0)]), ChunkPick::Edited { lines: Vec::new(), before: Box::default() }];
    assert_eq!(render_picks(&f, &picks, false, false, None).unwrap(), "top\nt1\no0\nmid\nend\n");
}

#[test]
fn render_with_markers() {
    let f = two_chunks();
    let picks = [ChunkPick::Sides(Choice::Ours), ChunkPick::default()];
    let out = render_picks(&f, &picks, true, false, None).unwrap();
    assert_eq!(out, "top\no0\no1\nmid\n<<<<<<< main\nx\n=======\ny\n>>>>>>> other branch\nend\n");
    assert_eq!(crate::repo::marker_regions(out.as_bytes()), 1);
    // The base only with diff3-style markers and a real ancestor.
    let mut f3 = two_chunks();
    f3.base_markers = true;
    let out = render_picks(&f3, &picks, true, false, None).unwrap();
    assert!(out.contains("<<<<<<< main\nx\n||||||| base\nb\n=======\n"));
    f3.no_ancestor = true;
    assert!(!render_picks(&f3, &picks, true, false, None).unwrap().contains("|||||||"));
    // Typed markers are written as typed.
    let typed = ChunkPick::Edited { lines: lines(&["<<<<<<< mine", "q", "=======", ">>>>>>> yours"]), before: Box::default() };
    let out = render_picks(&f, &[ChunkPick::Sides(Choice::Theirs), typed], true, false, None).unwrap();
    assert!(out.contains("mid\n<<<<<<< mine\nq\n=======\n>>>>>>> yours\nend\n"));
}

#[test]
fn markers_outgrow_marker_like_content() {
    let f = file(vec![
        Segment::Clean(lines(&["========="])),
        Segment::Conflict(Chunk { base: Vec::new(), ours: lines(&["a"]), theirs: lines(&["b"]) }),
    ]);
    let out = render_picks(&f, &[ChunkPick::default()], true, false, None).unwrap();
    let ten = |c: char| c.to_string().repeat(10);
    assert_eq!(out, format!("=========\n{} main\na\n{}\nb\n{} other branch\n", ten('<'), ten('='), ten('>')));
}

#[test]
fn line_endings() {
    let crlf = |v: &[&str]| v.iter().map(|s| format!("{s}\r\n")).collect::<Vec<_>>();
    let f = file(vec![
        Segment::Clean(crlf(&["top"])),
        Segment::Conflict(Chunk { base: Vec::new(), ours: crlf(&["a"]), theirs: vec!["b".into()] }),
    ]);
    assert_eq!(
        file_eol(match &f.content {
            ConflictContent::Text(s) => s,
            _ => unreachable!(),
        }),
        "\r\n"
    );
    let out = render_picks(&f, &[ChunkPick::default()], true, false, None).unwrap();
    // Theirs has no final newline: the end marker still starts its own line.
    assert_eq!(out, "top\r\n<<<<<<< main\r\na\r\n=======\r\nb\r\n>>>>>>> other branch\r\n");
    let lf = render_picks(&f, &[ChunkPick::default()], true, true, None).unwrap();
    assert!(!lf.contains('\r'));
    assert_eq!(lf, out.replace("\r\n", "\n"));
    assert_eq!(file_eol(&[Segment::Clean(vec!["x".into()])]), "\n");
}

#[test]
fn ranges_cover_each_chunk() {
    let f = two_chunks();
    let picks = [ChunkPick::Lines(vec![r(Theirs, 2)]), ChunkPick::default()];
    let mut ranges = Vec::new();
    let out = render_picks(&f, &picks, true, true, Some(&mut ranges)).unwrap();
    assert_eq!(ranges.len(), 2);
    assert_eq!(&out[ranges[0].clone()], "t2\n");
    assert_eq!(&out[ranges[1].clone()], "<<<<<<< main\nx\n=======\ny\n>>>>>>> other branch\n");
    // A chunk with no lines has an empty range at the start of the next line.
    let picks = [ChunkPick::Edited { lines: Vec::new(), before: Box::default() }, ChunkPick::Sides(Choice::Ours)];
    let out = render_picks(&f, &picks, true, true, Some(&mut ranges)).unwrap();
    assert_eq!((ranges[0].clone(), &out[ranges[1].clone()]), (4..4, "x\n"));
}
