//! The environment that makes git run Trellis as its gpg.program. Pure.

use super::ENV_WRAPPER;
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Command-scope config (git 2.31 or newer reads `GIT_CONFIG_COUNT`) setting `gpg.program` to
/// `exe`, plus the wrapper marker. Empty when Trellis itself was started with
/// `GIT_CONFIG_COUNT` set (`inherited_count`): those entries are kept, and GnuPG's own window
/// is used.
pub fn vars(exe: &Path, inherited_count: Option<&OsStr>) -> Vec<(&'static str, OsString)> {
    if inherited_count.is_some_and(|c| !c.is_empty()) {
        return Vec::new();
    }
    vec![
        ("GIT_CONFIG_COUNT", "1".into()),
        ("GIT_CONFIG_KEY_0", "gpg.program".into()),
        ("GIT_CONFIG_VALUE_0", crate::native_path::native(exe).into_os_string()),
        (ENV_WRAPPER, "1".into()),
    ]
}

/// Whether a command run with the caller environment `env` may prompt: not with
/// [`crate::gitcli::NO_PROMPTS`] (whose empty `GIT_ASKPASS` marks it).
pub fn allows_prompts(env: &[(&str, &str)]) -> bool {
    !env.contains(&("GIT_ASKPASS", ""))
}
