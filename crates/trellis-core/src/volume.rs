//! Whether a folder is on a network or virtual file system, where file watching is unreliable:
//! NFS, SMB/CIFS, WSL's 9P (`/mnt/c` inside WSL, `\\wsl$` from Windows), FUSE mounts, macOS
//! network volumes. Their changes made by other machines (or the other side of WSL) send no
//! events, so the app says so up front and re-reads such repositories on a timer.

use std::path::Path;

/// The kind of remote or virtual file system `path` is on ("SMB", "NFS", "WSL"…), or None for a
/// local disk or when it can't be told.
pub fn remote_kind(path: &Path) -> Option<&'static str> {
    platform::remote_kind(path)
}

/// Linux: the file system kind for a `statfs` `f_type` magic number, when it is remote or
/// virtual.
pub fn linux_remote_kind(magic: u32) -> Option<&'static str> {
    Some(match magic {
        0x6969 => "NFS",
        0xFF53_4D42 | 0xFE53_4D42 | 0x517B => "SMB",
        0x0102_1997 => "9P (WSL or a virtual machine share)",
        0x6573_5546 => "FUSE",
        0x5346_414F => "AFS",
        0x00C3_6400 => "Ceph",
        0x7375_7245 => "Coda",
        0x564C => "NCP",
        _ => return None,
    })
}

/// macOS: from `statfs`: a volume without `MNT_LOCAL` is remote, named by its file system type
/// (`smbfs`, `nfs`, `afpfs`, `webdav`).
pub fn macos_remote_kind(local: bool, fs_type: &str) -> Option<&'static str> {
    if local {
        return None;
    }
    Some(match fs_type {
        "smbfs" => "SMB",
        "nfs" => "NFS",
        "afpfs" => "AFP",
        "webdav" => "WebDAV",
        _ => "a network volume",
    })
}

/// `GetDriveTypeW`'s answer for a network drive.
pub const DRIVE_REMOTE: u32 = 4;

