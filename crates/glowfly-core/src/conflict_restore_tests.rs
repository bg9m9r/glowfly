use super::*;
use crate::conflict_pick::render;
use crate::repo::{ConflictIds, Operation, SideLabels};
use Choice::{Ours, OursThenTheirs, Theirs, TheirsThenOurs};

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| format!("{s}\n")).collect()
}

fn chunk(base: &[&str], ours: &[&str], theirs: &[&str]) -> Segment {
    Segment::Conflict(Chunk { base: lines(base), ours: lines(ours), theirs: lines(theirs) })
}

fn file(segs: Vec<Segment>) -> ConflictFile {
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() },
        content: ConflictContent::Text(segs),
        no_ancestor: false,
        base_markers: false,
    }
}

/// top1-3, chunk 0 (ours o1 o2, theirs t1), m1-3, chunk 1 (ours x, theirs y), e1-2.
fn two() -> ConflictFile {
    file(vec![
        Segment::Clean(lines(&["top1", "top2", "top3"])),
        chunk(&["b"], &["o1", "o2"], &["t1"]),
        Segment::Clean(lines(&["m1", "m2", "m3"])),
        chunk(&["x0"], &["x"], &["y"]),
        Segment::Clean(lines(&["e1", "e2"])),
    ])
}

/// The working text: the clean runs as they are around `c0` and `c1`.
fn text(c0: &str, c1: &str) -> String {
    format!("top1\ntop2\ntop3\n{c0}m1\nm2\nm3\n{c1}e1\ne2\n")
}

const GIT0: &str = "<<<<<<< HEAD\no1\no2\n=======\nt1\n>>>>>>> feature/x\n";
const GIT1: &str = "<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> feature/x\n";

fn picks(r: Option<SavedPicks>) -> Vec<ChunkPick> {
    r.expect("picks come back").picks
}

#[test]
fn git_markers_alone_bring_nothing_back() {
    assert_eq!(restore(&two(), &text(GIT0, GIT1)), None);
    // Any marker size, and the merge's own file with CR LF endings.
    let nine = "<<<<<<<<< a\nx\n=========\ny\n>>>>>>>>> b\n";
    assert_eq!(restore(&two(), &text(GIT0, nine)), None);
    assert_eq!(restore(&two(), &text(GIT0, GIT1).replace('\n', "\r\n")), None);
}

#[test]
fn a_base_section_still_counts_as_not_picked() {
    let diff3 = "<<<<<<< HEAD\no1\no2\n||||||| merged common ancestors\nb\n=======\nt1\n>>>>>>> other\n";
    assert_eq!(picks(restore(&two(), &text(diff3, "x\n"))), [ChunkPick::default(), Ours.into()]);
}

#[test]
fn whole_sides_come_back() {
    assert_eq!(picks(restore(&two(), &text("o1\no2\n", "y\nx\n"))), [Ours.into(), TheirsThenOurs.into()]);
    assert_eq!(picks(restore(&two(), &text("o1\no2\nt1\n", GIT1))), [OursThenTheirs.into(), ChunkPick::default()]);
    assert_eq!(picks(restore(&two(), &text(GIT0, "y\n"))), [ChunkPick::default(), Theirs.into()]);
}

#[test]
fn the_trivial_pick_is_tried_first() {
    // Every version the same: ours is the base, so theirs is the trivial pick, though ours reads
    // the same.
    let f = file(vec![Segment::Clean(lines(&["a1"])), chunk(&["s"], &["s"], &["s"]), Segment::Clean(lines(&["z"]))]);
    assert_eq!(picks(restore(&f, "a1\ns\nz\n")), [Theirs.into()]);
}

#[test]
fn single_lines_come_back_in_file_order() {
    let f = file(vec![Segment::Clean(lines(&["top"])), chunk(&[], &["d", "p"], &["d", "q"]), Segment::Clean(lines(&["end"]))]);
    let got = picks(restore(&f, "top\nq\nd\nd\nend\n"));
    let r = |side, line| LineRef { side, line };
    assert_eq!(got, [ChunkPick::Lines(vec![r(Side::Theirs, 1), r(Side::Ours, 0), r(Side::Theirs, 0)])]);
    assert_eq!(render(&f, &got, &CleanEdits::new(), false, false, None).unwrap(), "top\nq\nd\nd\nend\n");
}

#[test]
fn other_text_comes_back_typed() {
    let typed = ChunkPick::Edited { lines: lines(&["mine", "too"]), before: Box::default() };
    assert_eq!(picks(restore(&two(), &text("mine\ntoo\n", GIT1))), [typed, ChunkPick::default()]);
    let emptied = ChunkPick::Edited { lines: Vec::new(), before: Box::default() };
    let got = picks(restore(&two(), &text("", GIT1)));
    assert_eq!(got, [emptied, ChunkPick::default()]);
    assert!(!got[0].is_open());
}

#[test]
fn mismatched_markers_bring_nothing_back() {
    let other_ours = "<<<<<<< HEAD\no1\nzz\n=======\nt1\n>>>>>>> b\n";
    assert_eq!(restore(&two(), &text(other_ours, "y\n")), None);
    assert_eq!(restore(&two(), &text(&format!("{GIT0}{GIT0}"), "y\n")), None, "two regions for one chunk");
    assert_eq!(restore(&two(), &text("<<<<<<< HEAD\no1\n", "y\n")), None, "a marker left in typed text");
    assert_eq!(restore(&two(), &text("mine\n||||||| base\n", "y\n")), None, "a base marker left in typed text");
    assert_eq!(restore(&two(), &text(GIT0, "y\n").replace("m2\n", ">>>>>>> b\n")), None, "a marker in an edited clean run");
}

