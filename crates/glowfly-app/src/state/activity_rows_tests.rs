use super::*;
use crate::state::logs::Earlier;
use ActivityRow::{Entry, Routine, Session};

fn run(title: &str, ok: bool) -> LogEntry {
    let status = if ok { "done" } else { "FAILED" };
    let run = RunInfo { title: title.into(), routine: is_routine_label(title), ..RunInfo::default() };
    LogEntry { run: Some(run), ..LogEntry::new(ok, format!("{title} {status}\n$ git fetch")) }
}

fn auto(ok: bool) -> LogEntry {
    run(AUTO_FETCH, ok)
}

fn note(text: &str) -> LogEntry {
    LogEntry::new(true, text)
}

fn earlier(mut e: LogEntry, session: i64) -> LogEntry {
    e.earlier = Some(Earlier { session, at: session + 5 });
    e
}

#[test]
fn rows_fold_consecutive_routine_successes() {
    let log = [note("Checked out main"), auto(true), auto(true), auto(true), run("Push", true), auto(true)];
    assert_eq!(rows(&log), [Entry(0), Routine { first: 1, last: 3, count: 3 }, Entry(4), Routine { first: 5, last: 5, count: 1 }]);
    let lfs = [auto(true), run(AUTO_FETCH_LFS, true)];
    assert_eq!(rows(&lfs), [Routine { first: 0, last: 1, count: 2 }], "the LFS download after a fetch folds in too");
}

#[test]
fn failures_notes_and_sessions_break_a_streak() {
    let log = [auto(true), auto(false), auto(true), note("Extended logging is on."), auto(true)];
    assert_eq!(
        rows(&log),
        [
            Routine { first: 0, last: 0, count: 1 },
            Entry(1),
            Routine { first: 2, last: 2, count: 1 },
            Entry(3),
            Routine { first: 4, last: 4, count: 1 }
        ]
    );
    let log = [earlier(auto(true), 10), earlier(auto(true), 20), auto(true), auto(true)];
    assert_eq!(
        rows(&log),
        [
            Session(SessionHead::Earlier(10)),
            Routine { first: 0, last: 0, count: 1 },
            Session(SessionHead::Earlier(20)),
            Routine { first: 1, last: 1, count: 1 },
            Session(SessionHead::This),
            Routine { first: 2, last: 3, count: 2 },
        ]
    );
    assert_eq!(rows(&[note("a"), note("b")]), [Entry(0), Entry(1)], "no header without earlier sessions");
    assert!(rows(&[]).is_empty());
}

#[test]
fn cap_routine_keeps_a_streak_short_and_counts_the_rest() {
    let mut log = vec![note("first")];
    log.extend((0..40).map(|_| auto(true)));
    let (first_id, last_id) = (log[1].id, log[40].id);
    log.push(run("Push", true));
    cap_routine(&mut log);
    assert_eq!(log.len(), 1 + ROUTINE_KEEP + 1);
    assert_eq!(log[1].id, first_id, "the group keeps its first entry");
    assert_eq!(log[ROUTINE_KEEP].id, last_id, "and the newest");
    assert_eq!(log[1].folded, 10);
    assert_eq!(rows(&log), [Entry(0), Routine { first: 1, last: ROUTINE_KEEP, count: 40 }, Entry(ROUTINE_KEEP + 1)]);
    // Capping again, as every new line does, adds the next one dropped to the count.
    log.insert(ROUTINE_KEEP + 1, auto(true));
    cap_routine(&mut log);
    assert_eq!(log[1].folded, 11);
    assert_eq!(rows(&log)[1], Routine { first: 1, last: ROUTINE_KEEP, count: 41 });
}

#[test]
fn legacy_run_reads_older_plain_lines() {
    let fetch = legacy_run("Fetch done\n$ git fetch", true).expect("a run");
    assert_eq!(fetch, RunInfo { title: "Fetch".into(), summary: String::new(), took_ms: None, routine: false });
    let auto = legacy_run("Auto-fetch done\n$ git fetch --all\nFrom origin", true).expect("a run");
    assert!(auto.routine);
    assert_eq!(auto.summary, "From origin");
    let push = legacy_run("Push FAILED\n$ git push\nerror: failed to push some refs", false).expect("a run");
    assert_eq!((push.title.as_str(), push.summary.as_str()), ("Push", "error: failed to push some refs"));
    assert_eq!(legacy_run("Push FAILED\n$ git push", true), None, "the status must match");
    assert_eq!(legacy_run("Checked out main", true), None);
    assert_eq!(legacy_run("Rebase done", true), None, "a single line stays a note");
    assert_eq!(legacy_run(" done\nx", true), None, "no label");
}

#[test]
fn what_a_row_shows() {
    let mut push = run("Push", true);
    push.text = "Push done\n$ git push\nEverything up-to-date".into();
    push.run.as_mut().unwrap().summary = "Everything up-to-date".into();
    assert_eq!(title_of(&push), "Push");
    assert_eq!(output_of(&push), "$ git push\nEverything up-to-date");
    assert!(foldable(&push));
    let one = note("Another operation is still running.");
    assert_eq!((title_of(&one), output_of(&one), foldable(&one)), ("Another operation is still running.", "", false));
    let two = note("Couldn't save:\npermission denied\n");
    assert_eq!((title_of(&two), output_of(&two), foldable(&two)), ("Couldn't save:", "permission denied\n", true));
    assert!(!foldable(&note("One line\n\n")), "blank lines don't count");
}
