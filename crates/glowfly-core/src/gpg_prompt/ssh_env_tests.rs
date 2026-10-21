use super::ssh_env::{config_args, may_sign, parse_config, vars, vars_for};
use crate::askpass::ENV_SSH_SIGNING_KEY;
use crate::gitcli::args;
use crate::signing::SigningFormat;
use crate::testutil::{empty_global_config, scratch_repo, set_config};
use std::ffi::OsString;
use std::path::Path;

/// The marker's value, if `v` has it.
fn marker(v: &[(&'static str, OsString)]) -> Option<String> {
    v.iter().find(|(k, _)| *k == ENV_SSH_SIGNING_KEY).map(|(_, x)| x.to_string_lossy().into_owned())
}

#[test]
fn the_marker_is_set_only_for_ssh_format() {
    let path = crate::paths::temp_dir().join("id_ed25519.pub").display().to_string();
    assert_eq!(vars(SigningFormat::Ssh, Some(&path)), [(ENV_SSH_SIGNING_KEY, OsString::from(&path))]);
    assert!(vars(SigningFormat::OpenPgp, Some(&path)).is_empty());
    assert!(vars(SigningFormat::X509, Some(&path)).is_empty());
    for key in [None, Some(""), Some("key::ssh-ed25519 AAAA x"), Some("ssh-ed25519 AAAA x")] {
        assert!(vars(SigningFormat::Ssh, key).is_empty(), "{key:?}: ssh-agent only");
    }
}

#[test]
fn config_is_read_like_git_reads_it() {
    let z = "gpg.format\nssh\0user.signingkey\n/k\0gpg.format\nopenpgp\0";
    assert_eq!(parse_config(z), (SigningFormat::OpenPgp, Some("/k".into())), "the last format wins");
    assert_eq!(parse_config("gpg.format\nssh\0user.signingkey\n/a\0user.signingkey\n/b\0"), (SigningFormat::Ssh, Some("/b".into())));
    assert_eq!(parse_config(""), (SigningFormat::OpenPgp, None));

    let a = config_args(&args(&["-c", "gpg.format=ssh", "--no-pager", "commit", "-c", "x"]));
    let want = ["-c", "gpg.format=ssh", "--no-pager", "config", "-z", "--type=path", "--get-regexp", r"^(gpg\.format|user\.signingkey)$"];
    assert_eq!(a, want);

    for sub in ["commit", "tag", "merge", "rebase", "cherry-pick", "revert", "am", "pull", "commit-tree"] {
        assert!(may_sign(&args(&[sub])), "{sub}");
        assert!(may_sign(&args(&["-c", "x=y", sub])), "{sub} behind -c");
    }
    for sub in ["status", "fetch", "push", "stash"] {
        assert!(!may_sign(&args(&[sub, "commit"])), "{sub}");
    }
}

#[test]
fn a_fixture_repo_gets_the_marker_for_signing_commands() {
    let dir = scratch_repo("ssh-sign-env");
    let global = empty_global_config(&dir);
    let home = dir.with_file_name(format!("{}-home", dir.file_name().unwrap().to_string_lossy()));
    std::fs::create_dir_all(&home).unwrap();
    let home_text = home.display().to_string();
    let mut owned: Vec<(&str, &str)> = global.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    owned.extend([("HOME", home_text.as_str()), ("USERPROFILE", home_text.as_str())]);
    let env = owned.as_slice();
    let commit = args(&["commit", "-m", "x"]);

    let key = crate::paths::temp_dir().join("trellis-ssh-sign-env-key.pub");
    let key_text = key.display().to_string();
    set_config(&dir, &[("gpg.format", "ssh"), ("user.signingkey", &key_text)]);
    let got = marker(&vars_for(&dir, &commit, env)).expect("a marker for commit");
    assert!(crate::paths::same_spelling(Path::new(&got), &key), "{got:?} is not {key:?}");
    assert!(vars_for(&dir, &args(&["status"]), env).is_empty(), "status doesn't sign");

    set_config(&dir, &[("gpg.format", "openpgp")]);
    assert!(vars_for(&dir, &commit, env).is_empty());
    let got = marker(&vars_for(&dir, &args(&["-c", "gpg.format=ssh", "commit", "-m", "x"]), env));
    assert!(got.is_some(), "the command line's -c counts");

    set_config(&dir, &[("gpg.format", "ssh"), ("user.signingkey", "~/k.pub")]);
    let got = marker(&vars_for(&dir, &commit, env)).expect("a marker for a home-relative key");
    let want = home.join("k.pub");
    assert!(crate::paths::same_spelling(Path::new(&got), &want), "{got:?} is not {want:?}");

    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&dir);
}
