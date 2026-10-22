//! Git's built-in file system monitor (`git fsmonitor--daemon`, on Windows and macOS with git
//! 2.36 or newer): which OSes and gits have it, the commands that check, start and turn it on,
//! and reading their answers.

use crate::git_version::Version;
use crate::gitcli::{self, GitOutput};
use crate::opens::Os;
use std::path::Path;
use std::time::Duration;

/// The oldest git (major, minor) with `git fsmonitor--daemon` on Windows and macOS.
pub const FSMONITOR_DAEMON: (u32, u32) = (2, 36);

/// How long `fsmonitor--daemon start` may take: git itself waits up to 60 s for the background
/// daemon to come online.
const MONITOR_START_TIMEOUT: Duration = Duration::from_secs(75);

/// What can be offered to speed git's status up with a file system monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorOffer {
    /// Git's built-in daemon (`core.fsmonitor=true`).
    Daemon,
    /// The OS has the built-in daemon, but this git is older than [`FSMONITOR_DAEMON`].
    TooOld(Version),
    /// No built-in daemon on this OS: Watchman with git's fsmonitor-watchman hook, in words only.
    Watchman,
}

/// What `os` with a git of `version` can use (an unknown version counts as new enough).
pub fn monitor_offer_for(os: Os, version: Option<Version>) -> MonitorOffer {
    match (os, version) {
        (Os::Linux, _) => MonitorOffer::Watchman,
        (Os::Windows | Os::MacOs, Some(v)) if (v.0, v.1) < FSMONITOR_DAEMON => MonitorOffer::TooOld(v),
        (Os::Windows | Os::MacOs, _) => MonitorOffer::Daemon,
    }
}

/// [`monitor_offer_for`] on this OS.
pub fn monitor_offer(version: Option<Version>) -> MonitorOffer {
    monitor_offer_for(Os::current(), version)
}

fn line(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

/// Asks whether the daemon is watching this working directory (gitcli arguments, without "git").
pub fn monitor_status_cmd() -> Vec<String> {
    line(&["fsmonitor--daemon", "status"])
}

/// Starts the daemon in the background; git waits for it to come online.
pub fn monitor_start_cmd() -> Vec<String> {
    line(&["fsmonitor--daemon", "start"])
}

/// Turns the built-in monitor on: the plain boolean, never a hook's path.
pub fn monitor_enable_cmd() -> Vec<String> {
    line(&["config", "core.fsmonitor", "true"])
}

/// A status read that may write the index, so git records the monitor's token (and the
/// untracked cache) there; Glowfly's own reads use `--no-optional-locks` and never do.
pub fn monitor_record_cmd() -> Vec<String> {
    line(&["status", "--porcelain", "--untracked-files=all"])
}

/// Why the monitor wasn't turned on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonitorTrouble {
    /// This git has no built-in daemon (built without it, or older than 2.36).
    Unsupported,
    /// The repository is on a network drive, which the daemon refuses.
    Remote,
    /// The daemon refused the repository; git's line, without its "error: " prefix.
    Incompatible(String),
    /// The daemon started but says it isn't watching.
    NotWatching,
    /// A command failed; git's last line.
    Failed(String),
}

impl MonitorTrouble {
    /// The plain-language sentence for the job's toast and log headline.
    pub fn text(&self) -> String {
        match self {
            Self::Unsupported => "This git was built without the file system monitor, so it was left off.".into(),
            Self::Remote => "Git's file system monitor doesn't watch repositories on network drives, so it was left off.".into(),
            Self::Incompatible(why) => {
                format!("Git's file system monitor can't watch this repository (git says: {why}), so it was left off.")
            }
            Self::NotWatching => "Git's file system monitor started but isn't watching this repository; this drive's file system may not \
                                  support it. It was left off."
                .into(),
            Self::Failed(why) => format!("Git's file system monitor didn't start: {}. It was left off.", why.trim_end_matches('.')),
        }
    }
}

/// How turning the monitor on went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonitorOutcome {
    /// The daemon is watching and core.fsmonitor is set; `started` when this run started it.
    Watching {
        started: bool,
    },
    Trouble(MonitorTrouble),
}

