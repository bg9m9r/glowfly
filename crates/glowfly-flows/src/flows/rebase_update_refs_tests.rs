//! Headless tests for the rebase panel with `rebase.updateRefs`: Start asks before an edit
//! leaves a stacked branch on another commit, and a branch alias git would move wrongly holds
//! Start back, in the user's words.

use super::rebase_tests::repo;
use super::selection_tests::git;
use super::tests::drive;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::RebasePanel;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::{RebaseAction, RebaseBase, TodoStep};
use std::path::PathBuf;
use std::time::Instant;

/// main: first, c1..c3 with `rebase.updateRefs` on and `lower` at c2; the panel open from c1.
fn stacked(name: &str) -> (AppState, PathBuf, [Oid; 3]) {
    let (mut st, dir, c) = repo(name);
    git(&dir, &["config", "rebase.updateRefs", "true"]);
    git(&dir, &["branch", "lower", &c[1].to_string()]);
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(c[0]))));
    let p = panel(&st);
    assert!(!p.loading && p.items.get(2) == Some(&TodoStep::UpdateRef("refs/heads/lower".into())), "{:?}", p.items);
    (st, dir, c)
}

fn panel(st: &AppState) -> &RebasePanel {
    st.session.rebase.view.as_ref().unwrap()
}

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn start(st: &mut AppState) -> Vec<Effect> {
    up(st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }))
}

#[test]
fn start_asks_before_a_stacked_branch_loses_its_commit() {
    let (mut st, dir, _) = stacked("rebase-update-refs-ask");
    // c3 dropped or moved above c2 doesn't change where lower ends.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 3, action: RebaseAction::Drop }));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 3, to: 1 }));
    assert!(panel(&st).moved_refs().is_empty(), "{:?}", panel(&st).items);
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseReset));
    // c3 fixed up into c2: lower gets the combined commit.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 3, action: RebaseAction::Fixup }));
    assert_eq!(panel(&st).moved_refs(), ["lower"]);
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 3, action: RebaseAction::Pick }));
    // c2 fixed up into c1: lower then ends on the combined commit.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Fixup }));
    assert_eq!(panel(&st).moved_refs(), ["lower"]);
    assert!(start(&mut st).is_empty());
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, .. }) => {
            assert_eq!(title, "Move stacked branches");
            assert!(body.contains(": lower."), "{body}");
        }
        _ => panic!("expected a confirmation"),
    }
    st.modal = None;
    // Dropping c2 asks once, naming lower beside the drop.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Drop }));
    assert!(start(&mut st).is_empty());
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, .. }) => {
            assert_eq!(title, "Drop commits");
            assert!(body.starts_with("Drop 1 commit from main?") && body.contains(": lower."), "{body}");
        }
        _ => panic!("expected a confirmation"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_alias_of_a_branch_checked_out_elsewhere_holds_start() {
    let (mut st, dir, c) = repo("rebase-update-refs-alias");
    git(&dir, &["config", "rebase.updateRefs", "true"]);
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "-b", "lower", &wt.to_string_lossy(), &c[1].to_string()]);
    git(&dir, &["symbolic-ref", "refs/heads/alias", "refs/heads/lower"]);
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(c[0]))));
    let p = panel(&st);
    let blocker = p.start_blocker(st.snap.as_ref().unwrap(), false).expect("Start is held");
    assert_eq!(blocker, p.plan.as_ref().unwrap().held[0].text());
    let en = p.held_text(Lang::En, &blocker).unwrap();
    assert!(en.starts_with("alias points at lower, which is checked out in another worktree"), "{en}");
    let de = p.held_text(Lang::De, &blocker).unwrap();
    assert!(de.starts_with("alias zeigt auf lower"), "{de}");
    assert!(start(&mut st).is_empty());
    assert!(st.modal.is_none() && st.job.is_none(), "nothing starts");
    git(&dir, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
    let _ = std::fs::remove_dir_all(&dir);
}
