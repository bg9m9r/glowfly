//! Running inside a Flatpak sandbox (Linux). What the sandbox changes, and what Glowfly does
//! about it:
//!
//! - `/.flatpak-info` exists only inside a sandbox, and `FLATPAK_ID` names the app. The app's
//!   files are under `/app`, whose `bin` comes first on PATH (the bundled git is `/app/bin/git`).
//! - `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` and `XDG_STATE_HOME` point into
//!   `~/.var/app/<id>/`. Glowfly's own settings, cache and logs stay there; the host's values
//!   come as `HOST_XDG_*` (when the host set them, else the XDG default under `HOME`, which is
//!   the real home). Locations shared with the host (git's XDG config files, the home Trash)
//!   are read through [`host_var`] and [`child_config_home`].
//! - git reads `$XDG_CONFIG_HOME/git/config`, `.../git/ignore` and `.../git/attributes`, so the
//!   programs Glowfly starts get the host's `XDG_CONFIG_HOME` (`env_path::apply`), and libgit2's
//!   XDG search path is set to it once at start ([`init`]).
//! - `$XDG_RUNTIME_DIR/app/<id>` is shared by every running instance of the app; the rest of
//!   the runtime folder is per instance (`settings::paths::runtime_dir_for`).
//! - `/tmp` is private to the sandbox.
//!
//! The decisions are pure functions of explicit inputs; the `*_with`-less readers wrap them.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Where the app's own files are inside the sandbox.
pub const APP_ROOT: &str = "/app";
/// The file that exists only inside a Flatpak sandbox.
pub const INFO_FILE: &str = "/.flatpak-info";

/// The XDG variables Flatpak points into the app's own folder, exporting the host's values as
/// `HOST_<name>`.
const HOST_MAPPED: [&str; 4] = ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"];

/// Whether `id` can be a Flatpak app id, safe to use as a folder name: 1 to 255 ASCII letters,
/// digits, `.`, `_` or `-`, not starting with `.` and without `..`.
pub fn app_id_ok(id: &OsStr) -> bool {
    let Some(s) = id.to_str() else { return false };
    (1..=255).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !s.starts_with('.')
        && !s.contains("..")
}

/// Whether a program at `exe` runs in a Flatpak sandbox: the info file exists, or `FLATPAK_ID`
/// is a valid id and the program is under [`APP_ROOT`] (a host program that inherited
/// `FLATPAK_ID` doesn't count).
pub fn in_flatpak_with(exe: &Path, info_file: bool, flatpak_id: Option<&OsStr>) -> bool {
    info_file || (flatpak_id.is_some_and(app_id_ok) && exe.starts_with(APP_ROOT))
}

/// Whether this program runs in a Flatpak sandbox; never outside Linux. Read once.
pub fn in_flatpak() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    static IN: OnceLock<bool> = OnceLock::new();
    *IN.get_or_init(|| {
        let exe = std::env::current_exe().unwrap_or_default();
        in_flatpak_with(&exe, Path::new(INFO_FILE).exists(), std::env::var_os("FLATPAK_ID").as_deref())
    })
}

/// An environment variable as the host sees it. Outside Flatpak, or for a variable Flatpak
/// doesn't remap, it is `env(name)`. Inside, an XDG folder is `HOST_<name>`, kept only when it is
/// a non-empty absolute path; None then means the XDG default under `HOME` applies.
pub fn host_var_with(name: &str, in_flatpak: bool, env: impl Fn(&str) -> Option<OsString>) -> Option<OsString> {
    if !in_flatpak || !HOST_MAPPED.contains(&name) {
        return env(name);
    }
    env(&format!("HOST_{name}")).filter(|v| Path::new(v).is_absolute())
}

/// [`host_var_with`] for this process.
pub fn host_var(name: &str) -> Option<OsString> {
    host_var_with(name, in_flatpak(), |k| std::env::var_os(k))
}

/// The host's XDG config folder: an absolute `HOST_XDG_CONFIG_HOME`, else `HOME/.config`.
pub fn host_config_home_with(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let abs = |k: &str| env(k).map(PathBuf::from).filter(|p| p.is_absolute());
    abs("HOST_XDG_CONFIG_HOME").or_else(|| abs("HOME").map(|h| h.join(".config")))
}

