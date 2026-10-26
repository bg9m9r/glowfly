//! The index's submodules with their recorded and checked-out commits, classified for the left
//! panel.

use crate::commit_text;
use crate::error::Result;
use git2::{Oid, Repository, Sort, Submodule, SubmoduleIgnore, SubmoduleStatus};
use std::path::Path;

/// How a submodule's checkout relates to the commit the parent records for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubmoduleState {
    /// Checked out at the recorded commit with nothing changed inside.
    InSync,
    /// The checked-out commit differs from the recorded one.
    OutOfSync,
    /// Uncommitted changes or untracked files inside, at the recorded commit.
    Dirty,
    /// Both of the above: another commit, and changes inside.
    DirtyOutOfSync,
    /// An empty folder: never initialized, or deinitialized.
    Uninitialized,
    /// The folder is gone but the index still has the gitlink.
    Missing,
}

/// One submodule gitlink in the parent's index.
#[derive(Clone, PartialEq, Debug)]
pub struct SubmoduleInfo {
    /// The .gitmodules name, or the path for a gitlink without an entry there.
    pub name: String,
    /// Repo-relative, with '/'.
    pub path: String,
    /// As written in .gitmodules, so it may be relative; None when there is no entry.
    pub url: Option<String>,
    /// The URL git clones it from: this repository's `submodule.<name>.url` when set (an
    /// initialized submodule), else `url` resolved against the default remote's URL or the
    /// working tree (`submodule_ops::resolve_url`). None when `url` is.
    pub clone_url: Option<String>,
    /// .gitmodules' `submodule.<name>.branch`.
    pub branch: Option<String>,
    /// In the index but not in HEAD: added (staged) and not committed yet.
    pub added: bool,
    /// The gitlink in the parent's index.
    pub recorded: Oid,
    /// The submodule's HEAD; None when uninitialized.
    pub checked_out: Option<Oid>,
    pub state: SubmoduleState,
    /// Commits (id, summary), newest first and at most `LEFT_BEHIND_MAX`, that the checkout
    /// reaches but no branch, tag or remote-tracking branch inside the submodule nor the
    /// recorded commit contains. Updating to the recorded commit leaves them reachable only from
    /// the submodule's reflog. Usually commits made on the detached HEAD a previous update left.
    pub left_behind: Vec<(Oid, String)>,
}

/// The most commits `SubmoduleInfo::left_behind` lists.
pub const LEFT_BEHIND_MAX: usize = 50;

impl SubmoduleInfo {
    pub fn is_initialized(&self) -> bool {
        !matches!(self.state, SubmoduleState::Uninitialized | SubmoduleState::Missing)
    }

    pub fn is_out_of_sync(&self) -> bool {
        matches!(self.state, SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync)
    }

    pub fn is_dirty(&self) -> bool {
        matches!(self.state, SubmoduleState::Dirty | SubmoduleState::DirtyOutOfSync)
    }
}

