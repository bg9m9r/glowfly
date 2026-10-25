//! Starting the panel's rebase with uncommitted changes: one question (also covering drops),
//! then `--autostash`, and the result when git couldn't put the changes back.

use super::rebase_tests::{running_job, with_panel};
use super::tests::drive;
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, RebaseMsg, RepoMsg};
use crate::state::rebase::STASH_LINE;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::rebase::{Outcome, RebaseAction, RebaseBase, StartRequest};
use std::time::Instant;

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn confirm(st: &AppState) -> (&str, &str, &str) {
    match &st.modal {
        Some(Modal::Confirm { title, body, button, .. } | Modal::ConfirmDanger { title, body, button, .. }) => (title, body, button),
        _ => panic!("expected a confirmation"),
    }
}

fn started(fx: &[Effect]) -> &StartRequest {
    match fx {
        [Effect::Rebase(RebaseEffect::Start { request, .. })] => request,
        other => panic!("expected a rebase, got {other:?}"),
    }
}

fn start() -> Msg {
    Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false })
}

#[test]
fn a_dirty_tree_asks_then_stashes() {
    let (mut st, dir, _) = with_panel("rebase-stash-ask", RebaseBase::FromCommit(Oid::zero()));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 0 }));
    std::fs::write(dir.join("a.txt"), "one\nedited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(up(&mut st, start()).is_empty());
    assert_eq!(confirm(&st), ("Stash changes and rebase", STASH_LINE, "Start rebase"));
    assert!(started(&up(&mut st, Msg::ConfirmAccepted)).autostash);
    st.job = None;
    st.rebase.as_mut().unwrap().starting = false;

    // Drops and the stash share one question.
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Drop }));
    assert!(up(&mut st, start()).is_empty());
    let (title, body, _) = confirm(&st);
    assert_eq!(title, "Drop commits");
    assert_eq!(body, format!("Drop 1 commit from main? Their changes will be removed.\n\n{STASH_LINE}"));
    assert!(started(&up(&mut st, Msg::ConfirmAccepted)).autostash);
    st.job = None;
    st.rebase.as_mut().unwrap().starting = false;

    // A question that didn't mention stashing asks again once the tree is dirty.
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: true, autostash: false }));
    assert!(fx.is_empty());
    assert!(confirm(&st).1.ends_with(STASH_LINE));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_kept_stash_is_reported() {
    let (mut st, dir, _) = with_panel("rebase-stash-kept", RebaseBase::FromCommit(Oid::zero()));
    st.job = Some(running_job(&dir));
    let result = Ok(Outcome::Finished { output: String::new(), stash_kept: true });
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseFinished { result, undo: None }));
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok);
    assert_eq!(toast.text, "Interactive rebase finished, but your stashed changes didn't apply cleanly. They are kept in the stash list.");
    assert!(st.rebase.is_none(), "the rebase ran, so the panel closes");
    let _ = std::fs::remove_dir_all(&dir);
}
