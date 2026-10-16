//! Re-reading the unstaged rows under a few work-tree paths, for a file watcher that saw only
//! those change. The staged list (HEAD against the index) can't change without the index
//! changing, and an index change always reads the whole status, so only the work tree is read.
//!
//! Paths that can't be read on their own give `Ok(None)`, and the caller reads the whole status:
//! none or more than `watch::MAX_WATCH_PATHS`; one that is empty, absolute or has empty, `.` or
//! `..` parts; an ignore or attributes file (or the root's `.gitmodules`), which changes how
//! other paths read; in git's mode, a command line over `CLI_PATH_BYTES`, or git failing.

use super::status_merge::covers;
use super::{FileEntry, collide, rpath, status, status_cli};
use crate::error::Result;
use crate::watch::{MAX_WATCH_PATHS, prefixes};
use git2::{Index, Repository, StatusOptions, StatusShow};
use std::path::Path;

/// The unstaged rows under some work-tree paths, read again (see `merge_unstaged`).
#[derive(Clone, Debug, PartialEq)]
pub struct PathsStatus {
    /// The paths actually read: requested ones NFC-normalized (core.precomposeunicode), widened to a
    /// submodule's or nested repository's root, de-duplicated, with any path below another dropped; sorted.
    pub covered: Vec<String>,
    /// Unstaged rows under `covered`, sorted by path, twins marked, skip-worktree deletions and
    /// conflicted paths left out (conflicts change only through the index).
    pub unstaged: Vec<FileEntry>,
    pub state: git2::RepositoryState,
    /// A covered path is a submodule (its row may change; read the submodule list again).
    pub submodule_touched: bool,
}

/// The most bytes of paths one `git status` command line carries: well under Windows' limit
/// of 32K characters for a whole command line.
pub const CLI_PATH_BYTES: usize = 16 * 1024;

/// None: these paths can't be read on their own (see the module doc); read the whole status.
pub fn status_of_paths(workdir: &Path, paths: &[String], git_status: bool) -> Result<Option<PathsStatus>> {
    if paths.is_empty() || paths.len() > MAX_WATCH_PATHS || !paths.iter().all(|p| readable(p)) {
        return Ok(None);
    }
    let repo = Repository::open(workdir)?;
    let index = repo.index()?;
    // Git composes the paths it is given on such a system; so does this read.
    let precompose = repo.config().and_then(|c| c.get_bool("core.precomposeunicode")).unwrap_or(false);
    let mut submodule_touched = false;
    let mut covered: Vec<String> = paths
        .iter()
        .map(|p| {
            let p = if precompose { collide::nfc(p) } else { p.clone() };
            let (root, gitlink) = widen(&index, workdir, p);
            submodule_touched |= gitlink;
            root
        })
        .collect();
    // Index paths one file holds here (twins) are read together: a change to one is a change to all.
    let twins = collide::collisions(&index);
    let more: Vec<String> =
        twins.iter().filter(|c| c.paths.iter().any(|p| covers(&covered, p, false))).flat_map(|c| c.paths.iter().cloned()).collect();
    covered.extend(more);
    normalize(&mut covered);
    let mut rows = if git_status && status::wants_git(&repo) {
        match status_cli::status_of(workdir, &covered) {
            Some((unstaged, _staged)) => unstaged,
            None => return Ok(None),
        }
    } else {
        libgit2_rows(&repo, &covered)?
    };
    // Conflicted paths are listed once, from the index, by the whole read.
    rows.retain(|r| r.status != '!' && !(1..=3).any(|stage| index.get_path(Path::new(&r.path), stage).is_some()));
    // Marked as a whole read marks them (`status::mark_twins`), from the index scanned above.
    collide::mark(&mut rows, &twins, collide::folding(repo.path()), collide::ignorecase(&repo));
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Some(PathsStatus { covered, unstaged: rows, state: repo.state(), submodule_touched }))
}

/// Whether `p` is a plain relative, '/'-separated work-tree path that can be read on its own.
fn readable(p: &str) -> bool {
    let b = p.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    if p.is_empty() || p.starts_with('/') || drive || Path::new(p).is_absolute() {
        return false;
    }
    let parts: Vec<&str> = p.split('/').collect();
    if parts.iter().any(|c| c.is_empty() || *c == "." || *c == "..") {
        return false;
    }
    !matches!(parts.last(), Some(&(".gitignore" | ".gitattributes"))) && parts != [".gitmodules"]
}

/// `p`, or the root of the submodule (a gitlink in the index: true) or nested repository (a
/// `.git` on disk) it is inside, so that repository's row is read the way a whole read lists it.
fn widen(index: &Index, workdir: &Path, p: String) -> (String, bool) {
    for prefix in prefixes(&p).chain(std::iter::once(p.as_str())) {
        if index.get_path(Path::new(prefix), 0).is_some_and(|e| e.mode == 0o160000) {
            return (prefix.to_owned(), true);
        }
        if prefix != p && workdir.join(prefix).join(".git").symlink_metadata().is_ok() {
            return (prefix.to_owned(), false);
        }
    }
    (p, false)
}

/// Sorts and de-duplicates `paths`, dropping each one below another.
fn normalize(paths: &mut Vec<String>) {
    paths.sort();
    paths.dedup();
    let all = paths.clone();
    paths.retain(|p| !prefixes(p).any(|a| all.binary_search_by(|x| x.as_str().cmp(a)).is_ok()));
}

/// The unstaged rows under `covered`, read by libgit2 alone (as `status::libgit2_status` does).
/// The paths go to libgit2's iterators as a literal path list: no globbing, a folder covers
/// everything below it, and other folders aren't walked.
fn libgit2_rows(repo: &Repository, covered: &[String]) -> Result<Vec<FileEntry>> {
    let mut opts = StatusOptions::new();
    opts.show(StatusShow::Workdir).include_untracked(true).recurse_untracked_dirs(true).include_ignored(false).disable_pathspec_match(true);
    for p in covered {
        opts.pathspec(p);
    }
    // A .gitmodules that can't be parsed fails the status of every gitlink, as in a whole read.
    let statuses = match repo.statuses(Some(&mut opts)) {
        Ok(s) => s,
        Err(_) => repo.statuses(Some(opts.exclude_submodules(true)))?,
    };
    let mut rows = Vec::new();
    for e in statuses.iter() {
        let s = e.status();
        let bytes = e.index_to_workdir().and_then(|d| d.new_file().path_bytes()).unwrap_or_else(|| e.path_bytes());
        if s.is_conflicted() {
            rows.push(rpath::entry(bytes, '!'));
        } else if let Some(c) = status::worktree_char(s) {
            rows.push(rpath::entry(bytes, c));
        }
    }
    status::drop_skipped(repo, &mut rows)?;
    Ok(rows)
}

#[cfg(test)]
#[path = "status_paths_tests.rs"]
mod tests;
