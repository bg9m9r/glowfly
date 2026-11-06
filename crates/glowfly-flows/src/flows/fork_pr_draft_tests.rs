//! Headless tests for the create dialog's draft across Push to your fork…: what was typed rides
//! through the Fork dialog and the push, and comes back when the dialog reopens after the push,
//! a cancel or a failure. The service is answered by hand.

use super::fork_pr_tests::{creator, fork, git_job, origin, run};
use super::push_pr_fork_tests::with_fork;
use super::push_pr_tests::{cleanup, with_host};
use super::tests::{drive, git};
use super::update;
use glowfly_core::forge::{CreatedFork, Fork, ForkLookup};
use glowfly_core::gitcli::args;
use glowfly_model::effect::{Effect, ForgeError};
use glowfly_model::msg::{DialogMsg, Dismiss, ForgeMsg, JobMsg, Msg, RepoMsg};
use glowfly_model::remote::RemoteMsg;
use glowfly_model::state::forks::{ForkStep, ForkThenPr};
use glowfly_model::state::push_pr::{PrDraft, PrStart};
use glowfly_model::state::{AppState, Modal, RunningJob};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn typed_draft() -> PrDraft {
    PrDraft {
        branch: "topic".into(),
        from: "origin".into(),
        title: "Typed title".into(),
        description: "Why it matters".into(),
        items: "12".into(),
        draft: true,
    }
}

/// Opens the create dialog for `topic`, types into it, then Push to your fork… (the Fork
/// dialog's lookup isn't answered).
fn typed(st: &mut AppState) {
    drive(st, Msg::Forge(ForgeMsg::CreatePrDialog { source: "topic".into(), target: None }));
    let Some(Modal::CreatePr { title, description, items, draft, .. }) = st.modal.as_mut() else { panic!("the create dialog") };
    (*title, *description, *items, *draft) = ("Typed title".into(), "Why it matters".into(), "12".into(), true);
    update(st, Msg::Forge(ForgeMsg::PushToMyFork { branch: "topic".into(), target: Some("main".into()) }), Instant::now());
}

/// The lookup's answer for the open Fork dialog.
fn found(st: &mut AppState, mine: Option<Fork>) {
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    run(
        st,
        RemoteMsg::MyForkFound {
            workdir,
            target: origin(),
            result: Box::new(Ok(ForkLookup { user: "ada".into(), mine, own_repository: false })),
        },
    );
}

/// The fork request's answer for the open Fork dialog.
fn created(st: &mut AppState, result: Result<CreatedFork, ForgeError>) -> Vec<Effect> {
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    run(st, RemoteMsg::ForkCreated { workdir, target: origin(), name: "ada".into(), result: Box::new(result) })
}

/// Whether the create dialog is open with the typed draft, target `main` and From `from`.
fn restored(st: &AppState, from: &str) -> bool {
    matches!(&st.modal, Some(Modal::CreatePr { target, title, description, items, draft, repos, .. })
        if title == "Typed title" && description == "Why it matters" && items == "12" && *draft
            && target == "main" && repos.from == from)
}

