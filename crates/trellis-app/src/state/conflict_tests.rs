use super::*;
use trellis_core::repo::{Choice, ConflictIds, Operation, SideLabels};

#[test]
fn user_choices_skip_automatic_and_open_chunks() {
    let mut v = ConflictView::loading("a.txt".into());
    v.picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::default(), ChunkPick::Sides(Choice::Theirs)];
    v.auto = vec![None, None, Some(Trivial::Identical)];
    assert_eq!(v.user_choices(), 1, "the automatic pick and the open chunk don't count");
    v.picks[1] = ChunkPick::Edited { lines: vec!["x\n".into()], before: Box::new(ChunkPick::default()) };
    assert_eq!(v.user_choices(), 2, "typed output counts");
    v.auto[2] = None;
    assert_eq!(v.user_choices(), 3, "an automatic pick the user changed counts");
    assert_eq!(ConflictView::loading("b.txt".into()).user_choices(), 0);
}

#[test]
fn user_choices_skip_what_the_file_holds() {
    let mut v = ConflictView::loading("a.txt".into());
    v.picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs)];
    v.auto = vec![None, None];
    v.baseline = (vec![ChunkPick::Sides(Choice::Ours), ChunkPick::default()], CleanEdits::from([(2, vec!["kept\n".into()])]));
    v.cleans = v.baseline.1.clone();
    assert_eq!(v.user_choices(), 1, "only the pick not in the file");
    v.cleans.insert(4, vec!["new\n".into()]);
    assert_eq!(v.user_choices(), 2, "a clean run edited since");
    v.cleans.clear();
    assert_eq!(v.user_choices(), 2, "a clean edit in the file dropped");
}

/// top, a chunk whose sides agree (picked automatically: ours), mid, a real conflict, end.
fn file() -> ConflictFile {
    let line = |s: &str| vec![format!("{s}\n")];
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() },
        content: ConflictContent::Text(vec![
            Segment::Clean(line("top")),
            Segment::Conflict(Chunk { base: line("b"), ours: line("same"), theirs: line("same") }),
            Segment::Clean(line("mid")),
            Segment::Conflict(Chunk { base: line("b"), ours: line("x"), theirs: line("y") }),
            Segment::Clean(line("end")),
        ]),
        no_ancestor: false,
        base_markers: false,
    }
}

#[test]
fn saved_picks_come_back_over_the_automatic_ones() {
    let mut v = ConflictView::loading("f.txt".into());
    v.opened.insert(2);
    let cleans = CleanEdits::from([(4, vec!["END\n".into()])]);
    let saved = SavedPicks { picks: vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs)], cleans: cleans.clone() };
    v.set_file_saved(file(), Some(saved));
    assert_eq!(v.picks, [ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs)]);
    assert_eq!(v.auto, [Some(Trivial::Identical), None], "the automatic pick came back as it was: still tagged");
    assert_eq!((v.restored, &v.cleans, v.user_choices()), (2, &cleans, 0));
    assert!(v.opened.is_empty(), "folds close");
    assert_eq!(v.output().unwrap(), "top\nsame\nmid\ny\nEND\n");
    // Nothing saved: as before.
    v.set_file(file());
    assert_eq!((v.restored, v.cleans.len(), v.current), (0, 0, 1));
    assert_eq!(v.picks[1], ChunkPick::default());
}
