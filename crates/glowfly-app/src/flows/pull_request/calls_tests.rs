//! Headless tests for the pull request panel's service calls: loading the page, comments,
//! reviews and merging, with replies fed by hand (the calls themselves never run here).

use super::tests::{cleanup, github, pr_target, rev, setup};
use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::flows::finish_keeping;
use crate::msg::{ForgeMsg, Msg, RepoMsg, TabsMsg};
use crate::pull_request::{CallJob, PrCall, PrEffect, PrReply, PullRequestMsg};
use crate::state::AppState;
use crate::state::pull_request::{Page, PrAction, PrTab};
use crate::update::update;
use glowfly_core::forge::{
    Auth, Entry, EntryKind, MergeMethod, MergeState, Merged, PrDetail, PrPage, PrState, Provider, ReviewEvent, ReviewStyle, Target,
};
use glowfly_core::settings::RepoTrust;
use std::path::PathBuf;
use std::time::Instant;

pub(super) fn run(st: &mut AppState, msg: PullRequestMsg) -> Vec<Effect> {
    let fx = update(st, Msg::PullRequest(msg), Instant::now());
    finish_keeping(st, fx)
}

/// The service calls among `fx`, but for the checks (checks_tests.rs counts those).
pub(super) fn calls(fx: &[Effect]) -> Vec<&CallJob> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::PullRequest(PrEffect::Call(j)) if !matches!(j.call, PrCall::Checks { .. }) => Some(&**j),
            _ => None,
        })
        .collect()
}

pub(super) fn loads(fx: &[Effect]) -> usize {
    calls(fx).iter().filter(|j| matches!(j.call, PrCall::Load)).count()
}

pub(super) fn refreshes(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Forge { call: ForgeCall::Refresh { .. }, .. })).count()
}

