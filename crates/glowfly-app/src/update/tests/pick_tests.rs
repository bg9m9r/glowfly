//! Headless tests for reverting several commits, the mainline dialog for a merge commit, and
//! selecting the WIP row when a cherry-pick or revert stops. Cherry-picking several commits is
//! in `cherry_pick_tests`.

use super::cherry_pick_tests::feature_repo;
use super::selection_tests::{fixture, git, head, refresh, up};
use super::tests::drive;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::pick::PickOp;
use crate::state::selection::CommitBlock;
use crate::state::{Modal, Selection};
use git2::{Oid, RepositoryState};
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::pick_ops::Picking;
use glowfly_core::undo::{MoveOp, UndoKind};

fn last_log(st: &crate::state::AppState) -> Option<&str> {
    st.log.last().map(|l| l.text.as_str())
}

#[test]
fn revert_runs_newest_first_in_one_job() {
    let (mut st, f) = fixture("pick-revert-order");
    let fx = up(&mut st, Msg::Selection(SelectionMsg::Revert(vec![f.c(4), f.c(5)])));
    let expected = args(&["revert", "--no-edit", &f.c(5).to_string(), &f.c(4).to_string()]);
    let record = Some(UndoKind::Move { op: MoveOp::Revert, detail: Some("2 commits".into()) });
    let [Effect::Pick(j)] = &fx[..] else { panic!("{fx:?}") };
    assert_eq!((&j.cmds, &j.record, j.picking, &j.oids), (&vec![expected], &record, Picking::Revert, &vec![f.c(5), f.c(4)]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Revert 2 commits"));
    let _ = std::fs::remove_dir_all(&f.dir);
}

/// The fixture with `side` merged into main (--no-ff); returns the merge.
pub(super) fn with_merge(st: &mut crate::state::AppState, f: &super::selection_tests::Fixture) -> Oid {
    git(&f.dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    refresh(st);
    head(&f.dir)
}

#[test]
fn merges_open_the_mainline_dialog_with_the_others() {
    let (mut st, f) = fixture("pick-mainline");
    let merge = with_merge(&mut st, &f);
    assert!(up(&mut st, Msg::Selection(SelectionMsg::Revert(vec![f.c(5), merge]))).is_empty());
    let Some(Modal::Mainline(form)) = &st.modal else { panic!("the mainline dialog opens") };
    assert_eq!((form.op, &form.oids, form.merges.len(), form.plain), (PickOp::Revert, &vec![merge, f.c(5)], 1, Some(f.c(5))));
    st.modal = None;

    // A single cherry-pick of a commit HEAD already has is refused.
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f.c(4)]))).is_empty());
    assert_eq!(last_log(&st), Some(CommitBlock::PickOneOnHead.text(Lang::En)));

    git(&f.dir, &["checkout", "-q", "-b", "other", &f.c(1).to_string()]);
    refresh(&mut st);
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![merge]))).is_empty());
    let Some(Modal::Mainline(form)) = &st.modal else { panic!("the mainline dialog opens") };
    assert!(form.single());
    let m = &form.merges[0];
    assert_eq!((form.op, m.oid, form.choice, m.parents.len(), form.choices()), (PickOp::CherryPick, merge, 0, 2, 2));
    assert_eq!(m.parents[0].oid, f.c(5));
    assert_eq!(m.parents[1].oid, f.side);
    assert_eq!(m.parents[1].summary.as_deref(), Some("side"));
    assert!(m.parents[1].labels.contains(&"side".to_owned()), "{:?}", m.parents[1].labels);
    st.modal = None;

    let pick = |op, mainline| Msg::Selection(SelectionMsg::PickWithMainline { op, oids: vec![merge], mainline });
    let fx = up(&mut st, pick(PickOp::CherryPick, 1));
    let expected = args(&["cherry-pick", "-m", "1", &merge.to_string()]);
    assert!(matches!(&fx[..], [Effect::Pick(j)] if j.cmds == vec![expected]), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Cherry-pick"));
    st.job = None;
    for bad in [0, 3] {
        assert!(up(&mut st, pick(PickOp::CherryPick, bad)).is_empty());
        assert_eq!(last_log(&st), Some(super::pick::MAINLINE_MISFIT));
    }
    let fx = up(&mut st, pick(PickOp::Revert, 1));
    let expected = args(&["revert", "--no-edit", "-m", "1", &merge.to_string()]);
    assert!(matches!(&fx[..], [Effect::Pick(j)] if j.cmds == vec![expected]), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Revert"));
    st.job = None;
    // A mainline for commits without a merge doesn't fit them.
    let plain = Msg::Selection(SelectionMsg::PickWithMainline { op: PickOp::CherryPick, oids: vec![f.c(5)], mainline: 1 });
    assert!(up(&mut st, plain).is_empty());
    assert_eq!(last_log(&st), Some(super::pick::MAINLINE_MISFIT));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_mainline_cherry_pick_brings_the_branch_over() {
    let (mut st, f) = fixture("pick-mainline-e2e");
    let merge = with_merge(&mut st, &f);
    git(&f.dir, &["checkout", "-q", "-b", "other", &f.c(5).to_string()]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Selection(SelectionMsg::PickWithMainline { op: PickOp::CherryPick, oids: vec![merge], mainline: 1 }));
    assert!(st.job.is_none());
    assert!(f.dir.join("s.txt").exists(), "side's file is on `other`");
    let parents = git(&f.dir, &["rev-list", "--parents", "-1", "HEAD"]);
    assert_eq!(parents.split_whitespace().count(), 2, "the pick is an ordinary commit: {parents}");
    assert_eq!(git(&f.dir, &["log", "--format=%s", "-1", "other"]).trim(), "merge side");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_multi_revert_restores_the_file() {
    let (mut st, dir, f) = feature_repo("pick-revert-e2e", &[("f1", "base\n1\n"), ("f2", "base\n1\n2\n")], None);
    git(&dir, &["checkout", "-q", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Selection(SelectionMsg::Revert(vec![f[1], f[0]])));
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Revert 2 commits succeeded"));
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n"), "base\n");
    let _ = std::fs::remove_dir_all(&dir);
}

fn labels(state: RepositoryState) -> Vec<&'static str> {
    crate::ui::common::state_actions(state).unwrap().iter().map(|(l, _)| *l).collect()
}

#[test]
fn a_stopped_cherry_pick_selects_the_wip_row_and_skips_on() {
    let (mut st, dir, f) = feature_repo("pick-stop-skip", &[("x1", "theirs\n"), ("x2", "theirs\nmore\n")], Some("ours\n"));
    // Something else open: the stop moves to the WIP row anyway.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f[0])));
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f[0], f[1]])));
    let state = st.snap.as_ref().unwrap().state;
    assert!(matches!(state, RepositoryState::CherryPick | RepositoryState::CherryPickSequence), "{state:?}");
    assert_eq!(st.selection, Selection::Wip);
    assert_eq!(labels(state), ["Continue", "Skip commit", "Abort cherry-pick"]);
    let (label, skip) = crate::ui::common::state_actions(state).unwrap()[1];
    let recorded = st.undo.undo.len();
    drive(&mut st, Msg::git(label, skip));
    assert_eq!(st.undo.undo.len(), recorded, "a skip is never recorded for undo");
    // x2 conflicts as well once x1 is skipped (it builds on x1's line), so it stops at it or ends.
    let after = st.snap.as_ref().unwrap().state;
    assert!(matches!(after, RepositoryState::Clean | RepositoryState::CherryPick | RepositoryState::CherryPickSequence), "{after:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stopped_revert_selects_the_wip_row() {
    let (mut st, dir, f) = feature_repo("pick-revert-stop", &[("f1", "base\n1\n"), ("f2", "base\n1\n2\n")], None);
    git(&dir, &["checkout", "-q", "feature"]);
    std::fs::write(dir.join("f.txt"), "base\n1\n2\n3\n").unwrap();
    super::selection_tests::commit_on(&dir, "f3", 20);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f[1])));
    // Reverting f1 alone (after f3 changed the next lines) conflicts.
    drive(&mut st, Msg::Selection(SelectionMsg::Revert(vec![f[0], f[1]])));
    let state = st.snap.as_ref().unwrap().state;
    assert!(matches!(state, RepositoryState::Revert | RepositoryState::RevertSequence), "{state:?}");
    assert_eq!(st.selection, Selection::Wip);
    assert_eq!(labels(state), ["Continue", "Skip commit", "Abort revert"]);
    let _ = std::fs::remove_dir_all(&dir);
}
