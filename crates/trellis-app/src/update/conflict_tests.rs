//! Headless tests for the conflict view: loading, picking, stepping, saving and closing.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::conflict_rows::Trivial;
use trellis_core::gitcli::{args, run};
use trellis_core::repo::{
    Choice, Chunk, ConflictContent, ConflictFile, ConflictIds, DiffTarget, FileDiff, Operation, Segment, Side, SideLabels,
};

/// A repository in the middle of a merge where a.txt conflicts in one chunk ("ours" against
/// "theirs" on its second line).
pub(super) fn conflicted_repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("a.txt"), "one\ntheirs\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "one\nours\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

/// A repository in the middle of a merge where a.txt and b.txt each conflict in one chunk.
pub(super) fn two_conflicts(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("b.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    for f in ["a.txt", "b.txt"] {
        std::fs::write(dir.join(f), "one\ntheirs\n").unwrap();
    }
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    for f in ["a.txt", "b.txt"] {
        std::fs::write(dir.join(f), "one\nours\n").unwrap();
    }
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

/// The repository opened, with the conflict view loaded on a.txt.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = conflicted_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    (st, dir)
}

/// A file with `n` one-line chunks between clean lines (no repository behind it).
fn fake_file(n: usize) -> ConflictFile {
    let line = |s: &str| vec![format!("{s}\n")];
    let mut segs = vec![Segment::Clean(line("top"))];
    for i in 0..n {
        segs.push(Segment::Conflict(Chunk { base: line("base"), ours: line(&format!("ours {i}")), theirs: line(&format!("theirs {i}")) }));
        segs.push(Segment::Clean(line("between")));
    }
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() },
        content: ConflictContent::Text(segs),
        no_ancestor: false,
    }
}

fn with_fake(n: usize) -> AppState {
    let mut st = state();
    let mut view = ConflictView::loading("f.txt".into());
    view.set_file(fake_file(n));
    st.conflict = Some(view);
    st
}

fn read(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("a.txt")).unwrap()
}

#[test]
fn opening_a_conflicted_file_loads_the_chunks() {
    let (st, dir) = opened("conflict-open");
    let c = st.conflict.as_ref().expect("view open");
    let f = c.file.as_ref().expect("loaded");
    assert_eq!(f.chunk_count(), 1);
    assert_eq!(c.choices, [Choice::Unresolved]);
    assert_eq!(f.labels.theirs, "other");
    assert!(st.diff.is_none());
    assert_eq!(st.view.conflict_scroll_to, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_conflict_closes_the_diff_and_open_diff_closes_the_conflict() {
    let dir = conflicted_repo("conflict-vs-diff");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.diff = Some((
        DiffTarget::Unstaged("a.txt".into()),
        FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }.into(),
    ));
    let fx = update(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())), Instant::now());
    match &fx[..] {
        [Effect::LoadConflict { path, generation, .. }] => assert_eq!((path.as_str(), *generation), ("a.txt", st.conflict_gen)),
        other => panic!("expected a conflict load, got {other:?}"),
    }
    assert!(st.diff.is_none() && st.conflict.is_some());
    update(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))), Instant::now());
    assert!(st.conflict.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_conflict_results_are_ignored() {
    let mut st = state();
    st.conflict = Some(ConflictView::loading("f.txt".into()));
    st.conflict_gen = 5;
    let loaded = |generation, path: &str, result| Msg::Conflict(ConflictMsg::ConflictLoaded { path: path.into(), generation, result });
    update(&mut st, loaded(4, "f.txt", Ok(fake_file(1))), Instant::now());
    update(&mut st, loaded(5, "g.txt", Ok(fake_file(1))), Instant::now());
    assert!(st.conflict.as_ref().is_some_and(|c| c.file.is_none()), "still loading");
    update(&mut st, loaded(5, "f.txt", Err("boom".into())), Instant::now());
    assert!(st.conflict.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "boom"));
}

