//! Headless tests for the pull request panel's Checks section: when it asks for the checks,
//! what a reply does, and the header's toggle, with replies fed by hand.

use super::super::update;
use super::calls_tests::{done, err, loads, opened, run, view};
use super::tests::{github, rev, setup};
use crate::effect::{Effect, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg, TabsMsg};
use crate::pull_request::{CallJob, PrCall, PrEffect, PrReply, PullRequestMsg};
use crate::state::AppState;
use crate::state::pull_request::Page;
use crate::state::pull_request_checks::ChecksPane;
use glowfly_core::forge::{Auth, CheckList, CheckRun, CheckState, Checks, Provider, RunState, Target};
use glowfly_core::settings::RepoTrust;
use std::time::{Duration, Instant};

/// The checks calls among `fx`.
fn checks_calls(fx: &[Effect]) -> Vec<&CallJob> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::PullRequest(PrEffect::Call(j)) if matches!(j.call, PrCall::Checks { .. }) => Some(&**j),
            _ => None,
        })
        .collect()
}

fn check(name: &str, state: RunState) -> CheckRun {
    CheckRun {
        name: name.into(),
        group: None,
        state,
        required: None,
        started: None,
        finished: None,
        url: String::new(),
        description: String::new(),
    }
}

fn list(sha: &str, states: &[RunState]) -> CheckList {
    CheckList { sha: sha.into(), runs: states.iter().map(|&s| check("c", s)).collect(), complete: true }
}

fn checks_reply(st: &AppState, result: Result<CheckList, ForgeError>) -> Msg {
    let workdir = st.workdir().unwrap();
    let reply = Box::new(PrReply::Checks(result));
    Msg::PullRequest(PullRequestMsg::Replied { workdir, target: github(), id: 7, generation: st.session.pull_request.checks_gen, reply })
}

/// Feeds the checks answer to the open panel at `at`.
fn answer(st: &mut AppState, result: Result<CheckList, ForgeError>, at: Instant) {
    let msg = checks_reply(st, result);
    update(st, msg, at);
}

fn head(st: &AppState) -> String {
    st.session.forge.as_ref().unwrap().prs[0].head.clone().unwrap()
}

fn listed_again(st: &mut AppState, f: impl FnOnce(&mut glowfly_core::forge::PullRequest)) -> Vec<Effect> {
    let mut pr = st.session.forge.as_ref().unwrap().prs[0].clone();
    f(&mut pr);
    let reply = ForgeReply { target: github(), auth: None, results: vec![ForgeResult::Prs(Ok(vec![pr]))] };
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now())
}

#[test]
fn opening_asks_for_checks_with_the_head() {
    let (st, fx, dirs) = opened("prk-open");
    let jobs = checks_calls(&fx);
    assert_eq!((jobs.len(), loads(&fx)), (1, 1));
    assert!(matches!(&jobs[0].call, PrCall::Checks { head: Some(h) } if *h == head(&st)));
    assert_eq!((jobs[0].id, jobs[0].generation, &jobs[0].req.target), (7, st.session.pull_request.checks_gen, &github()));
    let c = &view(&st).checks;
    assert_eq!((&c.pane, c.open, c.in_flight, c.reloading), (&ChecksPane::Loading, true, true, false));
    done(st, dirs);
}

#[test]
fn unsupported_host_asks_for_checks_without_a_page() {
    let (mut st, work, bare, _) = setup("prk-unsupported", &[]);
    st.session.forge.as_mut().unwrap().host =
        Some(Target { provider: Provider::AzureDevOps, host: "dev.azure.com".into(), project: "web".into(), ..github() });
    let fx = run(&mut st, PullRequestMsg::Open(7));
    assert_eq!((checks_calls(&fx).len(), loads(&fx)), (1, 0));
    assert_eq!(view(&st).page, Page::Unsupported);
    assert_eq!(view(&st).checks.pane, ChecksPane::Loading);
    done(st, [work, bare]);
}

#[test]
fn untrusted_repository_holds_checks() {
    let (mut st, work, bare, _) = setup("prk-held", &[]);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    let fx = run(&mut st, PullRequestMsg::Open(7));
    assert!(checks_calls(&fx).is_empty());
    assert_eq!(view(&st).checks.pane, ChecksPane::Held);
    assert!(checks_calls(&run(&mut st, PullRequestMsg::ReloadChecks)).is_empty());
    done(st, [work, bare]);
}

#[test]
fn a_reply_shows_the_list_and_stale_ones_are_dropped() {
    let (mut st, _, dirs) = opened("prk-reply");
    let ok = || Ok(list("abc", &[RunState::Passed]));
    let mut stale = checks_reply(&st, ok());
    let mut elsewhere = checks_reply(&st, ok());
    if let (Msg::PullRequest(PullRequestMsg::Replied { generation, .. }), Msg::PullRequest(PullRequestMsg::Replied { workdir, .. })) =
        (&mut stale, &mut elsewhere)
    {
        *generation -= 1;
        *workdir = glowfly_core::paths::temp_dir().join("elsewhere");
    }
    update(&mut st, stale, Instant::now());
    update(&mut st, elsewhere, Instant::now());
    assert_eq!(view(&st).checks.pane, ChecksPane::Loading, "an older request, another repository");
    let t0 = Instant::now();
    answer(&mut st, ok(), t0);
    let c = &view(&st).checks;
    assert_eq!((&c.pane, c.in_flight, c.fetched), (&ChecksPane::Loaded(list("abc", &[RunState::Passed])), false, Some(t0)));

    let late = checks_reply(&st, Ok(CheckList::empty("x")));
    run(&mut st, PullRequestMsg::Close);
    update(&mut st, late, Instant::now());
    assert!(st.session.pull_request.view.is_none(), "a closed panel stays closed");
    assert!(st.log.is_empty());
    done(st, dirs);
}

