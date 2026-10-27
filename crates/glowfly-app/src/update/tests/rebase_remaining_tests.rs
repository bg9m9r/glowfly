//! Editing the remaining steps of a running interactive rebase: opening the panel on them, the
//! list arriving, Update steps asking about new drops and running the edit as a job, its
//! result, and the rebase ending or moving on while the panel is open.

use super::rebase_tests::{running_job, with_panel};
use super::selection_tests::git;
use super::tests::drive;
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, RebaseMsg, RepoMsg, TabsMsg};
use crate::state::rebase::{PanelSource, RebasePanel, UNCHANGED};
use crate::state::rebase_remaining::{EDIT_JOB, ENDED, UPDATED};
use crate::state::{AppState, Modal};
use git2::{Oid, RepositoryState};
use glowfly_core::rebase::{self, RebaseAction, RebaseBase, TodoStep};
use glowfly_core::repo::{DiffTarget, FileDiff};
use std::path::PathBuf;
use std::time::Instant;

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn panel(st: &AppState) -> &RebasePanel {
    st.session.rebase.view.as_ref().expect("panel open")
}

fn last_toast(st: &AppState) -> (bool, &str) {
    st.toasts.last().map(|t| (t.ok, t.text.as_str())).unwrap_or((false, ""))
}

fn last_log(st: &AppState) -> &str {
    st.log.last().map(|l| l.text.as_str()).unwrap_or("")
}

/// A rebase of c1..c3 stopped to edit c1, with c2 and c3 left (`edits` more rows to edit).
fn stopped_with(name: &str, edits: &[usize]) -> (AppState, PathBuf, [Oid; 3]) {
    let (mut st, dir, c) = with_panel(name, RebaseBase::FromCommit(Oid::zero()));
    for &index in [0].iter().chain(edits) {
        drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index, action: RebaseAction::Edit }));
    }
    drive(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::RebaseInteractive);
    assert!(st.session.rebase.view.is_none() && st.job.is_none());
    (st, dir, c)
}

fn stopped(name: &str) -> (AppState, PathBuf, [Oid; 3]) {
    stopped_with(name, &[])
}

#[test]
fn opening_needs_an_interactive_rebase() {
    let (mut st, dir, _) = super::rebase_tests::repo("remaining-refused");
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::OpenRemaining));
    assert!(fx.is_empty() && st.session.rebase.view.is_none());
    assert_eq!(last_toast(&st), (false, rebase::NOT_INTERACTIVE));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_loads_the_steps() {
    let (mut st, dir, c) = stopped("remaining-open");
    let diff = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.session.diff.view = Some((DiffTarget::Commit(c[0], "c1.txt".into()), diff.into()));
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::OpenRemaining));
    assert!(st.session.diff.view.is_none(), "the panel takes the diff's place");
    assert!(
        matches!(&fx[..], [Effect::Rebase(RebaseEffect::LoadRemaining { generation, .. })] if *generation == st.session.rebase.generation),
        "{fx:?}"
    );
    assert!(panel(&st).loading && panel(&st).source == PanelSource::Remaining);

    // An older request's result is ignored.
    let stale = Msg::Rebase(RebaseMsg::RemainingLoaded { generation: st.session.rebase.generation - 1, result: Err("old".into()) });
    up(&mut st, stale);
    assert!(panel(&st).loading);
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRemaining));
    let p = panel(&st);
    assert!(!p.loading);
    let oids: Vec<Oid> = p.items.iter().filter_map(TodoStep::oid).collect();
    assert_eq!(oids, [c[1], c[2]]);
    assert_eq!(p.subtitle(), format!("2 steps left in the rebase of main onto {} · 1 done", panel_onto(&st)));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(p.start_blocker(snap, false).as_deref(), Some(UNCHANGED));
    let _ = std::fs::remove_dir_all(&dir);
}

fn panel_onto(st: &AppState) -> String {
    panel(st).remaining.as_ref().unwrap().onto_label.clone()
}

#[test]
fn update_steps_asks_about_drops_and_runs_the_edit() {
    let (mut st, dir, c) = stopped("remaining-update");
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRemaining));
    // Uncommitted changes and conflicted files don't matter here.
    std::fs::write(dir.join("c1.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.session.rebase.view.is_some());
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 0, action: RebaseAction::Drop }));
    assert_eq!(panel(&st).start_blocker(st.snap.as_ref().unwrap(), false), None);
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
    assert!(fx.is_empty());
    match &st.modal {
        Some(Modal::ConfirmDanger { title, button, .. }) => assert_eq!((title.as_str(), button.as_str()), ("Drop commits", "Update steps")),
        _ => panic!("expected a confirmation"),
    }
    let fx = up(&mut st, Msg::ConfirmAccepted);
    let [Effect::Rebase(RebaseEffect::EditTodo { request, .. })] = &fx[..] else { panic!("expected the edit, got {fx:?}") };
    assert_eq!(request.steps[0].commit().map(|i| (i.oid, i.action)), Some((c[1], RebaseAction::Drop)));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(EDIT_JOB));
    assert!(panel(&st).starting);

    // The edit keeps the undo history.
    let a = glowfly_core::undo::Action::CreateTag { name: "v1".into(), oid: c[0] };
    st.undo.record(a.clone());
    up(&mut st, Msg::Rebase(RebaseMsg::RemainingEdited { result: Ok(()) }));
    assert!(st.session.rebase.view.is_none(), "the panel closes");
    assert_eq!(last_toast(&st), (true, UPDATED));
    assert_eq!(st.undo.undo.last(), Some(&a));
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_edit_keeps_the_panel() {
    let (mut st, dir, _) = stopped("remaining-refused-edit");
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRemaining));
    st.job = Some(running_job(&dir));
    st.session.rebase.view.as_mut().unwrap().starting = true;
    up(&mut st, Msg::Rebase(RebaseMsg::RemainingEdited { result: Err(rebase::TODO_MOVED.into()) }));
    let p = panel(&st);
    assert!(!p.starting && p.stale);
    assert_eq!(p.error.as_deref(), Some(rebase::TODO_MOVED));
    assert!(st.job.is_none());
    assert_eq!(last_toast(&st), (false, format!("Couldn't update the rebase steps: {}", rebase::TODO_MOVED).as_str()));
    git(&dir, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_panel_follows_the_rebase() {
    let (mut st, dir, _) = stopped_with("remaining-ended", &[1]);
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRemaining));
    // Moved on (a step done in a terminal): stale.
    git(&dir, &["rebase", "--continue"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::RebaseInteractive);
    assert!(panel(&st).stale);
    assert_eq!(panel(&st).start_blocker(st.snap.as_ref().unwrap(), false).as_deref(), Some(rebase::TODO_MOVED));
    git(&dir, &["rebase", "--abort"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // Ended: the panel closes.
    assert!(st.session.rebase.view.is_none());
    assert_eq!(last_log(&st), ENDED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_parked_loading_panel_asks_again() {
    let (mut st, a, _) = stopped("remaining-tabs-a");
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRemaining));
    let b = super::tests::scratch_repo("remaining-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    st.session.rebase.view.as_mut().unwrap().reload();
    up(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    let fx = up(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let gen_now = st.session.rebase.generation;
    assert!(
        fx.iter().any(|e| matches!(e, Effect::Rebase(RebaseEffect::LoadRemaining { generation, .. }) if *generation == gen_now)),
        "{fx:?}"
    );
    git(&a, &["rebase", "--abort"]);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}
