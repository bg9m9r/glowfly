//! Operating-system integration: the recent-repositories and tabs files, what is at the listed
//! folders, terminals, file managers (opening a folder, or showing a file selected in it), the
//! window icon, and the drawn Windows frame's glue with the OS (`native_window`).

use std::path::{Path, PathBuf};

mod icon;
pub mod native_window;
mod terminal;
pub use icon::app_icon;
pub use terminal::open_terminal;

fn recent_file() -> Option<PathBuf> {
    trellis_core::settings::config_dir().map(|d| d.join("recent.txt"))
}

/// Recently opened repositories, newest first, including folders that no longer exist. Reads
/// only Trellis's own file: nothing in the list is looked at (a network share may not answer).
pub fn load_recent() -> Vec<PathBuf> {
    recent_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|s| parse_recent(&s)).unwrap_or_default()
}

/// The recent list in recent.txt: one path per line (`paths::encode`, which leaves plain paths
/// as earlier versions wrote them).
pub(crate) fn parse_recent(text: &str) -> Vec<PathBuf> {
    text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l)).filter(|l| !l.is_empty()).map(trellis_core::paths::decode).collect()
}

fn format_recent(list: &[PathBuf]) -> String {
    list.iter().map(|p| trellis_core::paths::encode(p)).collect::<Vec<_>>().join("\n")
}

/// What is at each of `paths`: whether it is a folder (false when gone, or unreachable: a
/// network path or a drive that isn't connected), and its identity.
pub fn check_folders(paths: &[PathBuf]) -> Vec<crate::msg::FolderCheck> {
    paths
        .iter()
        .map(|p| {
            let present = p.is_dir();
            let folder = if present { trellis_core::paths::FolderId::of(p) } else { None };
            crate::msg::FolderCheck { path: p.clone(), present, folder }
        })
        .collect()
}

/// Saves the recent-repositories list, newest first.
pub fn save_recent(list: &[PathBuf]) -> std::io::Result<()> {
    let Some(f) = recent_file() else { return Ok(()) };
    save_recent_to(&f, list)
}

/// Writes the recent-repositories list to `file`.
pub fn save_recent_to(file: &Path, list: &[PathBuf]) -> std::io::Result<()> {
    trellis_core::persist::atomic_write(file, format_recent(list).as_bytes())
}

fn tabs_file() -> Option<PathBuf> {
    trellis_core::settings::config_dir().map(|d| d.join("tabs.txt"))
}

/// The open tabs of a saved session: their folders, their labels (one per folder) and the
/// index of the active one.
pub type SavedTabs = (Vec<PathBuf>, Vec<Option<String>>, usize);

/// The tabs open when Trellis last closed, their labels and the index of the active one.
/// Nothing is looked at here (this runs before the window opens, and a network share may not
/// answer): a tab whose folder is gone fails to load when selected, like any other.
pub fn load_tabs() -> SavedTabs {
    tabs_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| parse_tabs(&t)).unwrap_or_default()
}

/// tabs.txt: the active index on the first line, then one repository per line
/// (`paths::encode`, which escapes every control character), followed by a tab character and
/// the tab's label when it has one. Files from earlier versions have no labels.
pub(crate) fn parse_tabs(text: &str) -> SavedTabs {
    let mut lines = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l));
    let active: usize = lines.next().and_then(|l| l.trim().parse().ok()).unwrap_or(0);
    let (paths, labels): (Vec<PathBuf>, Vec<Option<String>>) = lines
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (path, label) = l.split_once('\t').unwrap_or((l, ""));
            let label: String = label.chars().filter(|c| !c.is_control()).collect();
            let label = label.trim();
            (trellis_core::paths::decode(path), (!label.is_empty()).then(|| label.to_owned()))
        })
        .unzip();
    let active = active.min(paths.len().saturating_sub(1));
    (paths, labels, active)
}

fn format_tabs(paths: &[PathBuf], labels: &[Option<String>], active: usize) -> String {
    let mut text = active.to_string();
    for (k, p) in paths.iter().enumerate() {
        text.push('\n');
        text.push_str(&trellis_core::paths::encode(p));
        if let Some(label) = labels.get(k).and_then(Option::as_deref) {
            let label: String = label.chars().filter(|c| !c.is_control()).collect();
            text.push('\t');
            text.push_str(label.trim());
        }
    }
    text
}

pub fn save_tabs(paths: &[PathBuf], labels: &[Option<String>], active: usize) -> std::io::Result<()> {
    let Some(f) = tabs_file() else { return Ok(()) };
    save_tabs_to(&f, paths, labels, active)
}

/// Writes the open tabs to `file`.
pub fn save_tabs_to(file: &Path, paths: &[PathBuf], labels: &[Option<String>], active: usize) -> std::io::Result<()> {
    trellis_core::persist::atomic_write(file, format_tabs(paths, labels, active).as_bytes())
}

/// The file manager and the folder to give it for showing `dir`, in this system's spelling
/// (Explorer doesn't understand libgit2's `C:/x/y/` and opens its default folder instead).
pub(crate) fn file_manager_command(dir: &Path) -> (&'static str, PathBuf) {
    let cmd = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    (cmd, trellis_core::native_path::native(dir))
}

