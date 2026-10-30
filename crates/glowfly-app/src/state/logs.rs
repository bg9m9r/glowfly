//! The activity logs in memory, one per tab, and what the logs panel shows: earlier sessions'
//! entries, the lines waiting to be written to the activity file, the failure waiting to be
//! announced, the open tab and the error or performance log read for it, the rows folded open
//! and the fixes failed runs offer.
//!
//! An entry is a note or a run (a finished job with its `RunInfo`, update/logs_runs.rs). The
//! active tab's log is `AppState::log`; every other tab keeps its own in `Tab::log`
//! (update/tabs.rs swaps them, update/logs_tabs.rs hands out earlier entries).

use super::tabs::TabId;
use super::{AppState, ToastAction};
use crate::logs::{LogKind, LogsMsg, NewLine};
use crate::msg::Msg;
use glowfly_core::logs::{ActivityEntry, LogTail, RunInfo};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// The most entries a tab's activity log keeps (earlier sessions' included), and the activity
/// file for all repositories together.
pub const LOG_LIMIT: usize = 500;

/// The most fixes kept for failed runs; the oldest go first.
pub const FIX_LIMIT: usize = 20;

/// When an entry from an earlier session was written (Unix seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Earlier {
    /// When its session started.
    pub session: i64,
    pub at: i64,
}

/// An activity log entry's name for the session: open rows and fixes are keyed by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryId(u64);

impl EntryId {
    pub fn next() -> EntryId {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        EntryId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub id: EntryId,
    pub ok: bool,
    pub text: String,
    /// Set for entries from an earlier session, read from the activity file.
    pub earlier: Option<Earlier>,
    /// When it was logged, in Unix seconds: `view.now` for this session's entries, the file's
    /// time for earlier ones.
    pub at: i64,
    /// The finished operation it records; None for a note.
    pub run: Option<RunInfo>,
    /// How many older routine successes this entry also stands for, dropped by the routine cap
    /// (state/activity_rows.rs). Never written to the file.
    pub folded: u32,
}

impl LogEntry {
    pub fn new(ok: bool, text: impl Into<String>) -> Self {
        Self { id: EntryId::next(), ok, text: text.into(), earlier: None, at: 0, run: None, folded: 0 }
    }

    /// An earlier session's entry; a plain run line an older build wrote becomes a run.
    pub fn earlier(e: ActivityEntry) -> Self {
        let run = e.run.or_else(|| super::activity_rows::legacy_run(&e.text, e.ok));
        let earlier = Some(Earlier { session: e.session, at: e.at });
        Self { id: EntryId::next(), ok: e.ok, text: e.text, earlier, at: e.at, run, folded: 0 }
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
    /// The entry Details opens.
    pub entry: Option<EntryId>,
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
    /// The activity rows folded open (a routine group by its first entry's id).
    pub open: HashSet<EntryId>,
    /// The fixes failed runs offer, newest last, at most `FIX_LIMIT`. Each runs once, from the
    /// panel or from the failure toast's button (a `LogsMsg::Fix` proxy).
    pub fixes: Vec<(EntryId, ToastAction)>,
    /// A second fix beside a run's fix (Try again beside Delete lock file), from the panel or
    /// the toast's second button (a `LogsMsg::AltFix` proxy). Running either drops both.
    pub alt_fixes: Vec<(EntryId, ToastAction)>,
    /// When the active job started running (stamped at the end of `update`,
    /// update/logs_runs.rs).
    pub job_started: Option<Instant>,
    /// The active job's spec with the tab it started in, for Try again if it fails
    /// (update/job_retry.rs); set by `start_job`, cleared by any other job.
    pub job_retry: Option<super::job_spec::Retry>,
    /// The last conflicts toast, until the status read after its job counts the files.
    pub recount: Option<super::conflict_files::Recount>,
}

impl LogsState {
    /// The lines to append to the activity file now: Some only while persistence is on and
    /// lines are waiting.
    pub fn take_append(&mut self) -> Option<Vec<NewLine>> {
        (self.persist == Persist::On && !self.pending.is_empty()).then(|| std::mem::take(&mut self.pending))
    }

    /// Keeps `action` as run `id`'s fix and returns the button that runs it through
    /// `LogsMsg::Fix`, for the failure toast.
    pub fn keep_fix(&mut self, id: EntryId, action: ToastAction) -> ToastAction {
        let msg = Box::new(Msg::Logs(LogsMsg::Fix(id)));
        let proxy = ToastAction { label: action.label.clone(), msg, workdir: action.workdir.clone() };
        self.fixes.retain(|(i, _)| *i != id);
        self.alt_fixes.retain(|(i, _)| *i != id);
        self.fixes.push((id, action));
        if self.fixes.len() > FIX_LIMIT {
            let (gone, _) = self.fixes.remove(0);
            self.alt_fixes.retain(|(i, _)| *i != gone);
        }
        proxy
    }

