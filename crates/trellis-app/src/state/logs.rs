//! The activity logs in memory, one per tab, and what the logs panel shows: earlier sessions'
//! entries, the lines waiting to be written to the activity file, the failure waiting to be
//! announced, the open tab and the error or performance log read for it.
//!
//! The active tab's log is `AppState::log`; every other tab keeps its own in `Tab::log`
//! (update/tabs.rs swaps them, update/logs_tabs.rs hands out earlier entries).

use super::AppState;
use super::tabs::TabId;
use crate::logs::{LogKind, NewLine};
use std::path::{Path, PathBuf};
use trellis_core::logs::{ActivityEntry, LogTail};

/// The most entries a tab's activity log keeps (earlier sessions' included), and the activity
/// file for all repositories together.
pub const LOG_LIMIT: usize = 500;

/// When an entry from an earlier session was written (Unix seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Earlier {
    /// When its session started.
    pub session: i64,
    pub at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub ok: bool,
    pub text: String,
    /// Set for entries from an earlier session, read from the activity file.
    pub earlier: Option<Earlier>,
}

impl LogEntry {
    pub fn new(ok: bool, text: impl Into<String>) -> Self {
        Self { ok, text: text.into(), earlier: None }
    }

    pub fn earlier(e: ActivityEntry) -> Self {
        Self { ok: e.ok, text: e.text, earlier: Some(Earlier { session: e.session, at: e.at }) }
    }
}

/// Whether new activity lines are written to the activity file.
#[derive(Default, PartialEq, Debug, Clone, Copy)]
pub enum Persist {
    /// Until the earlier sessions' entries are read: lines wait in `pending`.
    #[default]
    Waiting,
    On,
    /// The file couldn't be read or written; the log is kept for this session only.
    Off,
}

/// The error or performance log read for its tab; `tail` is None while it is read.
#[derive(Clone, Debug, PartialEq)]
pub struct LogFileView {
    pub kind: LogKind,
    pub tail: Option<Result<LogTail, String>>,
}

/// A failure to announce at the end of `update` (update/logs_tabs.rs).
#[derive(Clone, Debug, PartialEq)]
pub struct Announce {
    /// The tab whose log has the line; None when no tab was active.
    pub tab: Option<TabId>,
    /// The toast's text: the line's first line, shortened.
    pub text: String,
}

/// The longest headline a failure toast shows, in characters.
const HEADLINE_CHARS: usize = 200;

#[derive(Default)]
pub struct LogsState {
    pub persist: Persist,
    /// Lines waiting to be written to the activity file (at most `LOG_LIMIT`), flushed at the
    /// end of `update` once the file has loaded.
    pub pending: Vec<NewLine>,
    /// The activity panel's tab.
    pub tab: LogKind,
    pub file: Option<LogFileView>,
    /// Incremented for every file read; older `FileRead` results are ignored.
    pub file_gen: u64,
    /// The logs folder, once the runner has reported it.
    pub dir: Option<PathBuf>,
    /// The repositories whose tabs were cleared before the activity file was read (None: a tab
    /// without one): their lines are removed from it once it has been.
    pub clear_on_load: Vec<Option<PathBuf>>,
    /// Earlier sessions' entries for repositories no tab showed when the file was read, given
    /// to the tab that opens one later in the session.
    pub unclaimed: Vec<ActivityEntry>,
    /// The first failure logged loudly since the last announcement.
    pub announce: Option<Announce>,
}

impl LogsState {
    /// The lines to append to the activity file now: Some only while persistence is on and
    /// lines are waiting.
    pub fn take_append(&mut self) -> Option<Vec<NewLine>> {
        (self.persist == Persist::On && !self.pending.is_empty()).then(|| std::mem::take(&mut self.pending))
    }
}

/// Drops all but the newest `LOG_LIMIT` entries of `log`.
pub fn keep_newest<T>(log: &mut Vec<T>) {
    if log.len() > LOG_LIMIT {
        log.drain(..log.len() - LOG_LIMIT);
    }
}

/// What a failure toast shows of `text`: its first non-empty line, at most `HEADLINE_CHARS`
/// characters (then "...").
pub fn headline(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    match line.char_indices().nth(HEADLINE_CHARS) {
        Some((end, _)) => format!("{}...", &line[..end]),
        None => line.to_owned(),
    }
}

impl AppState {
    /// Adds to the active tab's activity log. It never opens the panel: a failure is announced
    /// by a toast with Details at the end of `update`.
    pub fn log(&mut self, ok: bool, text: impl Into<String>) {
        self.log_to(None, ok, text.into(), true);
    }

    /// Adds to the active tab's activity log without announcing a failure.
    pub fn log_quiet(&mut self, ok: bool, text: impl Into<String>) {
        self.log_to(None, ok, text.into(), false);
    }

    /// Adds to the activity log of the tab showing `repo` (the active tab's when no tab shows
    /// it, tagged with `repo` in the file); a failure is announced as by `log`.
    pub fn log_in(&mut self, repo: Option<&Path>, ok: bool, text: impl Into<String>) {
        self.log_to(repo, ok, text.into(), true);
    }

    /// `log_in` without announcing a failure.
    pub fn log_quiet_in(&mut self, repo: Option<&Path>, ok: bool, text: impl Into<String>) {
        self.log_to(repo, ok, text.into(), false);
    }

    /// Adds a line to a tab's log, dropping the oldest entries past `LOG_LIMIT` (auto-fetch
    /// adds one a minute for as long as the app runs). The line is also queued for the activity
    /// file, tagged with the tab's repository, unless persistence is off.
    fn log_to(&mut self, repo: Option<&Path>, ok: bool, text: String, loud: bool) {
        let active = self.tabs.get(self.active_tab);
        // Another tab showing `repo`. With no active tab (only inside a tab handler,
        // `usize::MAX`) the line stays in `log`.
        let other = match (repo, active) {
            (Some(r), Some(t)) if !t.is(r) => self.tabs.iter().position(|t| t.is(r)),
            _ => None,
        };
        let tag = match (other, repo) {
            (Some(i), _) => self.tabs[i].path.clone(),
            (None, Some(r)) if !active.is_some_and(|t| t.is(r)) => Some(r.to_path_buf()),
            _ => active.and_then(|t| t.path.clone()),
        };
        let tab = other.map(|i| self.tabs[i].id).or(active.map(|t| t.id));
        if self.logs.persist != Persist::Off {
            self.logs.pending.push(NewLine { ok, text: text.clone(), repo: tag });
            keep_newest(&mut self.logs.pending);
        }
        if loud && !ok && self.logs.announce.is_none() {
            self.logs.announce = Some(Announce { tab, text: headline(&text) });
        }
        let log = match other {
            Some(i) => &mut self.tabs[i].log,
            None => &mut self.log,
        };
        log.push(LogEntry::new(ok, text));
        keep_newest(log);
    }
}
