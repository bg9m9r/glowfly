//! Headless tests for the interactive rebase panel: opening and loading, editing the list,
//! the start rules, staleness, and tabs. End-to-end runs are in `rebase_e2e_tests`.

use super::rebase::JOB_LABEL;
use super::selection_tests::{commit_on, git, head};
use super::tests::{drive, state};
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::state::conflict::ConflictView;
use crate::state::history::FileHistoryView;
use crate::state::rebase::{CONFLICTED_FILES, Editing, RebasePanel, UNCHANGED};
use crate::state::{AppState, Modal, RunningJob};
use git2::{Oid, RepositoryState};
use glowfly_core::rebase::{self, Outcome, RebaseAction, RebaseBase, RebasePlan, TodoItem, TodoStep, Upstream};
use glowfly_core::repo::{DiffTarget, FileDiff, FileEntry};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A clean repository on main: "first", then c1..c3 (each adding c<n>.txt), opened.
pub(super) fn repo(name: &str) -> (AppState, PathBuf, [Oid; 3]) {
    let dir = super::tests::scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    let mut c = [Oid::zero(); 3];
    for (i, oid) in c.iter_mut().enumerate() {
        std::fs::write(dir.join(format!("c{}.txt", i + 1)), format!("{}\n", i + 1)).unwrap();
        *oid = commit_on(&dir, &format!("c{}", i + 1), i as u32 + 2);
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, c)
}

/// The repository with the panel loaded from `base`.
pub(super) fn with_panel(name: &str, base: RebaseBase) -> (AppState, PathBuf, [Oid; 3]) {
    let (mut st, dir, c) = repo(name);
    let base = match base {
        RebaseBase::FromCommit(o) if o.is_zero() => RebaseBase::FromCommit(c[0]),
        b => b,
    };
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(base)));
    assert!(st.session.rebase.view.as_ref().is_some_and(|p| !p.loading && p.items.len() == 3), "the list loaded");
    (st, dir, c)
}

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn panel(st: &AppState) -> &RebasePanel {
    st.session.rebase.view.as_ref().expect("panel open")
}

fn actions(st: &AppState) -> Vec<RebaseAction> {
    panel(st).items.iter().filter_map(TodoStep::commit).map(|i| i.action).collect()
}

fn order(st: &AppState) -> Vec<Oid> {
    panel(st).items.iter().filter_map(TodoStep::oid).collect()
}

fn last_log(st: &AppState) -> &str {
    st.log.last().map(|l| l.text.as_str()).unwrap_or("")
}

fn fake_oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn fake_plan(n: u8) -> RebasePlan {
    let item = |k: u8| TodoItem {
        oid: fake_oid(k),
        action: RebaseAction::Pick,
        summary: format!("commit {k}"),
        message: format!("commit {k}\n\nbody {k}\n"),
        new_message: None,
        combined_message: None,
        author: "Test".into(),
        time: 1_700_000_000,
    };
    RebasePlan {
        head: fake_oid(99),
        branch: "main".into(),
        upstream: Upstream::Root,
        onto_label: String::new(),
        steps: (1..=n).map(|k| TodoStep::Commit(item(k))).collect(),
    }
}

/// A panel filled from `fake_plan(3)`, with no repository behind it.
fn fake_panel() -> AppState {
    let mut st = state();
    let mut p = RebasePanel::loading(RebaseBase::FromCommit(fake_oid(1)));
    p.fill(fake_plan(3));
    st.session.rebase.view = Some(p);
    st
}