pub(super) fn git_jobs(fx: &[Effect]) -> Vec<Vec<Vec<String>>> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::RunGit { cmds, .. } => Some(cmds.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn detail(head: &str) -> PrDetail {
    PrDetail {
        state: PrState::Open,
        draft: false,
        locked: false,
        mergeable: Some(true),
        merge_state: MergeState::Clean,
        head_sha: head.into(),
        base_sha: String::new(),
        same_repo: true,
        author: "ada".into(),
        me: "grace".into(),
        can_merge: Some(true),
        methods: vec![MergeMethod::Squash, MergeMethod::Rebase],
        deletes_merged_branches: false,
        pending_review: None,
        style: ReviewStyle::Verdicts,
    }
}

pub(super) fn reply_msg(st: &AppState, reply: PrReply) -> PullRequestMsg {
    let workdir = st.workdir().unwrap();
    PullRequestMsg::Replied { workdir, target: github(), id: 7, generation: st.session.pull_request.page_gen, reply: Box::new(reply) }
}

/// The opened panel with a page loaded: pull request 7, open, as `f` changes it.
pub(super) fn loaded(st: &mut AppState, f: impl FnOnce(&mut PrDetail)) {
    let head = st.session.forge.as_ref().unwrap().prs[0].head.clone().unwrap_or_default();
    let mut d = detail(&head);
    f(&mut d);
    let page = PrPage { detail: d, entries: Vec::new(), threads: Vec::new(), threads_note: None };
    let msg = reply_msg(st, PrReply::Loaded(Ok(page)));
    run(st, msg);
}

/// `setup`, opened, returning the effects the opening left.
pub(super) fn opened(name: &str) -> (AppState, Vec<Effect>, [PathBuf; 2]) {
    let (mut st, work, bare, _) = setup(name, &[]);
    let fx = run(&mut st, PullRequestMsg::Open(7));
    (st, fx, [work, bare])
}

pub(super) fn err(message: &str) -> ForgeError {
    ForgeError { message: message.into(), auth: false }
}

pub(super) fn merged_ok(delete: bool) -> PrReply {
    let result = Ok(Merged { sha: "m1".into() });
    PrReply::Merged { method: MergeMethod::Squash, remote: "origin".into(), source: "feature".into(), delete, delete_local: false, result }
}

pub(super) fn view(st: &AppState) -> &crate::state::pull_request::PrView {
    st.session.pull_request.view.as_ref().expect("the panel is open")
}

pub(super) fn done(st: AppState, dirs: [PathBuf; 2]) {
    drop(st);
    cleanup(&[&dirs[0], &dirs[1]]);
}

#[test]
fn open_github_pr_requests_the_page() {
    let (mut st, work, bare, _) = setup("prc-open", &[]);
    let auth = Auth::from_token(Provider::GitHub, None, "tok", "test");
    st.session.forge.as_mut().unwrap().auth.insert(github().account().key(), auth);
    let fx = run(&mut st, PullRequestMsg::Open(7));
    let jobs = calls(&fx);
    assert_eq!(jobs.len(), 1);
    let job = jobs[0];
    assert!(matches!(job.call, PrCall::Load));
    assert_eq!((job.id, job.generation, &job.req.target), (7, st.session.pull_request.page_gen, &github()));
    assert!(job.req.auth.is_some(), "the session's credential goes along");
    assert_eq!(job.req.workdir, st.workdir().unwrap());
    assert_eq!(view(&st).page, Page::Loading);
    done(st, [work, bare]);
}

#[test]
fn open_on_azure_devops_sends_no_call() {
    let (mut st, work, bare, _) = setup("prc-azure", &[]);
    let azure = Target { provider: Provider::AzureDevOps, host: "dev.azure.com".into(), project: "web".into(), ..github() };
    st.session.forge.as_mut().unwrap().host = Some(azure);
    let fx = run(&mut st, PullRequestMsg::Open(7));
    assert!(calls(&fx).is_empty());
    assert_eq!(view(&st).page, Page::Unsupported);
    assert!(run(&mut st, PullRequestMsg::Reload).is_empty());
    done(st, [work, bare]);
}

#[test]
fn untrusted_repository_holds_the_load() {
    let (mut st, work, bare, _) = setup("prc-held", &[]);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    let fx = run(&mut st, PullRequestMsg::Open(7));
    assert!(calls(&fx).is_empty());
    assert_eq!(view(&st).page, Page::Held);
    done(st, [work, bare]);
}

#[test]
fn stale_generation_and_other_workdir_and_closed_view_ignore_loaded() {
    let (mut st, _, dirs) = opened("prc-stale");
    let page = PrPage { detail: detail("h"), entries: Vec::new(), threads: Vec::new(), threads_note: None };
    let fresh = st.session.pull_request.page_gen;
    let mut stale = reply_msg(&st, PrReply::Loaded(Ok(page.clone())));
    if let PullRequestMsg::Replied { generation, .. } = &mut stale {
        *generation = fresh - 1;
    }
    run(&mut st, stale);
    assert_eq!(view(&st).page, Page::Loading, "an older request");
    let mut elsewhere = reply_msg(&st, PrReply::Loaded(Ok(page.clone())));
    if let PullRequestMsg::Replied { workdir, .. } = &mut elsewhere {
        *workdir = PathBuf::from("C:/elsewhere");
    }
    run(&mut st, elsewhere);
    assert_eq!(view(&st).page, Page::Loading, "another repository");
    let mut other = reply_msg(&st, PrReply::Loaded(Ok(page.clone())));
    if let PullRequestMsg::Replied { id, .. } = &mut other {
        *id = 8;
    }
    run(&mut st, other);
    assert_eq!(view(&st).page, Page::Loading, "another pull request");
    let late = reply_msg(&st, PrReply::Loaded(Ok(page)));
    run(&mut st, PullRequestMsg::Close);
    run(&mut st, late);
    assert!(st.session.pull_request.view.is_none(), "a closed panel stays closed");
    assert!(st.log.is_empty());
    done(st, dirs);
}

#[test]
fn loaded_page_prefills_method() {
    let (mut st, _, dirs) = opened("prc-prefill");
    loaded(&mut st, |_| {});
    let v = view(&st);
    assert!(matches!(&v.page, Page::Loaded(p) if p.detail.state == PrState::Open));
    assert_eq!(v.forms.merge_method, Some(MergeMethod::Squash), "the first allowed method");
    // A failed reload keeps the page and only logs.
    let fx = run(&mut st, PullRequestMsg::Reload);
    assert_eq!(loads(&fx), 1);
    assert!(view(&st).reloading);
    let msg = reply_msg(&st, PrReply::Loaded(Err(err("timed out"))));
    run(&mut st, msg);
    assert!(view(&st).loaded().is_some() && !view(&st).reloading);
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("timed out")));
    done(st, dirs);
}

