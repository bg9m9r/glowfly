//! Where Glowfly keeps its own files (settings, recent repositories, tabs; the avatar cache is in
//! `cache_dir`, the logs in `log_dir`):
//!
//! - Windows: `%APPDATA%\Glowfly`
//! - macOS: `~/Library/Application Support/Glowfly`
//! - Linux and other Unix: `$XDG_CONFIG_HOME/glowfly`, or `~/.config/glowfly`
//!
//! Every location is a function of the app's name ([`AppName`]), so the folders the app used
//! while it was called Trellis are spelled by the same code; `migrate` moves them once.

use std::ffi::OsString;
use std::path::PathBuf;

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

/// The app's name as its folders spell it: `title` on Windows and macOS, `lower` on XDG systems.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct AppName {
    pub(crate) title: &'static str,
    pub(crate) lower: &'static str,
}

/// The current name.
pub(crate) const GLOWFLY: AppName = AppName { title: "Glowfly", lower: "glowfly" };
/// The name before Glowfly; its folders are moved to Glowfly's once (`migrate`).
pub(crate) const TRELLIS: AppName = AppName { title: "Trellis", lower: "trellis" };

/// An environment variable as a path, if it is set to something absolute. The XDG spec says to
/// ignore relative values, and an empty `HOME` would put files in the working directory.
pub(crate) fn abs_var(env: &impl Fn(&str) -> Option<OsString>, name: &str) -> Option<PathBuf> {
    env(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// `$XDG_CONFIG_HOME`, falling back to `~/.config`.
fn xdg_config_home(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    abs_var(env, "XDG_CONFIG_HOME").or_else(|| abs_var(env, "HOME").map(|h| h.join(".config")))
}

pub(crate) fn config_dir_named(os: Os, name: AppName, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    match os {
        Os::Windows => abs_var(&env, "APPDATA").map(|a| a.join(name.title)),
        Os::Mac => abs_var(&env, "HOME").map(|h| h.join("Library").join("Application Support").join(name.title)),
        Os::Xdg => xdg_config_home(&env).map(|c| c.join(name.lower)),
    }
}

pub(crate) fn config_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    config_dir_named(os, GLOWFLY, env)
}

pub fn config_dir() -> Option<PathBuf> {
    config_dir_for(Os::CURRENT, |k| std::env::var_os(k))
}

/// Where Glowfly keeps files it can fetch again (the avatar cache): out of the roaming profile
/// and the settings backups.
///
/// - Windows: `%LOCALAPPDATA%\Glowfly`
/// - macOS: `~/Library/Caches/Glowfly`
/// - Linux and other Unix: `$XDG_CACHE_HOME/glowfly`, or `~/.cache/glowfly`
pub(crate) fn cache_dir_named(os: Os, name: AppName, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    match os {
        Os::Windows => abs_var(&env, "LOCALAPPDATA").map(|a| a.join(name.title)),
        Os::Mac => abs_var(&env, "HOME").map(|h| h.join("Library").join("Caches").join(name.title)),
        Os::Xdg => abs_var(&env, "XDG_CACHE_HOME").or_else(|| abs_var(&env, "HOME").map(|h| h.join(".cache"))).map(|c| c.join(name.lower)),
    }
}

pub(crate) fn cache_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    cache_dir_named(os, GLOWFLY, env)
}

pub fn cache_dir() -> Option<PathBuf> {
    cache_dir_for(Os::CURRENT, |k| std::env::var_os(k))
}

/// Where Glowfly keeps its logs (the activity log, the error and performance logs): on this
/// machine only, never in a roaming profile.
///
/// - Windows: `%LOCALAPPDATA%\Glowfly\logs`
/// - macOS: `~/Library/Logs/Glowfly`
/// - Linux and other Unix: `$XDG_STATE_HOME/glowfly/logs`, or `~/.local/state/glowfly/logs`
pub(crate) fn log_dir_named(os: Os, name: AppName, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    match os {
        Os::Windows => abs_var(&env, "LOCALAPPDATA").map(|a| a.join(name.title).join("logs")),
        Os::Mac => abs_var(&env, "HOME").map(|h| h.join("Library").join("Logs").join(name.title)),
        Os::Xdg => abs_var(&env, "XDG_STATE_HOME")
            .or_else(|| abs_var(&env, "HOME").map(|h| h.join(".local").join("state")))
            .map(|s| s.join(name.lower).join("logs")),
    }
}

