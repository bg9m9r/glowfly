//! The private folder for the copies an external diff or merge tool is given: one session
//! folder per opening, each version in a folder named after it (`base/`, `ours/`, `abc1234/`)
//! under the file's own name, so tools show which is which and highlight by extension.
//!
//! The root is `glowfly-tools` in the system's temporary folder. On Unix that folder is shared,
//! so it is used only when it is a real folder (not a link) that this user owns and nobody else
//! can open (mode 0700); otherwise the copies go to Glowfly's cache folder instead. Copies are
//! read-only, except the merge tool's output, which is the working file itself.

use crate::persist::{WriteOptions, remove_tree, write_file};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// The roots this process made sessions in, for [`remove_own_sessions`].
static USED_ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// How old a session folder must be before a new session sweeps it away. Diff tools often hand
/// their files to a window that is already open, so a diff session is never removed when the
/// program exits; it goes when Glowfly quits ([`remove_own_sessions`]), and this sweep catches
/// those a Glowfly that didn't quit cleanly left.
pub const SWEEP_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The folder sessions are made in: `glowfly-tools` in the temporary folder when it is private
/// (see the module docs), else `tool-files` in Glowfly's cache folder.
pub fn root() -> io::Result<PathBuf> {
    root_in(&crate::paths::temp_dir().join("glowfly-tools"), crate::settings::cache_dir().map(|c| c.join("tool-files")).as_deref())
}

/// `shared` when [`private_root`] accepts it, else `fallback`.
pub(crate) fn root_in(shared: &Path, fallback: Option<&Path>) -> io::Result<PathBuf> {
    match private_root(shared) {
        Ok(p) => Ok(p),
        Err(e) => private_root(fallback.ok_or(e)?),
    }
}

/// `base`, created if missing (mode 0700 on Unix), when it is a real folder only this user can
/// open; an error otherwise.
pub(crate) fn private_root(base: &Path) -> io::Result<PathBuf> {
    make_dir(base, true)?;
    let meta = fs::symlink_metadata(base)?;
    if !meta.is_dir() {
        return Err(io::Error::other(format!("{} is not a folder", base.display())));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.mode() & 0o077 != 0 {
            return Err(io::Error::other(format!("{} can be opened by other users", base.display())));
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        // SAFETY: geteuid has no preconditions and cannot fail.
        if meta.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::other(format!("{} belongs to another user", base.display())));
        }
    }
    Ok(base.to_owned())
}

/// Creates the folder `p` (and with `parents`, the folders above it), private on Unix. An
/// existing folder is fine.
fn make_dir(p: &Path, parents: bool) -> io::Result<()> {
    let mut b = fs::DirBuilder::new();
    b.recursive(parents);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    match b.create(p) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && !parents => Err(e),
        r => r,
    }
}

/// One opening of a tool: a fresh folder under the root holding that opening's copies.
#[derive(Debug)]
pub struct Session {
    dir: PathBuf,
}

impl Session {
    /// A new, empty session folder in `root`, after sweeping away sessions older than
    /// [`SWEEP_AGE`].
    pub fn create(root: &Path) -> io::Result<Session> {
        sweep(root, SWEEP_AGE, SystemTime::now());
        if let Ok(mut used) = USED_ROOTS.lock()
            && !used.iter().any(|r| r == root)
        {
            used.push(root.to_owned());
        }
        for _ in 0..16 {
            let mut bytes = [0u8; 8];
            getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            let dir = root.join(format!("{}-{hex}", std::process::id()));
            match make_dir(&dir, false) {
                Ok(()) => return Ok(Session { dir }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other("could not make a new folder for the tool's files"))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Writes `bytes` as `<label>/<file name of repo_path>` in the session (`<label>-2/` and so
    /// on when `<label>` is taken), read-only. Returns the file's path.
    pub fn write(&self, label: &str, repo_path: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let label = match crate::names::portable(label, "-v") {
            l if l.is_empty() => "version".to_owned(),
            l => l,
        };
        let mut folder = self.dir.join(&label);
        let mut n = 2;
        while folder.symlink_metadata().is_ok() {
            folder = self.dir.join(format!("{label}-{n}"));
            n += 1;
        }
        make_dir(&folder, false)?;
        let file = folder.join(file_name_for(repo_path));
        write_file(&file, bytes, WriteOptions::default())?;
        if let Ok(m) = fs::metadata(&file) {
            let mut perms = m.permissions();
            perms.set_readonly(true);
            let _ = fs::set_permissions(&file, perms);
        }
        Ok(file)
    }

    /// Deletes the session folder and every copy in it (read-only ones too).
    pub fn remove(self) -> io::Result<()> {
        remove_tree(&self.dir)
    }
}

/// Removes the folders directly in `root` last changed before `now - older_than`. Links and
/// files are left alone, and nothing that fails stops the rest.
pub fn sweep(root: &Path, older_than: Duration, now: SystemTime) {
    let Some(cutoff) = now.checked_sub(older_than) else { return };
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() && meta.modified().is_ok_and(|m| m < cutoff) {
            let _ = remove_tree(&path);
        }
    }
}

/// Removes the sessions this process made, in every root it used: for when Glowfly quits, so a
/// diff tool's copies don't wait for the sweep. A copy a tool still holds open (Windows) stays
/// for the sweep.
pub fn remove_own_sessions() {
    let roots = USED_ROOTS.lock().map(|r| r.clone()).unwrap_or_default();
    for root in roots {
        remove_sessions_of(&root, std::process::id());
    }
}

/// Removes the session folders process `pid` made directly in `root` (named `<pid>-<hex>`).
/// Links and files are left alone, and nothing that fails stops the rest.
pub fn remove_sessions_of(root: &Path, pid: u32) {
    let prefix = format!("{pid}-");
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let mine = entry.file_name().to_str().is_some_and(|n| n.strip_prefix(&prefix).is_some_and(|hex| !hex.is_empty()));
        let path = entry.path();
        if mine && fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            let _ = remove_tree(&path);
        }
    }
}

/// Whether this process made a session in `root`.
#[cfg(test)]
pub(crate) fn used_root(root: &Path) -> bool {
    USED_ROOTS.lock().is_ok_and(|r| r.iter().any(|u| u == root))
}

/// The name a copy of `repo_path` gets: its last part, with any backslash made '-' (so a name
/// that has one on Unix stays one name) and changed just enough for Windows to hold it
/// (`names::portable`), or "file" when nothing is left. The extension is kept.
pub fn file_name_for(repo_path: &str) -> String {
    let last = repo_path.rsplit('/').next().unwrap_or(repo_path).replace('\\', "-");
    match crate::names::portable(&last, "-file") {
        n if n.is_empty() => "file".to_owned(),
        n => n,
    }
}

#[cfg(test)]
#[path = "temp_tests.rs"]
mod tests;