#[test]
fn post_comment_blocked_when_empty_or_busy() {
    let (mut st, _, dirs) = opened("prc-comment-blocked");
    st.session.pull_request.view.as_mut().unwrap().forms.comment = "Thanks".into();
    assert!(calls(&run(&mut st, PullRequestMsg::PostComment)).is_empty(), "not loaded yet");
    loaded(&mut st, |_| {});
    st.session.pull_request.view.as_mut().unwrap().forms.comment = "  ".into();
    assert!(calls(&run(&mut st, PullRequestMsg::PostComment)).is_empty(), "empty");
    st.session.pull_request.view.as_mut().unwrap().forms.comment = "Thanks".into();
    let fx = run(&mut st, PullRequestMsg::PostComment);
    assert!(matches!(&calls(&fx)[..], [j] if matches!(&j.call, PrCall::Comment(t) if t == "Thanks")));
    assert_eq!(view(&st).busy, Some(PrAction::Comment));
    assert!(calls(&run(&mut st, PullRequestMsg::PostComment)).is_empty(), "busy");
    done(st, dirs);
}

#[test]
fn commented_ok_appends_entry_and_clears_text() {
    let (mut st, _, dirs) = opened("prc-commented");
    loaded(&mut st, |_| {});
    st.session.pull_request.view.as_mut().unwrap().forms.comment = "Thanks".into();
    run(&mut st, PullRequestMsg::PostComment);
    let entry = Entry { author: "grace".into(), time: 5, kind: EntryKind::Comment, body: "Thanks".into(), url: String::new() };
    let msg = reply_msg(&st, PrReply::Commented(Ok(entry.clone())));
    run(&mut st, msg);
    let v = view(&st);
    assert_eq!((v.busy.clone(), v.forms.comment.as_str()), (None, ""));
    assert_eq!(v.loaded().unwrap().entries, [entry]);
    assert!(st.toasts.iter().any(|t| t.text == "Comment posted"));
    done(st, dirs);
}

#[test]
fn commented_err_keeps_text_sets_error_and_logs() {
    let (mut st, _, dirs) = opened("prc-comment-err");
    loaded(&mut st, |_| {});
    st.session.pull_request.view.as_mut().unwrap().forms.comment = "Thanks".into();
    run(&mut st, PullRequestMsg::PostComment);
    let msg = reply_msg(&st, PrReply::Commented(Err(err("HTTP 422: locked"))));
    run(&mut st, msg);
    let v = view(&st);
    assert_eq!(v.forms.comment, "Thanks");
    assert_eq!(v.action_error, Some((PrAction::Comment, "HTTP 422: locked".into())));
    assert_eq!(v.busy, None);
    let last = st.log.last().unwrap();
    assert_eq!((last.ok, last.text.as_str()), (false, "Commenting on #7 failed: HTTP 422: locked"));
    done(st, dirs);
}

#[test]
fn author_cannot_approve() {
    let (mut st, _, dirs) = opened("prc-author");
    loaded(&mut st, |d| d.me = "ada".into());
    let v = st.session.pull_request.view.as_mut().unwrap();
    assert_eq!(v.forms.review_event, ReviewEvent::Comment);
    v.forms.review_event = ReviewEvent::Approve;
    assert!(calls(&run(&mut st, PullRequestMsg::SubmitReview)).is_empty());
    assert_eq!(view(&st).busy, None);
    done(st, dirs);
}