pub(super) fn running_job(dir: &Path) -> RunningJob {
    RunningJob {
        label: JOB_LABEL.into(),
        workdir: Some(dir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: false,
        fetches: false,
    }
}

#[test]
fn opening_requests_the_list_and_closes_other_views() {
    let (mut st, dir, c) = repo("rebase-open");
    let diff = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.session.diff.view = Some((DiffTarget::Commit(c[0], "c1.txt".into()), diff.into()));
    st.session.history.view = Some(FileHistoryView::loading("c1.txt".into(), None));
    st.session.conflict.view = Some(ConflictView::loading("c1.txt".into()));
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(c[1]))));
    match &fx[..] {
        [e @ Effect::Rebase(RebaseEffect::LoadPlan { base, generation, .. })] => {
            assert_eq!((base, *generation), (&RebaseBase::FromCommit(c[1]), st.session.rebase.generation));
            assert!(e.scope().is_some(), "the result comes back for this tab only");
        }
        other => panic!("expected one list request, got {other:?}"),
    }
    assert!(st.session.diff.view.is_none() && st.session.history.view.is_none() && st.session.conflict.view.is_none());
    assert!(panel(&st).loading);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_is_refused_during_a_merge_and_on_a_detached_head() {
    let (mut st, dir, c) = repo("rebase-open-refused");
    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(c[0])))).is_empty());
    assert_eq!(last_log(&st), "Finish or abort the merge in progress before starting an interactive rebase.");
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text == last_log(&st)));
    assert!(st.session.rebase.view.is_none());

    st.snap.as_mut().unwrap().state = RepositoryState::Clean;
    st.snap.as_mut().unwrap().head_branch = None;
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(c[0])))).is_empty());
    assert_eq!(last_log(&st), rebase::DETACHED);
    assert!(st.session.rebase.view.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_and_failed_list_results() {
    let mut st = state();
    st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(fake_oid(1))));
    st.session.rebase.generation = 4;
    up(&mut st, Msg::Rebase(RebaseMsg::RebasePlanLoaded { generation: 3, result: Ok(fake_plan(2)) }));
    assert!(panel(&st).loading, "an older request's result is ignored");
    up(&mut st, Msg::Rebase(RebaseMsg::CloseRebase));
    up(&mut st, Msg::Rebase(RebaseMsg::RebasePlanLoaded { generation: 4, result: Ok(fake_plan(2)) }));
    assert!(st.session.rebase.view.is_none(), "a result after closing doesn't reopen the panel");

    st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(fake_oid(1))));
    let generation = st.session.rebase.generation;
    up(&mut st, Msg::Rebase(RebaseMsg::RebasePlanLoaded { generation, result: Ok(fake_plan(2)) }));
    assert_eq!(panel(&st).items.len(), 2);

    st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(fake_oid(1))));
    up(&mut st, Msg::Rebase(RebaseMsg::RebasePlanLoaded { generation, result: Err("1a2b3c4 is not on the current branch".into()) }));
    assert!(st.session.rebase.view.is_none());
    assert_eq!(last_log(&st), "1a2b3c4 is not on the current branch");
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text == "1a2b3c4 is not on the current branch"));
}

#[test]
fn editing_the_list() {
    use RebaseAction::*;
    let mut st = fake_panel();
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 2, action: Drop }));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 9, action: Edit }));
    assert_eq!(actions(&st), [Pick, Pick, Drop], "an index out of range is ignored");

    let original: Vec<Oid> = order(&st);
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 0, to: 2 }));
    assert_eq!(order(&st), [original[1], original[2], original[0]], "removed, then inserted, like MoveTab");
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 0 }));
    assert_eq!(order(&st), original);
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 0, to: 3 }));
    assert_eq!(order(&st), original, "out of range");

    // Reword opens the editor with the full message; Update stores the trimmed draft.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: Reword }));
    assert_eq!((panel(&st).editing, panel(&st).draft.as_str()), (Some(Editing::Message(1)), "commit 2\n\nbody 2"));
    st.session.rebase.view.as_mut().unwrap().draft = "New title\n\nNew body\n\n".into();
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSaveMessage));
    let item = panel(&st).items[1].commit().unwrap();
    assert_eq!((item.action, item.new_message.as_deref()), (Reword, Some("New title\n\nNew body")));
    assert_eq!(panel(&st).editing, None);

    // Leaving reword keeps the message; choosing it again brings it back.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: Pick }));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: Reword }));
    assert_eq!(panel(&st).draft, "New title\n\nNew body");
    // Cancel closes the editor and keeps the action.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseCancelMessage));
    assert_eq!((panel(&st).editing, actions(&st)[1]), (None, Reword));

    // The editor follows its row when rows move; double-click opens it on any row.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseEditMessage(0)));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 0, to: 1 }));
    assert_eq!(panel(&st).editing, Some(Editing::Message(1)));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseEditMessage(7)));
    assert_eq!(panel(&st).editing, Some(Editing::Message(1)), "out of range");

    up(&mut st, Msg::Rebase(RebaseMsg::RebaseReset));
    assert_eq!(order(&st), original);
    assert_eq!(actions(&st), [Pick, Pick, Pick]);
    assert!(panel(&st).items.iter().filter_map(TodoStep::commit).all(|i| i.new_message.is_none()) && panel(&st).editing.is_none());
}

