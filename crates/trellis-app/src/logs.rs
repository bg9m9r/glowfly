//! Messages and effects of the logs: the activity log kept across sessions, the error and
//! performance log tabs, the logs folder and extended logging (state/logs.rs, update/logs.rs,
//! runner/logs.rs, logging.rs, ui/activity_log.rs).

use crate::state::logs::EntryId;
use crate::state::tabs::TabId;
use std::path::PathBuf;
use trellis_core::gitcli::GitRun;
use trellis_core::i18n::Lang;
use trellis_core::logs::{ActivityEntry, LogTail, RunInfo};

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
            | Self::Fix(_) => None,
        }
    }
}

/// Work on the log files, run in order by the runner's log worker (runner/logs.rs).
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
}
