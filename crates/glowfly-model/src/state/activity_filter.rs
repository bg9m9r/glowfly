//! What the activity panel's Failures only toggle and search field leave of a tab's log: which
//! rows show (with either on there are no routine groups, every kept entry is its own row) and
//! whether an entry's row is drawn open (folded open by hand, or unfolded because the search
//! found its text in the output). Pure; ui/activity_rows.rs draws the rows and update/
//! logs_filter.rs acts on them, both through this file. It also counts what a filter leaves
//! (the header's "3 of 120", Clear's confirmation) and finds the entry a search step reaches.

use super::activity_rows::{ActivityRow, SessionHead, output_of, rows, title_of};
use super::logs::{EntryId, LogEntry, LogsState};
use glowfly_core::logs::output_lines;
use glowfly_core::matching::contains_lower;

/// The panel's two filters: Failures only and the lowercased, trimmed search text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub failures_only: bool,
    needle: String,
}

impl Filter {
    /// The filter for the toggle and the search field's text.
    pub fn new(failures_only: bool, search: &str) -> Filter {
        Filter { failures_only, needle: search.trim().to_lowercase() }
    }

    /// The text to look for: lowercase, trimmed, empty when there is no search.
    pub fn needle(&self) -> &str {
        &self.needle
    }

    /// Whether either filter is on.
    pub fn is_active(&self) -> bool {
        self.failures_only || !self.needle.is_empty()
    }

    /// Whether the search text is in a line of the entry's output, as the open row shows it.
    fn in_output(&self, e: &LogEntry) -> bool {
        output_lines(output_of(e)).any(|l| contains_lower(l, &self.needle))
    }

    /// Whether the entry passes both filters.
    pub(crate) fn keeps(&self, e: &LogEntry) -> bool {
        if self.failures_only && e.ok {
            return false;
        }
        self.needle.is_empty()
            || contains_lower(title_of(e), &self.needle)
            || e.run.as_ref().is_some_and(|r| contains_lower(&r.summary, &self.needle))
            || self.in_output(e)
    }

    /// Whether the search unfolds the entry's row: its text is in the output.
    pub(crate) fn unfolds(&self, e: &LogEntry) -> bool {
        !self.needle.is_empty() && self.in_output(e)
    }
}

/// Whether the row of entry `e` is drawn open: folded open by hand, or unfolded by the search
/// and not folded again since.
pub fn is_open(logs: &LogsState, e: &LogEntry, f: &Filter) -> bool {
    logs.open.contains(&e.id) || (f.unfolds(e) && !logs.search_closed.contains(&e.id))
}

/// The rows of `log` under `f`: all of `activity_rows::rows` with no filter; else only the kept
/// entries, each its own row (a matching auto-fetch is a normal run), with a session header only
/// above a shown row, and "This session" only under a shown earlier session's.
pub fn filtered_rows(log: &[LogEntry], f: &Filter) -> Vec<ActivityRow> {
    if !f.is_active() {
        return rows(log);
    }
    let mut out = Vec::new();
    let mut head = None;
    let mut earlier_shown = false;
    let mut this_shown = false;
    for (i, e) in log.iter().enumerate().filter(|(_, e)| f.keeps(e)) {
        match e.earlier.map(|x| x.session) {
            Some(s) if head != Some(s) => {
                head = Some(s);
                earlier_shown = true;
                out.push(ActivityRow::Session(SessionHead::Earlier(s)));
            }
            None if earlier_shown && !this_shown => {
                this_shown = true;
                out.push(ActivityRow::Session(SessionHead::This));
            }
            _ => {}
        }
        out.push(ActivityRow::Entry(i));
    }
    out
}

/// The ids of the rows the keyboard cursor stops on, in order: a run or note, or a routine
/// group by its first entry. Headers are skipped.
pub fn row_ids(log: &[LogEntry], f: &Filter) -> Vec<EntryId> {
    filtered_rows(log, f)
        .into_iter()
        .filter_map(|r| match r {
            ActivityRow::Entry(i) | ActivityRow::Routine { first: i, .. } => Some(log[i].id),
            ActivityRow::Session(_) => None,
        })
        .collect()
}

/// The indexes of the `lines` that contain `needle` (lowercase), all of them with no needle.
pub fn matching_lines(lines: &[String], needle: &str) -> Vec<usize> {
    (0..lines.len()).filter(|&i| contains_lower(&lines[i], needle)).collect()
}

/// How many entries or lines a filter leaves of all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchCount {
    pub shown: usize,
    pub total: usize,
}

impl MatchCount {
    /// How many the filter leaves out.
    pub fn hidden(&self) -> usize {
        self.total - self.shown
    }
}

/// The Activity count: None while no filter is on. With a filter on every kept entry is one row.
pub fn match_count(log: &[LogEntry], f: &Filter) -> Option<MatchCount> {
    f.is_active().then(|| MatchCount { shown: log.iter().filter(|e| f.keeps(e)).count(), total: log.len() })
}

/// A log tail's count for the lowercase `needle`: None for no needle.
pub fn line_count(lines: &[String], needle: &str) -> Option<MatchCount> {
    (!needle.is_empty()).then(|| MatchCount { shown: matching_lines(lines, needle).len(), total: lines.len() })
}

/// The 1-based place of entry `id` among the shown entries; None when it isn't shown or no filter
/// is on.
pub fn place(log: &[LogEntry], f: &Filter, id: EntryId) -> Option<usize> {
    if !f.is_active() {
        return None;
    }
    let mut n = 0;
    for e in log.iter().filter(|e| f.keeps(e)) {
        n += 1;
        if e.id == id {
            return Some(n);
        }
    }
    None
}

/// The shown entry a step from the cursor `from` reaches, wrapping at the ends; `back` steps to
/// the previous one. A cursor on an entry the filter hides steps from its place in the log. None
/// when nothing shows or no filter is on.
pub fn step_from(log: &[LogEntry], f: &Filter, from: Option<EntryId>, back: bool) -> Option<EntryId> {
    if !f.is_active() {
        return None;
    }
    let shown: Vec<usize> = (0..log.len()).filter(|&i| f.keeps(&log[i])).collect();
    let n = shown.len();
    let last = *shown.last()?;
    let pos = from.and_then(|id| log.iter().position(|e| e.id == id));
    let to = match pos {
        None => {
            if back {
                last
            } else {
                shown[0]
            }
        }
        Some(p) => match shown.iter().position(|&i| i == p) {
            Some(k) => shown[if back { (k + n - 1) % n } else { (k + 1) % n }],
            None if back => shown.iter().rev().find(|&&i| i < p).copied().unwrap_or(last),
            None => shown.iter().find(|&&i| i > p).copied().unwrap_or(shown[0]),
        },
    };
    Some(log[to].id)
}

#[cfg(test)]
#[path = "activity_filter_tests.rs"]
mod tests;
