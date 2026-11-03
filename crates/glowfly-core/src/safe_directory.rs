//! Repositories git refuses because another user owns them: a folder on a Windows share or
//! under `\\wsl$`, a FAT/exFAT drive, another account's folder, a Docker volume or an NFS mount
//! with another uid. libgit2 and git both refuse to open these (CVE-2022-24765) until the folder
//! is listed in the global `safe.directory` setting. libgit2's owner check is stricter than
//! git's in places (an Administrators-owned folder on Windows), so a refusal is checked with git
//! (`verdict`): a folder git opens is opened (`allowed`, for this process only), and only a real
//! refusal comes back as [`Error::NotOwned`], never "not a repository". Glowfly can then add
//! the folder to `safe.directory` in git's own spelling.

use crate::error::{Error, Result};
use crate::gitcli;
use git2::Repository;
use std::path::{Path, PathBuf};

pub mod allowed;
pub mod verdict;

/// A repository git refuses because another user owns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The repository's folder (`repo_root`).
    pub root: PathBuf,
    /// Its owner as git names it, when it does (Git for Windows).
    pub owner: Option<String>,
    /// The owner is the Administrators group (a folder made from a program run as administrator).
    pub admins: bool,
}

/// `Repository::discover(path)`, opening a folder libgit2 refuses for its owner when git opens
/// it; a real refusal is [`Error::NotOwned`], anything else [`Error::NotARepository`].
pub fn discover(path: &Path) -> Result<Repository> {
    opened(path, || Repository::discover(path))
}

/// [`discover`] for `Repository::open(path)` (no search above `path`).
pub fn open(path: &Path) -> Result<Repository> {
    opened(path, || Repository::open(path))
}

fn opened(path: &Path, open: impl Fn() -> std::result::Result<Repository, git2::Error>) -> Result<Repository> {
    match open() {
        Ok(r) => Ok(r),
        Err(e) if e.code() == git2::ErrorCode::Owner => {
            let root = repo_root(path);
            let v = verdict::ask(path, &root, gitcli::NO_PROMPTS);
            if v.opens && allowed::allow(&allow_values(path, &root)) {
                match open() {
                    Ok(r) => {
                        tracing::debug!(root = %root.display(), "git opens a folder libgit2 refused for its owner");
                        // libgit2's own spelling of the folder, for the opens that follow.
                        if let Some(w) = r.workdir() {
                            allowed::allow(&[config_value(w, cfg!(windows))]);
                        }
                        return Ok(r);
                    }
                    Err(e) => tracing::warn!("libgit2 still refuses {}: {e}", root.display()),
                }
            }
            Err(Error::NotOwned(Refusal { root, owner: v.owner, admins: v.admins }))
        }
        Err(_) => Err(Error::NotARepository(path.display().to_string())),
    }
}

/// The `safe.directory` values that let libgit2 open the repository at `root`, found from
/// `path`: its folder as spelled (made absolute, as libgit2 compares it) and as resolved.
fn allow_values(path: &Path, root: &Path) -> Vec<String> {
    let spelled = std::path::absolute(path)
        .ok()
        .and_then(|p| p.ancestors().find(|a| a.join(".git").symlink_metadata().is_ok()).map(Path::to_path_buf));
    let mut values: Vec<String> = spelled.iter().map(|p| config_value(p, cfg!(windows))).collect();
    let resolved = config_value(root, cfg!(windows));
    if !values.contains(&resolved) {
        values.push(resolved);
    }
    values
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
    fn a_folder_that_is_no_repository_is_called_so() {
        let dir = crate::paths::temp_dir().join(format!("glowfly-owner-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(matches!(open(&dir), Err(Error::NotARepository(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_values_allowed_name_the_folder_both_ways() {
        let dir = crate::testutil::scratch_repo("owner-values");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let root = repo_root(&dir.join("src"));
        let values = allow_values(&dir.join("src"), &root);
        assert!(values.contains(&config_value(&dir, cfg!(windows))), "{values:?}");
        assert!(values.contains(&config_value(&root, cfg!(windows))), "{values:?}");
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
        assert!(matches!(e, Error::NotOwned(ref r) if r.root == dir), "{e:?}");
    }
}
