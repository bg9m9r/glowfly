//! Headless tests for the logs: persistence of the activity log, the panel's tabs and extended
//! logging.

use super::super::tests::state;
use super::super::update;
use crate::effect::Effect;
use crate::logs::{EarlierLog, LogKind, LogsEffect, LogsMsg};
use crate::msg::{Msg, PrefsMsg};
use crate::state::logs::{Earlier, LOG_LIMIT, LogFileView, Persist};
use crate::state::{AppState, PrefPage, PrefsState};
use glowfly_core::gitcli::GitRun;
use glowfly_core::logs::{ActivityEntry, LogTail, git_run_line};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn logs(st: &mut AppState, m: LogsMsg) -> Vec<Effect> {
    run(st, Msg::Logs(m))
}

fn dir() -> PathBuf {
    glowfly_core::paths::temp_dir().join("glowfly-logs")
}

fn earlier(n: usize) -> Vec<ActivityEntry> {
    (0..n)
        .map(|i| ActivityEntry {
            session: 1_000,
            at: 1_000 + i as i64,
            ok: i % 2 == 0,
            text: format!("earlier {i}"),
            repo: None,
            run: None,
        })
        .collect()
}

fn loaded(st: &mut AppState, entries: Vec<ActivityEntry>) -> Vec<Effect> {
    logs(st, LogsMsg::ActivityLoaded(Ok(EarlierLog { dir: dir(), entries })))
}

/// The lines of each Append, as (ok, text); `repo` is checked in logs_tabs_tests.rs.
fn appends(fx: &[Effect]) -> Vec<Vec<(bool, String)>> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Logs(LogsEffect::Append(lines)) => Some(lines.iter().map(|l| (l.ok, l.text.clone())).collect()),
            _ => None,
        })
        .collect()
}

fn git_run(code: Option<i32>) -> GitRun {
    GitRun {
        cwd: dir().join("glowfly"),
        args: vec!["status".into(), "--porcelain=v2".into()],
        took: Duration::from_millis(34),
        code,
        ok: code.is_some(),
        timed_out: false,
    }
}

#[test]
fn start_loads_activity_and_sends_extended() {
    let mut st = state();
    let fx = logs(&mut st, LogsMsg::Start);
    assert!(matches!(fx[..], [Effect::Logs(LogsEffect::LoadActivity), Effect::Logs(LogsEffect::SetExtended(false))]), "{fx:?}");
}

#[test]
fn lines_wait_until_the_file_loaded() {
    let mut st = state();
    let fx = run(&mut st, Msg::Logged { ok: true, text: "fetched".into() });
    assert!(appends(&fx).is_empty(), "nothing is written before the file is read");
    let fx = loaded(&mut st, earlier(2));
    assert_eq!(appends(&fx), vec![vec![(true, "fetched".to_owned())]]);
    let texts: Vec<&str> = st.log.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["earlier 0", "earlier 1", "fetched"]);
    assert_eq!(st.log[0].earlier, Some(Earlier { session: 1_000, at: 1_000 }));
    assert!(st.log[1].earlier.is_some() && st.log[2].earlier.is_none());
    assert_eq!(st.logs.persist, Persist::On);
    assert_eq!(st.logs.dir, Some(dir()));
}

#[test]
fn logged_lines_are_appended_once_on() {
    let mut st = state();
    loaded(&mut st, Vec::new());
    let fx = run(&mut st, Msg::Logged { ok: false, text: "x".into() });
    assert_eq!(appends(&fx), vec![vec![(false, "x".to_owned())]]);
    assert!(st.logs.pending.is_empty());
    assert!(appends(&run(&mut st, Msg::Tick(Instant::now()))).is_empty(), "sent once");
}

