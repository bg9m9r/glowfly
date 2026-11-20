//! Reading the git configuration values Preferences shows, and writing one or a page's save;
//! reading the settings file again after it couldn't be read.

use super::execute::{Cx, Execute};
use glowfly_core::config_list::{ConfigList, parse_bool};
use glowfly_core::gitcli::{self, args};
use glowfly_model::effect::{ConfigWrite, PrefsEffect};
use glowfly_model::msg::{Msg, PrefsMsg};
use glowfly_model::state::prefs::{Autocrlf, GitConfigValues};
use std::path::Path;

/// Runs one `Effect::Prefs`: config values here, signing keys in keys.rs.
pub(super) fn run(e: PrefsEffect) -> Option<Msg> {
    match e {
        PrefsEffect::ReadGitConfig { cwd, has_repo } => Some(Msg::Prefs(PrefsMsg::GitConfigLoaded(read_git_config(&cwd, has_repo)))),
        PrefsEffect::SetGitConfig { cwd, global, key, value } => set(&cwd, global, &key, value.as_deref()),
        PrefsEffect::WriteGitConfig { cwd, ticket, writes } => {
            Some(Msg::Prefs(PrefsMsg::ConfigSaved { ticket, result: write_all(&cwd, &writes) }))
        }
        PrefsEffect::ListSigningKeys { format, program } => Some(super::keys::list(format, &program)),
        PrefsEffect::GenerateGpgKey { program, name, email, passphrase } => {
            Some(super::keys::generate_gpg(&program, &name, &email, &passphrase))
        }
        PrefsEffect::ExportPublicKey { format, program, key } => Some(super::keys::export_public(format, &program, &key)),
        PrefsEffect::ExtendGpgKey { program, key } => Some(super::keys::extend_gpg(&program, &key)),
        PrefsEffect::ReadSettings => Some(Msg::Prefs(PrefsMsg::SettingsReread(Box::new(glowfly_core::settings::Settings::reload())))),
    }
}

impl Execute for PrefsEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        run(self)
    }
}

/// Sets or (for None) unsets one config value; a failure goes to the activity log.
pub(super) fn set(cwd: &Path, global: bool, key: &str, value: Option<&str>) -> Option<Msg> {
    glowfly_core::identity::set_config(cwd, global, key, value, &[]).err().map(|e| Msg::Logged { ok: false, text: e.to_string() })
}

/// Writes each value in order, stopping at the first git refuses (its error).
fn write_all(cwd: &Path, writes: &[ConfigWrite]) -> Result<(), glowfly_core::identity::ConfigWriteFailed> {
    for w in writes {
        glowfly_core::identity::set_config(cwd, w.global, &w.key, w.value.as_deref(), &[])?;
    }
    Ok(())
}

pub(super) fn read_git_config(cwd: &Path, has_repo: bool) -> GitConfigValues {
    read_from(cwd, has_repo, None)
}

/// `read_git_config`, reading the global values from `global_file` instead (tests).
fn read_from(cwd: &Path, has_repo: bool, global_file: Option<&Path>) -> GitConfigValues {
    let global_scope: Vec<String> = match global_file {
        Some(f) => vec!["--file".into(), f.display().to_string()],
        None => args(&["--global"]),
    };
    read_with(has_repo, global_scope, &mut |scope| list(cwd, scope))
}

/// The values from one list per scope: the global one, and the repository's when there is one.
fn read_with(has_repo: bool, global_scope: Vec<String>, run: &mut dyn FnMut(Vec<String>) -> ConfigList) -> GitConfigValues {
    let global = run(global_scope);
    let local = if has_repo { run(args(&["--local"])) } else { ConfigList::default() };
    values(&global, &local)
}

/// One scope's entries (`git config <scope> --list -z`). A failed read (no file, a malformed
/// one, no repository) is empty, as each `--get` of it was.
fn list(cwd: &Path, scope: Vec<String>) -> ConfigList {
    let mut a = args(&["config"]);
    a.extend(scope);
    a.extend(args(&["--list", "-z"]));
    let out = gitcli::read(cwd, &a);
    if out.ok { ConfigList::parse(&out.stdout) } else { ConfigList::default() }
}

