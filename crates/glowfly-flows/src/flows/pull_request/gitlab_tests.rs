//! Headless tests for a GitLab merge request in the pull request panel: loading its page,
//! approving and taking an approval back, and merging by fast-forward.

use super::calls_tests::{calls, detail, git_jobs, loads, refreshes, run, view};
use super::tests::{cleanup, github, setup};
use crate::msg::Msg;
use crate::pull_request::{PrCall, PrReply, PullRequestMsg};
use crate::state::AppState;
use crate::update::update;
use glowfly_core::forge::{MergeMethod, Merged, PrDetail, PrPage, Provider, ReviewEvent, ReviewStyle, Target};
use std::path::PathBuf;
use std::time::Instant;

fn gitlab() -> Target {
    Target { provider: Provider::GitLab, host: "gitlab.com".into(), ..github() }
}

/// `setup` on a GitLab repository, with merge request 7's panel open.
fn opened(name: &str) -> (AppState, Vec<crate::effect::Effect>, [PathBuf; 2]) {
    let (mut st, work, bare, _) = setup(name, &[]);
    st.session.forge.as_mut().unwrap().host = Some(gitlab());
    let fx = run(&mut st, PullRequestMsg::Open(7));
    (st, fx, [work, bare])
}

fn reply_msg(st: &AppState, reply: PrReply) -> PullRequestMsg {
    let workdir = st.workdir().unwrap();
    PullRequestMsg::Replied { workdir, target: gitlab(), id: 7, generation: st.session.pull_request.page_gen, reply: Box::new(reply) }
}

/// The page loaded with GitLab's approvals style, as `f` changes it.
fn loaded(st: &mut AppState, f: impl FnOnce(&mut PrDetail)) {
    let mut d = detail("h7");
    d.style = ReviewStyle::Approvals { by_me: false, can_approve: Some(true) };
    d.methods = vec![MergeMethod::Merge, MergeMethod::Squash];
    f(&mut d);
    let page = PrPage { detail: d, entries: Vec::new(), threads: Vec::new(), threads_note: None };
    let msg = reply_msg(st, PrReply::Loaded(Ok(page)));
    run(st, msg);
}

fn done(st: AppState, dirs: [PathBuf; 2]) {
    drop(st);
    cleanup(&[&dirs[0], &dirs[1]]);
}

fn choose(st: &mut AppState, ev: ReviewEvent) {
    st.session.pull_request.view.as_mut().unwrap().forms.review_event = ev;
}

#[test]
fn opening_a_merge_request_loads_its_page() {
    let (st, fx, dirs) = opened("prg-open");
    let jobs = calls(&fx);
    assert_eq!(jobs.len(), 1);
    assert!(matches!(jobs[0].call, PrCall::Load));
    assert_eq!(jobs[0].req.target, gitlab());
    assert_eq!(view(&st).label, "!7");
    done(st, dirs);
}

#[test]
fn approve_sends_the_loaded_head() {
    let (mut st, _, dirs) = opened("prg-approve");
    loaded(&mut st, |_| {});
    choose(&mut st, ReviewEvent::Approve);
    let fx = run(&mut st, PullRequestMsg::SubmitReview);
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    assert!(
        matches!(&job.call, PrCall::Review { event: ReviewEvent::Approve, head_sha, pending: None, body } if head_sha == "h7" && body.is_empty()),
        "{:?}",
        job.call
    );
    done(st, dirs);
}

#[test]
fn approve_is_blocked_when_the_project_says_no() {
    let (mut st, _, dirs) = opened("prg-cannot");
    loaded(&mut st, |d| d.style = ReviewStyle::Approvals { by_me: false, can_approve: Some(false) });
    choose(&mut st, ReviewEvent::Approve);
    assert!(calls(&run(&mut st, PullRequestMsg::SubmitReview)).is_empty());
    assert_eq!(view(&st).busy, None);
    done(st, dirs);
}

#[test]
fn remove_approval_waits_for_an_approval() {
    let (mut st, _, dirs) = opened("prg-unapprove");
    loaded(&mut st, |_| {});
    choose(&mut st, ReviewEvent::Unapprove);
    assert!(calls(&run(&mut st, PullRequestMsg::SubmitReview)).is_empty(), "nothing to take back");
    loaded(&mut st, |d| d.style = ReviewStyle::Approvals { by_me: true, can_approve: Some(true) });
    choose(&mut st, ReviewEvent::Unapprove);
    st.session.pull_request.view.as_mut().unwrap().forms.review_body = "Not yet".into();
    let fx = run(&mut st, PullRequestMsg::SubmitReview);
    let jobs = calls(&fx);
    assert!(matches!(&jobs[..], [j] if matches!(&j.call, PrCall::Review { event: ReviewEvent::Unapprove, .. })), "{jobs:?}");
    let msg = reply_msg(&st, PrReply::Reviewed(ReviewEvent::Unapprove, Ok(())));
    let fx = run(&mut st, msg);
    assert_eq!((loads(&fx), refreshes(&fx)), (1, 1));
    assert_eq!(view(&st).forms.review_body, "");
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Removed your approval from !7"));
    assert!(st.toasts.iter().any(|t| t.text == "Removed your approval from !7"));
    done(st, dirs);
}

#[test]
fn fast_forward_merges_and_fetches() {
    let (mut st, _, dirs) = opened("prg-ff");
    loaded(&mut st, |d| d.methods = vec![MergeMethod::FastForward, MergeMethod::Squash]);
    assert_eq!(view(&st).forms.merge_method, Some(MergeMethod::FastForward), "the project's default is chosen");
    let fx = run(&mut st, PullRequestMsg::Merge);
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    let PrCall::Merge { req, .. } = &job.call else { panic!("{:?}", job.call) };
    assert_eq!((req.method, req.head_sha.as_str()), (MergeMethod::FastForward, "h7"));
    let result = Ok(Merged { sha: "h7".into() });
    let reply = PrReply::Merged {
        method: MergeMethod::FastForward,
        remote: "origin".into(),
        source: "feature".into(),
        delete: false,
        delete_local: false,
        result,
    };
    let msg = reply_msg(&st, reply);
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    let fetch = ["fetch", "--prune", "--", "origin"].map(String::from).to_vec();
    assert_eq!(git_jobs(&fx), [vec![fetch]]);
    assert!(st.log.iter().any(|l| l.ok && l.text == "Merged !7 into main (fast-forward)"));
    done(st, dirs);
}

#[test]
fn rebase_is_not_sent_to_gitlab() {
    let (mut st, _, dirs) = opened("prg-rebase");
    loaded(&mut st, |_| {});
    st.session.pull_request.view.as_mut().unwrap().forms.merge_method = Some(MergeMethod::Rebase);
    assert!(calls(&run(&mut st, PullRequestMsg::Merge)).is_empty());
    assert_eq!(view(&st).busy, None);
    done(st, dirs);
}