#[test]
fn start_is_refused_with_a_reason() {
    let (mut st, dir, c) = with_panel("rebase-start-refused", RebaseBase::FromCommit(Oid::zero()));
    let start = |st: &mut AppState| up(st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));

    // Unchanged lists from a commit have nothing to do.
    assert!(start(&mut st).is_empty());
    assert_eq!(last_log(&st), UNCHANGED);

    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 0 }));
    st.job = Some(running_job(&dir));
    assert!(start(&mut st).is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    st.job = None;

    // Conflicted files left by a stash apply block it; tracked changes only ask (rebase_stash_tests).
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.push(FileEntry::new("c1.txt", '!'));
    assert!(start(&mut st).is_empty());
    assert_eq!(last_log(&st), CONFLICTED_FILES);
    st.snap.as_mut().unwrap().unstaged = vec![FileEntry::new("new.txt", '?')];

    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 0, action: RebaseAction::Squash }));
    assert!(start(&mut st).is_empty());
    assert_eq!(last_log(&st), rebase::FIRST_COMBINES);
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 0, action: RebaseAction::Pick }));
    assert!(matches!(&start(&mut st)[..], [Effect::Rebase(RebaseEffect::Start { .. })]), "the list is fine now");
    st.job = None;
    st.session.rebase.view.as_mut().unwrap().starting = false;

    // HEAD moved: a refresh marks the list stale.
    std::fs::write(dir.join("c4.txt"), "4\n").unwrap();
    commit_on(&dir, "c4", 9);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(panel(&st).stale);
    assert!(start(&mut st).is_empty());
    assert_eq!(last_log(&st), rebase::STALE);
    // Reset reloads a stale list.
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::RebaseReset));
    assert!(matches!(&fx[..], [Effect::Rebase(RebaseEffect::LoadPlan { generation, .. })] if *generation == st.session.rebase.generation));
    assert!(panel(&st).loading);
    assert_eq!(c.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unchanged_list_onto_a_branch_can_start() {
    let (mut st, dir, _) = repo("rebase-start-onto");
    git(&dir, &["branch", "base", "HEAD~3"]);
    git(&dir, &["checkout", "-q", "base"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_on(&dir, "on base", 20);
    git(&dir, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::OntoBranch("base".into()))));
    assert_eq!(panel(&st).subtitle(), "Rebasing 3 commits of main onto base");
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
    assert!(matches!(&fx[..], [Effect::Rebase(RebaseEffect::Start { .. })]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn drops_are_confirmed_before_starting() {
    let (mut st, dir, c) = with_panel("rebase-start-drop", RebaseBase::FromCommit(Oid::zero()));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Drop }));
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 2, to: 0 }));
    assert!(up(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false })).is_empty());
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, button, .. }) => {
            assert_eq!((title.as_str(), button.as_str()), ("Drop commits", "Start rebase"));
            assert_eq!(body, "Drop 1 commit from main? Their changes will be removed.");
        }
        _ => panic!("expected a confirmation"),
    }
    let fx = up(&mut st, Msg::ConfirmAccepted);
    let [Effect::Rebase(RebaseEffect::Start { cwd, request, .. })] = &fx[..] else { panic!("expected one rebase, got {fx:?}") };
    assert!(super::tests::same_place(cwd, &dir));
    assert_eq!(request.head, head(&dir));
    assert_eq!(request.upstream, Upstream::Commit(st.snap.as_ref().unwrap().commits[3].oid));
    let oids: Vec<Oid> = request.items.iter().filter_map(TodoStep::oid).collect();
    assert_eq!(oids, [c[2], c[0], c[1]], "in panel order");
    assert_eq!(request.items[2].commit().unwrap().action, RebaseAction::Drop);
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(JOB_LABEL));
    assert!(panel(&st).starting);
    // Nothing changes while it runs, and Cancel is ignored.
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseMove { from: 0, to: 1 }));
    up(&mut st, Msg::Rebase(RebaseMsg::CloseRebase));
    assert_eq!(order(&st), [c[2], c[0], c[1]]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_start_keeps_the_panel() {
    let (mut st, dir, _) = with_panel("rebase-finished-err", RebaseBase::FromCommit(Oid::zero()));
    st.job = Some(running_job(&dir));
    st.session.rebase.view.as_mut().unwrap().starting = true;
    up(&mut st, Msg::Rebase(RebaseMsg::RebaseFinished { result: Err(rebase::CHANGED.into()), undo: None }));
    let p = panel(&st);
    assert!(!p.starting && p.stale);
    assert_eq!(p.error.as_deref(), Some(rebase::CHANGED));
    assert!(st.job.is_none());
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text == format!("Interactive rebase failed: {}", rebase::CHANGED)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_panel_is_parked_with_its_tab() {
    let (mut st, a, _) = with_panel("rebase-tabs-a", RebaseBase::FromCommit(Oid::zero()));
    let b = super::tests::scratch_repo("rebase-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert!(st.session.rebase.view.is_none(), "B has no panel");
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(panel(&st).items.len(), 3, "A's panel comes back");

    // A list still loading is requested again on return.
    st.session.rebase.view.as_mut().unwrap().reload();
    up(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    let fx = up(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let gen_now = st.session.rebase.generation;
    assert!(fx.iter().any(|e| matches!(e, Effect::Rebase(RebaseEffect::LoadPlan { generation, .. }) if *generation == gen_now)), "{fx:?}");

    // A result for A while B is active closes A's parked panel and leaves B's alone.
    st.job = Some(running_job(&a));
    st.session.rebase.view.as_mut().unwrap().starting = true;
    up(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    let mut other = RebasePanel::loading(RebaseBase::FromCommit(fake_oid(1)));
    other.fill(fake_plan(2));
    st.session.rebase.view = Some(other);
    up(
        &mut st,
        Msg::Rebase(RebaseMsg::RebaseFinished { result: Ok(Outcome::Finished { output: String::new(), stash_kept: false }), undo: None }),
    );
    assert_eq!(panel(&st).items.len(), 2, "the active tab's panel is untouched");
    let parked = st.tabs[0].parked.as_ref().expect("A is parked");
    assert!(parked.session.rebase.view.is_none(), "A's panel closed");
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Interactive rebase succeeded"));
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}
