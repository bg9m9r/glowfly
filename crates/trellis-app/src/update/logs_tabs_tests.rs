//! Headless tests for per-tab activity logs: failures announced by a toast with Details instead
//! of opening the panel, lines that follow their tab, and earlier entries handed out at load.

use crate::effect::{Effect, LoadKind};
use crate::logs::{EarlierLog, LogKind, LogsEffect, LogsMsg, NewLine};
use crate::msg::{JobMsg, Msg, PrefsMsg, RepoMsg, TabsMsg};
use crate::state::{AppState, RunningJob, Tab};
use crate::update::tests::state;
use crate::update::update;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::gitcli::GitRun;
use trellis_core::logs::ActivityEntry;

/// Whether a failure toast offers Details (for other areas' tests too).
pub(in crate::update) fn failure_with_details(st: &AppState) -> bool {
    st.toasts.iter().any(|t| !t.ok && t.action.as_ref().is_some_and(|a| a.label == "Details"))
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// An absolute path on every system; nothing is created there.
fn repo(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join(format!("trellis-logs-tabs-{name}"))
}

/// Tabs for `a` (active) and `b`, neither loaded.
fn two_tabs() -> (AppState, PathBuf, PathBuf) {
    let (a, b) = (repo("a"), repo("b"));
    let mut st = state();
    st.tabs = vec![Tab::unloaded(a.clone()), Tab::unloaded(b.clone())];
    st.active_tab = 0;
    (st, a, b)
}

fn texts(log: &[crate::state::LogEntry]) -> Vec<&str> {
    log.iter().map(|e| e.text.as_str()).collect()
}

fn failed(text: &str) -> Msg {
    Msg::Logged { ok: false, text: text.into() }
}

fn scoped(workdir: &Path, msg: Msg) -> Msg {
    Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(msg) }
}

fn entry(at: i64, text: &str, repo: Option<&Path>) -> ActivityEntry {
    ActivityEntry { session: 1, at, ok: true, text: text.into(), repo: repo.map(Path::to_path_buf), run: None }
}

fn loaded(st: &mut AppState, entries: Vec<ActivityEntry>) -> Vec<Effect> {
    run(st, Msg::Logs(LogsMsg::ActivityLoaded(Ok(EarlierLog { dir: repo("logs"), entries }))))
}

fn appended(fx: &[Effect]) -> Vec<NewLine> {
    fx.iter()
        .flat_map(|e| match e {
            Effect::Logs(LogsEffect::Append(lines)) => lines.clone(),
            _ => Vec::new(),
        })
        .collect()
}

/// The tab a toast's Details selects.
fn details_tab(st: &AppState) -> Option<crate::state::tabs::TabId> {
    st.toasts.iter().find_map(|t| match t.action.as_ref().map(|a| &*a.msg) {
        Some(Msg::Logs(LogsMsg::Details { tab, .. })) => Some(*tab),
        _ => None,
    })
}

#[test]
fn a_failure_doesnt_open_the_panel() {
    let mut st = state();
    run(&mut st, failed("Couldn't save x"));
    assert!(!st.view.show_log);
    assert_eq!(st.toasts.len(), 1);
    let t = &st.toasts[0];
    assert!(!t.ok);
    assert_eq!(t.text, "Couldn't save x");
    let action = t.action.as_ref().expect("Details");
    assert_eq!(action.label, "Details");
    assert!(matches!(&*action.msg, Msg::Logs(LogsMsg::Details { tab: id, .. }) if *id == st.tabs[st.active_tab].id));
    assert_eq!(action.workdir, None, "works from any tab");
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Couldn't save x"));
}

#[test]
fn a_failure_toast_gets_details_instead_of_a_second_toast() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsUnreadable("Settings could not be read".into())));
    assert_eq!(st.toasts.len(), 1);
    assert!(failure_with_details(&st));
    assert!(!st.view.show_log);
}