impl MonitorOutcome {
    pub fn is_watching(&self) -> bool {
        matches!(self, Self::Watching { .. })
    }
}

/// `line` without git's "error: " or "fatal: " prefix.
fn unprefixed(line: &str) -> String {
    let line = line.trim();
    line.strip_prefix("error: ").or_else(|| line.strip_prefix("fatal: ")).unwrap_or(line).trim().to_owned()
}

/// The refusal git's `stderr` names, if it is one the daemon gives before any subcommand.
pub(crate) fn trouble_in(stderr: &str) -> Option<MonitorTrouble> {
    if stderr.contains("not supported on this platform") || stderr.contains("is not a git command") {
        return Some(MonitorTrouble::Unsupported);
    }
    let refusal = stderr.lines().find(|l| l.contains("incompatible with fsmonitor"))?;
    Some(if refusal.contains("remote repository") { MonitorTrouble::Remote } else { MonitorTrouble::Incompatible(unprefixed(refusal)) })
}

/// What `fsmonitor--daemon status` said: watching or not, or a refusal (which exits 1 too, so
/// stderr is read first).
fn status_answer(out: &GitOutput) -> Result<bool, MonitorTrouble> {
    match trouble_in(&out.stderr) {
        Some(t) => Err(t),
        None => Ok(out.code == Some(0)),
    }
}

/// The failure of a command that printed `out`, in git's last words.
fn failed(out: &GitOutput) -> MonitorTrouble {
    let combined = out.combined();
    let last = combined.lines().rev().map(unprefixed).find(|l| !l.is_empty());
    MonitorTrouble::Failed(
        last.unwrap_or_else(|| if out.timed_out { "it didn't come online in time".into() } else { "git gave no reason".into() }),
    )
}

/// Runs `args`, adding "$ git <args>" and what it printed to `log`.
fn run(cwd: &Path, args: &[String], timeout: Duration, log: &mut String) -> GitOutput {
    log.push_str(&format!("$ git {}\n", args.join(" ")));
    let out = gitcli::output(cwd, args, "", gitcli::NO_PROMPTS, Some(timeout));
    let text = out.combined();
    if !text.is_empty() {
        log.push_str(&text);
        log.push('\n');
    }
    out
}

/// Checks whether git's daemon watches `cwd`, starts it when it doesn't, confirms it is
/// watching, and only then sets core.fsmonitor=true. Returns the outcome and the commands' log.
/// The index isn't written here: the caller runs [`monitor_record_cmd`] in its job's environment.
pub fn turn_on_monitor(cwd: &Path) -> (MonitorOutcome, String) {
    let mut log = String::new();
    let outcome = check_and_start(cwd, &mut log);
    let outcome = match outcome {
        MonitorOutcome::Watching { .. } => {
            let out = run(cwd, &monitor_enable_cmd(), gitcli::READ_TIMEOUT, &mut log);
            if out.ok { outcome } else { MonitorOutcome::Trouble(failed(&out)) }
        }
        trouble => trouble,
    };
    (outcome, log.trim_end().to_owned())
}

fn check_and_start(cwd: &Path, log: &mut String) -> MonitorOutcome {
    let status = |log: &mut String| status_answer(&run(cwd, &monitor_status_cmd(), gitcli::READ_TIMEOUT, log));
    match status(log) {
        Err(t) => return MonitorOutcome::Trouble(t),
        Ok(true) => return MonitorOutcome::Watching { started: false },
        Ok(false) => {}
    }
    let start = run(cwd, &monitor_start_cmd(), MONITOR_START_TIMEOUT, log);
    // Another git may have started it in between.
    if !start.ok && !start.stderr.contains("already running") {
        return MonitorOutcome::Trouble(trouble_in(&start.stderr).unwrap_or_else(|| failed(&start)));
    }
    match status(log) {
        Err(t) => MonitorOutcome::Trouble(t),
        Ok(true) => MonitorOutcome::Watching { started: true },
        Ok(false) => MonitorOutcome::Trouble(MonitorTrouble::NotWatching),
    }
}

#[cfg(test)]
#[path = "fsmonitor_tests.rs"]
mod tests;
