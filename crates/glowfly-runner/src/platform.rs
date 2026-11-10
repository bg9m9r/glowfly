//! The runner's side of the operating system: the recent-repositories and tabs files, what is
//! at the listed folders, terminals, file managers (opening a folder, or showing a file selected
//! in it), and the inbox where the folders and links the OS hands over wait until the runner
//! attaches (`open_events`). The window's own glue with the OS is the app's (its `platform` module).

use std::path::{Path, PathBuf};

pub mod open_events;
mod state_files;
mod terminal;
pub use state_files::{
    SaveRefused, clear_recent, clear_recent_to, load_recent, load_tabs, parse_recent, parse_tabs, save_recent, save_recent_to, save_tabs,
    save_tabs_to,
};
pub use terminal::open_terminal;

/// What is at each of `paths`: whether it is a folder (false when gone, or unreachable: a
/// network path or a drive that isn't connected), and its identity.
pub fn check_folders(paths: &[PathBuf]) -> Vec<glowfly_model::msg::FolderCheck> {
    paths
        .iter()
        .map(|p| {
            let present = p.is_dir();
            let folder = if present { glowfly_core::paths::FolderId::of(p) } else { None };
            glowfly_model::msg::FolderCheck { path: p.clone(), present, folder }
        })
        .collect()
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
    (cmd, glowfly_core::native_path::native(dir))
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
    let path = glowfly_core::native_path::native(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Its exit code is 1 even when it worked, so it isn't checked.
        let select = select_argument(&path).ok_or(std::io::ErrorKind::InvalidInput)?;
        std::process::Command::new("explorer").raw_arg(select).spawn()?;
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

/// Explorer's argument that selects `path`: it parses `/select,` itself and wants the path
/// quoted inside the same argument. None for a path with a `"` or a control character: no
/// Windows file has such a name, and it would not stay inside the quotes.
#[cfg(any(windows, test))]
fn select_argument(path: &Path) -> Option<String> {
    let shown = path.display().to_string();
    let plain = !shown.contains('"') && !shown.chars().any(char::is_control);
    plain.then(|| format!("/select,\"{shown}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_selects_only_a_path_that_stays_inside_its_quotes() {
        assert_eq!(select_argument(Path::new(r"C:\repo\R&D 100%\a.txt")).as_deref(), Some(r#"/select,"C:\repo\R&D 100%\a.txt""#));
        assert_eq!(select_argument(Path::new("C:\\repo\\a\"b.txt")), None);
        assert_eq!(select_argument(Path::new("C:\\repo\\a\nb.txt")), None);
    }

    #[test]
    fn restored_tabs_keep_folders_that_are_gone() {
        let gone = glowfly_core::paths::temp_dir().join(format!("glowfly-gone-tab-{}", std::process::id()));
        let (paths, _, active) = parse_tabs(&format!("0\n{}", glowfly_core::paths::encode(&gone)));
        assert_eq!((paths, active), (vec![gone.clone()], 0), "not checked on the UI thread");
        let checks = check_folders(std::slice::from_ref(&gone));
        assert!(!checks[0].present && checks[0].folder.is_none());
        let here = check_folders(&[glowfly_core::paths::temp_dir()]);
        assert!(here[0].present && here[0].folder.is_some());
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