/// Windows: `path` (as text) on a drive of `drive_type`. WSL's shares (`\\wsl$`,
/// `\\wsl.localhost`) are named as such; any other share or network drive is SMB.
pub fn windows_remote_kind(path: &str, drive_type: Option<u32>) -> Option<&'static str> {
    let lower = path.replace('/', "\\").to_ascii_lowercase();
    let lower = lower.strip_prefix(r"\\?\unc\").map(|r| format!(r"\\{r}")).unwrap_or(lower);
    if lower.starts_with(r"\\wsl$\") || lower.starts_with(r"\\wsl.localhost\") {
        return Some("WSL");
    }
    (drive_type == Some(DRIVE_REMOTE) || (lower.starts_with(r"\\") && !lower.starts_with(r"\\?\") && !lower.starts_with(r"\\.\")))
        .then_some("SMB")
}

#[cfg(target_os = "linux")]
mod platform {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    pub fn remote_kind(path: &Path) -> Option<&'static str> {
        let c = CString::new(path.as_os_str().as_bytes()).ok()?;
        let mut fs = std::mem::MaybeUninit::<libc::statfs>::zeroed();
        // SAFETY: `c` is NUL-terminated and `fs` is a properly sized, writable struct.
        if unsafe { libc::statfs(c.as_ptr(), fs.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: statfs succeeded, so it filled the struct.
        let magic = unsafe { fs.assume_init() }.f_type;
        // The magic numbers are 32-bit; `f_type`'s width and sign vary by architecture.
        super::linux_remote_kind(magic as u32)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    pub fn remote_kind(path: &Path) -> Option<&'static str> {
        let c = CString::new(path.as_os_str().as_bytes()).ok()?;
        let mut fs = std::mem::MaybeUninit::<libc::statfs>::zeroed();
        // SAFETY: `c` is NUL-terminated and `fs` is a properly sized, writable struct.
        if unsafe { libc::statfs(c.as_ptr(), fs.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: statfs succeeded, so it filled the struct.
        let fs = unsafe { fs.assume_init() };
        let local = fs.f_flags & libc::MNT_LOCAL as u32 != 0;
        // SAFETY: f_fstypename is a NUL-terminated name within its array.
        let name = unsafe { CStr::from_ptr(fs.f_fstypename.as_ptr()) }.to_string_lossy();
        super::macos_remote_kind(local, &name)
    }
}

#[cfg(windows)]
mod platform {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Path, Prefix};
    use windows::core::PCWSTR;

    /// The volume root `GetDriveTypeW` takes: `C:\` or `\\server\share\`.
    fn root(path: &Path) -> Option<std::ffi::OsString> {
        let Some(Component::Prefix(p)) = path.components().next() else { return None };
        let mut root = std::ffi::OsString::new();
        match p.kind() {
            Prefix::Disk(d) | Prefix::VerbatimDisk(d) => root.push(format!("{}:\\", char::from(d))),
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                root.push(r"\\");
                root.push(server);
                root.push(r"\");
                root.push(share);
                root.push(r"\");
            }
            _ => return None,
        }
        Some(root)
    }

    pub fn remote_kind(path: &Path) -> Option<&'static str> {
        let drive_type = root(path).map(|r| {
            let wide: Vec<u16> = r.encode_wide().chain(std::iter::once(0)).collect();
            // SAFETY: `wide` is NUL-terminated and outlives the call.
            unsafe { windows::Win32::Storage::FileSystem::GetDriveTypeW(PCWSTR(wide.as_ptr())) }
        });
        super::windows_remote_kind(&path.to_string_lossy(), drive_type)
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod platform {
    pub fn remote_kind(_: &std::path::Path) -> Option<&'static str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_file_systems_by_magic() {
        assert_eq!(linux_remote_kind(0x6969), Some("NFS"));
        assert_eq!(linux_remote_kind(0xFF53_4D42), Some("SMB"));
        assert_eq!(linux_remote_kind(0xFE53_4D42), Some("SMB"));
        assert!(linux_remote_kind(0x0102_1997).is_some_and(|k| k.contains("WSL")));
        assert_eq!(linux_remote_kind(0x6573_5546), Some("FUSE"));
        // ext4, btrfs, xfs, tmpfs, overlayfs are local.
        for local in [0xEF53, 0x9123_683E, 0x5846_5342, 0x0102_1994, 0x794C_7630] {
            assert_eq!(linux_remote_kind(local), None, "{local:#x}");
        }
    }

    #[test]
    fn macos_volumes_by_flag() {
        assert_eq!(macos_remote_kind(true, "apfs"), None);
        assert_eq!(macos_remote_kind(false, "smbfs"), Some("SMB"));
        assert_eq!(macos_remote_kind(false, "nfs"), Some("NFS"));
        assert_eq!(macos_remote_kind(false, "macfuse"), Some("a network volume"));
    }

    #[test]
    fn windows_drives_and_shares() {
        assert_eq!(windows_remote_kind(r"C:\work\app", Some(3)), None, "a fixed disk");
        assert_eq!(windows_remote_kind(r"Z:\work\app", Some(DRIVE_REMOTE)), Some("SMB"), "a mapped drive");
        assert_eq!(windows_remote_kind(r"\\nas\team\app", None), Some("SMB"));
        assert_eq!(windows_remote_kind("//nas/team/app", Some(DRIVE_REMOTE)), Some("SMB"));
        assert_eq!(windows_remote_kind(r"\\wsl$\Ubuntu\home\me\app", Some(DRIVE_REMOTE)), Some("WSL"));
        assert_eq!(windows_remote_kind(r"\\WSL.localhost\Ubuntu\home\me\app", None), Some("WSL"));
        assert_eq!(windows_remote_kind(r"\\?\UNC\wsl.localhost\Ubuntu\x", None), Some("WSL"));
        assert_eq!(windows_remote_kind(r"\\?\C:\work", Some(3)), None);
    }

    #[test]
    fn the_temporary_folder_is_local() {
        assert_eq!(remote_kind(&crate::paths::temp_dir()), None);
    }
}
