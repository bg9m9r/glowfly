//! The Recycle Bin on Windows: whether a volume has one, the drive's own settings for it, and
//! what a refusal means.

use super::{BinSettings, TrashProblem, TrashRefusal, problem_of_hresult, problem_of_win32};
use crate::native_path::wide;
use crate::win_registry::dword;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use windows::core::PCWSTR;

pub(super) fn has_recycle_bin(root: &OsStr) -> Result<(), String> {
    use windows::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
    let root = wide(root);
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    // SAFETY: `root` is NUL-terminated and `info` is a properly sized struct; both outlive the call.
    unsafe { SHQueryRecycleBinW(PCWSTR(root.as_ptr()), &mut info) }.map_err(|_| "this drive has no Recycle Bin".to_owned())
}

/// The mount point of the volume `full` is on, ending in '\': `C:\`, a folder a volume is
/// mounted in (`C:\mnt\data\`) or a share (`\\server\share\`). None when Windows can't say.
pub(super) fn volume_mount_point(full: &Path) -> Option<OsString> {
    use windows::Win32::Storage::FileSystem::GetVolumePathNameW;
    let path = wide(full.as_os_str());
    let mut out = [0u16; 1024];
    // SAFETY: `path` is NUL-terminated and `out` is a writable buffer of the length passed.
    unsafe { GetVolumePathNameW(PCWSTR(path.as_ptr()), &mut out) }.ok()?;
    let len = out.iter().position(|&c| c == 0).unwrap_or(out.len());
    let point = String::from_utf16(&out[..len]).ok()?;
    // A verbatim path gives a verbatim mount point; the shell and the registry want the plain
    // form. Anything else verbatim (a volume GUID path) is left to `volume_root_of`.
    let point = match point.strip_prefix(r"\\?\") {
        Some(rest) => match rest.strip_prefix(r"UNC\") {
            Some(share) => format!(r"\\{share}"),
            None if rest.as_bytes().get(1) == Some(&b':') => rest.to_owned(),
            None => return None,
        },
        None => point,
    };
    point.ends_with('\\').then(|| OsString::from(point))
}

/// The volume's GUID (`{…}`) for a drive root such as `C:\`; None for network shares and
/// anything else without a volume name.
fn volume_guid(root: &OsStr) -> Option<String> {
    use windows::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
    let root = wide(root);
    let mut name = [0u16; 64];
    // SAFETY: `root` is NUL-terminated and `name` is a writable buffer of the length passed.
    unsafe { GetVolumeNameForVolumeMountPointW(PCWSTR(root.as_ptr()), &mut name) }.ok()?;
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    // `\\?\Volume{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}\`
    let name = String::from_utf16(&name[..len]).ok()?;
    let start = name.find('{')?;
    let end = name.find('}')?;
    (start < end).then(|| name[start..=end].to_owned())
}

/// The Recycle Bin settings for the drive at `root`, as Explorer's Recycle Bin properties
/// store them for the current user (per volume, under `BitBucket\Volume\{GUID}`), plus the
/// `NoRecycleFiles` policy. Missing values are Windows' defaults: keep files, no size known.
pub(super) fn bin_settings(root: &OsStr) -> BinSettings {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let policy = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer";
    let policy_off =
        [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].into_iter().any(|h| dword(h, policy, "NoRecycleFiles").is_some_and(|v| v != 0));
    let Some(guid) = volume_guid(root) else { return BinSettings { policy_off, ..BinSettings::default() } };
    let key = format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{guid}");
    BinSettings {
        nuke_on_delete: dword(HKEY_CURRENT_USER, &key, "NukeOnDelete").is_some_and(|v| v != 0),
        max_capacity_mb: dword(HKEY_CURRENT_USER, &key, "MaxCapacity").map(u64::from),
        policy_off,
    }
}

/// What the trash's error means. The shell often reports only that the operation was aborted,
/// so the file is then opened the way a move needs (for delete, sharing everything) to see
/// whether another program holds it or access is denied.
pub(super) fn refusal(full: &Path, e: trash::Error) -> TrashRefusal {
    let detail = super::detail_of(&e);
    let problem = match &e {
        trash::Error::Os { code, .. } => problem_of_hresult(*code),
        _ => None,
    };
    TrashRefusal::new(problem.or_else(|| probe(full)).unwrap_or(TrashProblem::Other), detail)
}

fn probe(full: &Path) -> Option<TrashProblem> {
    use std::os::windows::fs::OpenOptionsExt;
    const DELETE: u32 = 0x0001_0000;
    const SHARE_ALL: u32 = 0x7;
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    match std::fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(SHARE_ALL)
        .custom_flags(BACKUP_SEMANTICS | OPEN_REPARSE_POINT)
        .open(full)
    {
        Ok(_) => None,
        Err(e) => e.raw_os_error().and_then(|c| problem_of_win32(c as u32)),
    }
}
