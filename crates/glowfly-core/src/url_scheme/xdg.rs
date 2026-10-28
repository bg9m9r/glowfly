//! The desktop entry and `xdg-mime` on Linux and the other XDG desktops.

use super::{DESKTOP_FILE, NO_XDG_MIME, SCHEME, SchemeStatus};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The user's applications folder: `$XDG_DATA_HOME/applications`, else
/// `~/.local/share/applications`.
pub(super) fn apps_dir() -> Option<PathBuf> {
    crate::settings::xdg_data_dir().map(|d| d.join("applications"))
}

pub(super) fn write_desktop_file(apps: &Path, exe: &Path) -> io::Result<()> {
    let text = super::desktop_entry(exe).map_err(io::Error::other)?;
    crate::persist::atomic_write(&apps.join(DESKTOP_FILE), text.as_bytes())
}

pub(super) fn remove_desktop_file(apps: &Path) -> io::Result<()> {
    crate::persist::remove_file(&apps.join(DESKTOP_FILE))
}

/// Runs `xdg-mime` with `args`; its trimmed output, or why it failed.
fn xdg_mime(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("xdg-mime");
    cmd.args(args);
    crate::env_path::apply(&mut cmd);
    match cmd.output() {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(NO_XDG_MIME.to_owned()),
        Err(e) => Err(format!("xdg-mime couldn't run: {e}")),
        Ok(out) if !out.status.success() => {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(err.lines().map(str::trim).find(|l| !l.is_empty()).map_or_else(|| "xdg-mime failed.".to_owned(), str::to_owned))
        }
        Ok(out) => Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned()),
    }
}

/// Tells the desktop the applications folder changed. Best effort: many desktops watch the
/// folder themselves, and the program may not be installed.
fn refresh_database(apps: &Path) {
    let mut cmd = Command::new("update-desktop-database");
    cmd.arg("-q").arg(apps);
    crate::env_path::apply(&mut cmd);
    let _ = cmd.output();
}

fn mime_type(scheme: &str) -> String {
    format!("x-scheme-handler/{scheme}")
}

/// The applications folders of `XDG_DATA_DIRS` (default `/usr/local/share:/usr/share`), where
/// a package installs its desktop entries.
fn system_apps_dirs() -> Vec<PathBuf> {
    let dirs = std::env::var_os("XDG_DATA_DIRS").filter(|d| !d.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    std::env::split_paths(&dirs).filter(|d| d.is_absolute()).map(|d| d.join("applications")).collect()
}

pub(super) fn status(exe: &Path) -> Result<SchemeStatus, String> {
    let default = xdg_mime(&["query", "default", &mime_type(SCHEME)])?;
    // The user's own entry wins over a packaged one, as desktops look it up.
    let ours = apps_dir().into_iter().chain(system_apps_dirs()).find_map(|a| std::fs::read_to_string(a.join(DESKTOP_FILE)).ok());
    Ok(super::decide_xdg(&default, ours.as_deref(), exe))
}

pub(super) fn register(exe: &Path) -> Result<SchemeStatus, String> {
    let apps = apps_dir().ok_or("There is no data folder for desktop entries (HOME isn't set).")?;
    write_desktop_file(&apps, exe).map_err(|e| format!("Couldn't write {}: {e}", apps.join(DESKTOP_FILE).display()))?;
    refresh_database(&apps);
    xdg_mime(&["default", DESKTOP_FILE, &mime_type(SCHEME)])?;
    status(exe)
}

/// Removes the desktop entry. xdg-mime can't unset a default, so the default may still name
/// the file; desktops ignore a default whose file is gone, and the status reads it that way.
pub(super) fn unregister(exe: &Path) -> Result<SchemeStatus, String> {
    let apps = apps_dir().ok_or("There is no data folder for desktop entries (HOME isn't set).")?;
    remove_desktop_file(&apps).map_err(|e| format!("Couldn't remove {}: {e}", apps.join(DESKTOP_FILE).display()))?;
    refresh_database(&apps);
    status(exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_file_is_written_and_removed() {
        let apps = crate::paths::temp_dir().join(format!("glowfly-xdg-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&apps);
        let exe = crate::paths::temp_dir().join("my tools").join("glowfly");
        write_desktop_file(&apps, &exe).unwrap();
        let text = std::fs::read_to_string(apps.join(DESKTOP_FILE)).unwrap();
        assert!(text.contains("MimeType=x-scheme-handler/glowfly;"), "{text}");
        assert_eq!(super::super::decide_xdg(DESKTOP_FILE, Some(&text), &exe), SchemeStatus::Registered);
        remove_desktop_file(&apps).unwrap();
        assert!(!apps.join(DESKTOP_FILE).exists());
        remove_desktop_file(&apps).unwrap();
        let _ = std::fs::remove_dir_all(&apps);
    }
}
