//! The spelling of a path that leaves Trellis. libgit2 reports folders its own way (`C:/x/y/`
//! on Windows: forward slashes and a trailing slash), and joining a repository-relative path
//! onto that keeps the slashes. That spelling stays internal, as a matching key; whatever goes
//! to the operating system (Explorer, a terminal, an editor's arguments) or onto the screen is
//! rebuilt from its components first, so it reads `C:\x\y`.

use std::path::{Path, PathBuf};

/// `path` in this system's own spelling: rebuilt from its components (so on Windows every
/// separator is `\`), without a trailing separator, and without the verbatim `\\?\` prefix
/// when it has a plain equivalent. On Unix only repeated and trailing slashes and `.`
/// components go; a backslash there is part of a name and stays.
pub fn native(path: &Path) -> PathBuf {
    crate::paths::plain(path).components().collect()
}

/// `native(path)` as text for the screen.
pub fn shown(path: &Path) -> String {
    native(path).display().to_string()
}

/// `s` as a NUL-terminated UTF-16 string for Windows API calls (unpaired surrogates kept).
#[cfg(windows)]
pub(crate) fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn libgit2_spellings_become_windows_paths() {
        assert_eq!(native(Path::new("C:/x/y/")), PathBuf::from(r"C:\x\y"));
        assert_eq!(native(Path::new("C:/x/y/src/a.txt")), PathBuf::from(r"C:\x\y\src\a.txt"));
        assert_eq!(native(Path::new(r"C:\x\y")), PathBuf::from(r"C:\x\y"));
        assert_eq!(native(Path::new(r"\\?\C:\x\y")), PathBuf::from(r"C:\x\y"));
        // A share, however it was spelled.
        assert_eq!(native(Path::new("//server/share/repo/")), PathBuf::from(r"\\server\share\repo"));
        assert_eq!(native(Path::new(r"\\?\UNC\server\share\repo")), PathBuf::from(r"\\server\share\repo"));
        assert_eq!(native(Path::new(r"\\wsl.localhost\Ubuntu\home\me/repo/")), PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\repo"));
        assert_eq!(shown(Path::new("C:/a/b/")), r"C:\a\b");
    }

    #[test]
    #[cfg(unix)]
    fn unix_paths_keep_their_names() {
        assert_eq!(native(Path::new("/home/me/repo/")), PathBuf::from("/home/me/repo"));
        assert_eq!(native(Path::new("/home/me//repo/./src")), PathBuf::from("/home/me/repo/src"));
        // A backslash is part of a file name on Unix.
        assert_eq!(native(Path::new(r"/repo/a\b")), PathBuf::from(r"/repo/a\b"));
        assert_eq!(shown(Path::new("/a/b/")), "/a/b");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(native(Path::new("")), PathBuf::new());
    }
}
