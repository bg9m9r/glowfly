//! Operating-system integration: the recent-repositories and tabs files, what is at the listed
//! folders, terminals, file managers (opening a folder, or showing a file selected in it).

use std::path::{Path, PathBuf};

mod terminal;
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
fn parse_recent(text: &str) -> Vec<PathBuf> {
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
    trellis_core::persist::atomic_write(&f, format_recent(list).as_bytes())
}

fn tabs_file() -> Option<PathBuf> {
    trellis_core::settings::config_dir().map(|d| d.join("tabs.txt"))
}

/// The tabs open when Trellis last closed and the index of the active one. Nothing is looked at
/// here (this runs before the window opens, and a network share may not answer): a tab whose
/// folder is gone fails to load when selected, like any other.
pub fn load_tabs() -> (Vec<PathBuf>, usize) {
    tabs_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| parse_tabs(&t)).unwrap_or_default()
}

/// tabs.txt: the active index on the first line, then one repository per line
/// (`paths::encode`).
fn parse_tabs(text: &str) -> (Vec<PathBuf>, usize) {
    let mut lines = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l));
    let active: usize = lines.next().and_then(|l| l.trim().parse().ok()).unwrap_or(0);
    let paths: Vec<PathBuf> = lines.filter(|l| !l.is_empty()).map(trellis_core::paths::decode).collect();
    let active = active.min(paths.len().saturating_sub(1));
    (paths, active)
}

fn format_tabs(paths: &[PathBuf], active: usize) -> String {
    let mut text = active.to_string();
    for p in paths {
        text.push('\n');
        text.push_str(&trellis_core::paths::encode(p));
    }
    text
}

pub fn save_tabs(paths: &[PathBuf], active: usize) -> std::io::Result<()> {
    let Some(f) = tabs_file() else { return Ok(()) };
    trellis_core::persist::atomic_write(&f, format_tabs(paths, active).as_bytes())
}

pub fn open_in_file_manager(dir: &Path) {
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    if let Err(e) = std::process::Command::new(cmd).arg(dir).spawn() {
        tracing::warn!("could not open the file manager: {e}");
    }
}

/// Shows `path` selected in the file manager: Explorer on Windows, Finder on macOS. Elsewhere
/// there is no common way to select a file, so its folder opens instead.
pub fn reveal_in_file_manager(path: &Path) -> std::io::Result<()> {
    // Rebuilding from components normalises separators (`C:\repo\src/a.txt` on Windows).
    let path: PathBuf = path.components().collect();
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
        assert_eq!(parse_tabs(&format_tabs(&paths, 1)), (paths.clone(), 1));
        assert_eq!(parse_recent(&format_recent(&paths)), paths);
        // Written by earlier versions: plain lines, Windows line endings, an index past the end.
        assert_eq!(parse_tabs("5\r\nC:/old/a/\r\n\r\nC:/old/b\r\n"), (vec![PathBuf::from("C:/old/a/"), PathBuf::from("C:/old/b")], 1));
        assert_eq!(parse_recent("/old/a\n/old/b\n"), [PathBuf::from("/old/a"), PathBuf::from("/old/b")]);
        assert_eq!(parse_tabs(""), (Vec::new(), 0));
    }

    #[test]
    fn restored_tabs_keep_folders_that_are_gone() {
        let gone = trellis_core::paths::temp_dir().join(format!("trellis-gone-tab-{}", std::process::id()));
        let (paths, active) = parse_tabs(&format_tabs(std::slice::from_ref(&gone), 0));
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
        assert_eq!(parse_tabs(&format_tabs(std::slice::from_ref(&p), 0)).0, [p.clone()]);
        assert_eq!(parse_recent(&format_recent(std::slice::from_ref(&p))), [p]);
    }
}
