//! Operating-system integration: the recent-repositories file and which listed folders are
//! missing, terminals, file managers (opening a folder, or showing a file selected in it).

use std::path::{Path, PathBuf};

mod terminal;
pub use terminal::open_terminal;

fn recent_file() -> Option<PathBuf> {
    trellis_core::settings::config_dir().map(|d| d.join("recent.txt"))
}

/// Recently opened repositories, newest first, including folders that no longer exist.
pub fn load_recent() -> Vec<PathBuf> {
    recent_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| s.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default()
}

/// The folders among `paths` that are gone: not a directory, or unreachable (a network path or
/// a drive that isn't connected).
pub fn missing_folders(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths.iter().filter(|p| !p.is_dir()).cloned().collect()
}

/// The first of `paths` that exists.
pub fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

/// Saves the recent-repositories list, newest first.
pub fn save_recent(list: &[PathBuf]) -> std::io::Result<()> {
    let Some(f) = recent_file() else { return Ok(()) };
    let text: Vec<String> = list.iter().map(|p| p.display().to_string()).collect();
    trellis_core::persist::atomic_write(&f, text.join("\n").as_bytes())
}

fn tabs_file() -> Option<PathBuf> {
    trellis_core::settings::config_dir().map(|d| d.join("tabs.txt"))
}

/// The tabs open when Trellis last closed (those that still exist) and the index of the active
/// one. The first line of the file is the active index; each further line is a repository.
pub fn load_tabs() -> (Vec<PathBuf>, usize) {
    let Some(text) = tabs_file().and_then(|f| std::fs::read_to_string(f).ok()) else { return (Vec::new(), 0) };
    let mut lines = text.lines();
    let active: usize = lines.next().and_then(|l| l.trim().parse().ok()).unwrap_or(0);
    let mut paths = Vec::new();
    let mut new_active = 0;
    for (i, line) in lines.filter(|l| !l.is_empty()).enumerate() {
        let p = PathBuf::from(line);
        if p.exists() {
            if i <= active {
                new_active = paths.len();
            }
            paths.push(p);
        }
    }
    (paths, new_active)
}

pub fn save_tabs(paths: &[PathBuf], active: usize) -> std::io::Result<()> {
    let Some(f) = tabs_file() else { return Ok(()) };
    let mut text = active.to_string();
    for p in paths {
        text.push('\n');
        text.push_str(&p.display().to_string());
    }
    trellis_core::persist::atomic_write(&f, text.as_bytes())
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
