//! Reading the git configuration values Preferences shows, and writing one.

use crate::effect::PrefsEffect;
use crate::msg::{Msg, PrefsMsg};
use crate::state::prefs::{Autocrlf, GitConfigValues};
use std::path::Path;
use trellis_core::gitcli::{self, args};

/// Runs one `Effect::Prefs`: config values here, signing keys in keys.rs.
pub(super) fn run(e: PrefsEffect) -> Option<Msg> {
    match e {
        PrefsEffect::ReadGitConfig { cwd, has_repo } => Some(Msg::Prefs(PrefsMsg::GitConfigLoaded(read_git_config(&cwd, has_repo)))),
        PrefsEffect::SetGitConfig { cwd, global, key, value } => set(&cwd, global, &key, value.as_deref()),
        PrefsEffect::ListSigningKeys { format, program } => Some(super::keys::list(format, &program)),
        PrefsEffect::GenerateGpgKey { program, name, email, passphrase } => {
            Some(super::keys::generate_gpg(&program, &name, &email, &passphrase))
        }
        PrefsEffect::ExportPublicKey { format, program, key } => Some(super::keys::export_public(format, &program, &key)),
    }
}

/// Sets or (for None) unsets one config value; a failure goes to the activity log.
pub(super) fn set(cwd: &Path, global: bool, key: &str, value: Option<&str>) -> Option<Msg> {
    trellis_core::identity::set_config(cwd, global, key, value, &[]).err().map(|text| Msg::Logged { ok: false, text })
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
    // `kind` is a `--type` (git's own reading of yes/on/1 and the rest), or None for text.
    let get = |global: bool, kind: Option<&str>, key: &str| -> Option<String> {
        if !global && !has_repo {
            return None;
        }
        let mut a = args(&["config"]);
        a.extend(if global { global_scope.clone() } else { args(&["--local"]) });
        a.extend(kind.map(|k| format!("--type={k}")));
        a.extend(args(&["--get", key]));
        let out = gitcli::read(cwd, &a);
        out.ok.then(|| out.stdout.trim().to_owned())
    };
    let read = |global: bool, key: &str| get(global, None, key).unwrap_or_default();
    // A value git can't read as a boolean shows as off; it is only rewritten if the user clicks.
    let flag = |key: &str| get(true, Some("bool"), key).as_deref() == Some("true");
    let autocrlf = match get(true, None, "core.autocrlf") {
        None => Autocrlf::Unset,
        Some(raw) if raw.eq_ignore_ascii_case("input") => Autocrlf::Input,
        Some(raw) => match get(true, Some("bool"), "core.autocrlf").as_deref() {
            Some("true") => Autocrlf::True,
            Some("false") => Autocrlf::False,
            _ => Autocrlf::Other(raw),
        },
    };
    let format = read(true, "gpg.format");
    GitConfigValues {
        longpaths: flag("core.longpaths"),
        autocrlf,
        user_name: read(true, "user.name"),
        user_email: read(true, "user.email"),
        gpgsign: flag("commit.gpgsign"),
        signing_format: if format.is_empty() { "openpgp".into() } else { format },
        signing_key: read(true, "user.signingkey"),
        repo_name: read(false, "user.name"),
        repo_email: read(false, "user.email"),
        hooks_path: read(false, "core.hooksPath"),
        gpg_program: read(true, "gpg.program"),
        ssh_program: read(true, "gpg.ssh.program"),
        x509_program: read(true, "gpg.x509.program"),
        tag_gpgsign: flag("tag.gpgsign"),
        ssh_allowed_signers: read(true, "gpg.ssh.allowedSignersFile"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_with(text: &str) -> GitConfigValues {
        let dir = trellis_core::paths::temp_dir().join(format!("trellis-config-bools-{}-{}", std::process::id(), text.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("gitconfig");
        std::fs::write(&file, text).unwrap();
        let v = read_from(&dir, false, Some(&file));
        let _ = std::fs::remove_dir_all(&dir);
        v
    }

    #[test]
    fn git_booleans_are_read_as_git_reads_them() {
        let v = read_with("[core]\n\tlongpaths = yes\n\tautocrlf = input\n[commit]\n\tgpgsign = 1\n[tag]\n\tgpgsign = On\n");
        assert!(v.longpaths && v.gpgsign && v.tag_gpgsign, "{v:?}");
        assert_eq!(v.autocrlf, Autocrlf::Input, "input is neither on nor off");
        let v = read_with("[core]\n\tlongpaths = off\n\tautocrlf = TRUE\n[commit]\n\tgpgsign = 0\n");
        assert!(!v.longpaths && !v.gpgsign, "{v:?}");
        assert_eq!(v.autocrlf, Autocrlf::True);
        let v = read_with("[core]\n\tautocrlf = no\n");
        assert_eq!(v.autocrlf, Autocrlf::False);
        let v = read_with("[user]\n\tname = x\n");
        assert_eq!(v.autocrlf, Autocrlf::Unset);
        assert!(!v.longpaths);
        let v = read_with("[core]\n\tautocrlf = sometimes\n\tlongpaths = maybe\n");
        assert_eq!(v.autocrlf, Autocrlf::Other("sometimes".into()), "shown as it is, not overwritten");
        assert!(!v.longpaths);
    }
}
