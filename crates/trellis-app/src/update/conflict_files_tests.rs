//! Headless tests for the Conflicted and Resolved Files lists: the markers check before Mark
//! resolved, Mark all resolved, Mark conflicted, the summary reads and their generations.

use super::conflict_tests::{conflicted_repo, two_conflicts};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{ConflictRead, Effect, LocalOp};
use crate::msg::{ConflictMsg, Inactive, Msg, RepoMsg, StageMsg, TabsMsg};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{ConflictSummary, FileEntry, ResolvedFile};

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    assert!(st.snap.is_some());
    st
}

fn send(st: &mut AppState, m: ConflictMsg) -> Vec<Effect> {
    update(st, Msg::Conflict(m), Instant::now())
}

fn is_conflicted(st: &AppState, path: &str) -> bool {
    st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!' && f.path == path))
}

fn owned(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

fn checks_markers(fx: &[Effect], want: &[&str]) -> bool {
    matches!(fx, [Effect::ReadConflictFiles { purpose: ConflictRead::BeforeMarkResolved, paths, .. }] if *paths == owned(want))
}

fn markers_in(dir: &Path, path: &str) -> usize {
    trellis_core::repo::marker_regions(&std::fs::read(dir.join(path)).unwrap())
}

#[test]
fn mark_resolved_with_markers_left_asks_first() {
    let dir = conflicted_repo("cf-markers-ask");
    let mut st = opened(&dir);
    let fx = send(&mut st, ConflictMsg::MarkResolved(owned(&["a.txt"])));
    assert!(checks_markers(&fx, &["a.txt"]), "{fx:?}");
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(owned(&["a.txt"]))));
    let Some(Modal::Confirm { title, body, button, then }) = st.modal.take() else { panic!("expected the markers warning") };
    assert_eq!((title.as_str(), button.as_str()), ("Conflict markers left", "Mark resolved"));
    assert!(body.starts_with("a.txt still has 1 conflict marker region."), "{body}");
    assert!(matches!(*then, Msg::Conflict(ConflictMsg::MarkResolvedConfirmed(ref p)) if *p == owned(&["a.txt"])));
    assert!(is_conflicted(&st, "a.txt"), "nothing staged before the answer");
    let fx = update(&mut st, *then, Instant::now());
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::MarkResolved(p), .. }] if *p == owned(&["a.txt"])), "{fx:?}");
    super::tests::finish(&mut st, fx);
    assert!(!is_conflicted(&st, "a.txt"));
    assert!(markers_in(&dir, "a.txt") == 1, "staged as it was");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_resolved_without_markers_stages_straight_away() {
    let dir = conflicted_repo("cf-markers-none");
    let mut st = opened(&dir);
    std::fs::write(dir.join("a.txt"), "one\nboth\n").unwrap();
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(owned(&["a.txt"]))));
    assert!(st.modal.is_none());
    assert!(!is_conflicted(&st, "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staging_a_conflicted_file_goes_through_the_markers_check() {
    let dir = conflicted_repo("cf-stage-file");
    let mut st = opened(&dir);
    let fx = update(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())), Instant::now());
    assert!(checks_markers(&fx, &["a.txt"]), "{fx:?}");
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, Msg::Stage(StageMsg::StageFile("new.txt".into())), Instant::now());
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::StageFile(p), .. }] if p == "new.txt"), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_all_resolved_stages_every_file_and_lists_them_as_resolved() {
    let dir = two_conflicts("cf-mark-all");
    let mut st = opened(&dir);
    for f in ["a.txt", "b.txt"] {
        std::fs::write(dir.join(f), "fixed\n").unwrap();
    }
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(owned(&["a.txt", "b.txt"]))));
    assert!(st.modal.is_none());
    assert!(!is_conflicted(&st, "a.txt") && !is_conflicted(&st, "b.txt"));
    let mut resolved: Vec<&str> = st.conflict_files.resolved.iter().map(|r| r.entry.path.as_str()).collect();
    resolved.sort();
    assert_eq!(resolved, ["a.txt", "b.txt"]);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(st.conflict_files.resolved_rows(snap).len(), 2, "shown while the merge is in progress");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_conflicted_brings_the_conflict_back() {
    let dir = conflicted_repo("cf-unresolve");
    let mut st = opened(&dir);
    assert!(send(&mut st, ConflictMsg::MarkConflicted(owned(&["a.txt"]))).is_empty(), "not resolved yet");
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolvedConfirmed(owned(&["a.txt"]))));
    assert!(!is_conflicted(&st, "a.txt"));
    assert_eq!(st.conflict_files.resolved.iter().map(|r| r.entry.path.as_str()).collect::<Vec<_>>(), ["a.txt"]);
    let fx = send(&mut st, ConflictMsg::MarkConflicted(owned(&["a.txt", "other.txt"])));
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::MarkConflicted(p), .. }] if *p == owned(&["a.txt"])), "{fx:?}");
    super::tests::finish(&mut st, fx);
    assert!(is_conflicted(&st, "a.txt"));
    assert!(markers_in(&dir, "a.txt") >= 1);
    assert!(st.conflict_files.resolved.is_empty(), "conflicted again");
    assert_eq!(st.conflict_files.count_note("a.txt"), Some(("1 conflict".into(), false)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_marked_while_the_merge_tool_has_the_file() {
    let (mut st, dir) = super::tool_compare_tests::running("cf-in-tool");
    let held = "a.txt is open in the merge tool: save and close it there first.";
    assert!(send(&mut st, ConflictMsg::MarkResolved(owned(&["a.txt"]))).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(held));
    assert!(send(&mut st, ConflictMsg::MarkResolvedConfirmed(owned(&["a.txt"]))).is_empty());
    st.log.clear();
    st.conflict_files.resolved = vec![ResolvedFile { entry: FileEntry::new("a.txt", ' '), both_sides: true }];
    assert!(send(&mut st, ConflictMsg::MarkConflicted(owned(&["a.txt"]))).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(held));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The summary read `fx` holds, by generation.
fn summary(fx: &[Effect]) -> Option<u64> {
    fx.iter().find_map(|e| match e {
        Effect::ReadConflictFiles { purpose: ConflictRead::Summary { generation }, .. } => Some(*generation),
        _ => None,
    })
}

#[test]
fn summaries_follow_the_status_and_drop_stale_reads() {
    let dir = conflicted_repo("cf-summary");
    let mut st = opened(&dir);
    assert_eq!(st.conflict_files.count_note("a.txt"), Some(("1 conflict".into(), false)), "read when the repository opened");
    let mut fx = Vec::new();
    super::conflict_files::want(&mut st, &mut fx);
    let generation = summary(&fx).expect("a summary read");
    let read = |generation, n| ConflictMsg::ConflictFilesRead {
        purpose: ConflictRead::Summary { generation },
        paths: owned(&["a.txt"]),
        result: Ok(ConflictSummary { counts: vec![("a.txt".into(), Some(n))], resolved: Vec::new() }),
    };
    send(&mut st, read(generation - 1, 7));
    assert_eq!(st.conflict_files.counts.get("a.txt"), Some(&Some(1)), "an older read is ignored");
    send(&mut st, read(generation, 3));
    assert_eq!(st.conflict_files.counts.get("a.txt"), Some(&Some(3)));

    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-q", "--no-edit"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.conflict_files.counts.is_empty() && st.conflict_files.resolved.is_empty(), "cleared after the merge commit");
    let mut fx = Vec::new();
    super::conflict_files::want(&mut st, &mut fx);
    assert!(fx.is_empty(), "nothing to read once it is over: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_failed_reads_reach_an_inactive_tab() {
    let read =
        |result| Msg::Conflict(ConflictMsg::ConflictFilesRead { purpose: ConflictRead::BeforeMarkResolved, paths: Vec::new(), result });
    assert!(matches!(read(Err("boom".into())).on_inactive(), Some(Inactive::Log(e)) if e == "boom"));
    assert!(read(Ok(ConflictSummary::default())).on_inactive().is_none());
    assert!(Msg::Conflict(ConflictMsg::MarkResolved(owned(&["a.txt"]))).on_inactive().is_none());
}

#[test]
fn the_lists_are_parked_with_the_tab_and_read_again_on_return() {
    let a = conflicted_repo("cf-tabs-a");
    let b: PathBuf = scratch_repo("cf-tabs-b");
    let mut st = opened(&a);
    let counts = st.conflict_files.counts.clone();
    assert!(!counts.is_empty());
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert!(st.conflict_files.counts.is_empty(), "the other tab has no conflicts");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(st.conflict_files.counts, counts, "kept while parked");
    assert_eq!(summary(&fx), Some(st.conflict_files.generation), "read again on return: {fx:?}");
    for d in [&a, &b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// `two_conflicts` with both files resolved with `git add`, the lists read.
pub(super) fn both_resolved(name: &str) -> (AppState, PathBuf) {
    let dir = two_conflicts(name);
    for f in ["a.txt", "b.txt"] {
        git(&dir, &["add", f]);
    }
    let st = opened(&dir);
    assert_eq!(st.conflict_files.resolved.len(), 2);
    (st, dir)
}

#[test]
fn mark_all_conflicted_sends_every_restorable_path() {
    use crate::state::conflict_files::restorable_paths;
    let (mut st, dir) = both_resolved("cf-mark-all-conflicted");
    let all = restorable_paths(&st);
    assert_eq!(all, owned(&["a.txt", "b.txt"]));
    let fx = send(&mut st, ConflictMsg::MarkConflicted(all));
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::MarkConflicted(p), .. }] if *p == owned(&["a.txt", "b.txt"])), "{fx:?}");
    // A record without both sides, and a file the merge tool has open, are left out.
    st.conflict_files.resolved[0].both_sides = false;
    st.job = Some(crate::state::RunningJob {
        label: crate::state::tool_compare::merge_label("b.txt"),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
    });
    assert!(restorable_paths(&st).is_empty());
    st.job = None;
    assert_eq!(restorable_paths(&st), owned(&["b.txt"]));
    let _ = std::fs::remove_dir_all(&dir);
}