#[test]
fn toggling_taking_all_resetting_and_stepping() {
    let mut st = with_fake(3);
    let send = |st: &mut AppState, m| update(st, m, Instant::now());
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 1, side: Side::Theirs }));
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 1, side: Side::Ours }));
    let c = st.conflict.as_ref().unwrap();
    assert_eq!((c.choices[1], c.current), (Choice::TheirsThenOurs, 1));
    assert_eq!(st.view.conflict_scroll_to, None, "picking doesn't scroll");
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictTakeAll(Side::Ours)));
    assert_eq!(st.conflict.as_ref().unwrap().choices, [Choice::Ours; 3]);
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictReset));
    assert_eq!(st.conflict.as_ref().unwrap().choices, [Choice::Unresolved; 3]);

    st.conflict.as_mut().unwrap().current = 0;
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictStep(1)));
    assert_eq!((st.conflict.as_ref().unwrap().current, st.view.conflict_scroll_to.take()), (1, Some(1)));
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictStep(1)));
    assert_eq!((st.conflict.as_ref().unwrap().current, st.view.conflict_scroll_to.take()), (2, Some(2)));
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictStep(1)));
    assert_eq!((st.conflict.as_ref().unwrap().current, st.view.conflict_scroll_to), (2, None), "clamped at the end");
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictSelect(0)));
    assert_eq!((st.conflict.as_ref().unwrap().current, st.view.conflict_scroll_to), (0, None));
    send(&mut st, Msg::Conflict(ConflictMsg::ConflictStep(-1)));
    assert_eq!(st.conflict.as_ref().unwrap().current, 0, "clamped at the start");
}

