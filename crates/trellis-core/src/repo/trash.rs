//! The system trash (the Recycle Bin on Windows, the Trash elsewhere), and why it refused a
//! file. Only [`TrashProblem::NoTrash`] means a permanent delete is the way out; a file in use or
//! a permission problem is worth fixing and retrying instead.

use std::path::Path;

/// Why the trash didn't take a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashProblem {
    /// There is definitely no trash for this file: the volume has none (a network share, some
    /// removable or RAM drives), it is turned off for the drive, or the file is too big for it.
    NoTrash,
    /// Another program has the file open (Windows).
    InUse,
    /// Not allowed: permissions, a locked file, a read-only volume.
    AccessDenied,
    /// Anything else. Retrying may help; nothing says the trash is missing.
    Other,
}

/// A refusal from the trash; the file is still where it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashRefusal {
    pub problem: TrashProblem,
    /// What the system said, for the message.
    pub detail: String,
}

impl TrashRefusal {
    pub fn new(problem: TrashProblem, detail: impl Into<String>) -> Self {
        Self { problem, detail: detail.into() }
    }
}

impl std::fmt::Display for TrashRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

/// Moves `full` to the system trash, or says why it can't. A refusal leaves the file in place
/// unless the trash itself failed halfway.
pub fn os_trash(full: &Path) -> Result<(), TrashRefusal> {
    // This crate's unit tests use stand-ins; the real trash only when asked for by hand.
    if cfg!(test) && std::env::var_os("TRELLIS_REAL_TRASH").is_none() {
        return Err(TrashRefusal::new(TrashProblem::Other, "tests never use the real trash"));
    }
    #[cfg(windows)]
    {
        // Without a Recycle Bin on the volume, or with it turned off, the shell would destroy the
        // file without asking, as no UI is shown: refuse first.
        let no_trash = |why: &str| TrashRefusal::new(TrashProblem::NoTrash, why);
        let root = volume_root_of(full).ok_or_else(|| no_trash("this path has no Recycle Bin"))?;
        win::has_recycle_bin(&root).map_err(|why| no_trash(&why))?;
        let size = std::fs::symlink_metadata(full).map(|m| if m.is_file() { m.len() } else { 0 }).unwrap_or(0);
        if let Some(why) = bin_refuses(&win::bin_settings(&root), size) {
            return Err(no_trash(&why));
        }
        trash::delete(full).map_err(|e| win::refusal(full, e))
    }
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // The file manager's method needs no automation permission, unlike asking the Finder.
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(full).map_err(|e| mac::refusal(full, e))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        trash::delete(full).map_err(|e| {
            let problem = match &e {
                trash::Error::FileSystem { source, .. } => problem_of_io(source.kind()),
                _ if no_home_trash(|k| std::env::var_os(k)) => Some(TrashProblem::NoTrash),
                _ => None,
            };
            TrashRefusal::new(problem.unwrap_or(TrashProblem::Other), detail_of(&e))
        })
    }
}

/// What an I/O error from the trash means.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
pub(crate) fn problem_of_io(kind: std::io::ErrorKind) -> Option<TrashProblem> {
    use std::io::ErrorKind as K;
    match kind {
        K::PermissionDenied | K::ReadOnlyFilesystem => Some(TrashProblem::AccessDenied),
        K::ResourceBusy | K::ExecutableFileBusy => Some(TrashProblem::InUse),
        _ => None,
    }
}

/// Whether there is nowhere for a home trash (freedesktop): neither `XDG_DATA_HOME` nor `HOME`
/// is set.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
pub(crate) fn no_home_trash(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> bool {
    ["XDG_DATA_HOME", "HOME"].iter().all(|k| var(k).is_none_or(|v| v.is_empty()))
}

/// What a Windows error code (from `GetLastError`) means.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn problem_of_win32(code: u32) -> Option<TrashProblem> {
    match code {
        // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION
        32 | 33 => Some(TrashProblem::InUse),
        // ERROR_ACCESS_DENIED, ERROR_WRITE_PROTECT
        5 | 19 => Some(TrashProblem::AccessDenied),
        _ => None,
    }
}

/// What an HRESULT from the shell's file operation means: its copy-engine codes, or a wrapped
/// Windows error.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn problem_of_hresult(hr: i32) -> Option<TrashProblem> {
    let hr = hr as u32;
    match hr {
        // COPYENGINE_E_SHARING_VIOLATION_SRC, _DEST
        0x8027_0027 | 0x8027_0028 => Some(TrashProblem::InUse),
        // COPYENGINE_E_ACCESS_DENIED_SRC, _DEST, COPYENGINE_E_REQUIRES_ELEVATION
        0x8027_0021 | 0x8027_0022 | 0x8027_0002 => Some(TrashProblem::AccessDenied),
        // COPYENGINE_E_RECYCLE_FORCE_NUKE, _SIZE_TOO_BIG, _PATH_TOO_LONG, _BIN_NOT_FOUND
        0x8027_0036 | 0x8027_0037 | 0x8027_0038 | 0x8027_003A => Some(TrashProblem::NoTrash),
        // HRESULT_FROM_WIN32
        _ if hr & 0xFFFF_0000 == 0x8007_0000 => problem_of_win32(hr & 0xFFFF),
        _ => None,
    }
}

/// What a volume's flags mean once the trash refused a file on it (macOS): a read-only volume
/// can't take any change, and a network volume has no Trash.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn problem_of_volume(local: bool, read_only: bool) -> Option<TrashProblem> {
    match (local, read_only) {
        (_, true) => Some(TrashProblem::AccessDenied),
        (false, false) => Some(TrashProblem::NoTrash),
        (true, false) => None,
    }
}

/// A drive's Recycle Bin settings (Windows): "Don't move files to the Recycle Bin", its maximum
/// size, and the group policy that turns it off everywhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct BinSettings {
    pub nuke_on_delete: bool,
    pub max_capacity_mb: Option<u64>,
    pub policy_off: bool,
}

/// Why the Recycle Bin with `settings` would destroy a file of `size` bytes instead of keeping
/// it (the shell does that without asking when no UI is shown), or `None` when it keeps it.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn bin_refuses(settings: &BinSettings, size: u64) -> Option<String> {
    if settings.policy_off {
        return Some("a policy on this computer turns the Recycle Bin off".into());
    }
    if settings.nuke_on_delete {
        return Some("the Recycle Bin is turned off for this drive (\"Don't move files to the Recycle Bin\")".into());
    }
    match settings.max_capacity_mb {
        Some(mb) if size > mb.saturating_mul(1024 * 1024) => Some(format!("it's larger than this drive's Recycle Bin ({mb} MB)")),
        _ => None,
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
mod win;

#[cfg(target_os = "macos")]
mod mac;

#[cfg(test)]
mod tests;

/// What the trash said, without the crate's wrapping.
fn detail_of(e: &trash::Error) -> String {
    match e {
        trash::Error::Unknown { description } | trash::Error::Os { description, .. } => description.clone(),
        #[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))]
        trash::Error::FileSystem { path, source } => format!("{}: {source}", path.display()),
        other => other.to_string(),
    }
}
