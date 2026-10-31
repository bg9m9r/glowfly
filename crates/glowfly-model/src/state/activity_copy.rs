//! What the logs panel's Copy entry and Copy log put on the clipboard: plain text with LF line
//! ends, times as "YYYY-MM-DD HH:MM:SS +HH:MM" whatever the date-format setting, so a bug report
//! reads the same everywhere. Pure; update/logs_copy.rs copies it.

use super::AppState;
use super::activity_rows::{ActivityRow, SessionHead, output_of, routine_ok, rows, title_of};
use super::logs::{EntryId, LogEntry};
use crate::logs::LogKind;
use glowfly_core::i18n::Lang;
use glowfly_core::logs::{LogTail, format_took, output_lines, stamp};
use std::time::Duration;

/// One entry: "<title> · <result> · <time>[ · <duration>]", the run's summary on its own line,
/// then the whole output as the panel shows it (progress redraws collapsed, blank lines at the
/// end left out). A note names its result only when it failed.
pub fn entry_text(e: &LogEntry, t: Lang, utc_offset: i32) -> String {
    let mut head = vec![title_of(e).to_owned()];
    if e.run.is_some() || !e.ok {
        head.push(if e.ok { t.tr("logs.activity.done") } else { t.tr("logs.activity.failed") }.to_owned());
    }
    head.push(stamp(e.at, utc_offset));
    if let Some(ms) = e.run.as_ref().and_then(|r| r.took_ms) {
        head.push(format_took(Duration::from_millis(ms)));
    }
    let mut lines = vec![head.join(" · ")];
    if let Some(summary) = e.run.as_ref().map(|r| r.summary.as_str()).filter(|s| !s.is_empty()) {
        lines.push(summary.to_owned());
    }
    let mut output: Vec<&str> = output_lines(output_of(e)).collect();
    while output.last().is_some_and(|l| l.trim().is_empty()) {
        output.pop();
    }
    lines.extend(output.into_iter().map(str::to_owned));
    lines.join("\n")
}

/// A routine group: each run's block, oldest first, with a blank line between them, and a note
/// of the runs the routine cap dropped.
fn group_text(group: &[LogEntry], t: Lang, utc_offset: i32) -> String {
    let mut blocks: Vec<String> = group.iter().map(|e| entry_text(e, t, utc_offset)).collect();
    if let Some(folded) = group.first().map(|e| e.folded).filter(|n| *n > 0)
        && let Some(last) = blocks.last_mut()
    {
        last.push('\n');
        last.push_str(&t.trn("logs.copy.folded", folded as usize, &[]));
    }
    blocks.join("\n\n")
}

/// What Copy entry copies for `id`: its block, or the whole group when it is a routine group's
/// first entry. None when it is no longer in `log`.
pub fn entry_copy(log: &[LogEntry], id: EntryId, t: Lang, utc_offset: i32) -> Option<String> {
    let i = log.iter().position(|e| e.id == id)?;
    if routine_ok(&log[i]) {
        let group = rows(log).into_iter().find_map(|r| match r {
            ActivityRow::Routine { first, last, .. } if first == i => Some(last),
            _ => None,
        });
        if let Some(last) = group {
            return Some(group_text(&log[i..=last], t, utc_offset));
        }
    }
    Some(entry_text(&log[i], t, utc_offset))
}

/// Everything a tab's log shows, oldest first: session headers as a line, each entry and group
/// as its block, with a blank line between them.
pub fn activity_text(log: &[LogEntry], t: Lang, utc_offset: i32) -> String {
    let blocks: Vec<String> = rows(log)
        .into_iter()
        .map(|row| match row {
            ActivityRow::Session(SessionHead::Earlier(s)) => t.trf("logs.activity.earlier_session", &[("date", &stamp(s, utc_offset))]),
            ActivityRow::Session(SessionHead::This) => t.tr("logs.activity.this_session").to_owned(),
            ActivityRow::Entry(i) => entry_text(&log[i], t, utc_offset),
            ActivityRow::Routine { first, last, .. } => group_text(&log[first..=last], t, utc_offset),
        })
        .collect();
    blocks.join("\n\n")
}

/// An error or performance log's lines as its tab shows them, after a note when the start of
/// the file was left out.
pub fn file_text(tail: &LogTail, t: Lang) -> String {
    let cut = tail.truncated.then(|| t.tr("logs.copy.cut"));
    cut.into_iter().chain(tail.lines.iter().map(String::as_str)).collect::<Vec<_>>().join("\n")
}

/// The tail the current file tab shows: None while it is read; Err for a failed read.
fn shown_tail(st: &AppState) -> Option<&Result<LogTail, String>> {
    st.logs.file.as_ref().filter(|v| v.kind == st.logs.tab).and_then(|v| v.tail.as_ref())
}

/// Why Copy log can't copy the current tab yet; None when it can.
pub fn copy_log_blocked(st: &AppState) -> Option<&'static str> {
    let t = st.settings.lang();
    let empty = Some(t.tr("logs.header.copy_log.empty"));
    match st.logs.tab {
        LogKind::Activity if st.log.is_empty() => empty,
        LogKind::Activity => None,
        _ => match shown_tail(st) {
            None => Some(t.tr("logs.header.copy_log.reading")),
            Some(Err(_)) => empty,
            Some(Ok(tail)) if tail.lines.is_empty() => empty,
            Some(Ok(_)) => None,
        },
    }
}

/// What Copy log copies for the current tab, and how many entries or lines it holds. None when
/// `copy_log_blocked`.
pub fn log_text(st: &AppState) -> Option<(String, usize)> {
    if copy_log_blocked(st).is_some() {
        return None;
    }
    let t = st.settings.lang();
    match st.logs.tab {
        LogKind::Activity => Some((activity_text(&st.log, t, st.view.utc_offset), st.log.len())),
        _ => match shown_tail(st)? {
            Ok(tail) => Some((file_text(tail, t), tail.lines.len())),
            Err(_) => None,
        },
    }
}

#[cfg(test)]
#[path = "activity_copy_tests.rs"]
mod tests;
