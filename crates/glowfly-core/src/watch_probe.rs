//! The file watcher's self-test: a scratch file placed in the repository's git dir right after
//! the watcher starts, whose event shows that changes reach Glowfly (some network drives, WSL
//! and 9P mounts accept a watch and then never report anything). The file is named `*.lock`, so
//! git and the watcher's classification ignore it like git's own lock files, sits in the git
//! dir (a linked worktree's own, never the work tree), and is removed as soon as it is seen, at
//! the timeout, or when the probe is dropped. A git dir that can't be written gets no probe.
//! Placing one first removes probes a crash left behind (`sweep_stale`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A probe this old is left over whoever placed it: one lives at most a few seconds.
pub const STALE_AFTER: Duration = Duration::from_secs(60);

/// A probe file that exists until this is dropped (or `remove` is called).
#[derive(Debug)]
pub struct ProbeFile {
    path: PathBuf,
    name: OsString,
    removed: bool,
}

impl ProbeFile {
    /// Creates a fresh probe file in `git_dir`; None when the folder is read-only or the file
    /// can't be created there (then whether events arrive is unknown).
    pub fn place(git_dir: &Path) -> Option<ProbeFile> {
        // On Windows a folder's read-only attribute only marks it as customized: the create
        // below is the test there.
        if cfg!(unix) && git_dir.metadata().is_ok_and(|m| m.permissions().readonly()) {
            return None;
        }
        sweep_stale(git_dir, SystemTime::now(), crate::git_procs::running);
        let stamp = SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
        let name = OsString::from(format!("glowfly-watch-{}-{stamp}.lock", std::process::id()));
        let path = git_dir.join(&name);
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).ok()?;
        Some(ProbeFile { path, name, removed: false })
    }

    /// Whether `path` (as a watcher reports it, in any spelling of the git dir) is this probe.
    pub fn is(&self, path: &Path) -> bool {
        path.file_name() == Some(self.name.as_os_str())
    }

    /// Removes the file (once; a file already gone is fine).
    pub fn remove(&mut self) {
        if !self.removed {
            self.removed = true;
            if let Err(e) = std::fs::remove_file(&self.path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                tracing::debug!("could not remove the watcher's probe {}: {e}", self.path.display());
            }
        }
    }
}

/// The pid in a probe's name, when `name` is exactly `glowfly-watch-<pid>-<nanos>.lock`.
fn probe_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("glowfly-watch-")?.strip_suffix(".lock")?;
    let (pid, stamp) = rest.split_once('-')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(pid) || !digits(stamp) {
        return None;
    }
    pid.parse().ok()
}

/// Removes the probes in `git_dir` that are left over: plain files (never links or folders)
/// named exactly as a probe, older than `STALE_AFTER` at `now` or placed by a process that no
/// longer runs (`running`: None when that can't be told, which keeps the file). Errors are
/// ignored: a probe left behind only costs a few bytes.
pub(crate) fn sweep_stale(git_dir: &Path, now: SystemTime, running: impl Fn(u32) -> Option<bool>) {
    let Ok(entries) = std::fs::read_dir(git_dir) else { return };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(probe_pid) else { continue };
        let Ok(meta) = entry.path().symlink_metadata() else { continue };
        if !meta.file_type().is_file() {
            continue;
        }
        let old = meta.modified().ok().and_then(|m| now.duration_since(m).ok()).is_some_and(|age| age > STALE_AFTER);
        if old || running(pid) == Some(false) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

impl Drop for ProbeFile {
    fn drop(&mut self) {
        self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::scratch_repo;

    #[test]
    fn the_probe_is_a_lock_in_the_git_dir_and_goes_away() {
        let dir = scratch_repo("watch-probe");
        let git_dir = dir.join(".git");
        let mut probe = ProbeFile::place(&git_dir).expect("a writable git dir");
        assert!(probe.path.starts_with(&git_dir) && probe.path.exists());
        assert!(probe.name.to_string_lossy().ends_with(".lock"), "git and the watcher ignore locks");
        assert!(probe.is(&PathBuf::from("C:/elsewhere/spelled/differently").join(&probe.name)), "matched by name");
        assert!(!probe.is(&git_dir.join("index")));
        let path = probe.path.clone();
        probe.remove();
        assert!(!path.exists());
        probe.remove();
        let dropped = ProbeFile::place(&git_dir).unwrap();
        let path = dropped.path.clone();
        drop(dropped);
        assert!(!path.exists(), "dropping removes it");
        assert_eq!(
            std::fs::read_dir(&git_dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("glowfly-watch")).count(),
            0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_left_over_probes_are_swept() {
        let dir = scratch_repo("watch-probe-stale");
        let git_dir = dir.join(".git");
        let made = |name: &str, age: Duration| {
            let f = std::fs::File::create(git_dir.join(name)).unwrap();
            f.set_modified(SystemTime::now() - age).unwrap();
        };
        let young = Duration::from_secs(1);
        made("glowfly-watch-111-1.lock", STALE_AFTER * 2); // old, its process running
        made("glowfly-watch-222-2.lock", young); // its process gone
        made("glowfly-watch-333-3.lock", young); // its process running
        made("glowfly-watch-444-4.lock", young); // can't be told
        made("glowfly-watch-x-5.lock", STALE_AFTER * 2); // not a probe's name
        made("glowfly-watch-555-5.lock.bak", STALE_AFTER * 2);
        made("index.lock", STALE_AFTER * 2);
        std::fs::create_dir(git_dir.join("glowfly-watch-666-6.lock")).unwrap();
        sweep_stale(&git_dir, SystemTime::now(), |pid| match pid {
            222 => Some(false),
            444 => None,
            _ => Some(true),
        });
        let mut left: Vec<String> =
            std::fs::read_dir(&git_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.retain(|n| n.contains(".lock"));
        left.sort();
        let want = [
            "glowfly-watch-333-3.lock",
            "glowfly-watch-444-4.lock",
            "glowfly-watch-555-5.lock.bak",
            "glowfly-watch-666-6.lock",
            "glowfly-watch-x-5.lock",
            "index.lock",
        ];
        assert_eq!(left, want);
        assert_eq!(probe_pid("glowfly-watch-12-34.lock"), Some(12));
        assert_eq!(probe_pid("glowfly-watch--34.lock"), None);
        assert_eq!(probe_pid("glowfly-watch-12-.lock"), None);
        assert_eq!(probe_pid("glowfly-watch-12-3-4.lock"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_named_as_a_probe_is_left_alone() {
        let dir = scratch_repo("watch-probe-link");
        let git_dir = dir.join(".git");
        let target = dir.join("kept.txt");
        std::fs::write(&target, "x").unwrap();
        std::os::unix::fs::symlink(&target, git_dir.join("glowfly-watch-1-1.lock")).unwrap();
        sweep_stale(&git_dir, SystemTime::now() + STALE_AFTER * 2, |_| Some(false));
        assert!(git_dir.join("glowfly-watch-1-1.lock").symlink_metadata().is_ok() && target.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_git_dir_gets_no_probe() {
        let dir = scratch_repo("watch-probe-missing");
        assert!(ProbeFile::place(&dir.join("no-such-folder")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_git_dir_gets_no_probe() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch_repo("watch-probe-ro");
        let git_dir = dir.join(".git");
        std::fs::set_permissions(&git_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(ProbeFile::place(&git_dir).is_none());
        std::fs::set_permissions(&git_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
