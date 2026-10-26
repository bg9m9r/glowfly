//! Headless tests for the rebase panel on a range with a merge (label, reset and merge rows),
//! and for rebasing onto a commit (`RebaseBase::OntoCommit`).

use super::rebase_quick_tests::up;
use super::rebase_tests::repo;
use super::selection_tests::{commit_on, git, head};
use super::tests::drive;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, RebaseMsg, RepoMsg};
use crate::state::AppState;
use crate::state::rebase::RebasePanel;
use crate::state::rebase_onto::onto_commit_blocker;
use git2::{Oid, RepositoryState};
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::{self, RebaseAction, RebaseBase, ResetTo, TodoStep};
use std::path::Path;

fn panel(st: &AppState) -> &RebasePanel {
    st.session.rebase.view.as_ref().expect("panel open")
}

fn short(o: Oid) -> String {
    o.to_string()[..7].to_owned()
}

/// A side branch from c1 with one commit, merged after c3; returns the side commit and the merge.
fn merge_side(st: &mut AppState, dir: &Path, c1: Oid) -> (Oid, Oid) {
    git(dir, &["checkout", "-q", "-b", "side", &c1.to_string()]);
    std::fs::write(dir.join("s.txt"), "s\n").unwrap();
    let side = commit_on(dir, "side", 10);
    git(dir, &["checkout", "-q", "main"]);
    git(dir, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    let merge = head(dir);
    drive(st, Msg::Repo(RepoMsg::Refresh));
    (side, merge)
}

#[test]
fn panel_with_merges() {
    let (mut st, dir, c) = repo("panel-merges");
    let (side, merge) = merge_side(&mut st, &dir, c[0]);
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(c[1]))));
    let p = panel(&st);
    assert!(!p.loading);
    // reset onto, side, label, reset onto, c2, c3, merge
    assert_eq!(p.items.len(), 7, "{:?}", p.items);
    assert_eq!(p.items[0], TodoStep::Reset(ResetTo::Onto));
    assert!(matches!(&p.items[2], TodoStep::Label { at, .. } if *at == side));
    assert_eq!(p.items[6].oid(), Some(merge));
    assert_eq!(p.subtitle(), format!("Rebasing 3 commits and 1 merge of main onto {} c1", short(c[0])));
    assert_eq!(p.step_text(0, None), format!("Continue from the new base ({} c1)", short(c[0])));
    assert_eq!(p.step_text(2, None), format!("at-{}: marks {} side for a later step", short(side), short(side)));
    assert_eq!(p.actions(6), RebaseAction::MERGE_ACTIONS);
    assert!(!p.movable(2) && !p.movable(6) && p.movable(1));

    // A merge row takes only Pick, Reword and Drop; fixed rows don't move.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 6, action: RebaseAction::Squash }));
    assert_eq!(panel(&st).items[6].item().unwrap().action, RebaseAction::Pick);
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 3 }));
    assert!(matches!(panel(&st).items[2], TodoStep::Label { .. }), "a label row stays put");
    // A commit row moves across a reset, to another line.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 1, to: 4 }));
    assert_eq!(panel(&st).items[4].oid(), Some(side));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseReset));

    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 6, action: RebaseAction::Drop }));
    assert_eq!(panel(&st).items[6].item().unwrap().action, RebaseAction::Drop);
    assert_eq!(panel(&st).drops(), 1, "a dropped merge counts");
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: true, autostash: false }));
    match &fx[..] {
        [Effect::Rebase(RebaseEffect::Start { request, .. })] => {
            assert!(request.items.iter().any(TodoStep::is_structure), "the request holds the structure");
            assert_eq!(request.items.len(), 7);
        }
        other => panic!("expected a start, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A commit row moved where git would pick it and a reset would then throw it away (between its
/// line's closing label and the next reset, or above the first reset) blocks Start.
#[test]
fn commit_lost_to_a_reset_blocks_start() {
    let (mut st, dir, c) = repo("panel-merges-lost");
    let (side, _) = merge_side(&mut st, &dir, c[0]);
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(c[1]))));
    // reset onto, side, label, reset onto, c2, c3, merge
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 1, to: 2 }));
    assert_eq!(panel(&st).items[2].oid(), Some(side), "the move itself is allowed");
    let blocker = panel(&st).start_blocker(st.snap.as_ref().unwrap(), false);
    assert_eq!(blocker.as_deref(), Some(rebase::LOST_TO_RESET));
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: true, autostash: false }));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Rebase(RebaseEffect::Start { .. }))), "{fx:?}");

    up(&mut st, Msg::Rebase(RebaseMsg::RebaseReset));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 4, to: 0 }));
    assert_eq!(panel(&st).items[0].oid(), Some(c[1]));
    let blocker = panel(&st).start_blocker(st.snap.as_ref().unwrap(), false);
    assert_eq!(blocker.as_deref(), Some(rebase::LOST_TO_RESET), "above the first reset");
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseReset));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 4, action: RebaseAction::Reword }));
    assert_ne!(panel(&st).start_blocker(st.snap.as_ref().unwrap(), false).as_deref(), Some(rebase::LOST_TO_RESET));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_onto_commit() {
    let (mut st, dir, c) = repo("panel-onto-commit");
    git(&dir, &["checkout", "-q", "-b", "other", &c[0].to_string()]);
    std::fs::write(dir.join("o.txt"), "o\n").unwrap();
    let other = commit_on(&dir, "other", 9);
    git(&dir, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoCommit(other))));
    let p = panel(&st);
    assert_eq!(p.items.len(), 2, "c2 and c3");
    assert_eq!(p.subtitle(), format!("Rebasing 2 commits of main onto {} other", short(other)));
    assert_eq!(p.start_blocker(st.snap.as_ref().unwrap(), false), None, "an unchanged list may start: the base moves");
    drive(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
    let log = git(&dir, &["log", "--format=%s"]);
    assert_eq!(log.lines().collect::<Vec<_>>(), ["c3", "c2", "other", "c1", "first"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn onto_commit_blockers() {
    let (mut st, dir, c) = repo("onto-commit-blockers");
    git(&dir, &["checkout", "-q", "-b", "other", &c[0].to_string()]);
    std::fs::write(dir.join("o.txt"), "o\n").unwrap();
    let other = commit_on(&dir, "other", 9);
    git(&dir, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_mut().unwrap();
    assert_eq!(onto_commit_blocker(Lang::En, snap, c[2]), Some(format!("main already contains {}.", short(c[2]))), "HEAD");
    assert_eq!(onto_commit_blocker(Lang::En, snap, c[0]), Some(rebase::contains_commit("main", c[0])), "an ancestor");
    assert_eq!(onto_commit_blocker(Lang::En, snap, other), None, "a commit on another branch");
    snap.state = RepositoryState::Merge;
    assert_eq!(onto_commit_blocker(Lang::En, snap, other), rebase::in_progress_text(RepositoryState::Merge));
    snap.state = RepositoryState::Clean;
    snap.head_branch = None;
    assert_eq!(onto_commit_blocker(Lang::En, snap, other).as_deref(), Some(rebase::DETACHED));
    let _ = std::fs::remove_dir_all(&dir);
}
