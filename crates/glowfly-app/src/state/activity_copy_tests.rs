use super::*;
use crate::state::logs::{Earlier, LogFileView};
use crate::state::notify::AUTO_FETCH;
use glowfly_core::logs::RunInfo;
use glowfly_core::settings::Settings;
use std::time::Instant;

/// 2026-03-04 05:06:07 UTC.
const AT: i64 = 1_772_600_767;
const EN: Lang = Lang::En;

fn run(title: &str, ok: bool, summary: &str, took_ms: Option<u64>, output: &str) -> LogEntry {
    let status = if ok { "done" } else { "FAILED" };
    let info = RunInfo { title: title.into(), summary: summary.into(), took_ms, routine: title == AUTO_FETCH };
    LogEntry { at: AT, run: Some(info), ..LogEntry::new(ok, format!("{title} {status}\n{output}")) }
}

fn note(ok: bool, text: &str) -> LogEntry {
    LogEntry { at: AT, ..LogEntry::new(ok, text) }
}

fn auto(at: i64) -> LogEntry {
    LogEntry { at, ..run(AUTO_FETCH, true, "", None, "$ git fetch") }
}

fn state() -> AppState {
    AppState::new(Settings::default(), Vec::new(), Instant::now())
}

#[test]
fn entry_text_for_runs_and_notes() {
    let push = run("Push", true, "Everything up-to-date", Some(1250), "$ git push\nEverything up-to-date\n");
    assert_eq!(
        entry_text(&push, EN, 0),
        "Push · done · 2026-03-04 05:06:07 +00:00 · 1.2 s\nEverything up-to-date\n$ git push\nEverything up-to-date"
    );
    let failed = run("Pull", false, "", None, "$ git pull");
    assert_eq!(entry_text(&failed, EN, 3600), "Pull · failed · 2026-03-04 06:06:07 +01:00\n$ git pull");
    assert_eq!(entry_text(&note(true, "Checked out main"), EN, 0), "Checked out main · 2026-03-04 05:06:07 +00:00");
    let broken = note(false, "Couldn't save:\npermission denied");
    assert_eq!(entry_text(&broken, EN, 0), "Couldn't save: · failed · 2026-03-04 05:06:07 +00:00\npermission denied");
    let progress = run("Fetch", true, "", None, "$ git fetch\nReceiving 10%\rReceiving 100%\r\n\n\n");
    assert_eq!(entry_text(&progress, EN, 0), "Fetch · done · 2026-03-04 05:06:07 +00:00\n$ git fetch\nReceiving 100%");
    let carriage = run("Fetch", true, "", None, "a\rb");
    assert!(entry_text(&carriage, EN, 0).ends_with("\nb"));
    let long: Vec<String> = (0..250).map(|i| format!("line {i}")).collect();
    let big = run("Fetch", true, "", None, &long.join("\n"));
    let text = entry_text(&big, EN, 0);
    assert_eq!(text.lines().count(), 251, "every output line, past the panel's cap");
    assert!(text.contains("\nline 0\n") && text.ends_with("line 249"));
}

#[test]
fn entry_copy_of_a_routine_group() {
    let mut log = vec![note(true, "Checked out main"), auto(AT + 60), auto(AT + 120), run("Push", true, "", None, "$ git push")];
    let one = |at: i64| entry_text(&auto(at), EN, 0);
    let both = format!("{}\n\n{}", one(AT + 60), one(AT + 120));
    assert_eq!(entry_copy(&log, log[1].id, EN, 0).as_deref(), Some(both.as_str()), "the group's first id copies all of it");
    assert_eq!(entry_copy(&log, log[3].id, EN, 0), Some(entry_text(&log[3], EN, 0)));
    assert_eq!(entry_copy(&log, crate::state::logs::EntryId::next(), EN, 0), None);
    log[1].folded = 3;
    let text = entry_copy(&log, log[1].id, EN, 0).unwrap();
    assert!(text.ends_with(&format!("{}\n(and 3 earlier runs not kept)", one(AT + 120))), "{text}");
    log[1].folded = 1;
    assert!(entry_copy(&log, log[1].id, EN, 0).unwrap().ends_with("\n(and 1 earlier run not kept)"));
}

#[test]
fn activity_text_has_session_heads() {
    let mut old = note(true, "Fetched origin");
    old.earlier = Some(Earlier { session: AT - 100, at: AT });
    let log = vec![old, note(true, "Checked out main")];
    assert_eq!(
        activity_text(&log, EN, 0),
        "Earlier session · 2026-03-04 05:04:27 +00:00\n\n\
         Fetched origin · 2026-03-04 05:06:07 +00:00\n\n\
         This session\n\n\
         Checked out main · 2026-03-04 05:06:07 +00:00"
    );
    assert_eq!(activity_text(&log[1..], EN, 0), "Checked out main · 2026-03-04 05:06:07 +00:00", "no head without earlier sessions");
}

#[test]
fn file_text_and_blocked_reasons() {
    let lines = vec!["one".to_owned(), "two".to_owned()];
    let cut = LogTail { lines: lines.clone(), truncated: true };
    assert_eq!(file_text(&cut, EN), "(Earlier lines of this file are left out. Export logs has all of it.)\none\ntwo");
    assert_eq!(file_text(&LogTail { lines, truncated: false }, EN), "one\ntwo");

    let empty = Some("There is nothing in this log yet.");
    let reading = Some("The log is still being read.");
    let mut st = state();
    st.log.clear();
    assert_eq!(copy_log_blocked(&st), empty);
    assert_eq!(log_text(&st), None);
    st.log.push(note(true, "Checked out main"));
    assert_eq!(copy_log_blocked(&st), None);
    assert_eq!(log_text(&st).map(|(_, n)| n), Some(1));

    st.logs.tab = LogKind::Errors;
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: None });
    assert_eq!(copy_log_blocked(&st), reading);
    st.logs.file = Some(LogFileView { kind: LogKind::Performance, tail: Some(Ok(cut.clone())) });
    assert_eq!(copy_log_blocked(&st), reading, "a view of another kind is still being replaced");
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: Some(Err("denied".into())) });
    assert_eq!(copy_log_blocked(&st), empty);
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: Some(Ok(LogTail::default())) });
    assert_eq!(copy_log_blocked(&st), empty);
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: Some(Ok(cut.clone())) });
    assert_eq!(copy_log_blocked(&st), None);
    assert_eq!(log_text(&st), Some((file_text(&cut, EN), 2)));
}
