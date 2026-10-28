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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
/// each with the version HEAD tracks. A place reached through a link inside the working tree
/// (the hooks folder, or a folder above it, is a tracked link) is recorded both as spelled
/// (the link itself, so another target is another version) and where it leads on disk (what
/// the hooks there are). Empty for a bare repository or hooks in the git dir.
pub(super) fn find(repo: &Repository, hooks_dir: Option<&Path>, hooks: &[PathBuf]) -> Vec<Follow> {
    let Some(workdir) = repo.workdir() else { return Vec::new() };
    let mut out: Vec<Follow> = Vec::new();
    let mut add = |path: String, hooks: bool| {
        let covered = out.iter().any(|f| f.hooks && within(&path, &f.path));
        if !covered && !out.iter().any(|f| f.path == path) {
            out.push(Follow { path, tracked: None, hooks });
        }
    };
    if let Some(dir) = hooks_dir {
        for rel in inside(repo, workdir, dir) {
            add(rel, true);
        }
    }
    for hook in hooks {
        let is_link = std::fs::symlink_metadata(hook).is_ok_and(|m| m.file_type().is_symlink());
        if is_link {
            for rel in inside(repo, workdir, &crate::paths::resolve(hook)) {
                add(rel, false);
            }
        }
    }
    // HEAD's tree is read only when something follows the working tree. One that can't be read
    // gets a version no commit has, so every incoming one is compared.
    if let Some(head) = (!out.is_empty()).then(|| repo.head().ok().and_then(|h| h.peel_to_tree().ok())).flatten() {
        for f in &mut out {
            f.tracked = tracked_at(repo, &head, &f.path).unwrap_or_else(|_| Some((f.path.clone(), Oid::zero())));
        }
    }
    out
}

/// Whether the `/`-separated `path` is `root` or inside it.
pub(super) fn within(path: &str, root: &str) -> bool {
    root.is_empty() || path == root || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}

