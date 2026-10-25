//! Messages and effects of the logs: the activity log kept across sessions, the error and
//! performance log tabs, the logs folder, extended logging, copying an entry or a whole log and
//! exporting every log as one bundle (state/logs.rs, state/activity_copy.rs, update/logs.rs,
//! update/logs_copy.rs, update/logs_export.rs, runner/logs.rs, logging.rs, ui/activity_log.rs).

use crate::effect::EffectMeta;
#[cfg(test)]
use crate::effect::SandboxRule;
use crate::state::logs::EntryId;
use crate::state::tabs::TabId;
use glowfly_core::gitcli::GitRun;
use glowfly_core::i18n::Lang;
use glowfly_core::logs::{ActivityEntry, BundleReport, LogTail, RunInfo};
use std::path::{Path, PathBuf};

/// The activity panel's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LogKind {
    #[default]
    Activity,
    Errors,
    Performance,
}

impl LogKind {
    pub const ALL: [LogKind; 3] = [LogKind::Activity, LogKind::Errors, LogKind::Performance];

    /// Its tab's name in `t`.
    pub fn name(self, t: Lang) -> &'static str {
        match self {
            LogKind::Activity => t.tr("logs.tab.activity"),
            LogKind::Errors => t.tr("logs.tab.errors"),
            LogKind::Performance => t.tr("logs.tab.performance"),
        }
    }

    /// Its tab's tooltip in `t`.
    pub fn tip(self, t: Lang) -> &'static str {
        match self {
            LogKind::Activity => t.tr("logs.tab.activity.tip"),
            LogKind::Errors => t.tr("logs.tab.errors.tip"),
            LogKind::Performance => t.tr("logs.tab.performance.tip"),
        }
    }
}

/// A new line for the activity file: the repository is that of the tab it was logged in (None
/// for a tab without one).
#[derive(Clone, Debug, PartialEq)]
pub struct NewLine {
    pub ok: bool,
    pub text: String,
    pub repo: Option<PathBuf>,
    /// The finished operation it records; None for a note.
    pub run: Option<RunInfo>,
}

/// The earlier sessions' activity, read from the logs folder `dir` at startup.
#[derive(Clone, Debug, PartialEq)]
pub struct EarlierLog {
    pub dir: PathBuf,
    pub entries: Vec<ActivityEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogsMsg {
    /// Sent once by the shell at startup: load the earlier sessions' activity and set the
    /// error log's level.
    Start,
    /// The activity file was read (runner/logs.rs); Err turns persistence off.
    ActivityLoaded(Result<EarlierLog, String>),
    /// Appending to the activity file failed three times in a row; persistence is off.
    AppendFailed(String),
    /// Open the activity panel on this tab (the file tabs read their file again).
    Show(LogKind),
    /// The end of the error or performance log, for request `generation`.
    FileRead { kind: LogKind, generation: u64, result: Result<LogTail, String> },
    /// Show the logs folder in the file manager.
    OpenFolder,
    /// Turn extended logging on or off (Preferences > General, the palette).
    SetExtended(bool),
    /// A git command ended while extended logging is on.
    GitRan(GitRun),
    /// A failure toast's Details: select that tab if it is still open, show its activity log
    /// and open the failed run's row (`entry`), scrolled into view.
    Details { tab: TabId, entry: Option<EntryId> },
    /// Fold an activity row (a routine group by its first entry) open, or closed again.
    Fold(EntryId),
    /// Run a failed run's fix: the panel's button, and the failure toast's.
    Fix(EntryId),
    /// Copy an activity entry: the row menu's Copy entry and the open row's Copy button. A
    /// routine group's first entry copies the whole group.
    CopyEntry(EntryId),
    /// Copy what the panel's current tab shows.
    CopyLog,
    /// Export every log as one bundle: ask where to save it (the More menu, the palette, Help).
    Export,
    /// The save dialog's answer; None: it was cancelled.
    ExportTo(Option<PathBuf>),
    /// The log worker wrote the bundle to `dest`, or failed to.
    Exported { dest: PathBuf, result: Result<BundleReport, String> },
    /// The export toast's action: show the bundle in the file manager.
    ShowExported(PathBuf),
}

impl LogsMsg {
    /// Never scoped: the logs are global.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Start
            | Self::ActivityLoaded(_)
            | Self::AppendFailed(_)
            | Self::Show(_)
            | Self::FileRead { .. }
            | Self::OpenFolder
            | Self::SetExtended(_)
            | Self::GitRan(_)
            | Self::Details { .. }
            | Self::Fold(_)
            | Self::Fix(_)
            | Self::CopyEntry(_)
            | Self::CopyLog
            | Self::Export
            | Self::ExportTo(_)
            | Self::Exported { .. }
            | Self::ShowExported(_) => None,
        }
    }
}

/// Work on the log files, run in order by the runner's log worker (runner/logs.rs), except the
/// save dialog, which waits on the user on a worker thread of its own.
#[derive(Debug)]
pub enum LogsEffect {
    /// Read the activity file; reported as `LogsMsg::ActivityLoaded`.
    LoadActivity,
    /// Append these lines to the activity file, stamped with the session and time.
    Append(Vec<NewLine>),
    /// Remove the lines of the tab for `repo` (None: a tab without a repository) from the
    /// activity file.
    ClearActivity { repo: Option<PathBuf> },
    /// Read the end of the error or performance log; reported as `LogsMsg::FileRead`.
    ReadFile { kind: LogKind, generation: u64 },
    /// Create the logs folder if needed and show it in the file manager.
    OpenFolder,
    /// Turn extended logging on or off: git commands reach the activity log, and the error log
    /// takes debug messages.
    SetExtended(bool),
    /// Ask where to save the bundle, suggesting `name`; reported as `LogsMsg::ExportTo`.
    PickExport { name: String },
    /// Write every log to `dest` as one bundle under `header`; reported as `LogsMsg::Exported`.
    Export { dest: PathBuf, header: Vec<(String, String)> },
}

impl LogsEffect {
    /// Whether the log worker runs it (in order with the other log work). The save dialog waits
    /// on the user, so it runs on a worker thread instead and holds nothing up.
    pub fn on_log_worker(&self) -> bool {
        !matches!(self, Self::PickExport { .. })
    }
}

impl EffectMeta for LogsEffect {
    fn name(&self) -> &'static str {
        match self {
            Self::LoadActivity => "load-activity",
            Self::Append(_) => "append-activity",
            Self::ClearActivity { .. } => "clear-activity",
            Self::ReadFile { .. } => "read-log",
            Self::OpenFolder => "open-logs-folder",
            Self::SetExtended(_) => "extended-logging",
            Self::PickExport { .. } => "pick-logs-export",
            Self::Export { .. } => "export-logs",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Glowfly's own files on the log worker, and a save dialog that waits on the user.
    fn timed(&self) -> bool {
        false
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    /// A no-op: the log worker is never attached in a test. The save dialog is dropped.
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        match self {
            Self::LoadActivity
            | Self::Append(_)
            | Self::ClearActivity { .. }
            | Self::ReadFile { .. }
            | Self::OpenFolder
            | Self::SetExtended(_)
            | Self::Export { .. } => SandboxRule::Run,
            Self::PickExport { .. } => SandboxRule::Drop,
        }
    }
}
