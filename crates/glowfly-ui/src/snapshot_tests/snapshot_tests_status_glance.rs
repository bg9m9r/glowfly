//! The status bar's glance items (design pass D15): the last failure with Details and its ×, in
//! Dark and Light (pixel goldens), "Fetched 4 min ago" with its per-remote tooltip and its click,
//! disabled with the reason while a job runs, and the running job's dots, text and Cancel at
//! the left with the failure making way for it.

use crate::logs::LogsMsg;
use crate::msg::{JobMsg, Msg};
use crate::state::{AppState, LogEntry, RunningJob};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{NOW, remove_fixture, with_repo};
use crate::ui::theme::{self, color};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::logs::RunInfo;
use glowfly_core::repo::RemoteInfo;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Only the status bar, 900 px wide, in the dark or light view palette.
fn bar(st: AppState, dark: bool) -> H {
    let p = theme::view_palette(dark);
    let size = egui::vec2(900.0, crate::ui::status::BAR_H);
    crate::ui::snapshot_check::builder().with_size(size).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_palette(&p);
            theme::apply_style(ctx, &p);
            ctx.set_theme(theme::egui_theme(&p));
            crate::ui::status::status_bar(ctx, st, msgs);
        },
        (st, Vec::new()),
    )
}

/// The fixture with remotes origin and upstream, fetched from origin 4 minutes ago, and a
/// failed Fetch all as its last run.
fn failed(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.view.now = NOW;
    let snap = st.snap.as_mut().unwrap();
    snap.remote_info = ["origin", "upstream"].map(|n| RemoteInfo { name: n.into(), ..RemoteInfo::default() }).to_vec();
    snap.fetched_at = Some(NOW - 4 * 60 - 10);
    snap.fetched_remotes = vec!["origin".into()];
    let run = RunInfo { title: "Fetch all".into(), summary: String::new(), took_ms: None, routine: false };
    st.log.push(LogEntry { run: Some(run), ..LogEntry::new(false, "Fetch all FAILED") });
    (st, dir)
}

fn sent(h: &H, want: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn status_bar_failure_dark() {
    let (st, dir) = failed(ThemeChoice::Dark, "status-failure-dark");
    let entry = st.log.last().unwrap().id;
    let mut h = bar(st, true);
    h.run();
    h.check_snapshot("status_bar_failure_dark");
    assert!(h.query_by_label("Fetch all failed").is_some());
    assert!(h.query_by_label("Fetched 4 min ago").is_some());
    h.get_by_label("Details").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Logs(LogsMsg::Details { entry: Some(e), .. }) if *e == entry)), "Details opens its run");
    h.get_by_label("Dismiss").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Logs(LogsMsg::DismissFailure(e)) if *e == entry)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn status_bar_failure_light() {
    let (st, dir) = failed(ThemeChoice::Light, "status-failure-light");
    let mut h = bar(st, false);
    h.run();
    h.check_snapshot("status_bar_failure_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_fetch_item_fetches_and_lists_each_remote() {
    let (st, dir) = failed(ThemeChoice::Dark, "status-fetch-click");
    let mut h = bar(st, true);
    h.run();
    h.get_by_label("Fetched 4 min ago").hover();
    h.run();
    h.run();
    assert!(h.query_by_label_contains("origin: Last fetched 4 minutes ago").is_some(), "its tooltip");
    assert!(h.query_by_label_contains("upstream: not in the last fetch").is_some());
    h.get_by_label("Fetched 4 min ago").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Job(JobMsg::Git { cmds, .. }) if cmds[0] == ["fetch", "--all", "--prune"])));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn while_a_job_runs_the_job_shows_and_the_fetch_item_waits() {
    let (mut st, dir) = failed(ThemeChoice::Dark, "status-fetch-busy");
    st.job = Some(RunningJob {
        label: "Fetch all".into(),
        workdir: Some(dir.clone()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_continue: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: true,
    });
    st.settings.reduce_motion = true;
    let mut h = bar(st, true);
    h.run();
    h.check_snapshot("status_bar_job");
    assert!(h.query_by_label("Fetch all failed").is_none(), "the job takes the failure's place");
    assert!(h.query_by_label("Fetch all…").is_some());
    let item = h.get_by_label("Fetching…");
    assert!(item.is_disabled(), "disabled while the job runs");
    item.hover();
    h.run();
    h.run();
    let why = crate::state::toolbar_tips::fetch_blocked(&h.state().0).unwrap();
    assert!(h.query_by_label_contains(&why).is_some(), "with the reason: {why}");
    h.get_by_label("Fetching…").click();
    h.run();
    assert!(!sent(&h, |m| matches!(m, Msg::Job(JobMsg::Git { .. }))), "no second fetch");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn without_a_remote_there_is_no_fetch_item() {
    let (mut st, dir) = failed(ThemeChoice::Dark, "status-no-remote");
    st.snap.as_mut().unwrap().remote_info.clear();
    let mut h = bar(st, true);
    h.run();
    assert!(h.query_by_label_contains("Fetched").is_none());
    drop(h);
    remove_fixture(&dir);
}