    /// Puts `action` first as run `id`'s fix, its fix so far becoming the second one. Returns
    /// the toast's buttons for both (`LogsMsg::Fix`, `LogsMsg::AltFix`); None, changing nothing,
    /// once the run's fix has run or gone.
    pub fn offer_first(&mut self, id: EntryId, action: ToastAction) -> Option<(ToastAction, ToastAction)> {
        let at = self.fixes.iter().position(|(i, _)| *i == id)?;
        let proxy = |a: &ToastAction, msg: LogsMsg| ToastAction {
            label: a.label.clone(),
            msg: Box::new(Msg::Logs(msg)),
            workdir: a.workdir.clone(),
        };
        let main = proxy(&action, LogsMsg::Fix(id));
        let before = std::mem::replace(&mut self.fixes[at].1, action);
        let alt = proxy(&before, LogsMsg::AltFix(id));
        self.alt_fixes.retain(|(i, _)| *i != id);
        self.alt_fixes.push((id, before));
        Some((main, alt))
    }

    /// Run `id`'s fix, while it hasn't run.
    pub fn fix(&self, id: EntryId) -> Option<&ToastAction> {
        self.fixes.iter().find(|(i, _)| *i == id).map(|(_, a)| a)
    }

    /// The label of run `id`'s fix.
    pub fn fix_label(&self, id: EntryId) -> Option<&str> {
        self.fix(id).map(|a| a.label.as_str())
    }

    /// Run `id`'s second fix, while neither has run.
    pub fn alt_fix(&self, id: EntryId) -> Option<&ToastAction> {
        self.alt_fixes.iter().find(|(i, _)| *i == id).map(|(_, a)| a)
    }

    /// Removes run `id`'s fixes and returns the first (or with `alt`, the second).
    pub fn take_fix(&mut self, id: EntryId, alt: bool) -> Option<ToastAction> {
        let main = self.fixes.iter().position(|(i, _)| *i == id).map(|i| self.fixes.remove(i).1);
        let second = self.alt_fixes.iter().position(|(i, _)| *i == id).map(|i| self.alt_fixes.remove(i).1);
        if alt { second } else { main }
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

/// `text` with the user name and password of every URL in it hidden
/// ([`glowfly_core::text::redact_credentials`]), for the activity log, toasts and desktop
/// notifications.
pub fn redacted(text: String) -> String {
    if text.contains("://") { glowfly_core::text::redact_credentials(&text) } else { text }
}

impl AppState {
    /// Adds to the active tab's activity log. It never opens the panel: a failure is announced
    /// by a toast with Details at the end of `update`.
    pub fn log(&mut self, ok: bool, text: impl Into<String>) {
        self.log_to(None, ok, text.into(), None, true);
    }

    /// Adds to the active tab's activity log without announcing a failure.
    pub fn log_quiet(&mut self, ok: bool, text: impl Into<String>) {
        self.log_to(None, ok, text.into(), None, false);
    }

    /// Adds to the activity log of the tab showing `repo` (the active tab's when no tab shows
    /// it, tagged with `repo` in the file); a failure is announced as by `log`.
    pub fn log_in(&mut self, repo: Option<&Path>, ok: bool, text: impl Into<String>) {
        self.log_to(repo, ok, text.into(), None, true);
    }

    /// `log_in` without announcing a failure.
    pub fn log_quiet_in(&mut self, repo: Option<&Path>, ok: bool, text: impl Into<String>) {
        self.log_to(repo, ok, text.into(), None, false);
    }

    /// Logs a finished job as a run in the tab showing `repo`, as `log_in` (`loud`) or
    /// `log_quiet_in` would. Returns the entry's id.
    pub fn log_run_in(&mut self, repo: Option<&Path>, ok: bool, text: String, run: RunInfo, loud: bool) -> EntryId {
        self.log_to(repo, ok, text, Some(run), loud)
    }

    /// Adds a line to a tab's log, dropping the oldest entries past `LOG_LIMIT` (auto-fetch
    /// adds one a minute for as long as the app runs; a long streak of them is capped first).
    /// The line is also queued for the activity file, tagged with the tab's repository, unless
    /// persistence is off. The user name and password of any URL in the text (a typed clone
    /// address, git's own output, a hook's) become `***` first, so no token reaches the panel,
    /// its toast or the file.
    fn log_to(&mut self, repo: Option<&Path>, ok: bool, text: String, run: Option<RunInfo>, loud: bool) -> EntryId {
        let text = redacted(text);
        let run = run.map(|r| RunInfo { summary: redacted(r.summary), ..r });
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
            self.logs.pending.push(NewLine { ok, text: text.clone(), repo: tag, run: run.clone() });
            keep_newest(&mut self.logs.pending);
        }
        let entry = LogEntry { at: self.view.now, run, ..LogEntry::new(ok, text) };
        let id = entry.id;
        if loud && !ok && self.logs.announce.is_none() {
            self.logs.announce = Some(Announce { tab, text: headline(&entry.text), entry: Some(id) });
        }
        let log = match other {
            Some(i) => &mut self.tabs[i].log,
            None => &mut self.log,
        };
        log.push(entry);
        super::activity_rows::cap_routine(log);
        keep_newest(log);
        id
    }
}
