//! Saving uncommitted changes as a patch: `git diff` of the chosen scope (every change against
//! HEAD, the staged ones, or the unstaged ones), optionally limited to some paths, plus one
//! `git diff --no-index` per untracked file to include, joined into one file `git apply` reads.

use super::EXPORT_TIMEOUT;
use crate::error::{Error, Result};
use crate::gitcli::{self, args};
use std::path::Path;

/// Which uncommitted changes go in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WipScope {
    /// Staged and unstaged changes, against HEAD.
    #[default]
    All,
    /// What the next commit would contain.
    Staged,
    /// Working-tree edits not yet staged.
    Unstaged,
}

/// A patch of uncommitted changes to save.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangesRequest {
    pub scope: WipScope,
    /// The tracked paths to include. Unless `picked`, empty means every tracked change.
    pub paths: Vec<String>,
    /// Whether `paths` is the exact list (files ticked one by one): then empty means none.
    pub picked: bool,
    /// Untracked files to include, each as a new file.
    pub untracked: Vec<String>,
    /// HEAD has no commit yet: every change is against the empty tree.
    pub unborn: bool,
}

/// The id git gives the empty tree.
pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
/// The most untracked files included one by one.
pub const MAX_UNTRACKED: usize = 1000;
/// Why a patch of no changes isn't saved.
pub const NO_CHANGES: &str = "There are no changes to save.";
/// How much of a command line the paths of one `git diff` may take (Windows' limit is 32767
/// characters).
const PATH_BUDGET: usize = 24_000;

/// The `git diff` of `scope`, limited to `paths` (taken literally) when there are any.
/// Binary changes are included; external diff programs, text conversion and colour are off.
pub fn diff_cmd(scope: WipScope, paths: &[String], unborn: bool) -> Vec<String> {
    let mut cmd = args(&[
        "--literal-pathspecs",
        "diff",
        "--binary",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--submodule=short",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ]);
    match scope {
        WipScope::Staged => cmd.push("--cached".into()),
        WipScope::All => cmd.push(if unborn { EMPTY_TREE } else { "HEAD" }.into()),
        WipScope::Unstaged => {}
    }
    if !paths.is_empty() {
        cmd.push("--".into());
        cmd.extend(paths.iter().cloned());
    }
    cmd
}

/// The diff that creates the untracked file `path`. Git reads "/dev/null" as no file on every
/// system.
pub fn untracked_cmd(path: &str) -> Vec<String> {
    args(&[
        "diff",
        "--no-index",
        "--binary",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        "--",
        "/dev/null",
        path,
    ])
}

/// `paths` in runs whose lengths together stay within `budget`, in order; a longer path is a
/// run of its own.
pub(super) fn chunks(paths: &[String], budget: usize) -> Vec<&[String]> {
    let mut out = Vec::new();
    let (mut start, mut used) = (0, 0);
    for (i, p) in paths.iter().enumerate() {
        let len = p.len() + 1;
        if i > start && used + len > budget {
            out.push(&paths[start..i]);
            (start, used) = (i, 0);
        }
        used += len;
    }
    if start < paths.len() {
        out.push(&paths[start..]);
    }
    out
}

/// Whether the untracked row `path` is a directory: status lists a nested repository as one
/// row ending in "/". A patch can't create one, and `git diff --no-index` fails on it, so it
/// is left out.
fn is_directory(workdir: &Path, path: &str) -> bool {
    path.ends_with('/') || std::fs::symlink_metadata(workdir.join(path)).is_ok_and(|m| m.is_dir())
}

/// The patch `req` asks for, and how many files it changes.
pub(crate) fn build_changes(workdir: &Path, req: &ChangesRequest) -> Result<(Vec<u8>, usize)> {
    if req.untracked.len() > MAX_UNTRACKED {
        return Err(Error::Other(format!(
            "Too many untracked files to include ({}); tick the ones you want under Only selected files.",
            req.untracked.len()
        )));
    }
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let runs: Vec<&[String]> =
        if req.paths.is_empty() { if req.picked { Vec::new() } else { vec![&[]] } } else { chunks(&req.paths, PATH_BUDGET) };
    for run in runs {
        let (out, bytes) = gitcli::output_bytes(workdir, &diff_cmd(req.scope, run, req.unborn), gitcli::NO_PROMPTS, Some(EXPORT_TIMEOUT));
        if !out.ok {
            return Err(Error::Other(format!("git diff failed: {}", out.stderr.trim())));
        }
        parts.push(bytes);
    }
    for path in req.untracked.iter().filter(|p| !is_directory(workdir, p)) {
        let (out, bytes) = gitcli::output_bytes(workdir, &untracked_cmd(path), gitcli::NO_PROMPTS, Some(EXPORT_TIMEOUT));
        // Exit code 1 means "they differ", which a new file always does.
        let failed = out.stderr.lines().any(|l| l.starts_with("error:") || l.starts_with("fatal:"));
        if !matches!(out.code, Some(0 | 1)) || failed {
            return Err(Error::Other(format!("git diff failed for {path}: {}", out.stderr.trim())));
        }
        parts.push(bytes);
    }
    let mut joined = Vec::new();
    for part in parts.into_iter().filter(|p| !p.is_empty()) {
        if joined.last().is_some_and(|b| *b != b'\n') {
            joined.push(b'\n');
        }
        joined.extend_from_slice(&part);
    }
    let files = joined.split(|b| *b == b'\n').filter(|l| l.starts_with(b"diff --git ")).count();
    if files == 0 {
        return Err(Error::Other(NO_CHANGES.to_owned()));
    }
    Ok((joined, files))
}

#[cfg(test)]
#[path = "wip_tests.rs"]
mod tests;
