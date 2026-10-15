//! The author identity git writes into commits (user.name, user.email, user.signingkey), and
//! the one path Trellis uses to write a git config value through the git CLI.

use crate::gitcli::{self, args};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// An author identity: what a profile keeps and what switching to it writes to git config.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub email: String,
    /// None keeps whatever key git has.
    pub signing_key: Option<String>,
}

/// The config writes that make `id` git's identity, in order: user.name and user.email (None
/// unsets a blank one), then user.signingkey only when the identity has a key.
pub fn writes(id: &Identity) -> Vec<(&'static str, Option<String>)> {
    let value = |v: &str| Some(v.trim().to_owned()).filter(|v| !v.is_empty());
    let mut out = vec![("user.name", value(&id.name)), ("user.email", value(&id.email))];
    if let Some(key) = id.signing_key.as_deref().and_then(value) {
        out.push(("user.signingkey", Some(key)));
    }
    out
}

/// Sets `key` to `value` in the global or the repository's config, or unsets it for None. An
/// unset of a key that isn't there (git exits 5) counts as done.
pub fn set_config(cwd: &Path, global: bool, key: &str, value: Option<&str>, env: &[(&str, &str)]) -> Result<(), String> {
    let scope = if global { "--global" } else { "--local" };
    let a = match value {
        Some(v) => args(&["config", scope, key, v]),
        None => args(&["config", scope, "--unset", key]),
    };
    let (ok, out) = gitcli::run_with_input(cwd, &a, "", env);
    if ok || value.is_none() { Ok(()) } else { Err(format!("git config {key} failed: {out}")) }
}

/// Writes `id` to the global config (see `writes`), stopping at the first failure. One write
/// at a time: parallel `git config` runs on the same file fight over its lock.
pub fn write_global(cwd: &Path, id: &Identity, env: &[(&str, &str)]) -> Result<(), String> {
    for (key, value) in writes(id) {
        set_config(cwd, true, key, value.as_deref(), env)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{empty_global_config, scratch_repo};

    fn id(name: &str, email: &str, key: Option<&str>) -> Identity {
        Identity { name: name.into(), email: email.into(), signing_key: key.map(str::to_owned) }
    }

    fn get(dir: &Path, env: &[(&str, &str)], scope: &str, key: &str) -> Option<String> {
        let (ok, out) = gitcli::run_with_input(dir, &args(&["config", scope, "--get", key]), "", env);
        ok.then(|| out.trim().to_owned())
    }

    fn borrowed(env: &[(String, String)]) -> Vec<(&str, &str)> {
        env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    #[test]
    fn writes_name_email_then_a_set_key() {
        let s = |v: &str| Some(v.to_owned());
        assert_eq!(
            writes(&id(" Ada ", "ada@example.com", Some(" ABCD "))),
            vec![("user.name", s("Ada")), ("user.email", s("ada@example.com")), ("user.signingkey", s("ABCD"))]
        );
        assert_eq!(writes(&id("  ", "", None)), vec![("user.name", None), ("user.email", None)]);
        assert_eq!(writes(&id("Ada", "a@b", Some("  "))).len(), 2, "a blank key writes nothing");
    }

    #[test]
    fn write_global_sets_the_global_file_only() {
        let dir = scratch_repo("identity-write");
        let env = empty_global_config(&dir);
        let env = borrowed(&env);
        write_global(&dir, &id("Ada", "ada@example.com", Some("ABCD")), &env).unwrap();
        assert_eq!(get(&dir, &env, "--global", "user.name").as_deref(), Some("Ada"));
        assert_eq!(get(&dir, &env, "--global", "user.email").as_deref(), Some("ada@example.com"));
        assert_eq!(get(&dir, &env, "--global", "user.signingkey").as_deref(), Some("ABCD"));
        assert_eq!(get(&dir, &env, "--local", "user.name").as_deref(), Some("Test"), "the repository's own identity stays");
        assert_eq!(get(&dir, &env, "--local", "user.email").as_deref(), Some("test@example.com"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn blanks_unset_and_no_key_keeps_the_key() {
        let dir = scratch_repo("identity-unset");
        let env = empty_global_config(&dir);
        let env = borrowed(&env);
        write_global(&dir, &id("Ada", "ada@example.com", Some("ABCD")), &env).unwrap();
        write_global(&dir, &id("", " ", None), &env).unwrap();
        assert_eq!(get(&dir, &env, "--global", "user.name"), None);
        assert_eq!(get(&dir, &env, "--global", "user.email"), None);
        assert_eq!(get(&dir, &env, "--global", "user.signingkey").as_deref(), Some("ABCD"));
        // Unsetting what isn't there is fine.
        assert_eq!(set_config(&dir, true, "user.name", None, &env), Ok(()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_write_stops_there() {
        let dir = scratch_repo("identity-fail");
        let mut env = empty_global_config(&dir);
        // A folder can't be written as a config file.
        let folder = dir.join("not-a-file");
        std::fs::create_dir_all(&folder).unwrap();
        env[0].1 = folder.display().to_string();
        let env = borrowed(&env);
        let err = write_global(&dir, &id("Ada", "ada@example.com", None), &env).unwrap_err();
        assert!(err.contains("user.name"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