#[test]
fn quiet_failures_announce_nothing() {
    let mut st = state();
    run(&mut st, Msg::Logs(LogsMsg::AppendFailed("disk full".into())));
    st.settings.extended_logging = true;
    let timed_out =
        GitRun { cwd: repo("a"), args: vec!["fetch".into()], took: Duration::from_secs(30), code: None, ok: false, timed_out: true };
    run(&mut st, Msg::Logs(LogsMsg::GitRan(timed_out)));
    assert_eq!(st.log.iter().filter(|l| !l.ok).count(), 2);
    assert!(st.toasts.is_empty() && !st.view.show_log);
    assert!(st.logs.announce.is_none());
}

#[test]
fn multiline_failures_toast_their_first_line() {
    let mut st = state();
    run(&mut st, failed("\nPush FAILED\nremote rejected"));
    assert_eq!(st.toasts.last().unwrap().text, "Push FAILED");
    assert_eq!(st.log.last().unwrap().text, "\nPush FAILED\nremote rejected", "the log keeps it whole");

    let long = "é".repeat(500);
    run(&mut st, failed(&long));
    let text = &st.toasts.last().unwrap().text;
    assert_eq!(text.chars().count(), 203, "200 characters and the ellipsis");
    assert!(text.ends_with("...") && text.starts_with("éé"));
}

#[test]
fn several_failures_in_one_update_announce_once() {
    let mut st = state();
    st.log(false, "first");
    st.log(false, "second");
    assert_eq!(st.logs.announce.as_ref().map(|a| a.text.as_str()), Some("first"), "left set outside update");
    run(&mut st, Msg::Tick(Instant::now()));
    assert_eq!(st.toasts.len(), 1);
    assert_eq!(st.toasts[0].text, "first");
}

#[test]
fn no_toast_while_the_panel_shows_this_tabs_log() {
    let mut st = state();
    st.view.show_log = true;
    st.logs.tab = LogKind::Activity;
    run(&mut st, failed("boom"));
    assert!(st.toasts.is_empty(), "the line is in front of the user already");
    st.logs.tab = LogKind::Errors;
    run(&mut st, failed("boom again"));
    assert!(failure_with_details(&st));
}

#[test]
fn details_opens_the_activity_tab() {
    let mut st = state();
    st.logs.tab = LogKind::Errors;
    run(&mut st, failed("Couldn't reach github.com"));
    let id = st.toasts[0].id;
    run(&mut st, Msg::ToastAction(id));
    assert!(st.view.show_log);
    assert_eq!(st.logs.tab, LogKind::Activity);
    assert!(st.toasts.iter().all(|t| t.id != id), "the toast is gone");
    assert!(st.log.iter().any(|l| l.text == "Couldn't reach github.com"));
}

#[test]
fn each_tab_keeps_its_own_log() {
    let (mut st, _, _) = two_tabs();
    st.log_quiet(true, "in a");
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    assert!(st.log.is_empty());
    assert_eq!(texts(&st.tabs[0].log), ["in a"]);
    st.log_quiet(true, "in b");
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(texts(&st.log), ["in a"]);
    assert_eq!(texts(&st.tabs[1].log), ["in b"]);
    assert!(st.tabs[0].log.is_empty());

    // Reordering moves each tab's log with it.
    run(&mut st, Msg::Tabs(TabsMsg::MoveTab { from: 1, to: 0 }));
    let b = st.tabs.iter().position(|t| t.path == Some(repo("b"))).unwrap();
    assert_ne!(b, st.active_tab);
    assert_eq!(texts(&st.tabs[b].log), ["in b"]);
    assert_eq!(texts(&st.log), ["in a"]);
}