#[test]
fn credentials_in_urls_never_reach_the_log_or_a_toast() {
    let mut st = state();
    loaded(&mut st, Vec::new());
    let leak = "fatal: unable to access 'https://ada:TOKEN123@example.com/o/r.git/': 403";
    let shown = "fatal: unable to access 'https://***@example.com/o/r.git/': 403";
    let fx = run(&mut st, Msg::Logged { ok: false, text: leak.into() });
    assert_eq!(appends(&fx), vec![vec![(false, shown.to_owned())]], "the file");
    assert_eq!(st.log.last().map(|e| e.text.as_str()), Some(shown), "the panel");
    let info = glowfly_core::logs::RunInfo { title: "Push".into(), summary: leak.into(), took_ms: None, routine: false };
    st.log_run_in(None, false, format!("Push FAILED\n{leak}"), info, true);
    let last = st.log.last().unwrap();
    assert!(!last.text.contains("TOKEN123") && last.run.as_ref().is_some_and(|r| r.summary == shown), "a run");
    assert!(st.logs.pending.iter().all(|l| !l.text.contains("TOKEN123") && l.run.as_ref().is_none_or(|r| r.summary == shown)));
    st.toast(false, leak, Instant::now());
    let id = st.toast_replacing(None, false, leak, Instant::now());
    st.toast_replacing(Some(id), false, leak, Instant::now());
    assert!(!st.toasts.is_empty() && st.toasts.iter().all(|t| !t.text.contains("TOKEN123")), "the toasts");
}

#[test]
fn earlier_entries_respect_log_limit() {
    let mut st = state();
    for i in 0..10 {
        st.log_quiet(true, format!("now {i}"));
    }
    loaded(&mut st, earlier(500));
    assert_eq!(st.log.len(), LOG_LIMIT);
    assert_eq!(st.log.iter().filter(|e| e.earlier.is_none()).count(), 10, "every current line is kept");
    assert_eq!(st.log[0].text, "earlier 10", "the oldest earlier entries go first");
    assert_eq!(st.log[LOG_LIMIT - 1].text, "now 9");
}

#[test]
fn clear_empties_panel_and_file() {
    let mut st = state();
    loaded(&mut st, earlier(3));
    st.log_quiet(true, "now");
    let fx = run(&mut st, Msg::ClearLog);
    assert!(st.log.is_empty(), "earlier entries go too");
    assert!(matches!(fx[..], [Effect::Logs(LogsEffect::ClearActivity { repo: None })]), "{fx:?}");

    // Before the file is read, Clear deletes it once it has been, without showing its entries.
    let mut st = state();
    st.log_quiet(true, "early");
    assert!(run(&mut st, Msg::ClearLog).is_empty());
    let fx = loaded(&mut st, earlier(3));
    assert!(matches!(fx[..], [Effect::Logs(LogsEffect::ClearActivity { repo: None })]), "{fx:?}");
    assert!(st.log.is_empty());
}

#[test]
fn load_failure_turns_persistence_off() {
    let mut st = state();
    st.log_quiet(true, "waiting");
    let fx = logs(&mut st, LogsMsg::ActivityLoaded(Err("access denied".into())));
    assert!(appends(&fx).is_empty());
    assert_eq!(st.logs.persist, Persist::Off);
    let last = st.log.last().unwrap();
    assert_eq!((last.ok, last.text.as_str()), (false, "Couldn't read the activity log from earlier sessions: access denied"));
    assert!(!st.view.show_log, "quiet");
    assert!(appends(&run(&mut st, Msg::Logged { ok: true, text: "later".into() })).is_empty());
    assert!(st.logs.pending.is_empty());
}

#[test]
fn append_failure_turns_it_off() {
    let mut st = state();
    loaded(&mut st, Vec::new());
    let fx = logs(&mut st, LogsMsg::AppendFailed("disk full".into()));
    assert!(appends(&fx).is_empty(), "the failure line itself isn't written");
    assert_eq!(st.logs.persist, Persist::Off);
    assert_eq!(st.log.last().unwrap().text, "Couldn't save the activity log: disk full. New entries are kept for this session only.");
    assert!(!st.view.show_log);
    assert!(appends(&run(&mut st, Msg::Logged { ok: false, text: "later".into() })).is_empty());
}

#[test]
fn set_extended_saves_and_tells_the_runner() {
    let mut st = state();
    let fx = logs(&mut st, LogsMsg::SetExtended(true));
    assert!(st.settings.extended_logging);
    assert!(matches!(fx[..], [Effect::SaveSettings(_), Effect::Logs(LogsEffect::SetExtended(true))]), "{fx:?}");
    assert_eq!(st.log.last().unwrap().text, "Extended logging is on: git commands and their timings are added here.");
    assert!(!st.view.show_log);
    assert!(logs(&mut st, LogsMsg::SetExtended(true)).is_empty(), "no change, no effects");
    logs(&mut st, LogsMsg::SetExtended(false));
    assert_eq!(st.log.last().unwrap().text, "Extended logging is off.");
}

