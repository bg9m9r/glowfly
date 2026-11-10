//! Headless tests for acting on several files picked in Conflicted Files or Resolved Files,
//! end to end through the runner: Mark resolved of the first and third of three conflicted
//! files stages those two in one undo entry (the middle one stays conflicted) and Edit > Undo
//! brings both conflicts back; Mark conflicted of two picked resolved files brings both back in
//! one undo entry.

use super::conflict_mark_undo_tests::opened;
use super::tests::{drive, git, scratch_repo};
use glowfly_core::gitcli::{args, run};
use glowfly_core::undo::{Action, ConflictChange};
use glowfly_model::msg::{ConflictMsg, JobMsg, Msg};
use glowfly_model::state::AppState;
use glowfly_model::state::conflict_picks::picked;
use glowfly_model::state::wip_lists::PickHow;
use std::path::PathBuf;

const FILES: [&str; 3] = ["a.txt", "b.txt", "c.txt"];

/// A repository in the middle of a merge where a.txt, b.txt and c.txt each conflict in one
/// chunk.
fn three_conflicts(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    let put = |side: &str| FILES.iter().for_each(|f| std::fs::write(dir.join(f), format!("one\n{side}\n")).unwrap());
    put("base");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    put("theirs");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    put("ours");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0, "merge should conflict");
    dir
}

fn conflicted(st: &AppState) -> Vec<&str> {
    st.snap.as_ref().map_or(Vec::new(), |s| s.unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect())
}

/// Ctrl/Cmd-clicks a.txt, then c.txt in that list: the picks with b.txt left out.
fn pick_a_and_c(st: &mut AppState, resolved: bool) -> Vec<String> {
    for path in ["a.txt", "c.txt"] {
        drive(st, Msg::Conflict(ConflictMsg::Pick { resolved, path: path.into(), how: PickHow::Toggle }));
    }
    picked(st, resolved).expect("two picks").paths
}

/// Marks the picked conflicted `paths` resolved, agreeing to stage the markers left in them.
fn mark_resolved(st: &mut AppState, paths: Vec<String>) {
    drive(st, Msg::Conflict(ConflictMsg::MarkResolved(paths)));
    assert!(st.modal.is_some(), "one markers warning for both");
    drive(st, Msg::ConfirmAccepted);
}

#[test]
fn mark_resolved_of_picks_with_a_gap_is_one_undo_entry() {
    let dir = three_conflicts("cpicks-flow-resolve");
    let mut st = opened(&dir);
    assert_eq!(conflicted(&st), FILES);
    let paths = pick_a_and_c(&mut st, false);
    assert_eq!(paths, ["a.txt", "c.txt"]);
    mark_resolved(&mut st, paths);
    assert_eq!(conflicted(&st), ["b.txt"], "the middle one stays conflicted");
    assert_eq!(st.session.undo.undo.len(), 1, "{:?}", st.session.undo);
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Conflicts { change: ConflictChange::MarkResolved, .. })));
    assert!(st.cues.conflict_picks.is_none());

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(conflicted(&st), FILES, "both conflicts are back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_conflicted_of_resolved_picks_is_one_undo_entry() {
    let dir = three_conflicts("cpicks-flow-conflicted");
    let mut st = opened(&dir);
    mark_resolved(&mut st, FILES.map(String::from).to_vec());
    assert!(conflicted(&st).is_empty());
    let resolved: Vec<&str> = st.session.conflict_files.resolved.iter().map(|r| r.entry.path.as_str()).collect();
    assert_eq!(resolved, FILES);

    let paths = pick_a_and_c(&mut st, true);
    let t = st.settings.lang();
    let body = glowfly_model::state::conflict_files::mark_conflicted_body(t, &paths);
    let then = Msg::Conflict(ConflictMsg::MarkConflicted(paths));
    let (title, button) = (t.tr("conflict.files.mark_conflicted.title"), t.tr("conflict.files.mark_conflicted.button"));
    drive(&mut st, Msg::confirm(title, body, button, then));
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(conflicted(&st), ["a.txt", "c.txt"], "both back in Conflicted Files");
    assert_eq!(st.session.undo.undo.len(), 2, "{:?}", st.session.undo);
    let last = st.session.undo.undo.last();
    assert!(matches!(last, Some(Action::Conflicts { change: ConflictChange::MarkConflicted, files, .. }) if files.len() == 2), "{last:?}");
    assert!(st.cues.conflict_picks.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
