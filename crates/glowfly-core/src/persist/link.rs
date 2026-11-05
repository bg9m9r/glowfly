//! Symbolic links Undo and the journal put back: [`write_link`] makes the link under a temporary
//! name beside its path and renames it over whatever file or link is there, so the path never
//! goes missing and a link there is replaced rather than followed; [`create_link`] makes one only
//! where nothing is. Targets come as git stores them (forward slashes); on Windows each becomes a
//! backslash and the link is a file link.

use super::replace::{clean_stale_temps, remove, replace, sync_dir};
use std::io;
use std::path::Path;

/// Makes `path` a symbolic link to `target`, replacing a file or link there (never following
/// one). A folder at `path` makes the rename fail, and that error is returned. On Windows a
/// target that isn't UTF-8 is refused, and so is a link Windows doesn't allow (Developer Mode
/// off and no admin rights).
pub fn write_link(path: &Path, target: &[u8]) -> io::Result<()> {
    let tmp = super::temp_sibling(path);
    make(target, &tmp)?;
    if let Err(e) = replace(&tmp, path, None) {
        let _ = remove(&tmp);
        return Err(e);
    }
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    sync_dir(dir);
    if let Some(name) = path.file_name() {
        clean_stale_temps(dir, &name.to_string_lossy());
    }
    Ok(())
}

/// Makes `path` a symbolic link to `target` only when nothing is there: anything at `path`
/// (a link too) fails with [`io::ErrorKind::AlreadyExists`], changing nothing.
pub fn create_link(path: &Path, target: &[u8]) -> io::Result<()> {
    make(target, path)?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        sync_dir(dir);
    }
    Ok(())
}

#[cfg(unix)]
fn make(target: &[u8], at: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(target), at)
}

#[cfg(windows)]
fn make(target: &[u8], at: &Path) -> io::Result<()> {
    let Ok(text) = std::str::from_utf8(target) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "the link's target isn't UTF-8, which Windows can't name"));
    };
    std::os::windows::fs::symlink_file(text.replace('/', "\\"), at).map_err(|e| match e.raw_os_error() {
        // ERROR_PRIVILEGE_NOT_HELD
        Some(1314) => io::Error::other("Windows didn't allow Glowfly to make the symbolic link; Developer Mode lets it"),
        _ => e,
    })
}

#[cfg(not(any(unix, windows)))]
fn make(_: &[u8], _: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "symbolic links can't be made on this system"))
}
