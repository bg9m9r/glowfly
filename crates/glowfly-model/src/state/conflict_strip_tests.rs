//! The conflict view's strip: its sentence per operation, the count, and Abort with Commit merge
//! or Continue, waiting while files are conflicted.

use super::*;
use crate::msg::RebaseMsg;
use crate::state::conflict::ConflictView;
use crate::state::op_fixture::{rows, snapshot};
use glowfly_core::rebase::Resume;
use glowfly_core::repo::{ConflictContent, ConflictFile, ConflictIds};
use glowfly_core::settings::Settings;

fn file(operation: Operation) -> ConflictFile {
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation, ours: "main".into(), theirs: "theme-tweaks".into() },
        content: ConflictContent::Text(Vec::new()),
        no_ancestor: false,
        base_markers: false,
    }
}

/// `state` with `conflicted` files conflicted, and the conflict view open on a file of
/// `operation` (still loading with `None`).
fn st(name: &str, state: RepositoryState, conflicted: usize, operation: Option<Operation>) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), std::time::Instant::now());
    let mut snap = snapshot(name);
    snap.state = state;
    let names: Vec<String> = (0..conflicted).map(|i| format!("c{i}.txt")).collect();
    snap.unstaged = rows(&names.iter().map(|n| (n.as_str(), '!')).collect::<Vec<_>>());
    st.snap = Some(snap);
    let mut view = ConflictView::loading("c0.txt".into());
    view.file = operation.map(file);
    st.session.conflict.view = Some(view);
    st
}

#[test]
fn a_merge_reads_as_a_sentence_and_commits_once_resolved() {
    let s = st("strip-merge", RepositoryState::Merge, 1, Some(Operation::Merge));
    let strip = conflict_strip(&s).unwrap();
    assert_eq!(
        (strip.title.as_str(), strip.detail.as_str(), strip.conflicted),
        ("Merging theme-tweaks into main", "1 conflicted file left", 1)
    );
    assert_eq!(strip.abort.map(|a| a.0), Some("Abort merge"));
    let f = strip.finish.unwrap();
    assert_eq!((f.label.as_str(), f.why.as_deref()), ("Commit merge", Some("Resolve 1 file first")));
    assert!(matches!(f.msg(), Msg::Commit(CommitMsg::CommitMerge)));

    let s = st("strip-merge-done", RepositoryState::Merge, 0, Some(Operation::Merge));
    let strip = conflict_strip(&s).unwrap();
    assert_eq!(strip.finish.unwrap().why, None);
    let s = st("strip-merge-two", RepositoryState::Merge, 2, None);
    let strip = conflict_strip(&s).unwrap();
    assert_eq!(strip.title, "Merge in progress", "the file still loading");
    assert_eq!(strip.finish.unwrap().why.as_deref(), Some("Resolve 2 files first"));
}

#[test]
fn other_operations_continue_by_name() {
    let s = st("strip-rebase", RepositoryState::RebaseMerge, 1, Some(Operation::Rebase));
    let strip = conflict_strip(&s).unwrap();
    assert_eq!(strip.title, "Rebasing onto main");
    assert_eq!(strip.abort.map(|a| a.0), Some("Abort rebase"));
    let f = strip.finish.unwrap();
    assert_eq!(f.label, "Continue rebase");
    assert!(matches!(f.msg(), Msg::Rebase(RebaseMsg::Resume(Resume::Continue))));
    for (state, op, title, label) in [
        (RepositoryState::CherryPick, Operation::CherryPick, "Cherry-picking theme-tweaks onto main", "Continue cherry-pick"),
        (RepositoryState::Revert, Operation::Revert, "Reverting theme-tweaks on main", "Continue revert"),
    ] {
        let strip = conflict_strip(&st("strip-other", state, 0, Some(op))).unwrap();
        assert_eq!((strip.title.as_str(), strip.finish.unwrap().label.as_str()), (title, label));
        assert_eq!(strip.detail, "All conflicts resolved");
    }
}

#[test]
fn conflicts_alone_have_no_buttons_and_a_closed_view_no_strip() {
    let mut s = st("strip-alone", RepositoryState::Clean, 1, Some(Operation::Other));
    let strip = conflict_strip(&s).unwrap();
    assert_eq!((strip.title.as_str(), strip.detail.as_str()), ("", "1 conflicted file left"));
    assert!(strip.abort.is_none() && strip.finish.is_none());
    s.session.conflict.view = None;
    assert!(conflict_strip(&s).is_none());
}
