//! The user's Downloads folder, where a Trellis update is saved:
//!
//! - Windows: the Downloads known folder (moved or not), else `%USERPROFILE%\Downloads`
//! - macOS: `~/Downloads`
//! - Linux and other Unix: `XDG_DOWNLOAD_DIR` from `user-dirs.dirs`, else `~/Downloads`
//!
//! When that isn't an existing folder, the cache folder's `updates` is used.

use super::paths::Os;
use std::ffi::OsString;
use std::path::PathBuf;

/// `XDG_DOWNLOAD_DIR` from the text of `user-dirs.dirs`: `"$HOME/…"` expanded with `home`, or
/// an absolute path; quotes removed. Anything else is ignored.
fn xdg_download_dir(user_dirs: &str, home: Option<&PathBuf>) -> Option<PathBuf> {
    let value = user_dirs.lines().find_map(|l| l.trim().strip_prefix("XDG_DOWNLOAD_DIR="))?.trim();
    let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value);
    if let Some(rest) = value.strip_prefix("$HOME") {
        let rest = rest.trim_start_matches('/');
        return home.map(|h| if rest.is_empty() { h.clone() } else { h.join(rest) });
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

/// The Downloads folder for `os`, from the environment `env` and (XDG) the text of
/// `user-dirs.dirs`; None without a home folder.
pub(crate) fn downloads_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>, user_dirs: Option<&str>) -> Option<PathBuf> {
    let abs = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    match os {
        Os::Windows => abs("USERPROFILE").map(|p| p.join("Downloads")),
        Os::Mac => abs("HOME").map(|h| h.join("Downloads")),
        Os::Xdg => {
            let home = abs("HOME");
            user_dirs.and_then(|t| xdg_download_dir(t, home.as_ref())).or_else(|| home.map(|h| h.join("Downloads")))
        }
    }
}

/// The Downloads known folder, wherever the user moved it.
#[cfg(windows)]
fn known_downloads() -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Downloads, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    // SAFETY: the returned string is read, then freed with CoTaskMemFree as the API requires.
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None).ok()?;
        let path = p.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(p.0 as *const _));
        path.filter(|p| p.is_absolute())
    }
}

/// Where a Trellis update is downloaded to (see the module docs); None without any folder.
pub fn downloads_dir() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k);
    #[cfg(windows)]
    let found = known_downloads().or_else(|| downloads_dir_for(Os::CURRENT, env, None));
    #[cfg(not(windows))]
    let found = {
        let user_dirs = if Os::CURRENT == Os::Xdg {
            let config = env("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| super::home_dir().map(|h| h.join(".config")));
            config.and_then(|c| std::fs::read_to_string(c.join("user-dirs.dirs")).ok())
        } else {
            None
        };
        downloads_dir_for(Os::CURRENT, env, user_dirs.as_deref())
    };
    found.filter(|d| d.is_dir()).or_else(|| super::cache_dir().map(|c| c.join("updates")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        crate::paths::temp_dir().join("trellis-downloads-home")
    }

    fn env_of(vars: Vec<(&'static str, PathBuf)>) -> impl Fn(&str) -> Option<OsString> {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone().into_os_string())
    }

    #[test]
    fn downloads_per_platform() {
        let r = root();
        let env = env_of(vec![("HOME", r.join("home")), ("USERPROFILE", r.join("profile"))]);
        assert_eq!(downloads_dir_for(Os::Windows, &env, None), Some(r.join("profile").join("Downloads")));
        assert_eq!(downloads_dir_for(Os::Mac, &env, None), Some(r.join("home").join("Downloads")));
        assert_eq!(downloads_dir_for(Os::Xdg, &env, None), Some(r.join("home").join("Downloads")));
    }

    #[test]
    fn xdg_user_dirs_are_read() {
        let r = root();
        let env = env_of(vec![("HOME", r.join("home"))]);
        let text = "# written by xdg-user-dirs-update\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/Dl\"\n";
        assert_eq!(downloads_dir_for(Os::Xdg, &env, Some(text)), Some(r.join("home").join("Dl")));
        let abs = r.join("elsewhere").join("Downloads");
        let text = format!("XDG_DOWNLOAD_DIR=\"{}\"\n", abs.display());
        assert_eq!(downloads_dir_for(Os::Xdg, &env, Some(&text)), Some(abs));
        // A relative value is ignored.
        assert_eq!(downloads_dir_for(Os::Xdg, &env, Some("XDG_DOWNLOAD_DIR=\"Dl\"\n")), Some(r.join("home").join("Downloads")));
        // Only Linux reads the file.
        assert_eq!(downloads_dir_for(Os::Mac, &env, Some(text.as_str())), Some(r.join("home").join("Downloads")));
    }

    #[test]
    fn no_home_gives_none() {
        let env = env_of(vec![("HOME", PathBuf::from("rel"))]);
        for os in [Os::Windows, Os::Mac, Os::Xdg] {
            assert_eq!(downloads_dir_for(os, &env, None), None, "{os:?}");
        }
        assert_eq!(downloads_dir_for(Os::Xdg, &env, Some("XDG_DOWNLOAD_DIR=\"$HOME/Dl\"\n")), None);
    }
}
