//! Moving a file to a new name without ever replacing a file already there.
//!
//! `std::fs::rename` replaces an existing target on every system, so checking first and then
//! renaming leaves a window in which a file that appears is overwritten. These moves let the
//! file system refuse instead, in the same step that moves the file.

use std::io;
use std::path::Path;

/// Moves `from` to `to` in one step, failing with `ErrorKind::AlreadyExists` when `to` exists.
#[cfg(windows)]
pub(crate) fn rename_noclobber(from: &Path, to: &Path) -> io::Result<()> {
    use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
    use windows::core::PCWSTR;
    let (from, to) = (wide(from), wide(to));
    // Without MOVEFILE_REPLACE_EXISTING, an existing target fails the call.
    // SAFETY: both buffers are NUL-terminated and outlive the call.
    unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), MOVE_FILE_FLAGS(0)) }.map_err(|e| {
        // A Win32 error wrapped in an HRESULT (0x8007xxxx): unwrap it so ErrorKind works.
        let code = e.code().0 as u32;
        if code & 0xFFFF_0000 == 0x8007_0000 { io::Error::from_raw_os_error((code & 0xFFFF) as i32) } else { e.into() }
    })
}

/// `path` as a NUL-terminated wide string with '\' separators; a long absolute path gets the
/// `\\?\` prefix, as the standard library adds, so it isn't cut off at 260 characters.
#[cfg(windows)]
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let (slash, back) = (u16::from(b'/'), u16::from(b'\\'));
    let mut v: Vec<u16> = path.as_os_str().encode_wide().map(|c| if c == slash { back } else { c }).collect();
    let lit = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    if v.len() >= 248 && !v.starts_with(&lit("\\\\?\\")) {
        if v.starts_with(&lit("\\\\")) {
            v.splice(..2, lit("\\\\?\\UNC\\"));
        } else if v.len() > 2 && v[1] == u16::from(b':') && v[2] == back {
            v.splice(..0, lit("\\\\?\\"));
        }
    }
    v.push(0);
    v
}

/// Moves `from` to `to`, failing with `ErrorKind::AlreadyExists` when `to` exists: a hard link
/// is made under the new name (which the system refuses if the name is taken), then the old name
/// is removed. File systems without hard links fall back to a checked rename.
#[cfg(not(windows))]
pub(crate) fn rename_noclobber(from: &Path, to: &Path) -> io::Result<()> {
    match std::fs::hard_link(from, to) {
        Ok(()) => std::fs::remove_file(from).inspect_err(|_| {
            let _ = std::fs::remove_file(to);
        }),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(e) => {
            // No hard links here (FAT, some network shares): the best left is check, then move.
            if to.symlink_metadata().is_ok() {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, e));
            }
            std::fs::rename(from, to)
        }
    }
}
