//! Git Flow snapshots of a Finish stopped at a merge conflict and of deleting a base branch: the
//! window banner offering Continue finish, the Git Flow panel's row, the in-progress banner's
//! line while the merge is open, the confirmation before deleting the development branch, and
//! the Remove worktree dialog's warning.

use super::snapshot_tests::{harness, remove_fixture, render};
use super::snapshot_tests_gitflow::{pushed, with_flow_branches};
use super::snapshot_tests_worktrees::with_worktrees;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::gitflow_base::{Deleting, guard};
use crate::state::gitflow_resume::StoppedFinish;
use crate::state::worktree_manage::RemoveForm;
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::gitflow::{Branches, FinishOptions, FlowBase, StartTips, WorkState, check_finish};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::ThemeChoice;

/// Finish of `branch` stopped at its merge number `merge` (0-based step index), from a real plan
/// and the fixture's tips.
fn stopped(st: &AppState, branch: &str, options: FinishOptions, merge: usize, ready: bool) -> StoppedFinish {
    let snap = st.snap.as_ref().unwrap();
    let s = snap.gitflow.settings().unwrap();
    let clean = WorkState { state: RepositoryState::Clean, tracked_changes: false, elsewhere: vec![] };
    let plan = check_finish(&s, branch, &options, &Branches::of(snap), &clean).unwrap();
    let resume = plan.rest_after_conflict(merge, &StartTips::of(snap, &plan).unwrap()).unwrap();
    StoppedFinish { resume, seen_merge: true, ready }
}

fn release() -> FinishOptions {
    FinishOptions { tag: "v1.0.0".into(), message: "Release 1.0.0".into(), keep: false }
}

#[test]
fn gitflow_resume_banner() {
    let (mut st, dir) = with_flow_branches(ThemeChoice::Dark, "gitflow-resume-banner");
    st.session.gitflow_resume = Some(stopped(&st, "release/1.0.0", release(), 1, true));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("gitflow_resume_banner");
    assert!(h.query_by_label("Forget").is_some());
    h.get_by_label("Continue finish").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::ContinueFinish))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_panel_resume() {
    let (mut st, dir) = with_flow_branches(ThemeChoice::Light, "gitflow-panel-resume");
    st.session.gitflow_panel = true;
    st.session.gitflow_resume = Some(stopped(&st, "release/1.0.0", release(), 1, true));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("gitflow_panel_resume");
    // The window banner's and the panel row's.
    assert_eq!(h.get_all_by_label("Forget").count(), 2);
    h.get_all_by_label("Forget").last().unwrap().click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::ForgetFinish))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_resume_waiting() {
    let (mut st, dir) = with_flow_branches(ThemeChoice::Dark, "gitflow-resume-waiting");
    st.session.gitflow_resume = Some(stopped(&st, "feature/login", FinishOptions::default(), 1, false));
    let snap = st.snap.as_mut().unwrap();
    snap.state = RepositoryState::Merge;
    snap.head_branch = Some("develop".into());
    st.session.selection = Selection::Wip;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("gitflow_resume_waiting");
    let line = "This merge is part of finishing feature/login. Once it is committed, Continue finish does the rest: delete \
                feature/login.";
    assert!(h.query_by_label(line).is_some());
    assert!(h.query_by_label("Continue finish").is_none(), "no banner while the merge is open");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_delete_base() {
    let (mut st, dir) = with_flow_branches(ThemeChoice::Dark, "gitflow-delete-base");
    let t = Lang::En;
    let plain = Msg::confirm_danger(
        t.tr("branch.delete.title"),
        t.trf("branch.delete.body", &[("branch", &"develop")]),
        t.tr("common.delete.button"),
        Msg::git("Delete branch", &["branch", "-d", "develop"]),
    );
    let Msg::ConfirmDanger { title, body, button, then, .. } = guard(t, st.snap.as_ref().unwrap(), "develop", Deleting::Local, plain)
    else {
        panic!("a danger confirmation")
    };
    assert_eq!(title, "Delete development branch");
    st.modal = Some(Modal::danger(title, body, button, *then));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("gitflow_delete_base");
    h.get_by_label("Delete anyway").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::ConfirmAccepted)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_remove_flow_base() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Light, "worktree-remove-flow-base");
    let wt = st.snap.as_ref().unwrap().worktrees.iter().find(|w| w.folder_name() == "feature-login").unwrap().clone();
    let form =
        RemoveForm { branch: Some("develop".into()), delete_branch: true, flow_base: Some(FlowBase::Development), ..RemoveForm::new(&wt) };
    st.modal = Some(Modal::RemoveWorktree(form));
    render("worktree_remove_flow_base", st, false);
    remove_fixture(&dir);
}