#[test]
fn review_needs_summary_for_request_changes() {
    let (mut st, _, dirs) = opened("prc-summary");
    loaded(&mut st, |_| {});
    st.session.pull_request.view.as_mut().unwrap().forms.review_event = ReviewEvent::RequestChanges;
    assert!(calls(&run(&mut st, PullRequestMsg::SubmitReview)).is_empty());
    st.session.pull_request.view.as_mut().unwrap().forms.review_body = "Rename it".into();
    let fx = run(&mut st, PullRequestMsg::SubmitReview);
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    assert!(matches!(&job.call, PrCall::Review { event: ReviewEvent::RequestChanges, body, pending: None, .. } if body == "Rename it"));
    done(st, dirs);
}

#[test]
fn review_with_pending_review_sends_its_id() {
    let (mut st, _, dirs) = opened("prc-pending");
    loaded(&mut st, |d| d.pending_review = Some(99));
    let fx = run(&mut st, PullRequestMsg::SubmitReview);
    let jobs = calls(&fx);
    assert!(matches!(&jobs[..], [j] if matches!(&j.call, PrCall::Review { event: ReviewEvent::Comment, pending: Some(99), .. })));
    done(st, dirs);
}

#[test]
fn reviewed_ok_reloads_page_and_refreshes_forge() {
    let (mut st, _, dirs) = opened("prc-reviewed");
    loaded(&mut st, |_| {});
    let v = st.session.pull_request.view.as_mut().unwrap();
    v.forms.review_event = ReviewEvent::Approve;
    v.forms.review_body = "Nice".into();
    run(&mut st, PullRequestMsg::SubmitReview);
    let msg = reply_msg(&st, PrReply::Reviewed(ReviewEvent::Approve, Ok(())));
    let fx = run(&mut st, msg);
    assert_eq!(loads(&fx), 1);
    assert_eq!(refreshes(&fx), 1);
    assert_eq!(view(&st).forms.review_body, "");
    assert!(view(&st).reloading);
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Approved #7"));
    done(st, dirs);
}

#[test]
fn auth_error_drops_cached_auth() {
    let (mut st, _, dirs) = opened("prc-auth");
    let key = github().account().key();
    st.session.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    let msg = reply_msg(&st, PrReply::Loaded(Err(ForgeError { message: "authentication failed: 401".into(), auth: true })));
    run(&mut st, msg);
    assert!(!st.session.forge.as_ref().unwrap().auth.contains_key(&key));
    assert_eq!(view(&st).page, Page::Failed("authentication failed: 401".into()));
    done(st, dirs);
}

#[test]
fn tab_activation_reloads_page() {
    let (mut st, _, dirs) = opened("prc-tabs");
    loaded(&mut st, |_| {});
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(loads(&fx), 1);
    assert!(view(&st).reloading, "the last page stays on screen");
    done(st, dirs);
}

#[test]
fn prs_changed_with_new_head_reloads_page() {
    let (mut st, work, bare, _) = setup("prc-head", &[]);
    run(&mut st, PullRequestMsg::Open(7));
    loaded(&mut st, |_| {});
    let mut listed = st.session.forge.as_ref().unwrap().prs[0].clone();
    let reply = |prs| ForgeReply { target: github(), auth: None, results: vec![ForgeResult::Prs(Ok(prs))] };
    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply(vec![listed.clone()]))), Instant::now());
    assert_eq!(loads(&fx), 0, "the same head");
    listed.head = Some(rev(&work, "main").to_string());
    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply(vec![listed]))), Instant::now());
    assert_eq!(loads(&fx), 1, "a new head");
    done(st, [work, bare]);
}

#[test]
fn tab_msg_and_opening_a_pr_file_switch_tabs() {
    let (mut st, _, dirs) = opened("prc-tab-msg");
    assert_eq!(view(&st).tab, PrTab::Overview);
    run(&mut st, PullRequestMsg::Tab(PrTab::Files));
    assert_eq!(view(&st).tab, PrTab::Files);
    run(&mut st, PullRequestMsg::Tab(PrTab::Overview));
    let f = pr_target(&st, "f.txt");
    let fx = update(&mut st, Msg::Repo(RepoMsg::OpenDiff(f)), Instant::now());
    finish_keeping(&mut st, fx);
    assert_eq!(view(&st).tab, PrTab::Files);
    done(st, dirs);
}
