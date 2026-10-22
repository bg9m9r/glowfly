//! Creating a stack's pull requests end to end: the push to the code host's remote first, one
//! create dialog per layer bottom first, Skip, Cancel, Esc, a failed create, layers that have
//! one already, a push that didn't land, a parked tab and the refusals. `origin` reads as
//! GitHub while pushes go to a bare repository.

use super::push_pr_tests::{cleanup, with_host};
use super::stack_push_tests::{commit, refresh};
use super::tests::{drive, git};
use super::update;
use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{DialogMsg, Dismiss, ForgeMsg, Msg, StackMsg, TabsMsg};
use crate::state::{AppState, Modal};
use glowfly_core::forge::{NewPullRequest, PullRequest};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// `with_host` plus main <- low <- mid, not pushed, low checked out and the work tree clean.
fn stacked(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, dir, bare) = with_host(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "-b", "low"]);
    commit(&dir, "l1");
    git(&dir, &["checkout", "-q", "-b", "mid"]);
    commit(&dir, "m1");
    git(&dir, &["checkout", "-q", "low"]);
    refresh(&mut st);
    assert!(st.snap.as_ref().unwrap().stacks.below("mid").is_some());
    (st, dir, bare)
}

/// `stacked` with both layers already on origin.
fn pushed(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, dir, bare) = stacked(name);
    drive(&mut st, Msg::Stack(StackMsg::Push { branch: "low".into() }));
    st.log.clear();
    (st, dir, bare)
}

fn create_prs() -> Msg {
    Msg::Stack(StackMsg::CreatePrs { branch: "mid".into() })
}

/// The open create dialog's (source, target).
fn dialog(st: &AppState) -> Option<(String, String)> {
    match &st.modal {
        Some(Modal::CreatePr { source, target, .. }) => Some((source.clone(), target.clone())),
        _ => None,
    }
}

fn running(st: &AppState) -> bool {
    st.forge.as_ref().is_some_and(|f| f.stack_prs.is_some())
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn pr(id: u64, source: &str, target: &str) -> PullRequest {
    PullRequest {
        id,
        title: source.into(),
        description: String::new(),
        source: source.into(),
        target: target.into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: format!("https://github.com/o/r/pull/{id}"),
        facets: Default::default(),
    }
}

/// What the dialog's Create sends: the dialog closes, then the request goes out.
fn create(st: &mut AppState) -> Vec<Effect> {
    let (source, target) = dialog(st).expect("a create dialog");
    drive(st, Msg::CloseModal);
    let new = NewPullRequest {
        source,
        target,
        title: "t".into(),
        description: String::new(),
        draft: false,
        issues: Vec::new(),
        source_repo: None,
    };
    update(st, Msg::Forge(ForgeMsg::CreatePr { pr: new, from: String::new(), into: String::new() }), Instant::now())
}

fn reply(st: &mut AppState, result: ForgeResult) {
    let target = st.forge.as_ref().unwrap().host.clone().unwrap();
    drive(st, Msg::Forge(ForgeMsg::ForgeReplied(ForgeReply { target, auth: None, results: vec![result] })));
}

fn done(dir: &Path, bare: &Path) {
    cleanup(dir, bare);
}

#[test]
fn the_stack_is_pushed_then_each_layer_gets_its_dialog_in_turn() {
    let (mut st, dir, bare) = stacked("stack-prs-run");
    drive(&mut st, create_prs());
    assert!(logged(&st, "low: pushed as a new branch, tracking origin/low"), "{:?}", st.log);
    assert_eq!(dialog(&st), Some(("low".into(), "main".into())));
    assert_eq!(st.forge.as_ref().unwrap().stack_step("low"), Some((1, 2)));
    let fx = create(&mut st);
    assert!(fx.iter().any(|e| matches!(e, Effect::Forge { call: ForgeCall::Create(_), .. })), "{fx:?}");
    assert!(running(&st), "waiting for the service");
    reply(&mut st, ForgeResult::Created(Ok(Box::new(pr(1, "low", "main")))));
    assert_eq!(dialog(&st), Some(("mid".into(), "low".into())), "mid targets the branch below it");
    assert_eq!(st.forge.as_ref().unwrap().stack_step("mid"), Some((2, 2)));
    create(&mut st);
    reply(&mut st, ForgeResult::Created(Ok(Box::new(pr(2, "mid", "low")))));
    assert!(!running(&st) && st.modal.is_none());
    assert!(logged(&st, "Stack done. Created: low, mid. Skipped: none."), "{:?}", st.log);
    done(&dir, &bare);
}

#[test]
fn layers_with_an_open_pr_are_skipped_and_a_pushed_stack_starts_at_once() {
    let (mut st, dir, bare) = pushed("stack-prs-open");
    st.forge.as_mut().unwrap().prs = vec![pr(4, "low", "main")];
    let fx = update(&mut st, create_prs(), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::Stack(_))), "nothing to push: {fx:?}");
    assert!(st.job.is_none());
    assert_eq!(dialog(&st), Some(("mid".into(), "low".into())));
    assert!(logged(&st, "low already has pull request #4; skipped."), "{:?}", st.log);
    assert_eq!(st.forge.as_ref().unwrap().stack_step("mid"), Some((2, 2)));
    done(&dir, &bare);
}

