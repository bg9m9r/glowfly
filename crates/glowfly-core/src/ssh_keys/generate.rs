//! Generating Ed25519 key pairs with `ssh-keygen`: where a new key goes, ssh-keygen's
//! arguments, and moving the new pair into place as one change.

use super::listing::read_pair;
use super::{SshKeyPair, pub_path, ssh_dir};
use crate::error::{Error, Result};
use crate::signing::command;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A key to generate. Its `Debug` output never shows the passphrase.
#[derive(Clone, PartialEq)]
pub struct NewKey {
    /// The private key's path; the public key goes next to it with `.pub` added.
    pub path: PathBuf,
    /// Empty: ssh-keygen's default (user@host).
    pub comment: String,
    /// Empty: no passphrase.
    pub passphrase: String,
}

impl std::fmt::Debug for NewKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let passphrase = if self.passphrase.is_empty() { "" } else { "<redacted>" };
        f.debug_struct("NewKey").field("path", &self.path).field("comment", &self.comment).field("passphrase", &passphrase).finish()
    }
}

/// Why a typed save location can't be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationProblem {
    Empty,
    Relative,
    PublicFile,
    NotAFile,
    /// Windows can't use the file name (checked on Windows only: the key stays on this
    /// computer, so elsewhere the name is fine).
    WindowsName,
}

impl LocationProblem {
    pub fn message(self) -> &'static str {
        match self {
            LocationProblem::Empty => "Enter where to save the private key.",
            LocationProblem::Relative => "Enter a full path, such as ~/.ssh/id_ed25519_work.",
            LocationProblem::PublicFile => "Enter the private key's path; the public key is saved next to it with .pub added.",
            LocationProblem::NotAFile => "Enter a file name, not a folder.",
            LocationProblem::WindowsName => {
                "Windows can't use that file name: no device name such as CON or NUL, none of < > : \" | ? *, and no dot or space at the end."
            }
        }
    }
}

/// Where a new key goes unless the user says otherwise.
pub fn default_location(home: &Path) -> PathBuf {
    ssh_dir(home).join("id_ed25519")
}

/// The private key path a typed save location means. `~/` (or `~\`) is the home folder. Pure:
/// nothing is read from the disk.
pub fn resolve_location(text: &str, home: &Path) -> Result<PathBuf, LocationProblem> {
    resolve_location_for(text, home, crate::names::blocks_here())
}

/// [`resolve_location`], refusing a file name Windows can't use when `windows`.
pub fn resolve_location_for(text: &str, home: &Path, windows: bool) -> Result<PathBuf, LocationProblem> {
    let t = text.trim();
    if t.is_empty() {
        return Err(LocationProblem::Empty);
    }
    if t == "~" || t.ends_with('/') || t.ends_with('\\') {
        return Err(LocationProblem::NotAFile);
    }
    let path = match t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")) {
        Some(rest) => rest.split(['/', '\\']).filter(|s| !s.is_empty()).fold(home.to_path_buf(), |p, s| p.join(s)),
        None => PathBuf::from(t),
    };
    if t.to_ascii_lowercase().ends_with(".pub") {
        return Err(LocationProblem::PublicFile);
    }
    if !path.is_absolute() {
        return Err(LocationProblem::Relative);
    }
    let name = t.rsplit(['/', '\\']).next().unwrap_or(t);
    if windows && crate::names::win_name_problem(name).is_some() {
        return Err(LocationProblem::WindowsName);
    }
    Ok(path)
}

/// Why `c` can't be a key comment: it would break the one-line public key.
pub fn comment_problem(c: &str) -> Option<&'static str> {
    c.chars().any(char::is_control).then_some("The comment can't contain line breaks or control characters.")
}

/// ssh-keygen's arguments for writing `new` to `out`. The passphrase is always given (empty for
/// none) so ssh-keygen never prompts; an empty comment leaves ssh-keygen's default.
///
/// ssh-keygen takes a new passphrase only as an argument or from a terminal, so it is briefly
/// visible in the process list to the same user or an administrator. It never reaches a log.
pub fn keygen_args(new: &NewKey, out: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-q", "-t", "ed25519", "-N"].into_iter().map(OsString::from).collect();
    a.push(new.passphrase.clone().into());
    if !new.comment.is_empty() {
        a.push("-C".into());
        a.push(new.comment.clone().into());
    }
    a.push("-f".into());
    a.push(out.as_os_str().to_owned());
    a
}

