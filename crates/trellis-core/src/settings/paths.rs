//! Where Trellis keeps its own files (settings, recent repositories, tabs, avatar cache):
//!
//! - Windows: `%APPDATA%\Trellis`
//! - macOS: `~/Library/Application Support/Trellis`
//! - Linux and other Unix: `$XDG_CONFIG_HOME/trellis`, or `~/.config/trellis`
//!
//! Older builds used other places (the app was called Riffle, and every platform but Windows
//! used `<XDG config>/Trellis`); `migrate_old_config_dir` moves the newest of those once.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The platform family the folder layout depends on. A parameter (rather than `cfg`) so every
/// layout is tested on every platform.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Os {
    Windows,
    Mac,
    /// Linux and the BSDs: the XDG layout.
    Xdg,
}

impl Os {
    pub(crate) const CURRENT: Os = if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::Mac
    } else {
        Os::Xdg
    };
}

/// An environment variable as a path, if it is set to something absolute. The XDG spec says to
/// ignore relative values, and an empty `HOME` would put files in the working directory.
fn abs_var(env: &impl Fn(&str) -> Option<OsString>, name: &str) -> Option<PathBuf> {
    env(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// `$XDG_CONFIG_HOME`, falling back to `~/.config`.
fn xdg_config_home(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    abs_var(env, "XDG_CONFIG_HOME").or_else(|| abs_var(env, "HOME").map(|h| h.join(".config")))
}

pub(crate) fn config_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    match os {
        Os::Windows => abs_var(&env, "APPDATA").map(|a| a.join("Trellis")),
        Os::Mac => abs_var(&env, "HOME").map(|h| h.join("Library").join("Application Support").join("Trellis")),
        Os::Xdg => xdg_config_home(&env).map(|c| c.join("trellis")),
    }
}

/// Folders earlier versions used, most recent first. Only the first that exists is moved.
pub(crate) fn old_config_dirs_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let mut bases = Vec::new();
    match os {
        Os::Windows => bases.extend(abs_var(&env, "APPDATA")),
        Os::Mac | Os::Xdg => {
            // Old builds took $XDG_CONFIG_HOME when set, else ~/.config; check both on macOS,
            // where the variable may have been set in one shell and not another.
            bases.extend(abs_var(&env, "XDG_CONFIG_HOME"));
            if os == Os::Mac || bases.is_empty() {
                bases.extend(abs_var(&env, "HOME").map(|h| h.join(".config")));
            }
        }
    }
    let mut dirs = Vec::new();
    for name in ["Trellis", "Riffle"] {
        for base in &bases {
            let dir = base.join(name);
            // On Windows the new folder is `%APPDATA%\Trellis` itself.
            if Some(&dir) != config_dir_for(os, &env).as_ref() && !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

pub fn config_dir() -> Option<PathBuf> {
    config_dir_for(Os::CURRENT, |k| std::env::var_os(k))
}

/// Moves the settings folder from where an earlier version kept it (see the module docs), once,
/// if there is nothing at the current location yet.
pub fn migrate_old_config_dir() -> std::io::Result<()> {
    let env = |k: &str| std::env::var_os(k);
    let Some(new) = config_dir_for(Os::CURRENT, env) else { return Ok(()) };
    migrate_first(&old_config_dirs_for(Os::CURRENT, env), &new)
}

pub(crate) fn migrate_first(old: &[PathBuf], new: &Path) -> std::io::Result<()> {
    match old.iter().find(|d| d.is_dir()) {
        Some(dir) => move_dir_if_new_missing(dir, new),
        None => Ok(()),
    }
}

pub(crate) fn move_dir_if_new_missing(old: &Path, new: &Path) -> std::io::Result<()> {
    // `exists` is also true when `new` only differs from `old` in case on a case-insensitive
    // disk (`~/.config/Trellis` vs `trellis`), which correctly leaves the folder alone.
    if !old.is_dir() || new.exists() {
        return Ok(());
    }
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(old, new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute fake home (absolute on the host, whatever it is) and an environment over it.
    fn root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("trellis-paths-{tag}-{}", std::process::id()))
    }

    fn env_of(vars: Vec<(&'static str, PathBuf)>) -> impl Fn(&str) -> Option<OsString> {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone().into_os_string())
    }

    #[test]
    fn config_dir_per_platform() {
        let r = root("dirs");
        let env = env_of(vec![("HOME", r.join("home")), ("APPDATA", r.join("appdata"))]);
        assert_eq!(config_dir_for(Os::Windows, &env), Some(r.join("appdata").join("Trellis")));
        assert_eq!(config_dir_for(Os::Mac, &env), Some(r.join("home").join("Library").join("Application Support").join("Trellis")));
        assert_eq!(config_dir_for(Os::Xdg, &env), Some(r.join("home").join(".config").join("trellis")));

        // XDG_CONFIG_HOME wins on Linux, and doesn't move the macOS folder.
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_CONFIG_HOME", r.join("xdg"))]);
        assert_eq!(config_dir_for(Os::Xdg, &env), Some(r.join("xdg").join("trellis")));
        assert_eq!(config_dir_for(Os::Mac, &env), Some(r.join("home").join("Library").join("Application Support").join("Trellis")));
    }

    #[test]
    fn empty_or_relative_variables_are_ignored() {
        let r = root("rel");
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_CONFIG_HOME", PathBuf::from("rel/dir"))]);
        assert_eq!(config_dir_for(Os::Xdg, &env), Some(r.join("home").join(".config").join("trellis")));
        let env = env_of(vec![("HOME", PathBuf::new()), ("APPDATA", PathBuf::new())]);
        assert_eq!(config_dir_for(Os::Mac, &env), None);
        assert_eq!(config_dir_for(Os::Xdg, &env), None);
        assert_eq!(config_dir_for(Os::Windows, &env), None);
    }

    #[test]
    fn old_dirs_per_platform() {
        let r = root("old");
        let (home, xdg) = (r.join("home"), r.join("xdg"));
        let env = env_of(vec![("HOME", home.clone()), ("APPDATA", r.join("appdata"))]);
        assert_eq!(old_config_dirs_for(Os::Windows, &env), vec![r.join("appdata").join("Riffle")]);
        let dot = home.join(".config");
        assert_eq!(old_config_dirs_for(Os::Mac, &env), vec![dot.join("Trellis"), dot.join("Riffle")]);
        assert_eq!(old_config_dirs_for(Os::Xdg, &env), vec![dot.join("Trellis"), dot.join("Riffle")]);

        let env = env_of(vec![("HOME", home.clone()), ("XDG_CONFIG_HOME", xdg.clone())]);
        assert_eq!(
            old_config_dirs_for(Os::Mac, &env),
            vec![xdg.join("Trellis"), dot.join("Trellis"), xdg.join("Riffle"), dot.join("Riffle")]
        );
        assert_eq!(old_config_dirs_for(Os::Xdg, &env), vec![xdg.join("Trellis"), xdg.join("Riffle")]);
    }

    #[test]
    fn macos_moves_the_old_dot_config_folder_once() {
        let r = root("mac");
        let env = env_of(vec![("HOME", r.join("home"))]);
        let old = r.join("home").join(".config").join("Trellis");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("settings.json"), "{}").unwrap();
        let new = config_dir_for(Os::Mac, &env).unwrap();
        migrate_first(&old_config_dirs_for(Os::Mac, &env), &new).unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{}");

        // A second run, with a stale old folder back, leaves both alone.
        std::fs::create_dir_all(&old).unwrap();
        migrate_first(&old_config_dirs_for(Os::Mac, &env), &new).unwrap();
        assert!(old.exists());
        assert!(new.join("settings.json").exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn old_config_dir_moves_once() {
        let base = root("riffle");
        let (old, new) = (base.join("Riffle"), base.join("Trellis"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("recent.txt"), "C:/repo").unwrap();
        move_dir_if_new_missing(&old, &new).unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(new.join("recent.txt")).unwrap(), "C:/repo");

        // An existing new folder is never overwritten.
        std::fs::create_dir_all(&old).unwrap();
        move_dir_if_new_missing(&old, &new).unwrap();
        assert!(old.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
