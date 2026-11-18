use super::*;
use crate::state::activity_rows::is_routine_label;
use crate::state::logs::Earlier;
use crate::state::notify::AUTO_FETCH;
use ActivityRow::{Entry, Routine, Session};
use glowfly_core::logs::RunInfo;

fn run(title: &str, ok: bool, summary: &str, output: &str) -> LogEntry {
    let status = if ok { "done" } else { "FAILED" };
    let info = RunInfo { title: title.into(), summary: summary.into(), routine: is_routine_label(title), ..RunInfo::default() };
    LogEntry { run: Some(info), ..LogEntry::new(ok, format!("{title} {status}\n{output}")) }
}

fn auto(output: &str) -> LogEntry {
    run(AUTO_FETCH, true, "", output)
}

fn note(ok: bool, text: &str) -> LogEntry {
    LogEntry::new(ok, text)
}

fn earlier(mut e: LogEntry, session: i64) -> LogEntry {
    e.earlier = Some(Earlier { session, at: session + 5 });
    e
}

fn f(failures: bool, search: &str) -> Filter {
    Filter::new(failures, search)
}

#[test]
fn no_filter_gives_the_plain_rows() {
    let log = [note(true, "Checked out"), auto("a"), auto("b"), run("Push", true, "", "x")];
    let plain = rows(&log);
    assert!(plain.iter().any(|r| matches!(r, Routine { .. })));
    for search in ["", "   ", "\t"] {
        let filter = f(false, search);
        assert!(!filter.is_active());
        assert_eq!(filtered_rows(&log, &filter), plain);
    }
}

#[test]
fn failures_only_keeps_failed_runs_and_failed_notes() {
    let mut cancelled = run("Pull", true, "Cancelled", "");
    cancelled.text = "Pull done\nCancelled".into();
    let log = [
        run("Push", true, "Pushed", "ok"),
        run("Pull", false, "Rejected", "fatal: no"),
        cancelled,
        auto("x"),
        run(AUTO_FETCH, false, "No route", "fatal: x"),
        note(false, "Couldn't save"),
        note(true, "Fine"),
    ];
    assert_eq!(filtered_rows(&log, &f(true, "")), [Entry(1), Entry(4), Entry(5)]);
}

#[test]
fn search_matches_title_cause_and_output_ignoring_case() {
    let log = [
        run("Fetch ALL", true, "", ""),
        run("Push", true, "Nothing Pushed Because", ""),
        run("Pull", true, "", "remote: Counting objects"),
        run("Merge", true, "", "plain"),
        note(true, "Selected Branch\nsecond line mentions Needle"),
    ];
    assert_eq!(filtered_rows(&log, &f(false, "fetch all")), [Entry(0)]);
    assert_eq!(filtered_rows(&log, &f(false, "  BECAUSE ")), [Entry(1)]);
    assert_eq!(filtered_rows(&log, &f(false, "counting")), [Entry(2)]);
    assert_eq!(filtered_rows(&log, &f(false, "selected branch")), [Entry(4)]);
    assert_eq!(filtered_rows(&log, &f(false, "needle")), [Entry(4)], "a note's later lines");
    // The stored status words are not searched.
    assert!(filtered_rows(&log, &f(false, "done")).is_empty());
    let failed = [run("Push", false, "", "")];
    assert!(filtered_rows(&failed, &f(false, "failed")).is_empty());
}

#[test]
fn search_reads_output_as_shown() {
    let log = [run("Clone", true, "", "Receiving 10%\rReceiving 100%, done.")];
    assert!(filtered_rows(&log, &f(false, "receiving 10%")).is_empty(), "overwritten by the redraw");
    assert_eq!(filtered_rows(&log, &f(false, "100%, done")), [Entry(0)]);
}

#[test]
fn a_search_lists_matching_routine_runs_as_their_own_rows() {
    let log = [auto("one"), auto("two"), auto("needle here"), auto("four"), auto("five")];
    assert!(matches!(rows(&log)[0], Routine { count: 5, .. }));
    assert_eq!(filtered_rows(&log, &f(false, "needle")), [Entry(2)]);
}

#[test]
fn headers_only_stand_over_shown_rows() {
    let log = [
        earlier(note(true, "alpha one"), 10),
        earlier(note(false, "beta one"), 20),
        earlier(note(true, "alpha two"), 20),
        note(true, "alpha three"),
        note(false, "beta two"),
    ];
    assert_eq!(
        filtered_rows(&log, &f(false, "alpha")),
        [Session(SessionHead::Earlier(10)), Entry(0), Session(SessionHead::Earlier(20)), Entry(2), Session(SessionHead::This), Entry(3)]
    );
    // The earlier session with no match has no header.
    assert_eq!(filtered_rows(&log, &f(false, "three")), [Entry(3)], "no Earlier header, no This header");
    assert_eq!(filtered_rows(&log, &f(false, "beta")), [Session(SessionHead::Earlier(20)), Entry(1), Session(SessionHead::This), Entry(4)]);
    assert_eq!(filtered_rows(&log, &f(true, "beta two")), [Entry(4)]);
    assert_eq!(filtered_rows(&log, &f(true, "alpha")), Vec::<ActivityRow>::new());
    assert_eq!(filtered_rows(&log, &f(true, "")), [Session(SessionHead::Earlier(20)), Entry(1), Session(SessionHead::This), Entry(4)]);
}

#[test]
fn an_output_match_unfolds_until_folded() {
    let hit = run("Pull", true, "", "line\nneedle line");
    let title_only = run("Needle job", true, "", "nothing");
    let mut logs = LogsState::default();
    let filter = f(false, "needle");
    assert!(is_open(&logs, &hit, &filter));
    assert!(!is_open(&logs, &title_only, &filter));
    assert!(!is_open(&logs, &hit, &f(false, "")));
    logs.search_closed.insert(hit.id);
    assert!(!is_open(&logs, &hit, &filter));
    logs.open.insert(hit.id);
    assert!(is_open(&logs, &hit, &filter), "any id in open is open");
    logs.open.insert(title_only.id);
    assert!(is_open(&logs, &title_only, &filter));
}

#[test]
fn row_ids_skip_headers_and_count_a_group_once() {
    let log = [earlier(auto("a"), 10), earlier(auto("b"), 10), earlier(note(true, "nnn"), 10), note(true, "m"), auto("c")];
    let ids = row_ids(&log, &f(false, ""));
    assert_eq!(ids, [log[0].id, log[2].id, log[3].id, log[4].id]);
    assert_eq!(row_ids(&log, &f(false, "nnn")), [log[2].id]);
}

#[test]
fn matching_lines_filters_a_tail() {
    let lines: Vec<String> = ["INFO start", "WARN Theme missing", "ERROR boom"].map(String::from).into();
    assert_eq!(matching_lines(&lines, ""), [0, 1, 2]);
    assert_eq!(matching_lines(&lines, "theme"), [1]);
    assert!(matching_lines(&lines, "zzz").is_empty());
}
