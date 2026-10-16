//! Messages and effects of the logs: the activity log kept across sessions, the error and
//! performance log tabs, the logs folder and extended logging (state/logs.rs, update/logs.rs,
//! runner/logs.rs, logging.rs, ui/activity_log.rs).

use std::path::PathBuf;
use trellis_core::gitcli::GitRun;
use trellis_core::logs::{ActivityEntry, LogTail};

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

    pub fn label(self) -> &'static str {
        match self {
            LogKind::Activity => "Activity",
            LogKind::Errors => "Errors",
            LogKind::Performance => "Performance",
        }
    }
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
            | Self::GitRan(_) => None,
        }
    }
}

/// Work on the log files, run in order by the runner's log worker (runner/logs.rs).
#[derive(Debug)]
pub enum LogsEffect {
    /// Read the activity file; reported as `LogsMsg::ActivityLoaded`.
    LoadActivity,
    /// Append these (ok, text) lines to the activity file, stamped with the session and time.
    Append(Vec<(bool, String)>),
    /// Delete the activity file.
    ClearActivity,
    /// Read the end of the error or performance log; reported as `LogsMsg::FileRead`.
    ReadFile { kind: LogKind, generation: u64 },
    /// Create the logs folder if needed and show it in the file manager.
    OpenFolder,
    /// Turn extended logging on or off: git commands reach the activity log, and the error log
    /// takes debug messages.
    SetExtended(bool),
}
