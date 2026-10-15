//! Reading the git configuration values Preferences shows, and writing one.

use crate::msg::Msg;
use crate::state::prefs::GitConfigValues;
use std::path::Path;
use trellis_core::gitcli::{self, args};

/// Sets or (for None) unsets one config value; a failure goes to the activity log.
pub(super) fn set(cwd: &Path, global: bool, key: &str, value: Option<&str>) -> Option<Msg> {
    trellis_core::identity::set_config(cwd, global, key, value, &[]).err().map(|text| Msg::Logged { ok: false, text })
}

pub(super) fn read_git_config(cwd: &Path, has_repo: bool) -> GitConfigValues {
    let read = |global: bool, key: &str| {
        if !global && !has_repo {
            return String::new();
        }
        let scope = if global { "--global" } else { "--local" };
        let out = gitcli::read(cwd, &args(&["config", scope, "--get", key]));
        if out.ok { out.stdout.trim().to_owned() } else { String::new() }
    };
    let format = read(true, "gpg.format");
    GitConfigValues {
        longpaths: read(true, "core.longpaths") == "true",
        autocrlf: read(true, "core.autocrlf") == "true",
        user_name: read(true, "user.name"),
        user_email: read(true, "user.email"),
        gpgsign: read(true, "commit.gpgsign") == "true",
        signing_format: if format.is_empty() { "openpgp".into() } else { format },
        signing_key: read(true, "user.signingkey"),
        repo_name: read(false, "user.name"),
        repo_email: read(false, "user.email"),
        hooks_path: read(false, "core.hooksPath"),
        gpg_program: read(true, "gpg.program"),
        ssh_program: read(true, "gpg.ssh.program"),
        x509_program: read(true, "gpg.x509.program"),
        tag_gpgsign: read(true, "tag.gpgsign") == "true",
        ssh_allowed_signers: read(true, "gpg.ssh.allowedSignersFile"),
    }
}
