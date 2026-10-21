//! How fast git can read a repository's status: the settings that speed git's own read up
//! (the untracked cache, the file system monitor), whether a read was slow enough to mention
//! them, and the commands that turn the untracked cache on.

use std::path::Path;
use std::time::Duration;

/// A whole status read taking at least this long is slow enough to suggest a speed-up.
pub const SLOW_STATUS: Duration = Duration::from_millis(1500);

/// What in a repository's config speeds up git's status read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusSpeedups {
    /// `core.fsmonitor`: the built-in daemon (true) or a hook.
    pub fsmonitor: bool,
    /// `core.untrackedCache` is true.
    pub untracked_cache: bool,
    /// `status.showUntrackedFiles`: None unset, Some(true) "all", Some(false) any other value.
    pub show_untracked_all: Option<bool>,
}

/// A whole status read that took `took`, with the repository's speed-ups at that time.
#[derive(Clone, Debug, PartialEq)]
pub struct SlowStatus {
    pub took: Duration,
    pub speedups: StatusSpeedups,
}

/// The speed-ups set for the repository at `workdir` (none when it can't be opened).
pub fn status_speedups(workdir: &Path) -> StatusSpeedups {
    git2::Repository::open(workdir).and_then(|r| r.config()).map(|c| speedups_of(&c)).unwrap_or_default()
}

pub(super) fn speedups_of(config: &git2::Config) -> StatusSpeedups {
    // core.fsmonitor is a boolean (the built-in daemon) or a hook's path.
    let fsmonitor = config.get_bool("core.fsmonitor").unwrap_or_else(|_| config.get_string("core.fsmonitor").is_ok_and(|s| !s.is_empty()));
    StatusSpeedups {
        fsmonitor,
        untracked_cache: config.get_bool("core.untrackedCache").unwrap_or(false),
        // Git compares the value exactly when it decides how the cache is kept.
        show_untracked_all: config.get_string("status.showUntrackedFiles").ok().map(|v| v == "all"),
    }
}

/// Whether turning the untracked cache on would help Glowfly's read (`--untracked-files=all`):
/// git uses the cache for that read only while `status.showUntrackedFiles` is unset (the job
/// sets it to `all`) or `all`.
pub fn untracked_cache_helps(s: StatusSpeedups) -> bool {
    !s.untracked_cache && s.show_untracked_all != Some(false)
}

/// The oldest git (major, minor) whose untracked cache serves `--untracked-files=all` once
/// `status.showUntrackedFiles=all` is set; older ones keep it only for the folder form.
pub const UNTRACKED_CACHE_ALL: (u32, u32) = (2, 37);

/// Whether a git of `version` can use the untracked cache for Glowfly's read (an unknown version
/// counts as able, as for `git_version::Feature`).
pub fn untracked_cache_works(version: Option<crate::git_version::Version>) -> bool {
    version.is_none_or(|v| (v.0, v.1) >= UNTRACKED_CACHE_ALL)
}

/// The job's command lines (gitcli arguments, without "git"): turn the cache on, keep it in the
/// `all` form Glowfly reads, add it to the index, and have git's status fill it in. That last
/// read takes optional locks, so the cache is written to the index.
pub fn untracked_cache_cmds(s: StatusSpeedups) -> Vec<Vec<String>> {
    let line = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut cmds = vec![line(&["config", "core.untrackedCache", "true"])];
    if s.show_untracked_all.is_none() {
        cmds.push(line(&["config", "status.showUntrackedFiles", "all"]));
    }
    cmds.push(line(&["update-index", "--untracked-cache"]));
    cmds.push(line(&["status", "--porcelain", "--untracked-files=all"]));
    cmds
}

/// Some when a whole status read of `workdir` took `took`, at least `SLOW_STATUS` (only then is
/// the config read).
pub fn slow(took: Duration, workdir: &Path) -> Option<SlowStatus> {
    (took >= SLOW_STATUS).then(|| SlowStatus { took, speedups: status_speedups(workdir) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{scratch_repo, set_config};

    #[test]
    fn speedups_follow_the_config() {
        let dir = scratch_repo("status-speedups");
        // Keep the developer's global settings out of what is read.
        let local = |dir: &Path| {
            let repo = git2::Repository::open(dir).unwrap();
            speedups_of(&repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap())
        };
        assert_eq!(local(&dir), StatusSpeedups::default());
        set_config(&dir, &[("core.untrackedCache", "true")]);
        assert_eq!(local(&dir), StatusSpeedups { untracked_cache: true, ..Default::default() });
        set_config(&dir, &[("core.fsmonitor", "true"), ("status.showUntrackedFiles", "normal")]);
        assert_eq!(local(&dir), StatusSpeedups { fsmonitor: true, untracked_cache: true, show_untracked_all: Some(false) });
        set_config(&dir, &[("status.showUntrackedFiles", "all"), ("core.fsmonitor", "false")]);
        assert_eq!(local(&dir).show_untracked_all, Some(true));
        assert!(!local(&dir).fsmonitor);
        set_config(&dir, &[("core.fsmonitor", ".git/hooks/watch")]);
        assert!(local(&dir).fsmonitor, "a hook's path");
        assert!(status_speedups(&dir).untracked_cache, "the repository's own config wins");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_untracked_cache_helps_only_the_all_form() {
        let s = |untracked_cache, show_untracked_all| StatusSpeedups { fsmonitor: false, untracked_cache, show_untracked_all };
        assert!(untracked_cache_helps(s(false, None)));
        assert!(untracked_cache_helps(s(false, Some(true))));
        assert!(!untracked_cache_helps(s(false, Some(false))));
        assert!(!untracked_cache_helps(s(true, None)));
        assert!(!untracked_cache_helps(s(true, Some(true))));
        assert!(untracked_cache_works(Some((2, 37, 0))) && untracked_cache_works(Some((3, 0, 0))) && untracked_cache_works(None));
        assert!(!untracked_cache_works(Some((2, 36, 9))));
    }

    #[test]
    fn the_job_sets_show_untracked_files_only_when_unset() {
        let unset = untracked_cache_cmds(StatusSpeedups::default());
        let firsts: Vec<String> = unset.iter().map(|c| c.join(" ")).collect();
        assert_eq!(
            firsts,
            [
                "config core.untrackedCache true",
                "config status.showUntrackedFiles all",
                "update-index --untracked-cache",
                "status --porcelain --untracked-files=all"
            ]
        );
        let all = untracked_cache_cmds(StatusSpeedups { show_untracked_all: Some(true), ..Default::default() });
        assert!(!all.iter().any(|c| c.contains(&"status.showUntrackedFiles".to_string())));
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn only_slow_reads_are_reported() {
        let dir = scratch_repo("status-slow");
        assert_eq!(slow(SLOW_STATUS - Duration::from_millis(1), &dir), None);
        let s = slow(SLOW_STATUS, &dir).expect("slow");
        assert_eq!(s.took, SLOW_STATUS);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
