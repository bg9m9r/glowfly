//! The activity panel's rows, worked out from a tab's log: session headers, one row per run or
//! note, and consecutive successful auto-fetches (routine runs) folded into one group. Also what
//! a row shows and copies, and the cap that keeps a long streak of routine runs from pushing
//! everything else out of the log. Pure; ui/activity_rows.rs draws the rows.

use super::logs::LogEntry;
use super::notify::{AUTO_FETCH, AUTO_FETCH_LFS};
use glowfly_core::logs::{RunInfo, last_message};

/// The most entries a streak of routine successes keeps; older ones are counted by the first.
pub const ROUTINE_KEEP: usize = 30;

/// Whether a job labelled `label` is a background auto-fetch.
pub fn is_routine_label(label: &str) -> bool {
    label == AUTO_FETCH || label == AUTO_FETCH_LFS
}

/// Whether `e` is a successful routine run, which folds with its neighbours.
pub fn routine_ok(e: &LogEntry) -> bool {
    e.ok && e.run.as_ref().is_some_and(|r| r.routine)
}

/// The run an older build's plain line records: its first line is "<label> done" (`ok`) or
/// "<label> FAILED" (not `ok`) followed by the job's output. None for anything else, which
/// stays a note.
pub fn legacy_run(text: &str, ok: bool) -> Option<RunInfo> {
    let (first, rest) = text.split_once('\n')?;
    let label = first.strip_suffix(if ok { " done" } else { " FAILED" })?;
    if label.trim().is_empty() {
        return None;
    }
    Some(RunInfo { title: label.to_owned(), summary: last_message(rest), took_ms: None, routine: is_routine_label(label) })
}

/// The session `e` belongs to: an earlier one's start, or None for this session.
fn session(e: &LogEntry) -> Option<i64> {
    e.earlier.map(|x| x.session)
}

/// The end (exclusive) of the streak of routine successes starting at `start`, one session's.
fn streak_end(log: &[LogEntry], start: usize) -> usize {
    let key = session(&log[start]);
    start + log[start..].iter().take_while(|e| routine_ok(e) && session(e) == key).count()
}

/// Keeps every streak of routine successes to `ROUTINE_KEEP` entries: the oldest after the
/// first go, and the first counts them in `folded`, so the group keeps its id, start time and
/// count.
pub fn cap_routine(log: &mut Vec<LogEntry>) {
    let mut i = 0;
    while i < log.len() {
        if !routine_ok(&log[i]) {
            i += 1;
            continue;
        }
        let mut end = streak_end(log, i);
        let excess = (end - i).saturating_sub(ROUTINE_KEEP);
        if excess > 0 {
            let dropped: u32 = log.drain(i + 1..i + 1 + excess).map(|e| 1 + e.folded).sum();
            log[i].folded += dropped;
            end -= excess;
        }
        i = end;
    }
}

/// A session header: an earlier session (by its start) or this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionHead {
    Earlier(i64),
    This,
}

/// One row of the activity panel, by index into the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityRow {
    Session(SessionHead),
    /// A run or a note.
    Entry(usize),
    /// Routine successes `first..=last`, which stand for `count` runs.
    Routine {
        first: usize,
        last: usize,
        count: u32,
    },
}

/// The rows of `log`: each earlier session's entries under its header, then this session's
/// under "This session" (only when there are earlier ones), with every streak of routine
/// successes as one `Routine` row.
pub fn rows(log: &[LogEntry]) -> Vec<ActivityRow> {
    let any_earlier = log.first().is_some_and(|e| e.earlier.is_some());
    let mut out = Vec::new();
    let mut shown = None;
    let mut current = false;
    let mut i = 0;
    while i < log.len() {
        match session(&log[i]) {
            Some(s) if shown != Some(s) => {
                shown = Some(s);
                out.push(ActivityRow::Session(SessionHead::Earlier(s)));
            }
            None if any_earlier && !current => {
                current = true;
                out.push(ActivityRow::Session(SessionHead::This));
            }
            _ => {}
        }
        if routine_ok(&log[i]) {
            let end = streak_end(log, i);
            let count = log[i..end].iter().map(|e| 1 + e.folded).sum();
            out.push(ActivityRow::Routine { first: i, last: end - 1, count });
            i = end;
        } else {
            out.push(ActivityRow::Entry(i));
            i += 1;
        }
    }
    out
}

/// What a row folds open: the text after the first line (a run's output, a note's details).
pub fn output_of(e: &LogEntry) -> &str {
    e.text.split_once('\n').map_or("", |(_, rest)| rest)
}

/// A row's title: the run's label, or the note's first line.
pub fn title_of(e: &LogEntry) -> &str {
    match &e.run {
        Some(r) => &r.title,
        None => e.text.lines().next().unwrap_or_default(),
    }
}

/// Whether the row folds open: a run, or a note of more than one line.
pub fn foldable(e: &LogEntry) -> bool {
    e.run.is_some() || e.text.lines().filter(|l| !l.trim().is_empty()).count() > 1
}

/// What Copy puts on the clipboard: "<title>: <summary>" and the whole output for a run, the
/// text for a note.
pub fn copy_text(e: &LogEntry) -> String {
    match &e.run {
        Some(r) if r.summary.is_empty() => format!("{}\n{}", r.title, output_of(e)),
        Some(r) => format!("{}: {}\n{}", r.title, r.summary, output_of(e)),
        None => e.text.clone(),
    }
}

#[cfg(test)]
#[path = "activity_rows_tests.rs"]
mod tests;
