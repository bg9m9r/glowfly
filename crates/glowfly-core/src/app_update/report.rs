//! How the last installer run went, read from the helper's `update.log` at the next start, so a
//! failed or cancelled MSI install isn't mistaken for nothing happening. The helper's words
//! (finish.rs) are matched, so a log written by an older helper reads the same. Once read, a
//! line saying so is added ([`SHOWN`]), so each run is reported once.

use super::finish::{self, BUSY, CANCELLED, FAILED, INSTALLED, INSTALLING, LOG_FILE, NOT_RUN, NOT_UPDATING};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Added after a run's result was shown.
pub const SHOWN: &str = "the result was shown at the next start";
/// Only the end of the log is read: the last run is there.
const TAIL_BYTES: u64 = 64 << 10;

/// What went wrong with the installer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallProblem {
    Cancelled,
    /// Another installation was running.
    Busy,
    /// msiexec failed, with its exit code when it had one.
    Failed(Option<i32>),
    /// msiexec couldn't be started, and why.
    NotRun(String),
}

/// The last run's problem, for the version it was installing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastInstall {
    pub version: String,
    pub problem: InstallProblem,
    /// The update log.
    pub log: PathBuf,
}

/// A log line without its time (`2026-10-29 12:00:00 `).
fn text(line: &str) -> &str {
    let b = line.as_bytes();
    let stamped = b.len() > 20
        && b[..19].iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b' ',
            13 | 16 => *c == b':',
            _ => c.is_ascii_digit(),
        })
        && b[19] == b' ';
    if stamped { &line[20..] } else { line }
}

/// The code in `Some(1603)); its log is …`.
fn exit_code(rest: &str) -> Option<i32> {
    let digits: String = rest.strip_prefix("Some(")?.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    digits.parse().ok()
}

/// The problem of the last run in `log` (the version and what went wrong); None when it
/// installed, changed nothing, was already shown, or there was no run.
pub fn last_problem(log: &str) -> Option<(String, InstallProblem)> {
    let mut run: Option<(String, Option<InstallProblem>)> = None;
    for line in log.lines().map(text) {
        if let Some(version) = line.strip_prefix(INSTALLING) {
            run = Some((version.trim().to_owned(), None));
        } else if line.starts_with(NOT_UPDATING) || line == SHOWN {
            run = None;
        } else if let Some((_, problem)) = run.as_mut() {
            let found = if line == INSTALLED {
                None
            } else if line == CANCELLED {
                Some(InstallProblem::Cancelled)
            } else if line == BUSY {
                Some(InstallProblem::Busy)
            } else if let Some(rest) = line.strip_prefix(FAILED) {
                Some(InstallProblem::Failed(exit_code(rest)))
            } else if let Some(why) = line.strip_prefix(NOT_RUN) {
                Some(InstallProblem::NotRun(why.to_owned()))
            } else {
                continue;
            };
            *problem = found;
        }
    }
    run.and_then(|(version, problem)| Some((version, problem?)))
}

/// The end of the file at `path` (at most `TAIL_BYTES`, from the start of a line).
fn read_tail(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let from = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(if from > 0 { text.split_once('\n').map_or_else(String::new, |(_, rest)| rest.to_owned()) } else { text })
}

/// Reads the last run's problem from the update log in `log_dir` and marks it shown, so the
/// next start says nothing. None when there is nothing to say.
pub fn take(log_dir: &Path) -> Option<LastInstall> {
    let log = log_dir.join(LOG_FILE);
    let (version, problem) = last_problem(&read_tail(&log)?)?;
    finish::append(log_dir, SHOWN);
    Some(LastInstall { version, problem, log })
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
