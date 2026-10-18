//! How a deleted working copy leaves the disk: moved to the system trash (the Recycle Bin on
//! Windows, the Trash elsewhere) so it can be restored from there, or removed for good.
//!
//! Symbolic links and junctions never go to the trash: some trash implementations resolve links
//! and would move the target, which can be outside the repository. A link is always removed
//! itself, and its target is left alone.

use super::trash::TrashRefusal;
use super::workfile::ensure_inside;
use crate::error::{Error, Result};
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::Path;

/// Moves one absolute path to the system trash. Production code passes [`super::os_trash`];
/// tests pass a stand-in so they never fill the real one.
pub type Trasher = fn(&Path) -> std::result::Result<(), TrashRefusal>;

/// How a working copy is deleted.
#[derive(Clone, Copy, Debug)]
pub enum Removal {
    /// Moved to the trash by this function, so it can be restored.
    Trash(Trasher),
    /// Removed from disk; it can't be brought back.
    Permanent,
}

/// How a working copy was deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deleted {
    Trashed,
    Permanently,
    /// A symbolic link or junction asked to go to the trash, removed itself instead (its target
    /// is untouched).
    LinkRemoved,
}

/// The platform's name for the trash, for messages.
pub(crate) const TRASH_NAME: &str = if cfg!(windows) { "Recycle Bin" } else { "Trash" };

/// Deletes the working copy at `rel` (whose metadata is `m`) the way `removal` asks.
///
/// In trash mode a refusal with the file still in place is `Error::TrashRefused` and nothing
/// has changed; an error after the file went anyway is `Error::Other`.
pub(super) fn remove_working_copy(root: &Path, rel: &str, m: &Metadata, removal: Removal) -> Result<Deleted> {
    let full = root.join(rel);
    let trasher = match removal {
        Removal::Permanent => {
            remove(&full, m)?;
            return Ok(Deleted::Permanently);
        }
        // Junctions count as symbolic links too.
        Removal::Trash(_) if m.file_type().is_symlink() => {
            remove(&full, m)?;
            return Ok(Deleted::LinkRemoved);
        }
        Removal::Trash(t) => t,
    };
    // The trash may resolve links in the parent folders: make sure none was swapped in since the
    // checks.
    ensure_inside(root, full.parent().unwrap_or(root))?;
    match trasher(&full) {
        Ok(()) => Ok(Deleted::Trashed),
        Err(refusal) if full.symlink_metadata().is_ok() => Err(Error::TrashRefused(refusal)),
        Err(reason) => Err(Error::Other(format!("{rel} was removed, but the {TRASH_NAME} reported: {reason}"))),
    }
}

/// Removes `full` from disk.
fn remove(full: &Path, m: &Metadata) -> std::io::Result<()> {
    match std::fs::remove_file(full) {
        // A link to a folder (or a junction) is removed as a folder on Windows; its target stays.
        Err(e) if cfg!(windows) && m.file_type().is_symlink() => std::fs::remove_dir(full).map_err(|_| e),
        Err(e) if cfg!(windows) && e.kind() == ErrorKind::PermissionDenied && m.permissions().readonly() => {
            let mut p = m.permissions();
            // Only on Windows, where this just clears the read-only attribute.
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            std::fs::set_permissions(full, p)?;
            std::fs::remove_file(full)
        }
        r => r,
    }
}
