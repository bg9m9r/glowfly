//! The author identity git writes into commits (user.name, user.email, user.signingkey), reading
//! the global identity, and the one path Glowfly uses to write a git config value through the
//! git CLI.

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

/// Writes `id` to the global config (see `writes`), one write at a time: parallel `git config`
/// runs on the same file fight over its lock. All or nothing: when a write fails, the keys
/// written before it get their earlier global values back, so git never keeps half of `id`.
pub fn write_global(cwd: &Path, id: &Identity, env: &[(&str, &str)]) -> Result<(), String> {
    write_global_all(cwd, &writes(id), env)
}

/// The identity in the global config (user.name, user.email, user.signingkey; blank or unset
/// values read as empty, a blank key as None). The one read the first-run welcome uses. Goes
/// through the git CLI, so includes and the XDG config count like they do for git.
pub fn read_global(cwd: &Path, env: &[(&str, &str)]) -> Result<Identity, String> {
    let name = global_value(cwd, "user.name", env)?.unwrap_or_default();
    let email = global_value(cwd, "user.email", env)?.unwrap_or_default();
    let signing_key = global_value(cwd, "user.signingkey", env)?.filter(|k| !k.trim().is_empty());
    Ok(Identity { name, email, signing_key })
}

/// Applies `writes` to the global config in order, undoing the done ones on the first failure.
fn write_global_all(cwd: &Path, writes: &[(&str, Option<String>)], env: &[(&str, &str)]) -> Result<(), String> {
    let mut done: Vec<(&str, Option<String>)> = Vec::new();
    for (key, value) in writes {
        let step = global_value(cwd, key, env).and_then(|before| {
            set_config(cwd, true, key, value.as_deref(), env)?;
            Ok(before)
        });
        match step {
            Ok(before) => done.push((key, before)),
            Err(e) => {
                let undone = done.iter().rev().all(|(k, v)| set_config(cwd, true, k, v.as_deref(), env).is_ok());
                return Err(if undone { e } else { format!("{e} (the values written before it could not be put back)") });
            }
        }
    }
    Ok(())
}

/// `key`'s value in the global config, None when it isn't set.
fn global_value(cwd: &Path, key: &str, env: &[(&str, &str)]) -> Result<Option<String>, String> {
    let out = gitcli::output(cwd, &args(&["config", "--global", "--get", key]), "", env, None);
    match (out.ok, out.code) {
        (true, _) => Ok(Some(out.stdout.trim_end_matches(['\r', '\n']).to_owned())),
        (false, Some(1)) => Ok(None),
        _ => Err(format!("git config {key} failed: {}", out.combined())),
    }
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

    #[test]
    fn read_global_reads_the_global_file() {
        let dir = scratch_repo("identity-read");
        let env = empty_global_config(&dir);
        let env = borrowed(&env);
        assert_eq!(read_global(&dir, &env), Ok(Identity::default()), "the repository's own Test is not global");
        write_global(&dir, &id("Ada", "ada@example.com", Some("ABCD")), &env).unwrap();
        assert_eq!(read_global(&dir, &env), Ok(id("Ada", "ada@example.com", Some("ABCD"))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_global_fails_on_an_unreadable_config() {
        let dir = scratch_repo("identity-read-fail");
        let env = empty_global_config(&dir);
        // A config file git can't parse (a folder reads as empty on some systems).
        std::fs::write(
            &env[0].1,
            "[user
	name = Ada
",
        )
        .unwrap();
        let env = borrowed(&env);
        let err = read_global(&dir, &env).unwrap_err();
        assert!(err.contains("user.name"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_write_puts_the_earlier_ones_back() {
        let dir = scratch_repo("identity-undo");
        let env = empty_global_config(&dir);
        let env = borrowed(&env);
        write_global(&dir, &id("Ada", "ada@work.example", None), &env).unwrap();
        let s = |v: &str| Some(v.to_owned());
        // The name and a key that wasn't set land, then the next write fails (an invalid key).
        let writes = [("user.name", s("Ada L")), ("user.signingkey", s("HOMEKEY")), ("not a key", s("x"))];
        let err = write_global_all(&dir, &writes, &env).unwrap_err();
        assert!(err.contains("not a key") && !err.contains("put back"), "{err}");
        assert_eq!(get(&dir, &env, "--global", "user.name").as_deref(), Some("Ada"));
        assert_eq!(get(&dir, &env, "--global", "user.email").as_deref(), Some("ada@work.example"));
        assert_eq!(get(&dir, &env, "--global", "user.signingkey"), None, "unset again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