/// Creates `dir` and any missing parents; on Unix, private to the user (as ssh expects).
fn create_private_dir(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
}

/// Generates an Ed25519 key pair at `new.path` (and `.pub`). Without `overwrite`, an existing
/// file there is `Error::WouldOverwrite`. The pair is written under a temporary name first and
/// then moved into place (see [`install_pair`]), so a failure leaves any old pair as it was.
pub fn generate(keygen: &Path, new: &NewKey, overwrite: bool) -> Result<SshKeyPair> {
    let path = &new.path;
    if path.is_dir() {
        return Err(Error::Other(format!("{} is a folder", path.display())));
    }
    let public = pub_path(path);
    if !overwrite && (path.exists() || public.exists()) {
        return Err(Error::WouldOverwrite(path.display().to_string()));
    }
    let (Some(parent), Some(name)) = (path.parent().filter(|p| !p.as_os_str().is_empty()), path.file_name()) else {
        return Err(Error::Other(format!("{} is not a file path", path.display())));
    };
    create_private_dir(parent)?;
    let tmp = parent.join(format!(".{}.glowfly-new-{}", name.to_string_lossy(), std::process::id()));
    let tmp_pub = pub_path(&tmp);
    let clean = || {
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&tmp_pub);
    };
    clean();
    let mut cmd = command(keygen);
    cmd.args(keygen_args(new, &tmp));
    let out = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            clean();
            return Err(Error::Other(format!("could not run ssh-keygen: {e}")));
        }
    };
    if !out.status.success() {
        clean();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let why = if stderr.is_empty() { out.status.to_string() } else { stderr };
        return Err(Error::Other(format!("ssh-keygen failed: {why}")));
    }
    if let Err(e) = install_pair([&tmp_pub, &tmp], [&public, path]) {
        clean();
        return Err(e.into());
    }
    read_pair(path, Some(keygen)).ok_or_else(|| Error::Other("the new key was not found".into()))
}

/// Moves the files `new` over `at`, in order, as one change: old files there are moved aside
/// first and deleted only once every new file is in place. If a move fails, the new files
/// already moved are taken out and the old ones go back, so the old key pair is never split
/// from its other half or lost. (A rename replaces a file on Windows and Unix alike, but not a
/// folder, and Windows refuses while another program holds the file.)
pub(super) fn install_pair(new: [&Path; 2], at: [&Path; 2]) -> std::io::Result<()> {
    let mut backups: Vec<(&Path, PathBuf)> = Vec::new();
    let undo = |backups: &[(&Path, PathBuf)], installed: &[&Path], e: std::io::Error| {
        for p in installed {
            let _ = crate::persist::remove_file(p);
        }
        let lost: Vec<String> = backups
            .iter()
            .filter(|(orig, backup)| std::fs::rename(backup, orig).is_err())
            .map(|(_, backup)| backup.display().to_string())
            .collect();
        if lost.is_empty() { e } else { std::io::Error::other(format!("{e}; the old key files are kept as {}", lost.join(" and "))) }
    };
    for target in at {
        // A folder in the way stays, and the move onto it fails below.
        if std::fs::symlink_metadata(target).is_ok_and(|m| !m.is_dir()) {
            match crate::persist::move_aside(target) {
                Ok(backup) => backups.push((target, backup)),
                Err(e) => return Err(undo(&backups, &[], e)),
            }
        }
    }
    let mut installed: Vec<&Path> = Vec::new();
    for (from, to) in new.into_iter().zip(at) {
        if let Err(e) = std::fs::rename(from, to) {
            return Err(undo(&backups, &installed, e));
        }
        installed.push(to);
    }
    for (_, backup) in backups {
        let _ = crate::persist::remove_file(&backup);
    }
    Ok(())
}