pub(crate) fn log_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    log_dir_named(os, GLOWFLY, env)
}

/// The logs folder for this system (see [`log_dir_for`]); None when the variables it needs are
/// missing, empty or relative.
pub fn log_dir() -> Option<PathBuf> {
    log_dir_for(Os::CURRENT, |k| std::env::var_os(k))
}

/// Where Glowfly keeps files that only matter while it runs (the running window's address for
/// links, `instance`): `$XDG_RUNTIME_DIR/glowfly` on Linux (a private, per-login folder), else
/// the cache folder.
pub(crate) fn runtime_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    match os {
        Os::Windows | Os::Mac => cache_dir_for(os, env),
        Os::Xdg => abs_var(&env, "XDG_RUNTIME_DIR").map(|r| r.join(GLOWFLY.lower)).or_else(|| cache_dir_for(os, env)),
    }
}

pub fn runtime_dir() -> Option<PathBuf> {
    runtime_dir_for(Os::CURRENT, |k| std::env::var_os(k))
}

/// The XDG data folder (`$XDG_DATA_HOME`, else `~/.local/share`), where desktop entries go
/// (`applications/`). Only the XDG layout has one.
pub(crate) fn data_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    match os {
        Os::Windows | Os::Mac => None,
        Os::Xdg => abs_var(&env, "XDG_DATA_HOME").or_else(|| abs_var(&env, "HOME").map(|h| h.join(".local").join("share"))),
    }
}

pub fn xdg_data_dir() -> Option<PathBuf> {
    data_dir_for(Os::CURRENT, |k| std::env::var_os(k))
}

/// The user's home folder as git and its ssh see it: `HOME` (Git for Windows sets it, and a user
/// may point it elsewhere), else `USERPROFILE` on Windows. Never an empty or relative value,
/// which would put `.ssh` in the working folder.
pub(crate) fn home_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    abs_var(&env, "HOME").or_else(|| if os == Os::Windows { abs_var(&env, "USERPROFILE") } else { None })
}