#[test]
fn skip_moves_on_and_cancel_stops() {
    let (mut st, dir, bare) = pushed("stack-prs-skip");
    drive(&mut st, create_prs());
    assert_eq!(dialog(&st).map(|d| d.0), Some("low".into()));
    drive(&mut st, Msg::CloseModal);
    drive(&mut st, Msg::Stack(StackMsg::SkipPr));
    assert_eq!(dialog(&st).map(|d| d.0), Some("mid".into()));
    drive(&mut st, Msg::CloseModal);
    drive(&mut st, Msg::Stack(StackMsg::SkipPr));
    assert!(!running(&st) && st.modal.is_none());
    assert!(logged(&st, "Stack done. Created: none. Skipped: low, mid."), "{:?}", st.log);

    drive(&mut st, create_prs());
    drive(&mut st, Msg::CloseModal);
    drive(&mut st, Msg::Stack(StackMsg::StopPrs));
    assert!(!running(&st));
    assert!(logged(&st, "Stopped the stack's create dialogs. Not created: low, mid."), "{:?}", st.log);
    done(&dir, &bare);
}

#[test]
fn escape_stops_an_untouched_dialog_but_create_does_not() {
    let (mut st, dir, bare) = pushed("stack-prs-escape");
    drive(&mut st, create_prs());
    drive(&mut st, Msg::Dialog(DialogMsg::Dismiss(Dismiss::Escape)));
    assert!(!running(&st) && st.modal.is_none());
    assert!(logged(&st, "Not created: low, mid."), "{:?}", st.log);

    st.log.clear();
    drive(&mut st, create_prs());
    create(&mut st);
    assert!(running(&st), "Create closes the dialog without stopping the run");
    reply(&mut st, ForgeResult::Created(Err(ForgeError { message: "Validation failed".into(), auth: false })));
    assert!(!running(&st));
    assert!(logged(&st, "Creating the pull request failed: Validation failed"));
    assert!(logged(&st, "Not created: low, mid."), "{:?}", st.log);
    done(&dir, &bare);
}

#[test]
fn a_push_that_did_not_land_opens_nothing() {
    let (mut st, dir, bare) = pushed("stack-prs-behind");
    super::branch_tests::remote_commit(&bare, "mid", "theirs.txt");
    super::push_pr_tests::fetch(&dir, &bare);
    refresh(&mut st);
    drive(&mut st, create_prs());
    assert!(logged(&st, "mid: not pushed; origin/mid has commits it lacks. Pull first."), "{:?}", st.log);
    assert!(st.modal.is_none(), "{:?}", dialog(&st));
    assert!(logged(&st, "Not every branch of the stack reached origin, so the create dialogs weren't opened."), "{:?}", st.log);
    assert!(!running(&st));
    done(&dir, &bare);
}

#[test]
fn a_parked_tab_drops_its_run() {
    let (mut st, dir, bare) = pushed("stack-prs-parked");
    drive(&mut st, create_prs());
    assert!(running(&st));
    drive(&mut st, Msg::CloseModal);
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    let log = &st.tabs[0].log;
    assert!(log.iter().any(|l| l.text == "The stack's create dialogs stopped: its tab is no longer active."), "{log:?}");
    assert!(log.iter().any(|l| l.text.contains("Not created: low, mid.")), "{log:?}");
    let parked = st.tabs[0].parked.as_ref().unwrap();
    assert!(parked.forge.as_ref().is_some_and(|f| f.stack_prs.is_none()));
    done(&dir, &bare);
}

#[test]
fn refusals_start_nothing() {
    let (mut st, dir, bare) = stacked("stack-prs-refusals");
    let refused = |st: &mut AppState, want: &str| {
        let fx = update(st, create_prs(), Instant::now());
        assert!(!fx.iter().any(|e| matches!(e, Effect::Stack(_) | Effect::Forge { .. })), "{fx:?}");
        assert!(st.modal.is_none());
        assert_eq!(st.log.last().map(|l| l.text.clone()).as_deref(), Some(want));
    };
    st.forge.as_mut().unwrap().prs = vec![pr(1, "low", "main"), pr(2, "mid", "low")];
    refused(&mut st, "Every branch in the stack already has an open pull request.");
    st.forge.as_mut().unwrap().prs.clear();
    st.snap.as_mut().unwrap().stacks.links[0].behind = true;
    refused(&mut st, "mid needs a restack onto low first.");
    st.forge = None;
    refused(&mut st, "origin isn't on a code host such as GitHub or GitLab, so it can't take pull requests.");
    done(&dir, &bare);
}
