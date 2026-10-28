//! Hooks that follow the working tree. When `core.hooksPath` points into the working tree
//! (`.githooks`, `.husky/_`), or a hook is a link to a file there, the hooks change with the
//! branch: a checkout, merge, pull or rebase runs the incoming commit's `post-checkout`,
//! `post-merge` or `post-rewrite` inside that same git command, before any rescan. The scan
//! records each such place with the version of it HEAD tracks (`Follow`), so the trust print
//! covers that version, and `incoming` reads another commit's version from its tree (objects
//! only: no checkout, filter, hook or fsmonitor runs) so a job that brings it in can be held
//! until the user trusts it.

use super::{TrustScan, is_hook_name};
use git2::{ObjectType, Oid, Repository, Tree};
use std::path::{Component, Path, PathBuf};

/// A place inside the working tree that hooks run from.
#[derive(Clone, Debug, PartialEq)]
pub struct Follow {
    /// Relative to the working tree, `/`-separated: the hooks folder, or a hook link's target.
    pub path: String,
    /// The nearest folder or file at or above `path` (never the whole tree, unless `path` is
    /// the top itself) that the commit tracks, with its object id; None when it tracks none.
    /// It covers what the hooks there run, tracked scripts beside them included (husky's
    /// `.husky/pre-commit` next to its `.husky/_` hooks folder).
    pub tracked: Option<(String, Oid)>,
    /// `path` is the hooks folder (not a link's target): an incoming commit's hooks are read
    /// from it.
    pub hooks: bool,
}

/// The places inside `repo`'s working tree that its hooks run from: `hooks_dir` when it is
/// there, and the target of each of `hooks` (paths in that folder) that is a link into it,
/// each with the version HEAD tracks. Empty for a bare repository or hooks in the git dir.
pub(super) fn find(repo: &Repository, hooks_dir: Option<&Path>, hooks: &[PathBuf]) -> Vec<Follow> {
    let Some(workdir) = repo.workdir() else { return Vec::new() };
    let mut out: Vec<Follow> = Vec::new();
    let mut add = |path: String, hooks: bool| {
        let covered = out.iter().any(|f| f.hooks && within(&path, &f.path));
        if !covered && !out.iter().any(|f| f.path == path) {
            out.push(Follow { path, tracked: None, hooks });
        }
    };
    if let Some(rel) = hooks_dir.and_then(|d| inside(repo, workdir, d)) {
        add(rel, true);
    }
    for hook in hooks {
        let is_link = std::fs::symlink_metadata(hook).is_ok_and(|m| m.file_type().is_symlink());
        if let Some(rel) = is_link.then(|| crate::paths::resolve(hook)).and_then(|t| inside(repo, workdir, &t)) {
            add(rel, false);
        }
    }
    // HEAD's tree is read only when something follows the working tree.
    if let Some(head) = (!out.is_empty()).then(|| repo.head().ok().and_then(|h| h.peel_to_tree().ok())).flatten() {
        for f in &mut out {
            f.tracked = tracked_at(repo, &head, &f.path).ok().flatten();
        }
    }
    out
}

/// Whether the `/`-separated `path` is `root` or inside it.
fn within(path: &str, root: &str) -> bool {
    root.is_empty() || path == root || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}

/// `path` relative to `workdir`, `/`-separated, when it is inside the working tree and not in
/// the git dir. Tried as spelled (a relative `core.hooksPath` is joined to the working tree),
/// then with both resolved on disk (an absolute one, or another spelling of the same folder).
fn inside(repo: &Repository, workdir: &Path, path: &Path) -> Option<String> {
    let git_dirs = [repo.path(), repo.commondir()];
    let in_git_dir = |p: &Path| git_dirs.iter().filter_map(|g| lexical(g)).any(|g| p.starts_with(g));
    let as_spelled = lexical(path).filter(|p| !in_git_dir(p)).and_then(|p| rel(&p, &lexical(workdir)?));
    let rel = as_spelled.or_else(|| {
        let p = crate::paths::resolve(path);
        let resolved_git = git_dirs.iter().any(|g| p.starts_with(crate::paths::resolve(g)));
        if resolved_git { None } else { rel(&p, &crate::paths::resolve(workdir)) }
    })?;
    // `.git` in any case: a folding file system finds the git dir under it.
    (!rel.split('/').next().is_some_and(|first| first.eq_ignore_ascii_case(".git"))).then_some(rel)
}

/// `path` with `.` parts dropped and `..` parts applied; None when `..` climbs past its start.
fn lexical(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

fn rel(path: &Path, root: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    Some(rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
}

/// The nearest entry at or above `path` in `tree` (see [`Follow::tracked`]). A part is looked
/// up exactly, else under another letter case when exactly one entry matches that way, as a
/// folding file system would find it.
fn tracked_at(repo: &Repository, tree: &Tree, path: &str) -> Result<Option<(String, Oid)>, git2::Error> {
    if path.is_empty() {
        return Ok(Some((String::new(), tree.id())));
    }
    let mut found = None;
    let mut here = tree.clone();
    let mut walked: Vec<String> = Vec::new();
    for part in path.split('/') {
        let lower = part.to_lowercase();
        let entry = match here.get_name(part) {
            Some(e) => Some(e.to_owned()),
            None => {
                let mut folded = here.iter().filter(|e| e.name().is_some_and(|n| n.to_lowercase() == lower));
                match (folded.next(), folded.next()) {
                    (Some(e), None) => Some(e.to_owned()),
                    _ => None,
                }
            }
        };
        let Some(entry) = entry else { break };
        walked.push(entry.name().unwrap_or(part).to_owned());
        found = Some((walked.join("/"), entry.id()));
        if entry.kind() != Some(ObjectType::Tree) {
            break;
        }
        here = repo.find_tree(entry.id())?;
    }
    Ok(found)
}

/// What the commit `oid` has in the places `follows` names: each with that commit's version,
/// and the hooks in its version of the hooks folder (read from the objects, with their
/// contents' digests: what they would be once checked out, line-ending changes aside). Reads
/// objects only. An error is an object that can't be read: the caller holds the job.
pub fn incoming(repo: &Repository, follows: &[Follow], oid: Oid) -> Result<TrustScan, git2::Error> {
    let tree = repo.find_commit(oid)?.tree()?;
    let mut out = TrustScan::default();
    for f in follows {
        let tracked = tracked_at(repo, &tree, &f.path)?;
        if f.hooks
            && let Some((path, id)) = &tracked
            && path.eq_ignore_ascii_case(&f.path)
            && let Ok(dir) = repo.find_tree(*id)
        {
            for e in dir.iter().filter(|e| e.kind() == Some(ObjectType::Blob)) {
                let Some(name) = e.name().filter(|n| is_hook_name(n)) else { continue };
                let blob = repo.find_blob(e.id())?;
                out.hook_digests.insert(name.to_owned(), super::content_digest(blob.content()));
                out.hooks.push(name.to_owned());
            }
            out.hooks_dir = repo.workdir().map(|w| w.join(&f.path));
        }
        out.follows.push(Follow { path: f.path.clone(), tracked, hooks: f.hooks });
    }
    out.hooks.sort();
    Ok(out)
}

/// Whether the print `p` was made on every version `incoming` found (see
/// [`super::print`]): the job that brings that commit in runs nothing the user didn't trust.
pub fn covered(p: &super::TrustPrint, incoming: &TrustScan) -> bool {
    incoming.follows.iter().all(|f| p.0.contains(&super::print::hash(&super::print::follow_item("", f))))
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
