//! The environment that makes git run Trellis as its gpg.program (pure, [`vars`]), and the
//! signing environment gitcli adds to each command ([`for_command`]).

use super::{ENV_GIT, ENV_WRAPPER};
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Command-scope config (git 2.31 or newer reads `GIT_CONFIG_COUNT`) setting `gpg.program` to
/// `exe`, plus the wrapper marker and the git program that runs it (`git`, see
/// [`super::program::wrapper_git`]). Empty when Trellis itself was started with
/// `GIT_CONFIG_COUNT` set (`inherited_count`): those entries are kept, and GnuPG's own window
/// is used.
pub fn vars(exe: &Path, inherited_count: Option<&OsStr>, git: &OsStr) -> Vec<(&'static str, OsString)> {
    if inherited_count.is_some_and(|c| !c.is_empty()) {
        return Vec::new();
    }
    vec![
        ("GIT_CONFIG_COUNT", "1".into()),
        ("GIT_CONFIG_KEY_0", "gpg.program".into()),
        ("GIT_CONFIG_VALUE_0", crate::native_path::native(exe).into_os_string()),
        (ENV_WRAPPER, "1".into()),
        (ENV_GIT, git.to_owned()),
    ]
}

/// Whether a command run with the caller environment `env` may prompt: not with
/// [`crate::gitcli::NO_PROMPTS`] (whose empty `GIT_ASKPASS` marks it).
pub fn allows_prompts(env: &[(&str, &str)]) -> bool {
    !env.contains(&("GIT_ASKPASS", ""))
}

/// The signing environment for `git <args>` in `cwd` with the caller environment `env`, once
/// the askpass helper is enabled (`askpass`) and for a command that may prompt: Trellis as
/// gpg.program ([`vars`]) and, for SSH-format signing, the key file ssh-keygen will ask about
/// ([`super::ssh_env::vars_for`]).
pub(crate) fn for_command(
    cwd: &Path,
    args: &[String],
    env: &[(&str, &str)],
    askpass: Option<&crate::gitcli::AskpassEnv>,
) -> Vec<(&'static str, OsString)> {
    let Some(a) = askpass.filter(|_| allows_prompts(env)) else { return Vec::new() };
    let mut out = vars(&a.exe, std::env::var_os("GIT_CONFIG_COUNT").as_deref(), &crate::git_program::program());
    out.extend(super::ssh_env::vars_for(cwd, args, env));
    out
}