/// What Preferences shows, from the global and the repository's own config.
fn values(global: &ConfigList, local: &ConfigList) -> GitConfigValues {
    let read = |c: &ConfigList, key: &str| c.text(key).map(str::trim).unwrap_or_default().to_owned();
    // A value git can't read as a boolean shows as off; it is only rewritten if the user clicks.
    let flag = |key: &str| global.bool(key) == Some(true);
    let autocrlf = match global.last("core.autocrlf") {
        None => Autocrlf::Unset,
        Some(v) => {
            let raw = v.unwrap_or("").trim();
            if raw.eq_ignore_ascii_case("input") {
                Autocrlf::Input
            } else {
                match parse_bool(v) {
                    Some(true) => Autocrlf::True,
                    Some(false) => Autocrlf::False,
                    None => Autocrlf::Other(raw.to_owned()),
                }
            }
        }
    };
    let format = read(global, "gpg.format");
    GitConfigValues {
        longpaths: flag("core.longpaths"),
        autocrlf,
        user_name: read(global, "user.name"),
        user_email: read(global, "user.email"),
        gpgsign: flag("commit.gpgsign"),
        signing_format: if format.is_empty() { "openpgp".into() } else { format },
        signing_key: read(global, "user.signingkey"),
        repo_name: read(local, "user.name"),
        repo_email: read(local, "user.email"),
        hooks_path: read(local, "core.hooksPath"),
        // A hook's path isn't a boolean: only git's own daemon reads as on.
        fsmonitor: local.bool("core.fsmonitor") == Some(true),
        gpg_program: read(global, "gpg.program"),
        ssh_program: read(global, "gpg.ssh.program"),
        x509_program: read(global, "gpg.x509.program"),
        tag_gpgsign: flag("tag.gpgsign"),
        ssh_allowed_signers: read(global, "gpg.ssh.allowedSignersFile"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_text(text: &str) -> GitConfigValues {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-config-bools-{}-{}", std::process::id(), text.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("gitconfig");
        std::fs::write(&file, text).unwrap();
        let v = read_from(&dir, false, Some(&file));
        let _ = std::fs::remove_dir_all(&dir);
        v
    }

    #[test]
    fn git_booleans_are_read_as_git_reads_them() {
        let v = read_text("[core]\n\tlongpaths = yes\n\tautocrlf = input\n[commit]\n\tgpgsign = 1\n[tag]\n\tgpgsign = On\n");
        assert!(v.longpaths && v.gpgsign && v.tag_gpgsign, "{v:?}");
        assert_eq!(v.autocrlf, Autocrlf::Input, "input is neither on nor off");
        let v = read_text("[core]\n\tlongpaths = off\n\tautocrlf = TRUE\n[commit]\n\tgpgsign = 0\n");
        assert!(!v.longpaths && !v.gpgsign, "{v:?}");
        assert_eq!(v.autocrlf, Autocrlf::True);
        let v = read_text("[core]\n\tautocrlf = no\n");
        assert_eq!(v.autocrlf, Autocrlf::False);
        let v = read_text("[user]\n\tname = x\n");
        assert_eq!(v.autocrlf, Autocrlf::Unset);
        assert!(!v.longpaths);
        let v = read_text("[core]\n\tautocrlf = sometimes\n\tlongpaths = maybe\n");
        assert_eq!(v.autocrlf, Autocrlf::Other("sometimes".into()), "shown as it is, not overwritten");
        assert!(!v.longpaths);
    }

    fn lists(global: &str, local: &str) -> GitConfigValues {
        values(&ConfigList::parse(global), &ConfigList::parse(local))
    }

    #[test]
    fn values_read_keys_in_any_case() {
        let v = lists("core.longpaths\0commit.gpgsign\ntrue\0core.autocrlf\0user.name\nA\0user.name\n B \0gpg.format\n\0", "");
        assert!(v.longpaths && v.gpgsign, "a valueless key is on: {v:?}");
        assert_eq!(v.autocrlf, Autocrlf::True);
        assert_eq!(v.user_name, "B", "the last value, trimmed");
        assert_eq!(v.signing_format, "openpgp");
        assert_eq!(lists("core.autocrlf\n\0", "").autocrlf, Autocrlf::False);
        assert_eq!(lists("core.autocrlf\n sometimes \0", "").autocrlf, Autocrlf::Other("sometimes".into()));
        assert_eq!(lists("core.autocrlf\nInput\0", "").autocrlf, Autocrlf::Input);
        assert_eq!(lists("", "").autocrlf, Autocrlf::Unset);
        assert!(!lists("", "core.fsmonitor\n/path/hook\0").fsmonitor);
        assert!(lists("", "core.fsmonitor\ntrue\0").fsmonitor);
        let v = lists("user.name\nglobal\0", "user.name\nrepo\0core.hookspath\n.hooks\0");
        assert_eq!((v.user_name.as_str(), v.repo_name.as_str(), v.hooks_path.as_str()), ("global", "repo", ".hooks"));
    }

    #[test]
    fn reads_local_values_with_a_repo() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-config-local-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("gitconfig");
        std::fs::write(&file, "").unwrap();
        let repo = glowfly_model::testing::scratch_repo("config-local-values");
        let v = read_from(&repo, true, Some(&file));
        assert_eq!((v.repo_name.as_str(), v.repo_email.as_str()), ("Test", "test@example.com"), "{v:?}");
        assert_eq!((v.user_name.as_str(), v.user_email.as_str()), ("", ""));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn preferences_read_runs_one_list_per_scope() {
        let scopes = |has_repo: bool| {
            let mut seen: Vec<Vec<String>> = Vec::new();
            read_with(has_repo, args(&["--global"]), &mut |scope| {
                seen.push(scope);
                ConfigList::default()
            });
            seen
        };
        assert_eq!(scopes(true), [args(&["--global"]), args(&["--local"])]);
        assert_eq!(scopes(false), [args(&["--global"])]);
    }
}
