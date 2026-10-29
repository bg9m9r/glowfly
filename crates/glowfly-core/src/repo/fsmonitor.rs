//! Git's built-in file system monitor (`git fsmonitor--daemon`, on Windows and macOS with git
//! 2.36 or newer): which OSes and gits have it (`git version --build-options` names it), the
//! commands that check, start, turn it on and off and stop it, and reading their answers.

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
    /// This git is new enough but was built without the daemon (`git version --build-options`
    /// doesn't name it): nothing to offer.
    NotBuilt,
}

/// What `os` with a git of `version` can use (an unknown version counts as new enough), given
/// whether its build options name the daemon (`daemon`, None while unknown: counted as built).
pub fn monitor_offer_for(os: Os, version: Option<Version>, daemon: Option<bool>) -> MonitorOffer {
    match (os, version) {
        (Os::Linux, _) => MonitorOffer::Watchman,
        (Os::Windows | Os::MacOs, Some(v)) if (v.0, v.1) < FSMONITOR_DAEMON => MonitorOffer::TooOld(v),
        (Os::Windows | Os::MacOs, _) if daemon == Some(false) => MonitorOffer::NotBuilt,
        (Os::Windows | Os::MacOs, _) => MonitorOffer::Daemon,
    }
}

/// [`monitor_offer_for`] on this OS.
pub fn monitor_offer(version: Option<Version>, daemon: Option<bool>) -> MonitorOffer {
    monitor_offer_for(Os::current(), version, daemon)
}

/// Whether `git version --build-options` printed `text` for a git built with the daemon.
pub fn daemon_in_build_options(text: &str) -> bool {
    text.lines().any(|l| l.trim() == "feature: fsmonitor--daemon")
}

/// Whether git's daemon could watch the working directory `workdir` and give git a token: this
/// OS has it, the git Glowfly runs was built with it, and the folder isn't on a network drive
/// (which the daemon refuses). Asks git, so callers ask rarely.
pub fn daemon_serves(workdir: &Path) -> bool {
    crate::volume::remote_kind(workdir).is_none() && monitor_offer(None, daemon_built(workdir)) == MonitorOffer::Daemon
}

/// Whether the git Glowfly runs was built with the daemon, asked in `cwd`; None when git didn't
/// answer. Older gits print no feature lines, so they read as built without it.
pub fn daemon_built(cwd: &Path) -> Option<bool> {
    let out = gitcli::read(cwd, &line(&["version", "--build-options"]));
    out.ok.then(|| daemon_in_build_options(&out.stdout))
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

/// Turns the monitor off for this repository: unsets the local core.fsmonitor, every value of
/// it (plain `--unset` refuses a key set more than once, with the same exit code as an unset one).
pub fn monitor_disable_cmd() -> Vec<String> {
    line(&["config", "--unset-all", "core.fsmonitor"])
}

/// Stops the daemon watching this working directory.
pub fn monitor_stop_cmd() -> Vec<String> {
    line(&["fsmonitor--daemon", "stop"])
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

/// Whether a stop that printed `out` left no daemon watching: it stopped, none was running, or
/// this git has none.
fn stopped(out: &GitOutput) -> bool {
    out.ok || out.stderr.contains("is not running") || trouble_in(&out.stderr) == Some(MonitorTrouble::Unsupported)
}

/// Stops the daemon watching `cwd`, if one is, so it lets go of the folder (on Windows it holds
/// the working directory open, and a worktree can't be removed or moved under it). Returns the
/// command's log, or "" when no daemon was running.
pub fn stop_monitor(cwd: &Path) -> String {
    let mut log = String::new();
    let out = run(cwd, &monitor_stop_cmd(), gitcli::READ_TIMEOUT, &mut log);
    if out.ok { log.trim_end().to_owned() } else { String::new() }
}

/// Turns the monitor off in `cwd`: unsets core.fsmonitor first, so no git started meanwhile
/// starts the daemon again, then stops the daemon. Returns whether both left it off, and the
/// commands' log. A core.fsmonitor that wasn't set locally (`--unset-all` exits 5 only then)
/// counts as unset.
pub fn turn_off_monitor(cwd: &Path) -> (bool, String) {
    let mut log = String::new();
    let unset = run(cwd, &monitor_disable_cmd(), gitcli::READ_TIMEOUT, &mut log);
    let unset_ok = unset.ok || unset.code == Some(5);
    let stop = run(cwd, &monitor_stop_cmd(), gitcli::READ_TIMEOUT, &mut log);
    (unset_ok && stopped(&stop), log.trim_end().to_owned())
}

#[cfg(test)]
#[path = "fsmonitor_tests.rs"]
pub(super) mod tests;
