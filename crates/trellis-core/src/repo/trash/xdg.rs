//! Linux and the BSDs: whether the trash folder on a file's drive can be used, checked the way
//! the trash crate picks one before it is called. When it can't, the crate would copy the file
//! into the home Trash without saying so (or fail); Trellis asks first instead
//! (`TrashProblem::HomeTrashOnly`). The decision itself is the pure `volume_trash_usable`.

use super::home::{AdminTrash, UserTrash, VolumeTrashFacts, home_trash_dir, parse_mounts, topdir_of, volume_trash_usable};
use super::{TrashProblem, TrashRefusal, no_home_trash};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Checks that the trash can take `full` without a silent copy across drives. Anything this
/// can't find out (no mount table, a path that won't resolve) is left to the crate, as before.
pub(super) fn check_volume(full: &Path) -> Result<(), TrashRefusal> {
    if no_home_trash(|k| std::env::var_os(k)) {
        return Err(TrashRefusal::new(TrashProblem::NoTrash, "neither XDG_DATA_HOME nor HOME is set, so there is no Trash"));
    }
    let table = std::fs::read_to_string("/proc/self/mounts").or_else(|_| std::fs::read_to_string("/etc/mtab"));
    let Ok(table) = table else { return Ok(()) };
    let mounts = parse_mounts(&table);
    let (Some(parent), Some(name)) = (full.parent(), full.file_name()) else { return Ok(()) };
    let Ok(parent) = parent.canonicalize() else { return Ok(()) };
    if !writable(&parent) {
        return Err(TrashRefusal::new(TrashProblem::AccessDenied, format!("you may not change {}", parent.display())));
    }
    let file = parent.join(name);
    let Some(home) = home_trash_dir(|k| std::env::var_os(k)).and_then(|d| existing_ancestor(&d)) else { return Ok(()) };
    let (Some(file_top), Some(home_top)) = (topdir_of(&file, &mounts), topdir_of(&home, &mounts)) else { return Ok(()) };
    if file_top == home_top || volume_trash_usable(&facts(file_top)) {
        return Ok(());
    }
    Err(TrashRefusal::new(TrashProblem::HomeTrashOnly, format!("{} has no trash folder you can use", file_top.display())))
}

/// The nearest folder at or above `p` that exists, resolved: the home Trash may not be made yet.
fn existing_ancestor(p: &Path) -> Option<PathBuf> {
    p.ancestors().find_map(|a| a.canonicalize().ok())
}

/// The trash folders on the drive whose top folder is `top`, as the crate looks at them.
fn facts(top: &Path) -> VolumeTrashFacts {
    // SAFETY: getuid takes no arguments and cannot fail.
    let uid = unsafe { libc::getuid() };
    let shared = top.join(".Trash");
    let admin = match std::fs::symlink_metadata(&shared) {
        Err(_) => AdminTrash::Missing,
        Ok(m) if m.file_type().is_symlink() || !m.is_dir() || m.permissions().mode() & 0o1000 == 0 => AdminTrash::Invalid,
        Ok(_) => {
            let mine = shared.join(uid.to_string());
            if mine.symlink_metadata().is_ok() { AdminTrash::UserFolder { writable: writable(&mine) } } else { AdminTrash::NoUserFolder }
        }
    };
    let own = top.join(format!(".Trash-{uid}"));
    let user = match (std::fs::metadata(&own), own.symlink_metadata()) {
        (Ok(m), _) if m.is_dir() => UserTrash::Folder { writable: writable(&own) },
        // A file, or a link to nothing.
        (Ok(_), _) | (Err(_), Ok(_)) => UserTrash::NotAFolder,
        (Err(_), Err(_)) => UserTrash::Missing { topdir_writable: writable(top) },
    };
    VolumeTrashFacts { admin, user }
}

/// Whether this user may write in `p` (create and remove entries, for a folder).
fn writable(p: &Path) -> bool {
    let Ok(c) = CString::new(p.as_os_str().as_bytes()) else { return false };
    // SAFETY: `c` is NUL-terminated.
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}
