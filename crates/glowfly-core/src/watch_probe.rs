//! The file watcher's self-test: a scratch file placed in the repository's git dir right after
//! the watcher starts, whose event shows that changes reach Glowfly (some network drives, WSL
//! and 9P mounts accept a watch and then never report anything). The file is named `*.lock`, so
//! git and the watcher's classification ignore it like git's own lock files, sits in the git
//! dir (a linked worktree's own, never the work tree), and is removed as soon as it is seen, at
//! the timeout, or when the probe is dropped. A git dir that can't be written gets no probe.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
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
