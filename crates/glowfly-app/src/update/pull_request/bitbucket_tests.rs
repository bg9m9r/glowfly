//! Headless tests for a Bitbucket pull request in the panel: loading its page, requesting
//! changes without a summary and taking the request back, the author's own pull request, and
//! merging with the repository's strategy.

use super::super::update;
use super::calls_tests::{calls, detail, git_jobs, loads, refreshes, run, view};
use super::tests::{cleanup, github, setup};
use crate::msg::Msg;
use crate::pull_request::{PrCall, PrReply, PullRequestMsg};
use crate::state::AppState;
use glowfly_core::forge::{MergeMethod, Merged, PrDetail, PrPage, Provider, ReviewEvent, ReviewStyle, Target, Vote};
use std::path::PathBuf;
use std::time::Instant;

fn cloud() -> Target {
    Target { provider: Provider::Bitbucket, host: "bitbucket.org".into(), ..github() }
}

/// `setup` on a repository on `host`, with pull request 7's panel open.
fn opened(name: &str, host: Target) -> (AppState, Vec<crate::effect::Effect>, [PathBuf; 2]) {
    let (mut st, work, bare, _) = setup(name, &[]);
    st.forge.as_mut().unwrap().host = Some(host);
    let fx = run(&mut st, PullRequestMsg::Open(7));
    (st, fx, [work, bare])
}

fn reply_msg(st: &AppState, reply: PrReply) -> PullRequestMsg {
    let workdir = st.workdir().unwrap();
    let target = st.forge.as_ref().unwrap().host.clone().unwrap();
    PullRequestMsg::Replied { workdir, target, id: 7, generation: st.pull_request_page_gen, reply: Box::new(reply) }
}

/// The page loaded with Bitbucket's votes style, as `f` changes it.
fn loaded(st: &mut AppState, f: impl FnOnce(&mut PrDetail)) {
    let mut d = detail("h7");
    d.style = ReviewStyle::Votes { mine: Vote::Neither };
    d.methods = vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::FastForward];
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
    st.pull_request.as_mut().unwrap().forms.review_event = ev;
}

#[test]
fn opening_a_bitbucket_pull_request_loads_its_page() {
    let (st, fx, dirs) = opened("prb-open", cloud());
    let jobs = calls(&fx);
    assert_eq!(jobs.len(), 1);
    assert!(matches!(jobs[0].call, PrCall::Load));
    assert_eq!(jobs[0].req.target, cloud());
    assert_eq!(view(&st).label, "#7");
    done(st, dirs);
}

#[test]
fn request_changes_sends_without_a_summary() {
    let (mut st, _, dirs) = opened("prb-request", cloud());
    loaded(&mut st, |_| {});
    choose(&mut st, ReviewEvent::RequestChanges);
    let fx = run(&mut st, PullRequestMsg::SubmitReview);
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    assert!(
        matches!(&job.call, PrCall::Review { event: ReviewEvent::RequestChanges, head_sha, body, .. } if head_sha == "h7" && body.is_empty()),
        "{:?}",
        job.call
    );
    done(st, dirs);
}

#[test]
fn remove_change_request_waits_for_one() {
    let (mut st, _, dirs) = opened("prb-unrequest", cloud());
    loaded(&mut st, |_| {});
    choose(&mut st, ReviewEvent::UnrequestChanges);
    assert!(calls(&run(&mut st, PullRequestMsg::SubmitReview)).is_empty(), "nothing to take back");
    loaded(&mut st, |d| d.style = ReviewStyle::Votes { mine: Vote::ChangesRequested });
    choose(&mut st, ReviewEvent::UnrequestChanges);
    st.pull_request.as_mut().unwrap().forms.review_body = "Fixed now".into();
    let fx = run(&mut st, PullRequestMsg::SubmitReview);
    let jobs = calls(&fx);
    assert!(matches!(&jobs[..], [j] if matches!(&j.call, PrCall::Review { event: ReviewEvent::UnrequestChanges, .. })), "{jobs:?}");
    let msg = reply_msg(&st, PrReply::Reviewed(ReviewEvent::UnrequestChanges, Ok(())));
    let fx = run(&mut st, msg);
    assert_eq!((loads(&fx), refreshes(&fx)), (1, 1));
    assert_eq!(view(&st).forms.review_body, "");
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Removed your change request from #7"));
    assert!(st.toasts.iter().any(|t| t.text == "Removed your change request from #7"));
    done(st, dirs);
}

#[test]
fn the_author_cant_vote() {
    let (mut st, _, dirs) = opened("prb-author", cloud());
    choose(&mut st, ReviewEvent::Approve);
    loaded(&mut st, |d| d.me = d.author.clone());
    assert_eq!(view(&st).forms.review_event, ReviewEvent::Comment, "prefill moves the author to Comment");
    choose(&mut st, ReviewEvent::Approve);
    assert!(calls(&run(&mut st, PullRequestMsg::SubmitReview)).is_empty());
    assert_eq!(view(&st).busy, None);
    done(st, dirs);
}

#[test]
fn squash_merges_and_fetches() {
    let (mut st, _, dirs) = opened("prb-squash", cloud());
    loaded(&mut st, |d| d.methods = vec![MergeMethod::Squash, MergeMethod::Merge]);
    assert_eq!(view(&st).forms.merge_method, Some(MergeMethod::Squash), "the repository's default is chosen");
    let fx = run(&mut st, PullRequestMsg::Merge);
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    let PrCall::Merge { req, .. } = &job.call else { panic!("{:?}", job.call) };
    assert_eq!((req.method, req.head_sha.as_str()), (MergeMethod::Squash, "h7"));
    let result = Ok(Merged { sha: "m1".into() });
    let reply = PrReply::Merged { method: MergeMethod::Squash, remote: "origin".into(), source: "feature".into(), delete: false, result };
    let msg = reply_msg(&st, reply);
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    let fetch = ["fetch", "--prune", "origin"].map(String::from).to_vec();
    assert_eq!(git_jobs(&fx), [vec![fetch]]);
    assert!(st.log.iter().any(|l| l.ok && l.text == "Merged #7 into main (squash)"));
    done(st, dirs);
}

#[test]
fn no_strategy_sends_nothing() {
    let (mut st, _, dirs) = opened("prb-none", cloud());
    loaded(&mut st, |d| d.methods.clear());
    assert!(calls(&run(&mut st, PullRequestMsg::Merge)).is_empty());
    assert_eq!(view(&st).busy, None);
    done(st, dirs);
}