#[test]
fn a_result_for_another_tab_lands_in_its_log() {
    let (mut st, _, b) = two_tabs();
    run(&mut st, scoped(&b, failed("boom")));
    assert_eq!(texts(&st.tabs[1].log), ["boom"]);
    assert!(st.log.is_empty());
    assert_eq!(st.toasts.len(), 1);
    assert_eq!(details_tab(&st), Some(st.tabs[1].id));

    let id = st.toasts[0].id;
    run(&mut st, Msg::ToastAction(id));
    assert_eq!(st.active_tab, 1);
    assert_eq!(texts(&st.log), ["boom"]);
    assert!(st.view.show_log);
}

#[test]
fn an_ok_line_for_another_tab_is_dropped() {
    let (mut st, _, b) = two_tabs();
    run(&mut st, scoped(&b, Msg::Logged { ok: true, text: "fine".into() }));
    assert!(st.tabs[1].log.is_empty() && st.log.is_empty());
}

/// A failed Push that ran in `workdir`.
fn push_failed_in(st: &mut AppState, workdir: &Path) {
    st.job = Some(RunningJob {
        label: "Push".into(),
        workdir: Some(workdir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    run(st, Msg::Job(JobMsg::JobDone { ok: false, output: "remote rejected".into(), undo: None }));
}

#[test]
fn details_opens_the_failed_run() {
    let (mut st, _, b) = two_tabs();
    push_failed_in(&mut st, &b);
    let id = st.tabs[1].log.last().expect("the run").id;
    st.logs.open.clear();
    let toast = st.toasts.iter().find(|t| !t.ok).expect("the failure toast").id;
    run(&mut st, Msg::ToastAction(toast));
    assert_eq!(st.active_tab, 1);
    assert!(st.view.show_log);
    assert_eq!(st.logs.tab, LogKind::Activity);
    assert!(st.logs.open.contains(&id), "the run is opened");
    assert_eq!(st.view.activity_reveal, Some(id), "and scrolled into view");
}

#[test]
fn a_finished_job_logs_in_its_own_tab() {
    let (mut st, _, b) = two_tabs();
    push_failed_in(&mut st, &b);
    assert!(st.tabs[1].log.iter().any(|l| l.text.starts_with("Push FAILED")), "{:?}", texts(&st.tabs[1].log));
    assert!(!st.log.iter().any(|l| l.text.starts_with("Push FAILED")));
    assert_eq!(st.toasts.iter().filter(|t| !t.ok).count(), 1, "the headline toast is the announcement");
    assert!(st.toasts.iter().any(|t| t.text.starts_with("Push failed")));
    assert_eq!(details_tab(&st), Some(st.tabs[1].id));
}

#[test]
fn git_ran_lines_follow_their_folder() {
    let (mut st, _, b) = two_tabs();
    st.settings.extended_logging = true;
    let ran = |cwd: PathBuf| GitRun {
        cwd,
        args: vec!["status".into()],
        took: Duration::from_millis(5),
        code: Some(0),
        ok: true,
        timed_out: false,
    };
    run(&mut st, Msg::Logs(LogsMsg::GitRan(ran(b.clone()))));
    assert_eq!(st.tabs[1].log.len(), 1);
    assert!(st.log.is_empty());
    run(&mut st, Msg::Logs(LogsMsg::GitRan(ran(repo("elsewhere")))));
    assert_eq!(st.log.len(), 1);
}

#[test]
fn new_lines_are_tagged_with_their_tab() {
    let (mut st, a, b) = two_tabs();
    loaded(&mut st, Vec::new());
    let fx = run(&mut st, failed("here"));
    assert_eq!(appended(&fx), [NewLine { ok: false, text: "here".into(), repo: Some(a.clone()), run: None }]);
    let fx = run(&mut st, scoped(&b, failed("there")));
    assert_eq!(appended(&fx), [NewLine { ok: false, text: "there".into(), repo: Some(b.clone()), run: None }]);
    let c = repo("c");
    let fx = run(&mut st, scoped(&c, failed("unopened")));
    assert_eq!(appended(&fx), [NewLine { ok: false, text: "unopened".into(), repo: Some(c), run: None }]);
    assert_eq!(st.log.last().unwrap().text, "unopened", "kept in the active tab's log");

    let mut st = state();
    loaded(&mut st, Vec::new());
    let fx = run(&mut st, failed("welcome"));
    assert_eq!(appended(&fx), [NewLine { ok: false, text: "welcome".into(), repo: None, run: None }]);
}

#[test]
fn earlier_entries_load_into_their_tabs() {
    let (mut st, a, b) = two_tabs();
    let c = repo("c");
    st.log_quiet(true, "now");
    loaded(
        &mut st,
        vec![
            entry(1, "a1", Some(&a)),
            entry(2, "b1", Some(&b)),
            entry(3, "c1", Some(&c)),
            entry(4, "none", None),
            entry(5, "a2", Some(&a)),
        ],
    );
    assert_eq!(texts(&st.log), ["a1", "none", "a2", "now"]);
    assert_eq!(texts(&st.tabs[1].log), ["b1"]);
    assert_eq!(st.logs.unclaimed, vec![entry(3, "c1", Some(&c))]);

    st.tabs.push(Tab::unloaded(c.clone()));
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(2)));
    assert_eq!(texts(&st.log), ["c1"]);
    assert!(st.logs.unclaimed.is_empty());
    assert_eq!(texts(&st.tabs[0].log), ["a1", "none", "a2", "now"], "earlier entries stay in front");
}

#[test]
fn clear_empties_only_this_tab() {
    let (mut st, a, b) = two_tabs();
    loaded(&mut st, Vec::new());
    st.log_quiet(true, "in a");
    run(&mut st, scoped(&b, failed("in b")));
    let fx = run(&mut st, Msg::ClearLog);
    assert!(st.log.is_empty());
    assert_eq!(texts(&st.tabs[1].log), ["in b"]);
    assert!(matches!(&fx[..], [Effect::Logs(LogsEffect::ClearActivity { repo: Some(r) })] if *r == a), "{fx:?}");

    // Before the file is read: that tab's earlier entries are skipped, the others' kept.
    let (mut st, a, b) = two_tabs();
    st.log_quiet(true, "early");
    assert!(run(&mut st, Msg::ClearLog).is_empty());
    let fx = loaded(&mut st, vec![entry(1, "old a", Some(&a)), entry(2, "old b", Some(&b))]);
    assert!(st.log.is_empty());
    assert_eq!(texts(&st.tabs[1].log), ["old b"]);
    assert!(matches!(&fx[..], [Effect::Logs(LogsEffect::ClearActivity { repo: Some(r) })] if *r == a), "{fx:?}");
}

#[test]
fn startup_lines_follow_the_restored_tab() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsUnreadable("Settings could not be read".into())));
    let (a, b) = (repo("a"), repo("b"));
    run(&mut st, Msg::Tabs(TabsMsg::RestoreTabs { paths: vec![a, b.clone()], labels: vec![None, None], active: 1 }));
    assert_eq!(st.tabs.len(), 2, "the startup tab is dropped");
    assert_eq!(st.tabs[st.active_tab].path.as_deref(), Some(b.as_path()));
    assert!(st.log.iter().any(|l| l.text == "Settings could not be read"), "{:?}", texts(&st.log));
}

#[test]
fn a_failed_open_keeps_its_error() {
    let mut st = state();
    let a = repo("a");
    st.tabs = vec![Tab::unloaded(a.clone())];
    st.log_quiet(true, "in a");
    let c = repo("c");
    run(&mut st, Msg::Repo(RepoMsg::Open(c.clone())));
    assert_eq!(st.tabs.len(), 2, "a new tab for c");
    let generation = st.load_gen;
    run(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result: Err("not a repository".into()) }));
    assert_eq!(st.tabs.len(), 1, "back to a");
    assert_eq!(st.tabs[st.active_tab].path.as_deref(), Some(a.as_path()));
    assert_eq!(texts(&st.log), ["in a", "not a repository"]);
    assert!(failure_with_details(&st));
}
