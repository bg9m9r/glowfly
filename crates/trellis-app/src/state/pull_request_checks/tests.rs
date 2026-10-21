use super::*;
use crate::state::pull_request_actions::tests::view;
use trellis_core::forge::{Checks, Provider, Target};

fn fs(provider: Option<Provider>) -> ForgeState {
    let host = provider.map(|provider| Target {
        provider,
        host: "example.com".into(),
        owner: "o".into(),
        project: String::new(),
        repo: "r".into(),
    });
    ForgeState::new(host, "origin".into(), None)
}

fn run(name: &str, state: RunState) -> CheckRun {
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

fn list(runs: Vec<CheckRun>) -> CheckList {
    CheckList { sha: "abc".into(), runs, complete: true }
}

fn loaded(runs: Vec<CheckRun>, fetched: Instant) -> PrChecks {
    PrChecks { pane: ChecksPane::Loaded(list(runs)), open: true, reloading: false, in_flight: false, fetched: Some(fetched) }
}

#[test]
fn code_hosts_load_checks_and_others_have_none() {
    for p in [Provider::GitHub, Provider::GitLab, Provider::AzureDevOps, Provider::BitbucketDataCenter] {
        let c = PrChecks::new(&fs(Some(p)));
        assert_eq!((c.pane, c.open, c.in_flight), (ChecksPane::Loading, true, false), "{p:?}");
    }
    assert_eq!(PrChecks::new(&fs(None)).pane, ChecksPane::Unsupported);
    assert_eq!(PrChecks::new(&fs(Some(Provider::JiraCloud))).pane, ChecksPane::Unsupported);
}

#[test]
fn running_checks_are_read_again_after_a_minute() {
    let t0 = Instant::now();
    let c = loaded(vec![run("a", RunState::Passed), run("b", RunState::Running)], t0);
    assert!(!c.due(t0 + Duration::from_secs(59)));
    assert!(c.due(t0 + Duration::from_secs(61)));
    assert_eq!(c.next_wake(t0), Some(Duration::from_secs(60)));
    assert_eq!(c.next_wake(t0 + Duration::from_secs(90)), Some(Duration::from_secs(1)), "at least a second");
    let queued = loaded(vec![run("q", RunState::Pending)], t0);
    assert!(queued.due(t0 + Duration::from_secs(61)));
    // Finished, in flight, or not loaded: never.
    let done = loaded(vec![run("a", RunState::Passed), run("f", RunState::Failed)], t0);
    assert!(!done.due(t0 + Duration::from_secs(600)) && done.next_wake(t0).is_none());
    let flying = PrChecks { in_flight: true, ..c.clone() };
    assert!(!flying.due(t0 + Duration::from_secs(600)) && flying.next_wake(t0).is_none());
    let loading = PrChecks::new(&fs(Some(Provider::GitHub)));
    assert!(!loading.due(t0) && loading.next_wake(t0).is_none());
}

#[test]
fn the_header_prefers_the_services_rollup() {
    let mut v = view(None);
    v.checks = loaded(vec![run("a", RunState::Passed), run("b", RunState::Failed)], Instant::now());
    assert_eq!(header_summary(&v), (Some(CheckState::Failed), "1 failed, 1 passed".to_owned()));
    v.pr.checks = Some(Checks::from_counts(2, 0, 0, true));
    assert_eq!(header_summary(&v).0, Some(CheckState::Passed), "the sidebar's marker");
    v.pr.checks = None;
    v.checks = loaded(Vec::new(), Instant::now());
    assert_eq!(header_summary(&v), (None, NO_STATUS.to_owned()));
    v.checks.pane = ChecksPane::Loading;
    assert_eq!(header_summary(&v), (None, String::new()));
}

#[test]
fn the_build_link_names_a_failure() {
    let mut v = view(None);
    let mut failed = run("b", RunState::Failed);
    failed.url = "https://ci.example/b".into();
    let mut passed = run("a", RunState::Passed);
    passed.url = "https://ci.example/a".into();
    v.checks = loaded(vec![run("none", RunState::Failed), failed, passed.clone()], Instant::now());
    assert_eq!(build_link(&v), Some(("Open failed build", "https://ci.example/b".into())), "the first run with a link");
    v.pr.checks = Some(Checks { url: "https://ci.example/all".into(), ..Checks::from_counts(1, 0, 0, true) });
    assert_eq!(build_link(&v), Some(("Open build page", "https://ci.example/all".into())));
    v.pr.checks = None;
    v.checks = loaded(vec![passed], Instant::now());
    assert_eq!(build_link(&v).map(|l| l.0), Some("Open build page"));
    v.checks = loaded(vec![run("x", RunState::Failed)], Instant::now());
    assert_eq!(build_link(&v), None);
}

#[test]
fn times_show_a_duration_or_how_long_it_has_run() {
    let now = 10_000;
    let mut r = run("a", RunState::Passed);
    assert_eq!(time_text(&r, now), None);
    (r.started, r.finished) = (Some(1_000), Some(1_134));
    assert_eq!(time_text(&r, now).as_deref(), Some("2m 14s"));
    r.finished = Some(900);
    assert_eq!(time_text(&r, now), None, "a finish before the start");
    let mut running = run("b", RunState::Running);
    running.started = Some(now - 200);
    assert_eq!(time_text(&running, now).as_deref(), Some("running 3m"));
    running.started = Some(now - 20);
    assert_eq!(time_text(&running, now).as_deref(), Some("running <1m"));
    let mut queued = run("c", RunState::Pending);
    queued.started = Some(now - 200);
    assert_eq!(time_text(&queued, now), None);
}

#[test]
fn tooltips_and_labels() {
    let now = 10_000;
    let mut r = run("build", RunState::Failed);
    r.group = Some("CI".into());
    assert_eq!(run_label(&r), "CI · build");
    assert_eq!(run_tooltip(&r, Lang::En, now), "CI · build\nFailed\nThe service gave no link for this check.");
    r.required = Some(true);
    r.url = "https://ci.example/1".into();
    r.description = "2 tests failed".into();
    (r.started, r.finished) = (Some(now - 7200), Some(now - 7200 + 134));
    assert_eq!(run_tooltip(&r, Lang::En, now), "CI · build\nFailed · Required\n2 tests failed\nStarted 2 hours ago\nTook 2m 14s");
    r.required = Some(false);
    assert!(run_tooltip(&r, Lang::En, now).contains("Failed · Optional"));

    let mut odd = run("evil\u{202e}name", RunState::Skipped);
    assert_eq!(run_label(&odd), "evilname");
    odd.group = Some("\u{202d}".into());
    assert_eq!(run_label(&odd), "evilname", "an empty group is left out");
    assert_eq!(empty_text("merge request"), "Nothing has reported a check or build on this merge request's latest commit yet.");
    assert_eq!(truncated_text(), "Showing the first 300 checks.");
}
