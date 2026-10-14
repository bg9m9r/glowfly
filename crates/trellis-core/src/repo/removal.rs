//! How a deleted working copy leaves the disk: moved to the system trash (the Recycle Bin on
//! Windows, the Trash elsewhere) so it can be restored from there, or removed for good.
//!
//! Symbolic links and junctions never go to the trash: some trash implementations resolve links
//! and would move the target, which can be outside the repository. A link is always removed
//! itself, and its target is left alone.

use super::workfile::ensure_inside;
use crate::error::{Error, Result};
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::Path;

/// Moves one absolute path to the system trash. Production code passes [`os_trash`]; tests pass
/// a stand-in so they never fill the real one.
pub type Trasher = fn(&Path) -> std::result::Result<(), String>;

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
const TRASH_NAME: &str = if cfg!(windows) { "Recycle Bin" } else { "Trash" };

/// Moves `full` to the system trash, or says why it can't. A refusal leaves the file in place
/// unless the trash itself failed halfway.
pub fn os_trash(full: &Path) -> std::result::Result<(), String> {
    // This crate's unit tests use stand-ins; the real trash only when asked for by hand.
    if cfg!(test) && std::env::var_os("TRELLIS_REAL_TRASH").is_none() {
        return Err("tests never use the real trash".into());
    }
    #[cfg(windows)]
    {
        // Without a Recycle Bin on the volume (network shares, some removable or RAM drives), the
        // shell would destroy the file without asking, as no UI is shown.
        let root = volume_root_of(full).ok_or("this path has no Recycle Bin")?;
        has_recycle_bin(&root)?;
        trash::delete(full).map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // The file manager's method needs no automation permission, unlike asking the Finder.
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(full).map_err(|e| e.to_string())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        trash::delete(full).map_err(|e| e.to_string())
    }
}

/// The root of the volume `full` is on, as the shell names it: `C:\` or `\\server\share\`. A
/// root is short, so it never runs into the path length limit. None for other path forms.
#[cfg(windows)]
pub(super) fn volume_root_of(full: &Path) -> Option<std::ffi::OsString> {
    use std::path::{Component, Prefix};
    let Some(Component::Prefix(p)) = full.components().next() else { return None };
    let mut root = std::ffi::OsString::new();
    match p.kind() {
        Prefix::Disk(d) | Prefix::VerbatimDisk(d) => root.push(format!("{}:\\", char::from(d))),
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            root.push("\\\\");
            root.push(server);
            root.push("\\");
            root.push(share);
            root.push("\\");
        }
        _ => return None,
    }
    Some(root)
}

#[cfg(windows)]
fn has_recycle_bin(root: &std::ffi::OsStr) -> std::result::Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
    use windows::core::PCWSTR;
    let wide: Vec<u16> = root.encode_wide().chain(std::iter::once(0)).collect();
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    // SAFETY: `wide` is NUL-terminated and `info` is a properly sized struct; both outlive the call.
    unsafe { SHQueryRecycleBinW(PCWSTR(wide.as_ptr()), &mut info) }.map_err(|_| "this drive has no Recycle Bin".to_owned())
}

/// Deletes the working copy at `rel` (whose metadata is `m`) the way `removal` asks.
///
/// In trash mode a refusal with the file still in place is `Error::NoTrash` and nothing has
/// changed; an error after the file went anyway is `Error::Other`.
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
        Err(reason) if full.symlink_metadata().is_ok() => Err(Error::NoTrash(reason)),
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
