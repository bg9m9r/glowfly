//! Headless tests for merging from the pull request panel: what the merge sends, and what its
//! result starts (deleting the source branch and fetching), for the active tab, a closed panel
//! and another tab.

use super::calls_tests::{calls, done, err, git_jobs, loaded, loads, merged_ok, opened, refreshes, reply_msg, run, view};
use crate::msg::Msg;
use crate::pull_request::{PrCall, PrReply, PullRequestMsg};
use crate::state::pull_request::PrAction;
use crate::update::update;
use glowfly_core::forge::{MergeMethod, MergeState};
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn merge_blocked_on_conflicts_sends_nothing() {
    let (mut st, _, dirs) = opened("prc-conflicts");
    loaded(&mut st, |d| d.merge_state = MergeState::Conflicts);
    assert!(calls(&run(&mut st, PullRequestMsg::Merge)).is_empty());
    done(st, dirs);
}

#[test]
fn merge_sends_head_sha_and_method() {
    let (mut st, _, dirs) = opened("prc-merge");
    loaded(&mut st, |d| d.head_sha = "h1".into());
    let v = st.session.pull_request.view.as_mut().unwrap();
    v.forms.merge_method = Some(MergeMethod::Rebase);
    v.forms.merge_title = " Title ".into();
    v.forms.delete_branch = true;
    let fx = run(&mut st, PullRequestMsg::Merge);
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    let PrCall::Merge { req, remote, source, delete, delete_local } = &job.call else { panic!("{:?}", job.call) };
    assert_eq!((req.method, req.head_sha.as_str(), req.title.as_str()), (MergeMethod::Rebase, "h1", "Title"));
    assert_eq!((remote.as_str(), source.as_str(), *delete, *delete_local), ("origin", "feature", true, false));
    assert_eq!(view(&st).busy, Some(PrAction::Merge));
    done(st, dirs);
}

#[test]
fn merged_ok_with_delete_runs_push_delete_then_fetch() {
    let (mut st, _, dirs) = opened("prc-merged-delete");
    loaded(&mut st, |_| {});
    let msg = reply_msg(&st, merged_ok(true));
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    let push = ["push", "origin", "--delete", "feature"].map(String::from).to_vec();
    let fetch = ["fetch", "--prune", "origin"].map(String::from).to_vec();
    assert_eq!(git_jobs(&fx), [vec![push, fetch]]);
    assert_eq!((refreshes(&fx), loads(&fx)), (1, 1));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Delete feature and fetch origin"));
    assert!(st.log.iter().any(|l| l.ok && l.text == "Merged #7 into main (squash)"));
    done(st, dirs);
}

#[test]
fn merged_ok_without_delete_only_fetches() {
    let (mut st, _, dirs) = opened("prc-merged-fetch");
    loaded(&mut st, |_| {});
    let msg = reply_msg(&st, merged_ok(false));
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    assert_eq!(git_jobs(&fx), [vec![["fetch", "--prune", "origin"].map(String::from).to_vec()]]);
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch origin"));
    done(st, dirs);
}

#[test]
fn merged_ok_while_a_job_runs_logs_the_hint() {
    let (mut st, _, dirs) = opened("prc-merged-busy");
    loaded(&mut st, |_| {});
    update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now());
    assert!(st.job.is_some());
    let msg = reply_msg(&st, merged_ok(true));
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    assert!(git_jobs(&fx).is_empty());
    let hint = "Merged. Fetch origin to bring the merge into this repository. Delete feature from its remote branch menu.";
    assert!(st.log.iter().any(|l| l.text == hint), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    done(st, dirs);
}

#[test]
fn merged_err_sets_error_and_starts_no_job() {
    let (mut st, _, dirs) = opened("prc-merged-err");
    loaded(&mut st, |_| {});
    run(&mut st, PullRequestMsg::Merge);
    let reply = PrReply::Merged {
        method: MergeMethod::Squash,
        remote: "origin".into(),
        source: "feature".into(),
        delete: false,
        delete_local: false,
        result: Err(err("HTTP 405: not mergeable")),
    };
    let msg = reply_msg(&st, reply);
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    assert!(git_jobs(&fx).is_empty() && st.job.is_none());
    assert_eq!(view(&st).action_error, Some((PrAction::Merge, "HTTP 405: not mergeable".into())));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Merging #7 failed: HTTP 405: not mergeable"));
    done(st, dirs);
}

#[test]
fn merged_for_inactive_tab_only_logs() {
    let (mut st, _, dirs) = opened("prc-inactive");
    loaded(&mut st, |_| {});
    run(&mut st, PullRequestMsg::Merge);
    let mut msg = reply_msg(&st, merged_ok(true));
    if let PullRequestMsg::Replied { workdir, .. } = &mut msg {
        *workdir = PathBuf::from("C:/elsewhere");
    }
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    assert!(fx.is_empty() && st.job.is_none());
    assert_eq!(view(&st).busy, Some(PrAction::Merge), "this panel's own merge is still in flight");
    let last = st.log.last().unwrap();
    assert_eq!((last.ok, last.text.as_str()), (true, "Merged #7 (squash)"));
    assert!(!st.view.show_log, "logged quietly");
    done(st, dirs);
}

#[test]
fn merged_after_panel_closed_still_fetches() {
    let (mut st, _, dirs) = opened("prc-closed");
    loaded(&mut st, |_| {});
    let msg = reply_msg(&st, merged_ok(false));
    run(&mut st, PullRequestMsg::Close);
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    assert_eq!(git_jobs(&fx).len(), 1);
    assert_eq!(loads(&fx), 0, "no panel to reload");
    assert!(st.log.iter().any(|l| l.text == "Merged #7 into main (squash)"));
    done(st, dirs);
}
