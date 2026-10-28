//! The spelling of a path that leaves Glowfly. libgit2 reports folders its own way (`C:/x/y/`
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
    text(&native(path))
}

/// A native path as text. Rebuilding keeps a share prefix as it was spelled (`//server/share`),
/// so on Windows, where `/` can't be in a name, every `/` is written as `\`.
fn text(p: &Path) -> String {
    let s = p.display().to_string();
    if cfg!(windows) { s.replace('/', "\\") } else { s }
}

/// A listed folder as its name and its parent, for a row that leads with the name: `C:\work\api`
/// is `api` and `C:\work`, a share's `\\server\share\api` is `api` and `\\server\share\`. A root
/// (`C:\`, a bare share, `/`) is all name and no parent. On Unix a parent inside the home folder
/// starts with `~` (`~/src`); Windows has no such spelling, so it is never shortened there.
pub fn name_and_parent(path: &Path) -> (String, String) {
    #[cfg(unix)]
    let home = {
        static HOME: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
        HOME.get_or_init(|| crate::settings::home_dir().map(|h| native(&h))).as_deref()
    };
    #[cfg(not(unix))]
    let home = None;
    name_and_parent_in(path, home)
}

/// `name_and_parent`, with `home` the folder a parent is shortened to `~` under (None: never).
pub fn name_and_parent_in(path: &Path, home: Option<&Path>) -> (String, String) {
    let p = native(path);
    let (Some(name), Some(parent)) = (p.file_name(), p.parent()) else { return (text(&p), String::new()) };
    let name = Path::new(name).display().to_string();
    let parent_text = match home.and_then(|h| parent.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => text(parent),
    };
    (name, parent_text)
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
        assert_eq!(shown(Path::new("//server/share/repo/")), r"\\server\share\repo", "a share's prefix too");
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

    fn pair(a: &str, b: &str) -> (String, String) {
        (a.to_owned(), b.to_owned())
    }

    #[test]
    #[cfg(windows)]
    fn windows_rows_split_into_name_and_parent() {
        let split = |p: &str| name_and_parent_in(Path::new(p), None);
        assert_eq!(split("C:/work/api-server/"), pair("api-server", r"C:\work"));
        assert_eq!(split(r"C:\api"), pair("api", r"C:\"));
        assert_eq!(split(r"C:\"), pair(r"C:\", ""));
        assert_eq!(split("//server/share/team/api"), pair("api", r"\\server\share\team"));
        assert_eq!(split(r"\\server\share\api"), pair("api", r"\\server\share\"));
        assert_eq!(split(r"\\server\share"), pair(r"\\server\share\", ""));
        assert_eq!(split(r"\\?\C:\x\y"), pair("y", r"C:\x"));
        // Windows never shortens a parent to `~`.
        assert_eq!(name_and_parent(Path::new(r"C:\Users\me\src\api")), pair("api", r"C:\Users\me\src"));
    }

    #[test]
    #[cfg(unix)]
    fn unix_rows_split_and_shorten_home() {
        let split = |p: &str| name_and_parent_in(Path::new(p), Some(Path::new("/home/me")));
        assert_eq!(split("/home/me/src/api/"), pair("api", "~/src"));
        assert_eq!(split("/home/me/api"), pair("api", "~"));
        assert_eq!(split("/home/meet/api"), pair("api", "/home/meet"), "a whole component, not a prefix");
        assert_eq!(split("/srv/api"), pair("api", "/srv"));
        assert_eq!(split("/api"), pair("api", "/"));
        assert_eq!(split("/"), pair("/", ""));
        assert_eq!(name_and_parent_in(Path::new("/home/me/api"), None), pair("api", "/home/me"));
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(native(Path::new("")), PathBuf::new());
    }
}