/// Opens the folder `dir` in the file manager. Explorer's exit code is 1 even when it worked,
/// so only a failure to start it is reported.
pub fn open_in_file_manager(dir: &Path) -> std::io::Result<()> {
    let (cmd, dir) = file_manager_command(dir);
    std::process::Command::new(cmd).arg(dir).spawn().map(drop)
}

/// Shows `path` selected in the file manager: Explorer on Windows, Finder on macOS. Elsewhere
/// there is no common way to select a file, so its folder opens instead.
pub fn reveal_in_file_manager(path: &Path) -> std::io::Result<()> {
    // Normalises separators (`C:\repo\src/a.txt` on Windows) and drops a `\\?\` prefix.
    let path = trellis_core::native_path::native(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Explorer parses `/select,` itself and wants the path quoted inside the same argument.
        // Its exit code is 1 even when it worked, so it isn't checked.
        std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg("-R").arg(&path).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path.parent().unwrap_or(&path)).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_and_recent_files_round_trip_and_read_the_old_format() {
        let paths = vec![PathBuf::from("/w/a"), PathBuf::from("/w/two\nlines"), PathBuf::from(r"C:\w\b")];
        let none = vec![None; 3];
        assert_eq!(parse_tabs(&format_tabs(&paths, &none, 1)), (paths.clone(), none.clone(), 1));
        assert_eq!(parse_recent(&format_recent(&paths)), paths);
        // Written by earlier versions: plain lines, Windows line endings, an index past the end.
        let old = parse_tabs("5\r\nC:/old/a/\r\n\r\nC:/old/b\r\n");
        assert_eq!(old, (vec![PathBuf::from("C:/old/a/"), PathBuf::from("C:/old/b")], vec![None, None], 1));
        assert_eq!(parse_recent("/old/a\n/old/b\n"), [PathBuf::from("/old/a"), PathBuf::from("/old/b")]);
        assert_eq!(parse_tabs(""), (Vec::new(), Vec::new(), 0));
    }

    #[test]
    fn tabs_file_keeps_labels() {
        let paths = vec![PathBuf::from("/w/web"), PathBuf::from("/w/api")];
        let labels = vec![Some("Web app".to_owned()), None];
        assert_eq!(parse_tabs(&format_tabs(&paths, &labels, 0)), (paths.clone(), labels.clone(), 0));
        // A path holding a tab or a newline is encoded, so the label still splits off right.
        let odd = vec![PathBuf::from("/w/a\tb"), PathBuf::from("/w/two\nlines")];
        let labels = vec![Some("Tabbed".to_owned()), Some("Two".to_owned())];
        assert_eq!(parse_tabs(&format_tabs(&odd, &labels, 1)), (odd, labels, 1));
        // Labels with control characters (written by hand) are cleaned; an empty one is none.
        let (_, labels, _) = parse_tabs("0\n/w/a\t  Front\u{7}end \n/w/b\t \n/w/c\t\t");
        assert_eq!(labels, [Some("Frontend".to_owned()), None, None]);
        // Fewer labels than paths: the rest have none.
        assert_eq!(parse_tabs(&format_tabs(&paths, &[], 0)).1, [None, None]);
    }

    #[test]
    fn saves_to_a_named_file_round_trip() {
        let dir = trellis_core::paths::temp_dir().join(format!("trellis-platform-saves-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = vec![dir.join("web"), dir.join("api")];
        save_recent_to(&dir.join("recent.txt"), &paths).unwrap();
        assert_eq!(parse_recent(&std::fs::read_to_string(dir.join("recent.txt")).unwrap()), paths);
        let labels = vec![None, Some("API".to_owned())];
        save_tabs_to(&dir.join("tabs.txt"), &paths, &labels, 1).unwrap();
        assert_eq!(parse_tabs(&std::fs::read_to_string(dir.join("tabs.txt")).unwrap()), (paths, labels, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restored_tabs_keep_folders_that_are_gone() {
        let gone = trellis_core::paths::temp_dir().join(format!("trellis-gone-tab-{}", std::process::id()));
        let (paths, _, active) = parse_tabs(&format_tabs(std::slice::from_ref(&gone), &[], 0));
        assert_eq!((paths, active), (vec![gone.clone()], 0), "not checked on the UI thread");
        let checks = check_folders(std::slice::from_ref(&gone));
        assert!(!checks[0].present && checks[0].folder.is_none());
        let here = check_folders(&[trellis_core::paths::temp_dir()]);
        assert!(here[0].present && here[0].folder.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn names_that_are_not_utf8_survive_the_files() {
        use std::os::unix::ffi::OsStrExt;
        let p = PathBuf::from(std::ffi::OsStr::from_bytes(b"/w/caf\xe9"));
        assert_eq!(parse_tabs(&format_tabs(std::slice::from_ref(&p), &[], 0)).0, std::slice::from_ref(&p));
        assert_eq!(parse_recent(&format_recent(std::slice::from_ref(&p))), [p]);
    }

    #[test]
    fn the_file_manager_gets_this_systems_spelling() {
        let (_, dir) = file_manager_command(Path::new("C:/x/y/"));
        let want = if cfg!(windows) { r"C:\x\y" } else { "C:/x/y" };
        assert_eq!(dir, PathBuf::from(want));
        let (_, dir) = file_manager_command(Path::new("/home/me/repo/"));
        assert!(!dir.to_string_lossy().ends_with('/'), "{}", dir.display());
    }
}
