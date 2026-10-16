//! What a refusal from the macOS Trash means. The file manager reports only a localized
//! sentence, so the volume and the file are looked at instead.

use super::{TrashProblem, TrashRefusal, problem_of_volume};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub(super) fn refusal(full: &Path, e: trash::Error) -> TrashRefusal {
    let detail = super::detail_of(&e);
    let problem = full.parent().and_then(probe).or_else(|| locked(full)).unwrap_or(TrashProblem::Other);
    TrashRefusal::new(problem, detail)
}

/// The volume (read-only or on the network) and the folder (writable by this user).
fn probe(dir: &Path) -> Option<TrashProblem> {
    let c = CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::zeroed();
    // SAFETY: `c` is NUL-terminated and `fs` is a properly sized, writable struct.
    if unsafe { libc::statfs(c.as_ptr(), fs.as_mut_ptr()) } == 0 {
        // SAFETY: statfs succeeded, so it filled the struct.
        let flags = unsafe { fs.assume_init() }.f_flags;
        let local = flags & libc::MNT_LOCAL as u32 != 0;
        let read_only = flags & libc::MNT_RDONLY as u32 != 0;
        if let Some(p) = problem_of_volume(local, read_only) {
            return Some(p);
        }
    }
    // SAFETY: `c` is NUL-terminated.
    (unsafe { libc::access(c.as_ptr(), libc::W_OK) } != 0).then_some(TrashProblem::AccessDenied)
}

/// A file locked in the Finder (the immutable flags) can't be moved.
fn locked(full: &Path) -> Option<TrashProblem> {
    use std::os::macos::fs::MetadataExt;
    let flags = std::fs::symlink_metadata(full).ok()?.st_flags();
    (flags & (libc::UF_IMMUTABLE | libc::SF_IMMUTABLE) != 0).then_some(TrashProblem::AccessDenied)
}