pub fn home_dir() -> Option<PathBuf> {
    home_dir_for(Os::CURRENT, |k| std::env::var_os(k))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An absolute fake home (absolute on the host, whatever it is) and an environment over it.
    pub(crate) fn root(tag: &str) -> PathBuf {
        crate::paths::temp_dir().join(format!("glowfly-paths-{tag}-{}", std::process::id()))
    }

    pub(crate) fn env_of(vars: Vec<(&'static str, PathBuf)>) -> impl Fn(&str) -> Option<OsString> {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone().into_os_string())
    }

    #[test]
    fn config_dir_per_platform() {
        let r = root("dirs");
        let env = env_of(vec![("HOME", r.join("home")), ("APPDATA", r.join("appdata"))]);
        assert_eq!(config_dir_for(Os::Windows, &env), Some(r.join("appdata").join("Glowfly")));
        assert_eq!(config_dir_for(Os::Mac, &env), Some(r.join("home").join("Library").join("Application Support").join("Glowfly")));
        assert_eq!(config_dir_for(Os::Xdg, &env), Some(r.join("home").join(".config").join("glowfly")));

        // XDG_CONFIG_HOME wins on Linux, and doesn't move the macOS folder.
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_CONFIG_HOME", r.join("xdg"))]);
        assert_eq!(config_dir_for(Os::Xdg, &env), Some(r.join("xdg").join("glowfly")));
        assert_eq!(config_dir_for(Os::Mac, &env), Some(r.join("home").join("Library").join("Application Support").join("Glowfly")));
        // The old name's folders follow the same layout.
        assert_eq!(config_dir_named(Os::Xdg, TRELLIS, &env), Some(r.join("xdg").join("trellis")));
        assert_eq!(
            config_dir_named(Os::Mac, TRELLIS, &env),
            Some(r.join("home").join("Library").join("Application Support").join("Trellis"))
        );
    }

    #[test]
    fn empty_or_relative_variables_are_ignored() {
        let r = root("rel");
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_CONFIG_HOME", PathBuf::from("rel/dir"))]);
        assert_eq!(config_dir_for(Os::Xdg, &env), Some(r.join("home").join(".config").join("glowfly")));
        let env = env_of(vec![("HOME", PathBuf::new()), ("APPDATA", PathBuf::new())]);
        assert_eq!(config_dir_for(Os::Mac, &env), None);
        assert_eq!(config_dir_for(Os::Xdg, &env), None);
        assert_eq!(config_dir_for(Os::Windows, &env), None);
    }

    #[test]
    fn cache_and_home_per_platform() {
        let r = root("cache");
        let env = env_of(vec![("HOME", r.join("home")), ("LOCALAPPDATA", r.join("local")), ("USERPROFILE", r.join("profile"))]);
        assert_eq!(cache_dir_for(Os::Windows, &env), Some(r.join("local").join("Glowfly")));
        assert_eq!(cache_dir_for(Os::Mac, &env), Some(r.join("home").join("Library").join("Caches").join("Glowfly")));
        assert_eq!(cache_dir_for(Os::Xdg, &env), Some(r.join("home").join(".cache").join("glowfly")));
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_CACHE_HOME", r.join("xdg-cache"))]);
        assert_eq!(cache_dir_for(Os::Xdg, &env), Some(r.join("xdg-cache").join("glowfly")));

        // git's own HOME first; USERPROFILE only on Windows, and never an empty or relative one.
        let env = env_of(vec![("HOME", r.join("home")), ("USERPROFILE", r.join("profile"))]);
        assert_eq!(home_dir_for(Os::Windows, &env), Some(r.join("home")));
        let env = env_of(vec![("HOME", PathBuf::new()), ("USERPROFILE", r.join("profile"))]);
        assert_eq!(home_dir_for(Os::Windows, &env), Some(r.join("profile")));
        assert_eq!(home_dir_for(Os::Xdg, &env), None);
        let env = env_of(vec![("HOME", PathBuf::from("rel"))]);
        assert_eq!(home_dir_for(Os::Mac, &env), None);
    }

    #[test]
    fn log_dir_per_platform() {
        let r = root("logs");
        let env = env_of(vec![("HOME", r.join("home")), ("LOCALAPPDATA", r.join("local")), ("APPDATA", r.join("roaming"))]);
        assert_eq!(log_dir_for(Os::Windows, &env), Some(r.join("local").join("Glowfly").join("logs")));
        assert_eq!(log_dir_for(Os::Mac, &env), Some(r.join("home").join("Library").join("Logs").join("Glowfly")));
        assert_eq!(log_dir_for(Os::Xdg, &env), Some(r.join("home").join(".local").join("state").join("glowfly").join("logs")));
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_STATE_HOME", r.join("state"))]);
        assert_eq!(log_dir_for(Os::Xdg, &env), Some(r.join("state").join("glowfly").join("logs")));

        // Relative or empty values are ignored.
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_STATE_HOME", PathBuf::from("rel/state"))]);
        assert_eq!(log_dir_for(Os::Xdg, &env), Some(r.join("home").join(".local").join("state").join("glowfly").join("logs")));
        let env = env_of(vec![("HOME", PathBuf::new()), ("LOCALAPPDATA", PathBuf::from("rel")), ("XDG_STATE_HOME", PathBuf::new())]);
        assert_eq!(log_dir_for(Os::Windows, &env), None);
        assert_eq!(log_dir_for(Os::Mac, &env), None);
        assert_eq!(log_dir_for(Os::Xdg, &env), None);
    }

    #[test]
    fn runtime_and_data_dirs_per_platform() {
        let r = root("runtime");
        let env = env_of(vec![("HOME", r.join("home")), ("LOCALAPPDATA", r.join("local")), ("XDG_RUNTIME_DIR", r.join("run"))]);
        for os in [Os::Windows, Os::Mac] {
            assert_eq!(runtime_dir_for(os, &env), cache_dir_for(os, &env));
            assert_eq!(data_dir_for(os, &env), None);
        }
        assert_eq!(runtime_dir_for(Os::Xdg, &env), Some(r.join("run").join("glowfly")));
        // Unset or relative: the cache folder.
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_RUNTIME_DIR", PathBuf::from("run"))]);
        assert_eq!(runtime_dir_for(Os::Xdg, &env), Some(r.join("home").join(".cache").join("glowfly")));
        let env = env_of(vec![("HOME", r.join("home"))]);
        assert_eq!(runtime_dir_for(Os::Xdg, &env), Some(r.join("home").join(".cache").join("glowfly")));
        assert_eq!(data_dir_for(Os::Xdg, &env), Some(r.join("home").join(".local").join("share")));
        let env = env_of(vec![("HOME", r.join("home")), ("XDG_DATA_HOME", r.join("data"))]);
        assert_eq!(data_dir_for(Os::Xdg, &env), Some(r.join("data")));
    }
}
