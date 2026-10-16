//! A folder's identity on disk, whatever it is called: the same on every spelling of it (a link,
//! a short 8.3 name, another case on a volume that ignores case, another Unicode form on one that
//! normalizes names), and different for two folders whose names differ only in case on a volume
//! that keeps them apart. Never saved: device numbers can change when a volume is mounted again.

use std::path::Path;

/// Device and inode on Unix; volume serial number and file id on Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FolderId {
    volume: u64,
    file: u128,
}

impl FolderId {
    /// The identity of what `path` names (links followed). None when it doesn't exist, can't be
    /// read, or lives on a volume that gives no file ids (some network shares report 0).
    pub fn of(path: &Path) -> Option<FolderId> {
        let id = imp::of(path)?;
        (id.file != 0).then_some(id)
    }

    /// An identity made up for tests.
    #[doc(hidden)]
    pub fn for_tests(n: u64) -> FolderId {
        FolderId { volume: u64::MAX, file: u128::from(n) + 1 }
    }
}

#[cfg(unix)]
mod imp {
    use super::FolderId;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    pub(super) fn of(path: &Path) -> Option<FolderId> {
        let meta = std::fs::metadata(path).ok()?;
        Some(FolderId { volume: meta.dev(), file: u128::from(meta.ino()) })
    }
}

#[cfg(windows)]
mod imp {
    use super::FolderId;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO, FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
    };

    /// FILE_READ_ATTRIBUTES: enough to ask for ids, and allowed where reading isn't.
    const READ_ATTRIBUTES: u32 = 0x80;
    /// FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE: never in anyone's way.
    const SHARE_ALL: u32 = 0x7;
    /// FILE_FLAG_BACKUP_SEMANTICS: needed to open a folder.
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;

    pub(super) fn of(path: &Path) -> Option<FolderId> {
        let file = std::fs::OpenOptions::new()
            .access_mode(READ_ATTRIBUTES)
            .share_mode(SHARE_ALL)
            .custom_flags(BACKUP_SEMANTICS)
            .open(path)
            .ok()?;
        let handle = HANDLE(file.as_raw_handle());
        // The 128-bit id first: ReFS ids don't fit the older call's 64 bits.
        let mut info = FILE_ID_INFO::default();
        let size = std::mem::size_of::<FILE_ID_INFO>() as u32;
        // SAFETY: `handle` is open for the duration of the call, and `info` is a writable
        // FILE_ID_INFO of exactly `size` bytes.
        let ex = unsafe { GetFileInformationByHandleEx(handle, FileIdInfo, (&raw mut info).cast(), size) };
        if ex.is_ok() {
            return Some(FolderId { volume: info.VolumeSerialNumber, file: u128::from_le_bytes(info.FileId.Identifier) });
        }
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: as above, with a BY_HANDLE_FILE_INFORMATION to fill.
        unsafe { GetFileInformationByHandle(handle, &mut info) }.ok()?;
        let file = (u128::from(info.nFileIndexHigh) << 32) | u128::from(info.nFileIndexLow);
        Some(FolderId { volume: u64::from(info.dwVolumeSerialNumber), file })
    }
}
