//! The environment that points a git child at the askpass helper ([`super::enable_askpass`]),
//! with the `DISPLAY` placeholder an OpenSSH before 8.4 needs ([`crate::ssh_version`]).

use super::{ASKPASS, AskpassEnv};
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// The environment that points git and ssh at the askpass helper. `SSH_ASKPASS_REQUIRE=force`
/// (OpenSSH 8.4 or newer) makes ssh use it even without a display. Older OpenSSH ignores that
/// and asks the helper only when `DISPLAY` is set and there is no terminal, so with such a
/// version ([`crate::ssh_version::legacy_askpass`]) an unset `DISPLAY` gets a placeholder.
pub fn askpass_vars(a: &AskpassEnv) -> Vec<(&'static str, OsString)> {
    askpass_vars_in(a, None, &[])
}

/// [`askpass_vars`] for a child started in `cwd` with `env`, whose ssh may be the repository's
/// `core.sshCommand`. That ssh's version is read only when `DISPLAY` is unset and `env` doesn't
/// turn ssh's prompts off ([`super::NO_PROMPTS`]).
pub fn askpass_vars_in(a: &AskpassEnv, cwd: Option<&Path>, env: &[(&str, &str)]) -> Vec<(&'static str, OsString)> {
    let display = std::env::var_os("DISPLAY");
    let unset = display.as_deref().is_none_or(OsStr::is_empty);
    let prompts = !env.iter().any(|&(k, v)| k == "SSH_ASKPASS_REQUIRE" && v == "never");
    askpass_vars_for(a, unset && prompts && crate::ssh_version::legacy_askpass_in(cwd, env), display.as_deref())
}

/// [`askpass_vars`] for an ssh that is `legacy` (before 8.4) and the inherited `display`.
pub fn askpass_vars_for(a: &AskpassEnv, legacy: bool, display: Option<&OsStr>) -> Vec<(&'static str, OsString)> {
    let mut vars = vec![
        ("GIT_ASKPASS", a.exe.clone().into_os_string()),
        ("SSH_ASKPASS", a.exe.clone().into_os_string()),
        ("SSH_ASKPASS_REQUIRE", "force".into()),
        (crate::askpass::ENV_ADDR, a.addr.clone().into()),
        (crate::askpass::ENV_TOKEN, a.token.clone().into()),
    ];
    if let Some(d) = display_placeholder(display, legacy) {
        vars.push(("DISPLAY", d.into()));
    }
    vars
}

/// [`askpass_vars`] once the app has enabled the helper; empty before that (and in tests), so a
/// child given only these can't prompt through the app.
pub fn askpass_env() -> Vec<(&'static str, OsString)> {
    ASKPASS.get().map(askpass_vars).unwrap_or_default()
}

/// [`askpass_env`] for a child started in `cwd` with `env` ([`askpass_vars_in`]).
pub fn askpass_env_in(cwd: &Path, env: &[(&str, &str)]) -> Vec<(&'static str, OsString)> {
    ASKPASS.get().map(|a| askpass_vars_in(a, Some(cwd), env)).unwrap_or_default()
}

/// The `DISPLAY` to set for ssh children so OpenSSH before 8.4 (`legacy`, Unix only) uses the
/// askpass helper: `:0` when `display` is unset or empty, else None. ssh only checks that it is
/// set; nothing connects to it unless X11 forwarding is configured.
pub fn display_placeholder(display: Option<&OsStr>, legacy: bool) -> Option<&'static str> {
    (legacy && display.is_none_or(|d| d.is_empty())).then_some(":0")
}