fn then_form(st: &AppState) -> Option<PrDraft> {
    creator(st).then_pr.as_ref().and_then(|t| t.form.clone())
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn escape(st: &mut AppState) {
    update(st, Msg::Dialog(DialogMsg::Dismiss(Dismiss::Escape)), Instant::now());
}

fn remove(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn push_to_your_fork_keeps_what_was_typed() {
    let (mut st, work, bare) = with_host("fork-draft-keep");
    typed(&mut st);
    let then = ForkThenPr { branch: "topic".into(), target: Some("main".into()), form: Some(typed_draft()) };
    assert_eq!(creator(&st).then_pr, Some(then));
    cleanup(&work, &bare);
}

#[test]
fn cancelling_the_fork_dialog_brings_the_draft_back() {
    let (mut st, work, bare) = with_host("fork-draft-cancel");
    typed(&mut st);
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(restored(&st, "origin"), "{:?}", st.modal.as_ref().map(|m| m.draft()));
    // The restored text counts as edited: Esc asks first.
    escape(&mut st);
    assert!(st.dialog.asking);
    assert!(restored(&st, "origin"));

    // Esc on the Fork dialog while it checks, and once it found the user's fork.
    update(&mut st, Msg::CloseModal, Instant::now());
    typed(&mut st);
    escape(&mut st);
    assert!(restored(&st, "origin"));
    update(&mut st, Msg::CloseModal, Instant::now());
    typed(&mut st);
    found(&mut st, Some(fork("ada")));
    assert!(matches!(creator(&st).step, ForkStep::Mine { .. }));
    escape(&mut st);
    assert!(restored(&st, "origin"));
    cleanup(&work, &bare);
}

#[test]
fn cancelling_without_a_create_dialog_only_closes() {
    let (mut st, work, bare) = with_host("fork-draft-none");
    update(&mut st, Msg::Forge(ForgeMsg::PushToMyFork { branch: "topic".into(), target: None }), Instant::now());
    assert!(creator(&st).then_pr.as_ref().is_some_and(|t| t.form.is_none()));
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(st.modal.is_none());
    cleanup(&work, &bare);
}

#[test]
fn the_draft_comes_back_once_the_push_to_the_fork_lands() {
    let (mut st, work, bare, ada) = with_fork("fork-draft-pushed");
    typed(&mut st);
    found(&mut st, Some(fork("ada")));
    drive(&mut st, Msg::Remote(RemoteMsg::AddMyFork));
    let (ok, out) = glowfly_core::gitcli::run(&ada, &args(&["rev-parse", "--verify", "refs/heads/topic"]));
    assert!(ok, "topic was pushed to the fork: {out}");
    assert!(restored(&st, "ada"), "{:?}", st.modal.as_ref().map(|m| m.draft()));
    assert!(st.session.forge.as_ref().is_some_and(|f| f.open_after_load.is_none()));
    cleanup(&work, &bare);
    remove(&[&ada]);
}

#[test]
fn a_failed_push_to_the_fork_goes_back_to_the_draft() {
    let (mut st, work, bare, ada) = with_fork("fork-draft-failed");
    let missing = glowfly_core::paths::temp_dir().join(format!("glowfly-fork-draft-missing-{}.git", std::process::id()));
    git(&work, &["config", "remote.ada.pushurl", &missing.display().to_string()]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    typed(&mut st);
    found(&mut st, Some(fork("ada")));
    drive(&mut st, Msg::Remote(RemoteMsg::AddMyFork));
    assert!(st.job.is_none());
    assert!(st.log.iter().any(|l| !l.ok), "the failed push is logged");
    assert!(restored(&st, "origin"), "{:?}", st.modal.as_ref().map(|m| m.draft()));
    cleanup(&work, &bare);
    remove(&[&ada]);
}

#[test]
fn a_failed_fork_request_keeps_the_draft_for_cancel() {
    let (mut st, work, bare) = with_host("fork-draft-create-failed");
    typed(&mut st);
    found(&mut st, None);
    run(&mut st, RemoteMsg::CreateFork);
    created(&mut st, Err(ForgeError { message: "boom".into(), auth: false }));
    let f = creator(&st);
    assert!(matches!(f.step, ForkStep::Ready { .. }) && f.error.is_some());
    assert_eq!(then_form(&st), Some(typed_draft()));
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(restored(&st, "origin"));
    cleanup(&work, &bare);
}

#[test]
fn a_fork_not_ready_yet_adds_the_remote_and_goes_back_to_the_draft() {
    let (mut st, work, bare) = with_host("fork-draft-not-ready");
    typed(&mut st);
    found(&mut st, None);
    run(&mut st, RemoteMsg::CreateFork);
    let fx = created(&mut st, Ok(CreatedFork { fork: fork("ada"), ready: false }));
    assert_eq!(git_job(&fx), [args(&["remote", "add", "--", "ada", "https://github.com/ada/r.git"])]);
    assert_eq!(st.job.as_ref().and_then(|j| j.then_pr.clone()), None);
    assert!(logged(&st, "still preparing the fork"));
    assert!(restored(&st, "origin"));
    cleanup(&work, &bare);
}

#[test]
fn a_taken_remote_name_goes_back_to_the_draft() {
    let (mut st, work, bare) = with_host("fork-draft-taken");
    typed(&mut st);
    found(&mut st, None);
    run(&mut st, RemoteMsg::CreateFork);
    git(&work, &["remote", "add", "ada", "https://github.com/someone/r.git"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = created(&mut st, Ok(CreatedFork { fork: fork("ada"), ready: true }));
    assert!(logged(&st, "is taken now"));
    assert!(st.job.is_none() && !fx.iter().any(|e| matches!(e, Effect::Job(_))));
    assert!(restored(&st, "origin"));
    cleanup(&work, &bare);
}

fn running(workdir: PathBuf, then_pr: Option<PrStart>) -> RunningJob {
    RunningJob {
        label: "Push".into(),
        workdir: Some(workdir),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_continue: false,
        then_pr,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    }
}

#[test]
fn a_refused_push_keeps_the_fork_dialog_for_cancel() {
    let (mut st, work, bare, ada) = with_fork("fork-draft-busy");
    typed(&mut st);
    found(&mut st, Some(fork("ada")));
    st.job = Some(running(work.clone(), None));
    let fx = run(&mut st, RemoteMsg::AddMyFork);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Job(_))), "{fx:?}");
    assert_eq!(then_form(&st), Some(typed_draft()));
    st.job = None;
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(restored(&st, "origin"));
    cleanup(&work, &bare);
    remove(&[&ada]);
}

#[test]
fn a_ready_fork_with_a_refused_push_goes_back_to_the_draft() {
    let (mut st, work, bare) = with_host("fork-draft-ready-busy");
    typed(&mut st);
    found(&mut st, None);
    run(&mut st, RemoteMsg::CreateFork);
    // Another job (an auto-fetch, say) started while the fork request was out.
    st.job = Some(running(work.clone(), None));
    let fx = created(&mut st, Ok(CreatedFork { fork: fork("ada"), ready: true }));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Job(_))), "{fx:?}");
    assert!(logged(&st, "Another operation is still running."));
    assert!(!logged(&st, "no longer exists"));
    assert!(restored(&st, "origin"), "{:?}", st.modal.as_ref().map(|m| m.draft()));
    cleanup(&work, &bare);
}

#[test]
fn a_failed_push_in_another_tab_never_reopens() {
    let (mut st, work, bare) = with_host("fork-draft-other-tab");
    let form = Some(typed_draft());
    let then = PrStart { source: "topic".into(), target: Some("main".into()), remote: Some("ada".into()), form };
    st.job = Some(running(bare.clone(), Some(then)));
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output: String::new(), undo: None }), Instant::now());
    assert!(st.modal.is_none());
    cleanup(&work, &bare);
}
