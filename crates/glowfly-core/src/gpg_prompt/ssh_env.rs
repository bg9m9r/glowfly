//! The environment that marks ssh-keygen's passphrase prompt as a signing prompt.
//!
//! With `gpg.format=ssh` git runs `gpg.ssh.program` (ssh-keygen) itself, not the gpg.program
//! wrapper. ssh-keygen asks for a protected key's passphrase through `SSH_ASKPASS`, which
//! gitcli already points at Trellis; this adds the `user.signingKey` file
//! ([`crate::askpass::ENV_SSH_SIGNING_KEY`]) for commands that may sign, so the helper can tell
//! that prompt from an ssh connection's ([`crate::askpass::ssh_sign`]).

use crate::askpass::ENV_SSH_SIGNING_KEY;
use crate::signing::SigningFormat;
use std::ffi::OsString;
use std::path::Path;

/// The git subcommands that can sign a commit or tag.
pub const MAY_SIGN: &[&str] = &["commit", "commit-tree", "merge", "rebase", "cherry-pick", "revert", "am", "pull", "tag"];

/// Whether `args` run a subcommand that can sign.
pub fn may_sign(args: &[String]) -> bool {
    crate::gitcli::subcommand(args).is_some_and(|s| MAY_SIGN.contains(&s))
}

/// The key file in a `user.signingKey` value. None when it is empty or a literal key
/// (`key::…`, or the older bare `ssh-…`): git then passes `-U` and ssh-keygen signs only
/// through ssh-agent, so it never asks for a passphrase.
pub fn key_file(value: &str) -> Option<&str> {
    let v = value.trim();
    (!v.is_empty() && !v.starts_with("key::") && !v.starts_with("ssh-")).then_some(v)
}

/// The marker for a command signing in `format` with `signing_key`: the key file, as git has
/// it, only for SSH-format signing with a key file. Pure.
pub fn vars(format: SigningFormat, signing_key: Option<&str>) -> Vec<(&'static str, OsString)> {
    match (format, signing_key.and_then(key_file)) {
        (SigningFormat::Ssh, Some(file)) => vec![(ENV_SSH_SIGNING_KEY, file.into())],
        _ => Vec::new(),
    }
}

/// The signing format and key from `git config -z --get-regexp` output (`key\nvalue\0`
/// entries, keys lowercased): the last `gpg.format` (OpenPGP when absent) and the last
/// `user.signingkey`, as git reads them. Pure.
pub fn parse_config(z: &str) -> (SigningFormat, Option<String>) {
    let (mut format, mut key) = (SigningFormat::OpenPgp, None);
    for (name, value) in z.split('\0').filter_map(|entry| entry.split_once('\n')) {
        match name {
            "gpg.format" => format = SigningFormat::from_config(value),
            "user.signingkey" => key = Some(value.to_owned()),
            _ => {}
        }
    }
    (format, key)
}

/// The `git config` query for the signing settings a command `args` sees: its leading global
/// options (so `-c gpg.format=ssh` counts), then the query. `user.signingKey` is read as a path
/// because git expands `~/` in it.
pub fn config_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-c" | "-C" => {
                out.push(a.clone());
                out.extend(it.next().cloned());
            }
            s if s.starts_with('-') => out.push(a.clone()),
            _ => break,
        }
    }
    out.extend(["config", "-z", "--type=path", "--get-regexp", r"^(gpg\.format|user\.signingkey)$"].map(String::from));
    out
}

/// [`vars`] for `git <args>` run in `cwd` with the caller environment `env`: empty unless the
/// command may sign; otherwise one quick `git config` read (with [`crate::gitcli::NO_PROMPTS`]
/// last, so it never prompts and gets no signing environment itself). A failed read (no
/// entries) gives OpenPGP, so nothing is added.
pub(crate) fn vars_for(cwd: &Path, args: &[String], env: &[(&str, &str)]) -> Vec<(&'static str, OsString)> {
    if !may_sign(args) {
        return Vec::new();
    }
    let merged: Vec<(&str, &str)> = env.iter().chain(crate::gitcli::NO_PROMPTS).copied().collect();
    let out = crate::gitcli::output(cwd, &config_args(args), "", &merged, Some(crate::gitcli::READ_TIMEOUT));
    let (format, key) = parse_config(&out.stdout);
    vars(format, key.as_deref())
}