#[test]
fn git_diff3_regions_with_shared_lines_bring_nothing_back() {
    // Ours and theirs both start (or end) with `m`: the stages hold it clean, while git's diff3
    // style keeps it inside the region on both sides.
    let f = file(vec![Segment::Clean(lines(&["top", "m"])), chunk(&["B"], &["X"], &["Y"]), Segment::Clean(lines(&["after"]))]);
    assert_eq!(restore(&f, "top\n<<<<<<< HEAD\nm\nX\n||||||| base\nB\n=======\nm\nY\n>>>>>>> other\nafter\n"), None);
    let f = file(vec![Segment::Clean(lines(&["top"])), chunk(&["B"], &["X"], &["Y"]), Segment::Clean(lines(&["m", "after"]))]);
    assert_eq!(restore(&f, "top\n<<<<<<< HEAD\nX\nm\n||||||| base\nB\n=======\nY\nm\n>>>>>>> other\nafter\n"), None);
    // With a pick saved for another chunk, the region still doesn't match and nothing comes back.
    let f = file(vec![
        Segment::Clean(lines(&["top", "m"])),
        chunk(&["B"], &["X"], &["Y"]),
        Segment::Clean(lines(&["mid"])),
        chunk(&["b"], &["x"], &["y"]),
        Segment::Clean(lines(&["end"])),
    ]);
    assert_eq!(restore(&f, "top\n<<<<<<< HEAD\nm\nX\n||||||| base\nB\n=======\nm\nY\n>>>>>>> other\nmid\nx\nend\n"), None);
}

#[test]
fn edited_clean_lines_come_back() {
    let got = restore(&two(), &text(GIT0, GIT1).replace("m2\n", "M2\nadded\n")).unwrap();
    assert_eq!(got.picks, [ChunkPick::default(), ChunkPick::default()]);
    assert_eq!(got.cleans, CleanEdits::from([(2, lines(&["m1", "M2", "added", "m3"]))]));
    // The file's line ending is used.
    let crlf = |v: &[&str]| v.iter().map(|s| format!("{s}\r\n")).collect::<Vec<_>>();
    let f = file(vec![
        Segment::Clean(crlf(&["a", "b", "c"])),
        Segment::Conflict(Chunk { base: Vec::new(), ours: crlf(&["o"]), theirs: crlf(&["t"]) }),
    ]);
    let got = restore(&f, "a\nB\nc\no\n").unwrap();
    assert_eq!((got.picks, got.cleans), (vec![Ours.into()], CleanEdits::from([(0, crlf(&["a", "B", "c"]))])));
}

#[test]
fn line_endings_are_ignored() {
    let working = text("o1\no2\n", GIT1).replace('\n', "\r\n");
    let got = restore(&two(), &working).unwrap();
    assert_eq!((got.picks, got.cleans), (vec![Ours.into(), ChunkPick::default()], CleanEdits::new()));
}

#[test]
fn adjacent_chunks_are_split() {
    let f = file(vec![
        Segment::Clean(lines(&["top"])),
        chunk(&[], &["a1"], &["a2"]),
        chunk(&[], &["b1"], &["b2"]),
        Segment::Clean(lines(&["end"])),
    ]);
    assert_eq!(picks(restore(&f, "top\na1\n<<<<<<< x\nb1\n=======\nb2\n>>>>>>> y\nend\n")), [Ours.into(), ChunkPick::default()]);
    assert_eq!(picks(restore(&f, "top\n<<<<<<< x\na1\n=======\na2\n>>>>>>> y\nb2\nend\n")), [ChunkPick::default(), Theirs.into()]);
    assert_eq!(restore(&f, "top\nzz\n<<<<<<< x\nb1\n=======\nb2\n>>>>>>> y\nend\n"), None, "no way to split");
}

#[test]
fn a_missing_clean_run_brings_nothing_back() {
    assert_eq!(restore(&two(), "top1\ntop2\ntop3\no1\no2\ny\ne1\ne2\n"), None);
}

#[test]
fn saved_picks_round_trip() {
    let f = two();
    let r = |side, line| LineRef { side, line };
    let first = [
        ChunkPick::default(),
        Ours.into(),
        Theirs.into(),
        OursThenTheirs.into(),
        ChunkPick::Lines(vec![r(Side::Theirs, 0), r(Side::Ours, 1)]),
        ChunkPick::Edited { lines: lines(&["typed"]), before: Box::default() },
    ];
    let second =
        [ChunkPick::default(), Theirs.into(), TheirsThenOurs.into(), ChunkPick::Edited { lines: Vec::new(), before: Box::default() }];
    let edits = [CleanEdits::new(), CleanEdits::from([(2, lines(&["m1", "MID", "m3"]))])];
    let chunks: Vec<&Chunk> = f.chunks().collect();
    for a in &first {
        for b in &second {
            for cleans in &edits {
                let picks = [a.clone(), b.clone()];
                let saved = render(&f, &picks, cleans, true, false, None).unwrap();
                let got = restore(&f, &saved);
                if cleans.is_empty() && picks.iter().all(|p| *p == ChunkPick::default()) {
                    assert_eq!(got, None);
                    continue;
                }
                let got = got.unwrap_or_else(|| panic!("{picks:?} {cleans:?} from {saved:?}"));
                assert_eq!(&got.cleans, cleans);
                for (k, (want, have)) in picks.iter().zip(&got.picks).enumerate() {
                    assert_eq!(want.is_open(), have.is_open(), "{picks:?}");
                    assert_eq!(want.out_lines(chunks[k]), have.out_lines(chunks[k]), "{picks:?}");
                }
                assert_eq!(render(&f, &got.picks, &got.cleans, true, false, None).unwrap(), saved);
            }
        }
    }
}