#[test]
fn save_is_refused_until_every_chunk_is_picked() {
    let (mut st, dir) = opened("conflict-save-refused");
    assert!(update(&mut st, Msg::Conflict(ConflictMsg::SaveConflict), Instant::now()).is_empty());
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours }), Instant::now());
    let fx = update(&mut st, Msg::Conflict(ConflictMsg::SaveConflict), Instant::now());
    let ids = st.conflict.as_ref().unwrap().file.as_ref().unwrap().ids.clone();
    match &fx[..] {
        [Effect::LocalOp { op: LocalOp::ResolveConflict { path, contents, expected }, .. }] => {
            assert_eq!((path.as_str(), contents.as_str()), ("a.txt", "one\nours\n"));
            assert_eq!(*expected, ids);
        }
        other => panic!("expected one resolve, got {other:?}"),
    }
    assert!(st.conflict.as_ref().unwrap().saving);
    assert!(update(&mut st, Msg::Conflict(ConflictMsg::SaveConflict), Instant::now()).is_empty(), "not twice");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_resolves_the_file_end_to_end() {
    let (mut st, dir) = opened("conflict-save");
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Theirs }), Instant::now());
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    assert_eq!(read(&dir), "one\ntheirs\n");
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.unstaged.iter().any(|f| f.status == '!'));
    assert!(snap.staged.iter().any(|f| f.path == "a.txt"));
    assert!(st.conflict.is_none());
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Resolved a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_resolved_closes_the_view() {
    let (mut st, dir) = opened("conflict-mark");
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    assert!(st.conflict.is_some(), "the markers warning comes first");
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(st.conflict.is_none());
    assert!(read(&dir).contains("<<<<<<<"), "marked resolved as it is");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_save_reenables_save() {
    let (mut st, dir) = opened("conflict-stale-save");
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours }), Instant::now());
    git(&dir, &["add", "a.txt"]); // resolved outside the app
    let fx = update(&mut st, Msg::Conflict(ConflictMsg::SaveConflict), Instant::now());
    assert_eq!(fx.len(), 1);
    // The write is refused and saving ends (the status reload that follows isn't run here).
    for effect in fx {
        if let Some(m) = crate::runner::execute(effect) {
            update(&mut st, m, Instant::now());
        }
    }
    assert!(st.log.iter().any(|l| !l.ok && l.text.contains("changed since it was opened")));
    assert!(st.conflict.as_ref().is_some_and(|c| !c.saving && c.can_save()), "Save is enabled again");
    assert!(read(&dir).contains("<<<<<<<"), "nothing written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolving_with_a_side_needs_the_open_file() {
    let (mut st, dir) = opened("conflict-side");
    assert!(
        update(&mut st, Msg::Conflict(ConflictMsg::ResolveWithSide { path: "b.txt".into(), side: Side::Ours }), Instant::now()).is_empty()
    );
    drive(&mut st, Msg::Conflict(ConflictMsg::ResolveWithSide { path: "a.txt".into(), side: Side::Theirs }));
    assert_eq!(read(&dir), "one\ntheirs\n");
    assert!(st.conflict.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `two_conflicts` opened, with the conflict view loaded on a.txt.
fn opened_two(name: &str) -> (AppState, PathBuf) {
    let dir = two_conflicts(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    assert!(st.conflict.as_ref().is_some_and(|c| c.file.is_some()));
    (st, dir)
}

#[test]
fn saving_steps_to_the_next_conflicted_file() {
    let (mut st, dir) = opened_two("conflict-step");
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours }), Instant::now());
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    let c = st.conflict.as_ref().expect("still open");
    assert_eq!(c.path, "b.txt");
    assert!(c.file.is_some() && !c.step_on_resolve, "b.txt loaded fresh");
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Resolved a.txt. Opened b.txt (1 conflicted file left)");
    // The last one closes the view as before.
    drive(&mut st, Msg::Conflict(ConflictMsg::ResolveWithSide { path: "b.txt".into(), side: Side::Theirs }));
    assert!(st.conflict.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Resolved b.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn marking_the_open_file_resolved_steps_too() {
    let (mut st, dir) = opened_two("conflict-step-mark");
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(vec!["a.txt".into()])));
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(st.conflict.as_ref().map(|c| c.path.as_str()), Some("b.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_resolved_outside_the_app_closes_the_view_without_stepping() {
    let (mut st, dir) = opened_two("conflict-step-outside");
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.conflict.is_none(), "closed, though b.txt is still conflicted");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Resolved a.txt (1 conflicted file left)"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trivial_chunks_are_picked_when_the_file_loads() {
    let mut f = fake_file(2);
    if let ConflictContent::Text(segs) = &mut f.content
        && let Segment::Conflict(c) = &mut segs[1]
    {
        c.ours = c.base.clone();
    }
    let mut view = ConflictView::loading("f.txt".into());
    view.set_file(f);
    assert_eq!((view.choices[0], view.auto[0]), (Choice::Theirs, Some(Trivial::Unchanged)));
    assert_eq!((view.choices[1], view.auto[1]), (Choice::Unresolved, None));
    assert_eq!((view.current, view.auto_count()), (1, 1), "on the first chunk still to pick");
    assert!(view.output().is_none(), "nothing is saved until every chunk is picked");

    let mut st = state();
    st.conflict = Some(view);
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours }), Instant::now());
    let c = st.conflict.as_ref().unwrap();
    assert_eq!((c.choices[0], c.auto[0]), (Choice::TheirsThenOurs, None), "a change clears the tag");
    st.conflict.as_mut().unwrap().auto[1] = Some(Trivial::Whitespace);
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictReset), Instant::now());
    let c = st.conflict.as_ref().unwrap();
    assert_eq!((c.choices.clone(), c.auto.clone()), (vec![Choice::Unresolved; 2], vec![None; 2]), "Reset clears every pick");

    // Every chunk picked automatically: Save is ready, and the view starts on the first.
    let mut f = fake_file(2);
    if let ConflictContent::Text(segs) = &mut f.content {
        for s in segs.iter_mut() {
            if let Segment::Conflict(c) = s {
                c.theirs = c.ours.clone();
            }
        }
    }
    let mut view = ConflictView::loading("f.txt".into());
    view.set_file(f);
    assert!(view.can_save());
    assert_eq!((view.auto, view.current), (vec![Some(Trivial::Identical); 2], 0));
}