/// The `XDG_CONFIG_HOME` the programs Glowfly starts get: the host's, inside Flatpak only.
pub fn child_config_home() -> Option<PathBuf> {
    static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    HOME.get_or_init(|| if in_flatpak() { host_config_home_with(|k| std::env::var_os(k)) } else { None }).clone()
}

/// Inside Flatpak, points libgit2's XDG config search path at the host's `<config>/git`, where
/// it looks for `config`. Does nothing elsewhere. Call once at start, before any libgit2 use; a
/// failure is only worth a warning (libgit2 then reads the sandbox's folder).
pub fn init() -> Result<(), String> {
    let Some(config) = child_config_home() else { return Ok(()) };
    let dir = config.join("git");
    // SAFETY: called once on the main thread at start, before any other thread exists and before
    // libgit2 is used, so nothing reads the search path while it changes.
    unsafe { git2::opts::set_search_path(git2::ConfigLevel::XDG, dir.as_path()) }
        .map_err(|e| format!("couldn't point libgit2 at {}: {e}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(vars: Vec<(&'static str, OsString)>) -> impl Fn(&str) -> Option<OsString> {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone())
    }

    #[test]
    fn flatpak_is_known_by_its_info_file_or_its_id_under_app() {
        let id = OsStr::new("io.github.bg9m9r.glowfly");
        let anywhere = crate::paths::temp_dir().join("glowfly");
        assert!(in_flatpak_with(&anywhere, true, None));
        assert!(in_flatpak_with(Path::new("/app/bin/glowfly"), false, Some(id)));
        assert!(!in_flatpak_with(Path::new("/usr/bin/glowfly"), false, Some(id)), "a host program that inherited the id");
        for bad in ["", "../x", ".x", "a/b"] {
            assert!(!in_flatpak_with(Path::new("/app/bin/glowfly"), false, Some(OsStr::new(bad))), "{bad:?}");
        }
        assert!(!in_flatpak_with(Path::new("/app/bin/glowfly"), false, None));
    }

    #[test]
    fn app_ids_are_checked() {
        for good in ["io.github.bg9m9r.glowfly", "org.example.App_2-x"] {
            assert!(app_id_ok(OsStr::new(good)), "{good}");
        }
        let long = "a".repeat(256);
        for bad in ["", ".x", "a..b", "a/b", "a b", &long] {
            assert!(!app_id_ok(OsStr::new(bad)), "{bad:?}");
        }
        assert!(app_id_ok(OsStr::new(&"a".repeat(255))));
    }

    #[test]
    fn host_xdg_folders_come_from_host_vars() {
        let root = crate::paths::temp_dir().join("glowfly-flatpak-host");
        let sandbox = root.join("var-app").join("config");
        let host = root.join("host-config");
        let home = root.join("home");
        let env = env_of(vec![
            ("XDG_CONFIG_HOME", sandbox.clone().into()),
            ("HOST_XDG_CONFIG_HOME", host.clone().into()),
            ("HOST_XDG_DATA_HOME", root.join("host-data").into()),
            ("HOME", home.clone().into()),
        ]);
        assert_eq!(host_var_with("XDG_CONFIG_HOME", false, &env), Some(sandbox.into()), "outside Flatpak");
        assert_eq!(host_var_with("XDG_CONFIG_HOME", true, &env), Some(host.clone().into()));
        assert_eq!(host_var_with("XDG_DATA_HOME", true, &env), Some(root.join("host-data").into()));
        assert_eq!(host_var_with("HOME", true, &env), Some(home.clone().into()), "HOME is the real home");
        for unset in [None, Some(OsString::new()), Some(OsString::from("rel/config"))] {
            let env = env_of(unset.iter().map(|v| ("HOST_XDG_CONFIG_HOME", v.clone())).collect());
            assert_eq!(host_var_with("XDG_CONFIG_HOME", true, env), None, "{unset:?}");
        }

        assert_eq!(host_config_home_with(&env), Some(host));
        let env = env_of(vec![("HOME", home.clone().into()), ("HOST_XDG_CONFIG_HOME", "rel".into())]);
        assert_eq!(host_config_home_with(&env), Some(home.join(".config")));
        assert_eq!(host_config_home_with(env_of(vec![])), None);
    }
}
