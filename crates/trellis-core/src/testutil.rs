//! Fixture helpers shared by `trellis-core` tests: throwaway repositories and git commands.

use crate::gitcli::{args, run};
use std::path::{Path, PathBuf};

/// Runs `git <a>` in `dir`, failing the test if it fails.
pub(crate) fn git(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
}

/// An empty repository on `main` with a fixed identity, in a fresh temporary folder.
pub(crate) fn scratch_repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trellis-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-b", "main"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["config", "commit.gpgsign", "false"]);
    dir
}

pub(crate) fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

/// A repository with one commit on `main`, pushed with upstream to a bare `origin` next to it.
/// Returns (working repository, bare repository).
pub(crate) fn with_origin(name: &str) -> (PathBuf, PathBuf) {
    let work = scratch_repo(name);
    write(&work, "a.txt", "one\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "first"]);
    let bare = work.with_file_name(format!("{}-origin.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "origin", &bare.display().to_string()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    (work, bare)
}

/// Commits `file` on `branch` in the bare repository `bare` (through a temporary clone), so
/// the remote gets ahead of local copies.
pub(crate) fn remote_commit(bare: &Path, branch: &str, file: &str, msg: &str) {
    let tmp = bare.with_file_name(format!("{}-clone", bare.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&tmp);
    let parent = tmp.parent().unwrap();
    git(parent, &["clone", "-q", "-b", branch, &bare.display().to_string(), &tmp.display().to_string()]);
    git(&tmp, &["config", "user.name", "Test"]);
    git(&tmp, &["config", "user.email", "test@example.com"]);
    git(&tmp, &["config", "commit.gpgsign", "false"]);
    write(&tmp, file, msg);
    git(&tmp, &["add", "."]);
    git(&tmp, &["commit", "-q", "-m", msg]);
    git(&tmp, &["push", "-q", "origin", branch]);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// A repository with one submodule at libs/lib, added from a sibling repository by its relative
/// URL and committed. Returns (parent, the submodule's checkout), with an identity set inside
/// the checkout so tests can commit there.
pub(crate) fn with_submodule(name: &str) -> (PathBuf, PathBuf) {
    let lib = scratch_repo(&format!("{name}-lib"));
    write(&lib, "lib.txt", "lib\n");
    git(&lib, &["add", "."]);
    git(&lib, &["commit", "-q", "-m", "lib"]);
    let parent = scratch_repo(name);
    write(&parent, "a.txt", "one\n");
    git(&parent, &["add", "."]);
    git(&parent, &["commit", "-q", "-m", "first"]);
    let url = format!("../{}", lib.file_name().unwrap().to_string_lossy());
    // Git refuses the file transport for submodules unless allowed on the command line.
    git(&parent, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "libs/lib"]);
    git(&parent, &["commit", "-q", "-m", "add lib"]);
    let sub = parent.join("libs/lib");
    git(&sub, &["config", "user.name", "Test"]);
    git(&sub, &["config", "user.email", "test@example.com"]);
    git(&sub, &["config", "commit.gpgsign", "false"]);
    (parent, sub)
}

/// Environment that keeps the developer's global and system git config out of a command: an
/// empty file (next to `dir`) as GIT_CONFIG_GLOBAL, and GIT_CONFIG_NOSYSTEM=1.
pub(crate) fn empty_global_config(dir: &Path) -> Vec<(String, String)> {
    let file = dir.with_file_name(format!("{}-gitconfig", dir.file_name().unwrap().to_string_lossy()));
    std::fs::write(&file, "").unwrap();
    vec![("GIT_CONFIG_GLOBAL".into(), file.display().to_string()), ("GIT_CONFIG_NOSYSTEM".into(), "1".into())]
}

/// Commits `file` (containing `msg`) in `dir`, signed with a new SSH key made next to `dir`.
/// Returns the commit and the public key file, or None (after saying so) when ssh-keygen is
/// missing or signing fails (e.g. key-permission complaints on Windows), so the caller skips.
pub(crate) fn ssh_signed_commit(dir: &Path, file: &str, msg: &str) -> Option<(git2::Oid, PathBuf)> {
    let skip = |why: &str| {
        eprintln!("ssh-keygen unavailable or signing failed, skipping: {why}");
        None
    };
    let key = dir.with_file_name(format!("{}-key", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_file(&key);
    let _ = std::fs::remove_file(key.with_extension("pub"));
    let keygen = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test@example.com", "-f"])
        .arg(&key)
        .stdin(std::process::Stdio::null())
        .output();
    match keygen {
        Ok(o) if o.status.success() => {}
        Ok(o) => return skip(&String::from_utf8_lossy(&o.stderr)),
        Err(e) => return skip(&e.to_string()),
    }
    let public = PathBuf::from(format!("{}.pub", key.display()));
    write(dir, file, msg);
    git(dir, &["add", file]);
    let env = empty_global_config(dir);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let signing_key = format!("user.signingkey={}", public.display());
    let a = ["-c", "gpg.format=ssh", "-c", &signing_key, "commit", "-q", "-S", "-m", msg];
    let (ok, out) = crate::gitcli::run_with_input(dir, &args(&a), "", &env);
    if !ok {
        return skip(&out);
    }
    let oid = git2::Repository::open(dir).ok()?.head().ok()?.target()?;
    Some((oid, public))
}
