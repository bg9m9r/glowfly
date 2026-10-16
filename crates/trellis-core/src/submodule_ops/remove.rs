//! Removing a submodule: what the confirmation names (its changes, the commits on no
//! remote-tracking branch inside it, its stashes, its stored module), and the removal itself:
//! `git submodule deinit -f`, `git rm -f`, then deleting `<git dir>/modules/<name>`, never
//! through a link and only when it looks like a git dir. The git dir is the repository's own:
//! in a linked worktree git keeps a separate store per worktree
//! (`.git/worktrees/<id>/modules`), not one in the common dir.

use super::{deinit_cmd, module_dir, rm_cmd};
use crate::error::Result;
use crate::gitcli;
use crate::native_path::{native, shown};
use crate::repo::{LEFT_BEHIND_MAX, SubmoduleInfo};
use git2::{Oid, Repository, Sort};
use std::path::{Path, PathBuf};

/// What removing a submodule would delete, read just before asking.
#[derive(Clone, Debug, PartialEq)]
pub struct RemovalFacts {
    pub info: SubmoduleInfo,
    /// Commits (id, summary), newest first and at most `LEFT_BEHIND_MAX`, on a branch or the
    /// HEAD inside the submodule but on no remote-tracking branch: deleting the stored module
    /// loses them.
    pub unpushed: Vec<(Oid, String)>,
    /// Stash entries inside the submodule.
    pub stashes: usize,
    /// `<git dir>/modules/<name>`, when it exists.
    pub module_dir: Option<PathBuf>,
}

/// The facts about the submodule at `path` in the repository at `workdir`; None when it is no
/// longer there.
pub fn removal_facts(workdir: &Path, path: &str) -> Result<Option<RemovalFacts>> {
    let Some(info) = crate::repo::submodules_at(workdir)?.into_iter().find(|m| m.path == path) else { return Ok(None) };
    let git_dir = native(Repository::open(workdir)?.path());
    let module = module_dir(&git_dir, &info.name).ok().filter(|d| d.symlink_metadata().is_ok());
    // One component at a time, so the folder uses the platform's separators.
    let folder = path.split('/').fold(workdir.to_path_buf(), |p, c| p.join(c));
    // An uninitialized submodule can still hold commits of its own in the stored module.
    let repo = Repository::open(&folder).ok().or_else(|| module.as_ref().and_then(|d| Repository::open(d).ok()));
    let unpushed = repo.as_ref().and_then(|r| unpushed(r).ok()).unwrap_or_default();
    let stashes = repo.as_ref().map(|r| r.reflog("refs/stash").map(|l| l.len()).unwrap_or(0)).unwrap_or(0);
    Ok(Some(RemovalFacts { info, unpushed, stashes, module_dir: module }))
}

/// Commits reachable from HEAD or a local branch but from no remote-tracking branch.
fn unpushed(repo: &Repository) -> std::result::Result<Vec<(Oid, String)>, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    if let Some(head) = repo.head().ok().and_then(|h| h.target()) {
        walk.push(head)?;
    }
    // Globs skip refs that don't point at a commit.
    walk.push_glob("refs/heads")?;
    walk.hide_glob("refs/remotes")?;
    walk.take(LEFT_BEHIND_MAX)
        .map(|id| {
            let id = id?;
            let summary = repo.find_commit(id)?.summary().unwrap_or_default().to_owned();
            Ok((id, summary))
        })
        .collect()
}

/// How a removal ended.
#[derive(Clone, Debug, PartialEq)]
pub enum RemoveOutcome {
    /// Deinitialized, out of the index and .gitmodules, and its stored module deleted.
    Removed,
    /// `git submodule deinit` failed; nothing changed.
    DeinitFailed,
    /// Deinitialized, but `git rm` failed: it is still in .gitmodules and the index.
    RmFailed,
    /// The git part worked, but the stored module at `dir` couldn't be deleted.
    ModuleLeft { dir: PathBuf, error: String },
}

/// The outcome of `remove` and its output: "$ git …" lines and git's text.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveReport {
    pub outcome: RemoveOutcome,
    pub output: String,
}

/// Removes the submodule called `name` at `path` from the repository at `workdir`: deinit,
/// `git rm`, then its stored module under the repository's git dir. Each step runs only when
/// the one before it worked.
pub fn remove(workdir: &Path, path: &str, name: &str) -> RemoveReport {
    let git_dir = match Repository::open(workdir) {
        Ok(r) => native(r.path()),
        Err(e) => return RemoveReport { outcome: RemoveOutcome::DeinitFailed, output: e.to_string() },
    };
    let mut output = String::new();
    let report = |outcome, output: String| RemoveReport { outcome, output: output.trim_end().to_owned() };
    if !run(workdir, &deinit_cmd(path), &mut output) {
        return report(RemoveOutcome::DeinitFailed, output);
    }
    if !run(workdir, &rm_cmd(path), &mut output) {
        return report(RemoveOutcome::RmFailed, output);
    }
    match delete_module(&git_dir, name) {
        Ok(Some(dir)) => {
            output.push_str(&format!("Deleted the stored module {}\n", shown(&dir)));
            report(RemoveOutcome::Removed, output)
        }
        Ok(None) => report(RemoveOutcome::Removed, output),
        Err((dir, error)) => report(RemoveOutcome::ModuleLeft { dir, error }, output),
    }
}

/// Runs `git <cmd>` in `cwd`, echoing it and its output into `output` as git jobs do.
fn run(cwd: &Path, cmd: &[String], output: &mut String) -> bool {
    output.push_str(&format!("$ git {}\n", cmd.join(" ")));
    let (ok, text) = gitcli::run_with_input(cwd, cmd, "", &[]);
    if !text.is_empty() {
        output.push_str(&text);
        output.push('\n');
    }
    ok
}

/// Deletes `<git_dir>/modules/<name>` and the folders above it that it leaves empty (never
/// `modules` itself). Ok(None) when there is nothing to delete; Err names the folder and why it
/// stays: an invalid name, a link on the way, a folder that isn't a git dir, or an I/O error.
fn delete_module(git_dir: &Path, name: &str) -> std::result::Result<Option<PathBuf>, (PathBuf, String)> {
    let modules = git_dir.join("modules");
    let dir = module_dir(git_dir, name).map_err(|e| (modules.clone(), e))?;
    if dir.symlink_metadata().is_err() {
        return Ok(None);
    }
    let parts: Vec<&str> = name.split('/').collect();
    let mut at = modules.clone();
    for i in 0..=parts.len() {
        if i > 0 {
            at.push(parts[i - 1]);
        }
        match at.symlink_metadata() {
            Ok(m) if m.file_type().is_symlink() => {
                return Err((dir, format!("{} is a link; Trellis doesn't delete through links", shown(&at))));
            }
            Ok(m) if !m.is_dir() => return Err((dir, format!("{} is not a folder", shown(&at)))),
            Ok(_) => {}
            Err(e) => return Err((dir, e.to_string())),
        }
    }
    if !dir.join("HEAD").symlink_metadata().is_ok_and(|m| m.is_file()) {
        return Err((dir, "it doesn't look like a git repository (it has no HEAD file)".to_owned()));
    }
    crate::persist::remove_tree(&dir).map_err(|e| (dir.clone(), e.to_string()))?;
    for depth in (1..parts.len()).rev() {
        let parent = parts[..depth].iter().fold(modules.clone(), |p, c| p.join(c));
        if std::fs::remove_dir(parent).is_err() {
            break;
        }
    }
    Ok(Some(dir))
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;