/// `path` relative to `workdir`, `/`-separated, for each way it is inside the working tree and
/// not in the git dir: as spelled (a relative `core.hooksPath` is joined to the working tree),
/// and resolved on disk when that is another place (a link inside the tree, an absolute path,
/// another spelling of the same folder). Both count: the spelled place has the link git checks
/// out, the resolved one the hooks it leads to.
fn inside(repo: &Repository, workdir: &Path, path: &Path) -> Vec<String> {
    let git_dirs = [repo.path(), repo.commondir()];
    let in_git_dir = |p: &Path| git_dirs.iter().filter_map(|g| lexical(g)).any(|g| p.starts_with(g));
    let as_spelled = lexical(path).filter(|p| !in_git_dir(p)).and_then(|p| rel(&p, &lexical(workdir)?));
    let real = crate::paths::resolve(&workdir.join(path));
    let resolved_git = git_dirs.iter().any(|g| real.starts_with(crate::paths::resolve(g)));
    let resolved = if resolved_git { None } else { rel(&real, &crate::paths::resolve(workdir)) };
    let mut out: Vec<String> = Vec::new();
    for r in [as_spelled, resolved].into_iter().flatten() {
        // `.git` in any spelling a folding file system takes for it finds the git dir under it.
        let git_named = r.split('/').next().is_some_and(|first| fold(first) == ".git");
        if !git_named && !out.contains(&r) {
            out.push(r);
        }
    }
    out
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

/// The name a folding file system sees for one part of a path: letter case and Unicode
/// normalization folded (NTFS, APFS, HFS+), the code points HFS+ ignores dropped, and the
/// trailing dots and spaces Windows drops removed. Folding more than this volume does only
/// makes more entries count as the one on disk: more is compared, never less.
fn fold(part: &str) -> String {
    let kept: String = part.chars().filter(|c| !ignorable(*c)).collect();
    crate::repo::name_key(kept.trim_end_matches(['.', ' ']))
}

/// The code points HFS+ leaves out of a name (the ones git's `is_hfs_dotgit` skips).
fn ignorable(c: char) -> bool {
    matches!(c as u32, 0x200c..=0x200f | 0x202a..=0x202e | 0x206a..=0x206f | 0xfeff)
}

/// The nearest entry at or above `path` in `tree` (see [`Follow::tracked`]). At each level every
/// entry whose name folds to the part's (`fold`) is one a folding file system could check out
/// there (`.GITHOOKS/post-checkout` beside `.githooks/post-checkout`): with one, the walk goes
/// on; with more, they are recorded together (their names, modes and ids hashed into one id)
/// and the walk stops, so any of them changing is another version.
fn tracked_at(repo: &Repository, tree: &Tree, path: &str) -> Result<Option<(String, Oid)>, git2::Error> {
    if path.is_empty() {
        return Ok(Some((String::new(), tree.id())));
    }
    let mut found = None;
    let mut here = tree.clone();
    let mut walked: Vec<String> = Vec::new();
    for part in path.split('/') {
        let want = fold(part);
        let mut matches: Vec<_> = here.iter().filter(|e| e.name().is_some_and(|n| fold(n) == want)).map(|e| e.to_owned()).collect();
        let Some(entry) = matches.first().cloned() else { break };
        if matches.len() > 1 {
            matches.sort_by(|a, b| a.name_bytes().cmp(b.name_bytes()));
            let mut all = Vec::new();
            for e in &matches {
                all.extend_from_slice(e.name_bytes());
                all.push(0);
                all.extend_from_slice(e.id().as_bytes());
                all.extend_from_slice(&e.filemode().to_be_bytes());
            }
            let names: Vec<&str> = matches.iter().filter_map(|e| e.name()).collect();
            walked.push(names.join("|"));
            return Ok(Some((walked.join("/"), Oid::hash_object(ObjectType::Blob, &all)?)));
        }
        walked.push(entry.name().unwrap_or(part).to_owned());
        found = Some((walked.join("/"), entry.id()));
        if entry.kind() != Some(ObjectType::Tree) {
            break;
        }
        here = repo.find_tree(entry.id())?;
    }
    Ok(found)
}

/// The largest hook `incoming` reads to list it with its digest; a bigger one is listed by
/// name only (the comparison uses the folder's id, which covers its contents anyway).
const MAX_HOOK_READ: usize = 1 << 20;

/// What the tree-ish `oid` (a commit or a tree) has in the places `follows` names, each with its
/// version there. Reads trees only. An error is a tree that can't be read (a partial clone
/// that fetched without trees, a damaged repository): nothing can be compared, so the caller
/// holds the job.
pub fn versions(repo: &Repository, follows: &[Follow], oid: Oid) -> Result<Vec<Follow>, git2::Error> {
    let tree = repo.find_object(oid, None)?.peel_to_tree()?;
    follows.iter().map(|f| Ok(Follow { path: f.path.clone(), tracked: tracked_at(repo, &tree, &f.path)?, hooks: f.hooks })).collect()
}

/// `versions`, with the hooks the incoming version of each hooks folder has, for the prompt
/// (read from the objects, with their contents' digests: what they would be once checked out,
/// line-ending changes aside). The listing is best effort: a hook whose object is missing (a
/// partial clone) or larger than `MAX_HOOK_READ` is listed without a digest.
pub fn incoming(repo: &Repository, follows: &[Follow], oid: Oid) -> Result<TrustScan, git2::Error> {
    let mut out = TrustScan { follows: versions(repo, follows, oid)?, ..TrustScan::default() };
    let odb = repo.odb().ok();
    for f in out.follows.iter().filter(|f| f.hooks) {
        let Some((path, id)) = &f.tracked else { continue };
        let Some(dir) = path.eq_ignore_ascii_case(&f.path).then(|| repo.find_tree(*id).ok()).flatten() else { continue };
        for e in dir.iter().filter(|e| e.kind() == Some(ObjectType::Blob)) {
            let Some(name) = e.name().filter(|n| is_hook_name(n)) else { continue };
            let small = odb.as_ref().and_then(|o| o.read_header(e.id()).ok()).is_some_and(|(size, _)| size <= MAX_HOOK_READ);
            if let Some(blob) = small.then(|| repo.find_blob(e.id()).ok()).flatten() {
                out.hook_digests.insert(name.to_owned(), super::content_digest(blob.content()));
            }
            out.hooks.push(name.to_owned());
        }
        out.hooks_dir = repo.workdir().map(|w| w.join(&f.path));
    }
    out.hooks.sort();
    out.hooks.dedup();
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
