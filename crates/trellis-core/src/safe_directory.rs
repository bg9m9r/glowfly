//! Repositories git refuses because another user owns them: a folder on a Windows share or
//! under `\\wsl$`, a FAT/exFAT drive, another account's folder, a Docker volume or an NFS mount
//! with another uid. libgit2 and git both refuse to open these (CVE-2022-24765) until the folder
//! is listed in the global `safe.directory` setting. Trellis says so, instead of "not a
//! repository", and can add the folder there in git's own spelling.

use crate::error::{Error, Result};
use crate::gitcli;
use std::path::{Path, PathBuf};

/// The error for a failed `Repository::discover`/`open` of `path`: [`Error::NotOwned`] with the
/// repository's folder when git refused it for its owner, else [`Error::NotARepository`].
pub fn open_error(path: &Path, e: &git2::Error) -> Error {
    if e.code() == git2::ErrorCode::Owner { Error::NotOwned(repo_root(path)) } else { Error::NotARepository(path.display().to_string()) }
}

/// The repository folder at or above `path`: the nearest one holding `.git` (a folder, or a
/// file for a linked worktree or submodule), resolved on disk as git compares it. `path` itself
/// when there is none.
pub fn repo_root(path: &Path) -> PathBuf {
    let root = path.ancestors().find(|a| a.join(".git").symlink_metadata().is_ok()).unwrap_or(path);
    crate::paths::resolve(root)
}

/// How `safe.directory` spells `path`: forward slashes on Windows, no trailing slash, and a share
/// (`\\server\share\repo`) as `%(prefix)///server/share/repo`, since git would otherwise read
/// the leading `//` as one `/`.
pub fn config_value(path: &Path, windows: bool) -> String {
    let text = crate::native_path::shown(path);
    if !windows {
        return text;
    }
    let slashed = text.replace('\\', "/");
    let trimmed = if slashed.len() > 3 { slashed.trim_end_matches('/').to_owned() } else { slashed };
    if trimmed.starts_with("//") { format!("%(prefix)/{trimmed}") } else { trimmed }
}

/// Adds `path` to the global `safe.directory` list, unless it is already there.
pub fn add(path: &Path) -> Result<()> {
    let value = config_value(path, cfg!(windows));
    let cwd = crate::paths::temp_dir();
    let listed = gitcli::read(&cwd, &gitcli::args(&["config", "--global", "--get-all", "safe.directory"]));
    if listed.stdout.lines().any(|l| l.trim() == value) {
        return Ok(());
    }
    let (ok, out) = gitcli::run(&cwd, &gitcli::args(&["config", "--global", "--add", "safe.directory", &value]));
    if ok { Ok(()) } else { Err(Error::Other(format!("git config --global --add safe.directory {value} failed: {out}"))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_owner_refusal_is_not_called_not_a_repository() {
        let dir = crate::testutil::scratch_repo("owner-error");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let refused = git2::Error::new(git2::ErrorCode::Owner, git2::ErrorClass::Config, "repository path is not owned by current user");
        match open_error(&dir.join("src"), &refused) {
            Error::NotOwned(root) => assert_eq!(root, crate::paths::resolve(&dir)),
            other => panic!("expected NotOwned, got {other:?}"),
        }
        let other = git2::Error::new(git2::ErrorCode::NotFound, git2::ErrorClass::Repository, "could not find repository");
        assert!(matches!(open_error(&dir, &other), Error::NotARepository(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_directory_uses_gits_spelling() {
        if cfg!(windows) {
            assert_eq!(config_value(Path::new(r"C:\work\app"), true), "C:/work/app");
            assert_eq!(config_value(Path::new("C:/work/app/"), true), "C:/work/app");
            assert_eq!(config_value(Path::new(r"\\server\share\repo"), true), "%(prefix)///server/share/repo");
            assert_eq!(
                config_value(Path::new(r"\\wsl.localhost\Ubuntu\home\me\repo"), true),
                "%(prefix)///wsl.localhost/Ubuntu/home/me/repo"
            );
            assert_eq!(config_value(Path::new(r"\\?\UNC\server\share\repo"), true), "%(prefix)///server/share/repo");
        } else {
            assert_eq!(config_value(Path::new("/srv/repos/app/"), false), "/srv/repos/app");
            assert_eq!(config_value(Path::new(r"/srv/a\b"), false), r"/srv/a\b", "a backslash is part of a Unix name");
        }
    }

    /// On a Linux CI runner (passwordless sudo): a repository owned by root is refused as such.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_repository_owned_by_root_is_refused_for_its_owner() {
        let dir = crate::testutil::scratch_repo("owner-root");
        let sudo = |a: &[&str]| std::process::Command::new("sudo").arg("-n").args(a).status().is_ok_and(|s| s.success());
        let all_safe =
            gitcli::read(&dir, &gitcli::args(&["config", "--get-all", "safe.directory"])).stdout.lines().any(|l| l.trim() == "*");
        if all_safe || !sudo(&["chown", "-R", "root:root", &dir.display().to_string()]) {
            eprintln!("skipped: needs passwordless sudo and no safe.directory=*");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let e = crate::repo::load(&dir, &crate::repo::LoadOptions::default()).err().expect("refused");
        let _ = sudo(&["rm", "-rf", &dir.display().to_string()]);
        assert!(matches!(e, Error::NotOwned(ref root) if *root == dir), "{e:?}");
    }
}