#[test]
fn a_failed_first_load_fails_and_a_failed_reload_keeps_the_list() {
    let (mut st, _, dirs) = opened("prk-fail");
    answer(&mut st, Err(err("HTTP 502: bad gateway")), Instant::now());
    assert_eq!(view(&st).checks.pane, ChecksPane::Failed("HTTP 502: bad gateway".into()));
    assert!(st.log.is_empty());
    assert_eq!(checks_calls(&run(&mut st, PullRequestMsg::ReloadChecks)).len(), 1, "Try again");
    assert_eq!(view(&st).checks.pane, ChecksPane::Loading);
    answer(&mut st, Ok(list("abc", &[RunState::Failed])), Instant::now());
    run(&mut st, PullRequestMsg::ReloadChecks);
    answer(&mut st, Err(err("timed out")), Instant::now());
    let c = &view(&st).checks;
    assert!(c.loaded().is_some() && !c.reloading && !c.in_flight);
    let line = st.log.last().expect("logged");
    assert!(!line.ok && line.text == "Reloading checks of #7 failed: timed out", "{}", line.text);
    done(st, dirs);
}

#[test]
fn an_auth_failure_forgets_the_cached_auth() {
    let (mut st, _, dirs) = opened("prk-auth");
    let key = github().account().key();
    st.session.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    answer(&mut st, Err(ForgeError { message: "authentication failed: 401".into(), auth: true }), Instant::now());
    assert!(!st.session.forge.as_ref().unwrap().auth.contains_key(&key));
    assert_eq!(view(&st).checks.pane, ChecksPane::Failed("authentication failed: 401".into()));
    done(st, dirs);
}

#[test]
fn reload_checks_keeps_the_list_and_marks_reloading() {
    let (mut st, _, dirs) = opened("prk-reload");
    answer(&mut st, Ok(list("abc", &[RunState::Passed])), Instant::now());
    let fx = run(&mut st, PullRequestMsg::ReloadChecks);
    assert_eq!((checks_calls(&fx).len(), loads(&fx)), (1, 0));
    let c = &view(&st).checks;
    assert!(c.reloading && c.in_flight && c.loaded().is_some());
    done(st, dirs);
}

#[test]
fn tick_re_asks_running_checks_after_a_minute() {
    let (mut st, _, dirs) = opened("prk-tick");
    let t0 = Instant::now();
    let tick = |st: &mut AppState, secs: u64| update(st, Msg::Tick(t0 + Duration::from_secs(secs)), t0 + Duration::from_secs(secs));
    assert!(checks_calls(&tick(&mut st, 120)).is_empty(), "in flight");
    answer(&mut st, Ok(list("abc", &[RunState::Passed, RunState::Running])), t0);
    assert!(checks_calls(&tick(&mut st, 59)).is_empty());
    assert_eq!(checks_calls(&tick(&mut st, 61)).len(), 1);
    assert!(checks_calls(&tick(&mut st, 200)).is_empty(), "not while in flight");
    answer(&mut st, Ok(list("abc", &[RunState::Passed, RunState::Failed])), t0);
    assert!(checks_calls(&tick(&mut st, 600)).is_empty(), "every check finished");
    done(st, dirs);
}

#[test]
fn a_changed_listed_summary_or_head_re_asks() {
    let (mut st, work, bare, _) = setup("prk-listed", &[]);
    run(&mut st, PullRequestMsg::Open(7));
    let sha = head(&st);
    answer(&mut st, Ok(list(&sha, &[RunState::Pending])), Instant::now());
    assert!(checks_calls(&listed_again(&mut st, |_| {})).is_empty(), "the same list");
    let failed = Checks::from_counts(0, 1, 0, true);
    let fx = listed_again(&mut st, |p| p.checks = Some(failed.clone()));
    assert_eq!(checks_calls(&fx).len(), 1, "pending turned to failed");
    assert_eq!(view(&st).pr.checks.as_ref().map(|c| c.state), Some(CheckState::Failed));
    answer(&mut st, Ok(list(&sha, &[RunState::Failed])), Instant::now());
    assert!(checks_calls(&listed_again(&mut st, |_| {})).is_empty());
    let fx = listed_again(&mut st, |p| p.head = Some(rev(&work, "main").to_string()));
    assert_eq!(checks_calls(&fx).len(), 1, "a new head");
    done(st, [work, bare]);
}

#[test]
fn toggle_checks_flips_open() {
    let (mut st, _, dirs) = opened("prk-toggle");
    assert!(view(&st).checks.open);
    assert!(run(&mut st, PullRequestMsg::ToggleChecks).is_empty());
    assert!(!view(&st).checks.open);
    run(&mut st, PullRequestMsg::ToggleChecks);
    assert!(view(&st).checks.open);
    assert!(PullRequestMsg::ToggleChecks.on_inactive().is_none() && PullRequestMsg::ReloadChecks.on_inactive().is_none());
    done(st, dirs);
}

#[test]
fn switching_tabs_back_re_asks() {
    let (mut st, _, dirs) = opened("prk-tabs");
    answer(&mut st, Ok(list("abc", &[RunState::Passed])), Instant::now());
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(checks_calls(&fx).len(), 1);
    assert!(view(&st).checks.reloading, "the last list stays on screen");
    done(st, dirs);
}