#[test]
fn set_extended_from_preferences_is_saved_once() {
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let fx = logs(&mut st, LogsMsg::SetExtended(true));
    assert_eq!(fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count(), 1);
    // The frame's SettingsEdited sees nothing new to save, and keeps the setting.
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    assert!(!fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "{fx:?}");
    assert!(st.settings.extended_logging);
}

#[test]
fn git_ran_is_logged_only_when_extended() {
    let mut st = state();
    logs(&mut st, LogsMsg::GitRan(git_run(Some(0))));
    assert!(st.log.is_empty(), "ignored while extended logging is off");

    st.settings.extended_logging = true;
    logs(&mut st, LogsMsg::GitRan(git_run(Some(1))));
    let last = st.log.last().unwrap();
    assert_eq!(last.text, git_run_line(&git_run(Some(1))));
    assert!(last.ok, "a non-zero exit is still a command that ran");
    assert!(!st.view.show_log);
    logs(&mut st, LogsMsg::GitRan(GitRun { timed_out: true, ..git_run(None) }));
    assert!(!st.log.last().unwrap().ok);
    assert!(!st.view.show_log, "quiet even when it failed");
}

#[test]
fn show_error_log_reads_the_file() {
    let mut st = state();
    let fx = logs(&mut st, LogsMsg::Show(LogKind::Errors));
    assert!(st.view.show_log);
    assert_eq!(st.logs.tab, LogKind::Errors);
    let generation = st.logs.file_gen;
    assert!(matches!(fx[..], [Effect::Logs(LogsEffect::ReadFile { kind: LogKind::Errors, generation: g })] if g == generation), "{fx:?}");
    assert_eq!(st.logs.file, Some(LogFileView { kind: LogKind::Errors, tail: None }));

    let tail = LogTail { lines: vec!["old".into()], truncated: false };
    logs(&mut st, LogsMsg::FileRead { kind: LogKind::Errors, generation: generation - 1, result: Ok(tail) });
    assert_eq!(st.logs.file.as_ref().unwrap().tail, None, "an older read is ignored");
    let wrong_kind = LogsMsg::FileRead { kind: LogKind::Performance, generation, result: Ok(LogTail::default()) };
    logs(&mut st, wrong_kind);
    assert_eq!(st.logs.file.as_ref().unwrap().tail, None);
    logs(&mut st, LogsMsg::FileRead { kind: LogKind::Errors, generation, result: Err("locked".into()) });
    assert_eq!(st.logs.file.as_ref().unwrap().tail, Some(Err("locked".to_owned())));
}

#[test]
fn show_activity_reads_nothing() {
    let mut st = state();
    logs(&mut st, LogsMsg::Show(LogKind::Performance));
    let fx = logs(&mut st, LogsMsg::Show(LogKind::Activity));
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.logs.tab, LogKind::Activity);
    assert!(st.view.show_log);
}

#[test]
fn open_folder_asks_the_runner() {
    let mut st = state();
    assert!(matches!(logs(&mut st, LogsMsg::OpenFolder)[..], [Effect::Logs(LogsEffect::OpenFolder)]));
}

#[test]
fn loaded_runs_come_back() {
    let mut st = state();
    let info =
        glowfly_core::logs::RunInfo { title: "Push".into(), summary: "Everything up-to-date".into(), took_ms: Some(900), routine: false };
    let saved = ActivityEntry { session: 1_000, at: 1_042, ok: true, text: "Push done".into(), repo: None, run: Some(info.clone()) };
    let legacy = ActivityEntry { session: 1_000, at: 1_050, ok: true, text: "Fetch done\n$ git fetch".into(), repo: None, run: None };
    loaded(&mut st, vec![saved, legacy]);
    assert_eq!(st.log[0].run.as_ref(), Some(&info));
    assert_eq!(st.log[0].at, 1_042);
    assert_eq!(st.log[1].run.as_ref().map(|r| r.title.as_str()), Some("Fetch"), "an older build's line is upgraded");

    // Clear forgets the cleared rows' open state and fixes.
    let id = st.log[0].id;
    st.logs.open.insert(id);
    let fix = crate::state::ToastAction { label: "Pull".into(), msg: Box::new(Msg::git("Pull", &["pull"])), workdir: None };
    st.logs.keep_fix(id, fix);
    let other = crate::state::logs::EntryId::next();
    st.logs.open.insert(other);
    run(&mut st, Msg::ClearLog);
    assert!(!st.logs.open.contains(&id) && st.logs.fix(id).is_none());
    assert!(st.logs.open.contains(&other), "another tab's open rows stay");
}
