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
    crate::partial::allow_extension();
    let dir = crate::paths::temp_dir().join(format!("trellis-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-b", "main"]);
    set_config(
        &dir,
        &[
            ("user.name", "Test"),
            ("user.email", "test@example.com"),
            ("core.autocrlf", "false"),
            ("commit.gpgsign", "false"),
            // Otherwise every commit starts a `git maintenance run --auto` process.
            ("maintenance.auto", "false"),
        ],
    );
    dir
}

/// Writes `entries` to the repository's own config file, as `git config <key> <value>` would,
/// without starting a git process for each (a few hundred ms apiece on a busy Windows machine).
pub(crate) fn set_config(dir: &Path, entries: &[(&str, &str)]) {
    let repo = git2::Repository::open(dir).unwrap();
    let mut config = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap();
    for (key, value) in entries {
        config.set_str(key, value).unwrap();
    }
}

/// A second name for the folder `dir`, made next to it (`<name>-alias`): a symlink, or on Windows
/// a junction where symlinks need rights the account lacks. None when neither can be made.
pub(crate) fn alias_of(dir: &Path) -> Option<PathBuf> {
    let link = dir.with_file_name(format!("{}-alias", dir.file_name()?.to_string_lossy()));
    let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(dir, &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(dir, &link).is_ok()
        || std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(&link).arg(dir).output().is_ok_and(|o| o.status.success());
    made.then_some(link)
}

pub(crate) fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

/// `file:///srv/x` on Unix, `file:///C:/x` on Windows.
pub(crate) fn file_url(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// A blobless partial clone (`--filter=blob:none`, through a file:// URL) of a source with three
/// commits: a.txt "one", then d/b.txt, then a.txt "one\ntwo". HEAD's blobs are checked out;
/// HEAD~2's a.txt (also HEAD~1's) is missing. Returns (source, clone).
pub(crate) fn partial_clone(name: &str) -> (PathBuf, PathBuf) {
    let src = scratch_repo(&format!("{name}-src"));
    set_config(&src, &[("uploadpack.allowFilter", "true"), ("uploadpack.allowAnySHA1InWant", "true")]);
    write(&src, "a.txt", "one\n");
    git(&src, &["add", "."]);
    git(&src, &["commit", "-q", "-m", "first"]);
    std::fs::create_dir_all(src.join("d")).unwrap();
    write(&src, "d/b.txt", "b\n");
    git(&src, &["add", "."]);
    git(&src, &["commit", "-q", "-m", "second"]);
    write(&src, "a.txt", "one\ntwo\n");
    git(&src, &["add", "."]);
    git(&src, &["commit", "-q", "-m", "third"]);
    let clone = src.with_file_name(format!("{}-partial", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&clone);
    git(src.parent().unwrap(), &["clone", "-q", "--filter=blob:none", &file_url(&src), &clone.display().to_string()]);
    set_config(
        &clone,
        &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("maintenance.auto", "false")],
    );
    (src, clone)
}

/// git-lfs's version, or None (with a note) so tests that need it can skip.
pub(crate) fn lfs_or_skip(test: &str) -> Option<String> {
    let v = crate::lfs::installed(&crate::paths::temp_dir());
    if v.is_none() {
        eprintln!("{test}: git-lfs is not installed, skipping");
    }
    v
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
    set_config(&tmp, &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false")]);
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
    set_config(&sub, &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false")]);
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