/// The submodules of `repo` that have a gitlink in its index, sorted by path. Never fails: a
/// broken .gitmodules gives an empty list, and an entry whose status can't be read is
/// classified from its ids alone.
pub(super) fn list(repo: &Repository) -> Vec<SubmoduleInfo> {
    let Ok(subs) = repo.submodules() else { return Vec::new() };
    // A snapshot: only a snapshot hands out values as bytes.
    let gitmodules = repo.workdir().and_then(|w| git2::Config::open(&w.join(".gitmodules")).and_then(|mut c| c.snapshot()).ok());
    let config = repo.config().and_then(|mut c| c.snapshot()).ok();
    let base = url_base(repo, config.as_ref());
    let workdir = repo.workdir().map(Path::to_path_buf);
    let mut out: Vec<SubmoduleInfo> = subs
        .iter()
        .filter_map(|sm| {
            if !path_is_text(sm, gitmodules.as_ref()) {
                return None;
            }
            let recorded = sm.index_id()?;
            let path = sm.path().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
            let name = sm.name().map(str::to_owned).unwrap_or_else(|| path.clone());
            let checked_out = sm.workdir_id();
            let flags = repo.submodule_status(&name, SubmoduleIgnore::None).unwrap_or(SubmoduleStatus::empty());
            // One component at a time, so the folder uses the platform's separators.
            let folder_exists =
                workdir.as_ref().is_some_and(|w| path.split('/').fold(w.clone(), |p, c| p.join(c)).symlink_metadata().is_ok());
            let state = classify(flags, recorded, checked_out, folder_exists);
            let left_behind = left_behind(sm, recorded, checked_out);
            let url = sm.url().map(str::to_owned);
            let configured = config.as_ref().and_then(|c| c.get_string(&format!("submodule.{name}.url")).ok());
            let clone_url = url.as_ref().map(|u| configured.unwrap_or_else(|| crate::submodule_ops::resolve_url(u, &base)));
            let branch = sm.branch().map(str::to_owned);
            let added = sm.head_id().is_none();
            Some(SubmoduleInfo { name, path, url, clone_url, branch, added, recorded, checked_out, state, left_behind })
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Whether `sm`'s path is UTF-8, so `Submodule::path()` can be called: git2 panics on Windows
/// for a path that isn't. The path is .gitmodules' `submodule.<name>.path`, or the name itself
/// for a gitlink .gitmodules doesn't list. Such a submodule is left out of the list.
fn path_is_text(sm: &Submodule<'_>, gitmodules: Option<&git2::Config>) -> bool {
    let Some(name) = sm.name() else { return false };
    match gitmodules.and_then(|c| c.get_bytes(&format!("submodule.{name}.path")).ok()) {
        Some(path) => std::str::from_utf8(path).is_ok(),
        None => true,
    }
}

/// The submodules of the repository at `workdir` (see `list`).
pub fn submodules_at(workdir: &Path) -> Result<Vec<SubmoduleInfo>> {
    let repo = Repository::open(workdir)?;
    Ok(list(&repo))
}

/// The submodules of the repository at `workdir` for a status reload: None without a
/// .gitmodules unless `known` (the last load found some, e.g. a gitlink without an entry there),
/// so a repository without submodules pays nothing.
pub fn submodules_if_any(workdir: &Path, known: bool) -> Result<Option<Vec<SubmoduleInfo>>> {
    if !known && !workdir.join(".gitmodules").exists() {
        return Ok(None);
    }
    submodules_at(workdir).map(Some)
}

/// What relative submodule URLs resolve against, as git does: the URL of the current branch's
/// remote (else `origin`), or the working tree's path ('/'-separated) when there is no such
/// remote.
fn url_base(repo: &Repository, config: Option<&git2::Config>) -> String {
    let branch = repo.head().ok().filter(|h| h.is_branch()).and_then(|h| h.shorthand().map(str::to_owned));
    let get = |key: String| config.and_then(|c| c.get_string(&key).ok());
    let remote = branch.and_then(|b| get(format!("branch.{b}.remote"))).unwrap_or_else(|| "origin".to_owned());
    get(format!("remote.{remote}.url")).unwrap_or_else(|| {
        let dir = repo.workdir().unwrap_or_else(|| repo.path());
        let text = dir.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
        text.trim_end_matches('/').to_owned()
    })
}

/// Compares ids rather than trusting WD_MODIFIED, so a failed status call still tells in sync
/// from out of sync. `folder_exists`: something is at the submodule's path in the working tree.
fn classify(flags: SubmoduleStatus, recorded: Oid, checked_out: Option<Oid>, folder_exists: bool) -> SubmoduleState {
    if !folder_exists || flags.contains(SubmoduleStatus::WD_DELETED) {
        return SubmoduleState::Missing;
    }
    if checked_out.is_none() || flags.contains(SubmoduleStatus::WD_UNINITIALIZED) {
        return SubmoduleState::Uninitialized;
    }
    let dirty = flags.intersects(SubmoduleStatus::WD_INDEX_MODIFIED | SubmoduleStatus::WD_WD_MODIFIED | SubmoduleStatus::WD_UNTRACKED);
    let out = checked_out != Some(recorded);
    match (dirty, out) {
        (false, false) => SubmoduleState::InSync,
        (false, true) => SubmoduleState::OutOfSync,
        (true, false) => SubmoduleState::Dirty,
        (true, true) => SubmoduleState::DirtyOutOfSync,
    }
}

/// See `SubmoduleInfo::left_behind`; empty when in sync, uninitialized or unreadable.
fn left_behind(sm: &Submodule, recorded: Oid, checked_out: Option<Oid>) -> Vec<(Oid, String)> {
    let Some(head) = checked_out.filter(|&c| c != recorded) else { return Vec::new() };
    let Ok(repo) = sm.open() else { return Vec::new() };
    walk_unreferenced(&repo, head, recorded).unwrap_or_default()
}

fn walk_unreferenced(repo: &Repository, head: Oid, recorded: Oid) -> std::result::Result<Vec<(Oid, String)>, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(head)?;
    // The recorded commit may not have been fetched into the submodule; then only refs hide.
    let _ = walk.hide(recorded);
    // Globs skip refs that don't point at a commit (a tag of a tree, say).
    for glob in ["refs/heads", "refs/tags", "refs/remotes"] {
        walk.hide_glob(glob)?;
    }
    walk.take(LEFT_BEHIND_MAX)
        .map(|id| {
            let id = id?;
            let summary = commit_text::summary(&repo.find_commit(id)?).into_owned();
            Ok((id, summary))
        })
        .collect()
}
