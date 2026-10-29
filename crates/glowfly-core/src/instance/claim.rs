//! Which window takes what a second process hands over: the one holding the lock on the
//! instance lock file ([`LOCK_NAME`](super::LOCK_NAME)). The OS releases the lock when its
//! process ends, crashed or not, so a lock is never stale, and only one process holds it at a
//! time, so two windows never both take links.
//!
//! The lock file is never removed or replaced: on Unix a lock belongs to the file's inode, so a
//! second file at the same path would let a second process lock it too.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// This process holds the lock; dropping it lets the next window take over.
#[derive(Debug)]
pub struct Claim {
    _lock: File,
}

/// What [`claim`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claimed {
    /// No other window runs: this process takes links and folders from now on.
    Here,
    /// Another window runs and takes them.
    Elsewhere,
}

/// The claim [`claim`] took, kept for [`serve`](super::serve).
static HELD: Mutex<Option<Claim>> = Mutex::new(None);

/// The lock file next to `file`, the instance file.
pub(super) fn lock_path(file: &Path) -> PathBuf {
    file.with_file_name(super::LOCK_NAME)
}

/// Opens (creating, never truncating or replacing) the lock file at `path`, readable by this
/// user only on Unix, in a folder only this user may enter when this call creates it.
pub(super) fn open_lock(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty())
        && std::fs::symlink_metadata(dir).is_err()
    {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
    }
    let mut open = OpenOptions::new();
    open.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    open.open(path)
}

/// Takes the lock at `path` when no other process holds it; None when one does.
pub fn claim_at(path: &Path) -> io::Result<Option<Claim>> {
    let file = open_lock(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(Claim { _lock: file })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

/// Waits until the lock `file` was opened on is free, and takes it.
pub(super) fn wait_for(file: File) -> io::Result<Claim> {
    file.lock()?;
    Ok(Claim { _lock: file })
}

/// Takes the lock in the runtime folder for this process if no other window holds it, and keeps
/// it for the listener. Called once at start-up, before deciding whether to hand the command
/// line over; an error (no runtime folder, a file system without locks) means nothing can be
/// told about other windows.
pub fn claim() -> io::Result<Claimed> {
    let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
    if held.is_some() {
        return Ok(Claimed::Here);
    }
    let file = super::default_file().ok_or_else(|| io::Error::other("there is no folder for the instance file (HOME isn't set)"))?;
    Ok(match claim_at(&lock_path(&file))? {
        Some(c) => {
            *held = Some(c);
            Claimed::Here
        }
        None => Claimed::Elsewhere,
    })
}

/// The claim [`claim`] took, if it took one.
pub(super) fn take_held() -> Option<Claim> {
    HELD.lock().unwrap_or_else(PoisonError::into_inner).take()
}
