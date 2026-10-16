//! The activity log in memory, and what the logs panel shows: earlier sessions' entries, the
//! lines waiting to be written to the activity file, the open tab and the error or performance
//! log read for it.

use super::AppState;
use crate::logs::LogKind;
use std::path::PathBuf;
use trellis_core::logs::{ActivityEntry, LogTail};

/// The most entries the activity log keeps (earlier sessions' included).
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

#[derive(Default)]
pub struct LogsState {
    pub persist: Persist,
    /// Lines waiting to be written to the activity file (at most `LOG_LIMIT`), flushed at the
    /// end of `update` once the file has loaded.
    pub pending: Vec<(bool, String)>,
    /// The activity panel's tab.
    pub tab: LogKind,
    pub file: Option<LogFileView>,
    /// Incremented for every file read; older `FileRead` results are ignored.
    pub file_gen: u64,
    /// The logs folder, once the runner has reported it.
    pub dir: Option<PathBuf>,
    /// Clear was pressed before the activity file was read: delete it once it has been.
    pub clear_on_load: bool,
}

impl LogsState {
    /// The lines to append to the activity file now: Some only while persistence is on and
    /// lines are waiting.
    pub fn take_append(&mut self) -> Option<Vec<(bool, String)>> {
        (self.persist == Persist::On && !self.pending.is_empty()).then(|| std::mem::take(&mut self.pending))
    }
}

impl AppState {
    /// Adds to the activity log, opening the panel for a failure.
    pub fn log(&mut self, ok: bool, text: impl Into<String>) {
        self.log_quiet(ok, text);
        if !ok {
            self.view.show_log = true;
        }
    }

    /// Adds to the activity log without opening it, dropping the oldest entries past
    /// `LOG_LIMIT` (auto-fetch adds one a minute for as long as the app runs). The line is also
    /// queued for the activity file unless persistence is off.
    pub fn log_quiet(&mut self, ok: bool, text: impl Into<String>) {
        let text = text.into();
        if self.logs.persist != Persist::Off {
            let pending = &mut self.logs.pending;
            pending.push((ok, text.clone()));
            if pending.len() > LOG_LIMIT {
                pending.drain(..pending.len() - LOG_LIMIT);
            }
        }
        self.log.push(LogEntry::new(ok, text));
        if self.log.len() > LOG_LIMIT {
            self.log.drain(..self.log.len() - LOG_LIMIT);
        }
    }
}
