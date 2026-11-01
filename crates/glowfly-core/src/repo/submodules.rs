//! The index's submodules with their recorded and checked-out commits, classified for the left
//! panel. After a whole status read, a submodule's dirtiness comes from that read where it can,
//! so its working tree isn't scanned a second time.

use super::FileEntry;
use crate::commit_text;
use crate::error::Result;
use git2::{Oid, Repository, Sort, Submodule, SubmoduleIgnore, SubmoduleStatus};
use std::collections::HashMap;
use std::path::Path;

#[path = "submodule_memo.rs"]
mod memo;
pub use memo::SubmoduleMemo;

#[cfg(test)]
#[path = "submodules_whole_tests.rs"]
mod whole_tests;

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
    /// A cone-mode sparse checkout leaves its folder out (`sparse_ops::cone_folders`). git still
    /// initializes it when asked, so the batch updates leave it out instead.
    pub outside_sparse: bool,
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
/// classified from its ids alone. `whole`: the unstaged rows of a whole status read just made,
/// which already tell most submodules' dirtiness (`Whole`); `memo` keeps left-behind walks.
pub(super) fn list(repo: &Repository, whole: Option<&[FileEntry]>, memo: Option<&SubmoduleMemo>) -> Vec<SubmoduleInfo> {
    let Ok(subs) = repo.submodules() else { return Vec::new() };
    // A snapshot: only a snapshot hands out values as bytes.
    let gitmodules = repo.workdir().and_then(|w| git2::Config::open(&w.join(".gitmodules")).and_then(|mut c| c.snapshot()).ok());
    let config = repo.config().and_then(|mut c| c.snapshot()).ok();
    let base = url_base(repo, config.as_ref());
    let workdir = repo.workdir().map(Path::to_path_buf);
    let cone = crate::sparse_ops::cone_folders(repo);
    let found: Vec<Found> = subs
        .iter()
        .filter_map(|sm| {
            if !path_is_text(sm, gitmodules.as_ref()) {
                return None;
            }
            let recorded = sm.index_id()?;
            let path = sm.path().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
            let name = sm.name().map(str::to_owned).unwrap_or_else(|| path.clone());
            // Without the working tree's own changes: no scan of the submodule's files.
            let cheap = repo.submodule_status(&name, SubmoduleIgnore::Dirty).ok();
            Some(Found { sm, path, name, recorded, cheap })
        })
        .collect();
    let whole = whole.and_then(|rows| Whole::of(rows, config.as_ref(), &found));
    let mut out: Vec<SubmoduleInfo> = found
        .into_iter()
        .map(|Found { sm, path, name, recorded, cheap }| {
            let checked_out = sm.workdir_id();
            // One component at a time, so the folder uses the platform's separators.
            let folder_exists =
                workdir.as_ref().is_some_and(|w| path.split('/').fold(w.clone(), |p, c| p.join(c)).symlink_metadata().is_ok());
            let known = Known {
                recorded,
                checked_out,
                folder_exists,
                ignore: ignore_of(sm, &name, config.as_ref()),
                read_ignore: sm.ignore_rule(),
            };
            let flags = flags(repo, &name, &path, cheap, &known, whole.as_ref(), memo);
            let state = classify(flags, recorded, checked_out, folder_exists);
            let left_behind = left_behind(sm, recorded, checked_out, memo);
            let url = sm.url().map(str::to_owned);
            let configured = config.as_ref().and_then(|c| c.get_string(&format!("submodule.{name}.url")).ok());
            let clone_url = url.as_ref().map(|u| configured.unwrap_or_else(|| crate::submodule_ops::resolve_url(u, &base)));
            let branch = sm.branch().map(str::to_owned);
            let added = sm.head_id().is_none();
            let outside_sparse = cone.as_ref().is_some_and(|f| !crate::sparse_ops::in_cone(&path, f));
            SubmoduleInfo { name, path, url, clone_url, branch, added, recorded, checked_out, state, left_behind, outside_sparse }
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// A submodule entry before its status is read; `cheap`: its status without the working
/// tree's own changes (no scan of its files), None when that can't be read.
struct Found<'r> {
    sm: &'r Submodule<'r>,
    path: String,
    name: String,
    recorded: Oid,
    cheap: Option<SubmoduleStatus>,
}

/// What `flags` knows of a submodule before deciding whether to scan it.
struct Known {
    recorded: Oid,
    checked_out: Option<Oid>,
    folder_exists: bool,
    /// Its `submodule.<name>.ignore` as git reads it (`ignore_of`).
    ignore: SubmoduleIgnore,
    /// The rule libgit2's read used: .gitmodules' value only (`Submodule::ignore_rule`).
    read_ignore: SubmoduleIgnore,
}

impl Known {
    /// Whether both reads saw everything: then no row means clean.
    fn sees_all(&self) -> bool {
        self.ignore == SubmoduleIgnore::None && self.read_ignore == SubmoduleIgnore::None
    }

    /// Whether both reads saw modified content: then a modified row in sync means dirty.
    fn sees_modified(&self) -> bool {
        let sees = |i| matches!(i, SubmoduleIgnore::None | SubmoduleIgnore::Untracked);
        sees(self.ignore) && sees(self.read_ignore)
    }
}

/// A whole status read's unstaged rows by path, trusted for the submodules' dirtiness. That
/// read hid what each submodule's `ignore` hides, and what `diff.ignoreSubmodules` hides when
/// git read it (libgit2 doesn't read that key), so with that key set it isn't trusted at all.
/// Nor is it when a submodule's status can't be read: libgit2's read then leaves every
/// submodule out (`status::libgit2_lists`). With `core.ignorecase` paths match folded.
struct Whole<'a> {
    rows: HashMap<std::borrow::Cow<'a, str>, char>,
    fold: bool,
}

impl<'a> Whole<'a> {
    fn of(rows: &'a [FileEntry], config: Option<&git2::Config>, found: &[Found<'_>]) -> Option<Self> {
        let config = config?;
        let diff_ignore = config.get_string("diff.ignoreSubmodules").ok();
        if diff_ignore.is_some_and(|v| !v.trim().eq_ignore_ascii_case("none")) || found.iter().any(|f| f.cheap.is_none()) {
            return None;
        }
        let fold = config.get_bool("core.ignorecase").unwrap_or(false);
        let key = |p: &'a str| if fold { p.to_lowercase().into() } else { p.into() };
        Some(Self { rows: rows.iter().map(|f| (key(f.path.as_str()), f.status)).collect(), fold })
    }

    fn get(&self, path: &str) -> Option<char> {
        if self.fold { self.rows.get(path.to_lowercase().as_str()) } else { self.rows.get(path) }.copied()
    }
}

/// The submodule's status flags as `classify` needs them. Its dirtiness matters only for a
/// checked-out submodule, and comes from the whole read where that read could see it: no row
/// with nothing ignored is clean, and a modified row at the recorded commit is dirty (one out of
/// sync is modified whether dirty or not). Otherwise the submodule is scanned.
fn flags(
    repo: &Repository,
    name: &str,
    path: &str,
    cheap: Option<SubmoduleStatus>,
    k: &Known,
    whole: Option<&Whole<'_>>,
    memo: Option<&SubmoduleMemo>,
) -> SubmoduleStatus {
    if let Some(cheap) = cheap {
        let gone = SubmoduleStatus::WD_DELETED | SubmoduleStatus::WD_UNINITIALIZED;
        if !k.folder_exists || k.checked_out.is_none() || cheap.intersects(gone) {
            return cheap;
        }
        if let Some(whole) = whole {
            let in_sync = k.checked_out == Some(k.recorded);
            match whole.get(path) {
                None if k.sees_all() => return cheap,
                Some('M') if in_sync && k.sees_modified() => {
                    return cheap | SubmoduleStatus::WD_WD_MODIFIED;
                }
                _ => {}
            }
        }
    }
    if let Some(m) = memo {
        m.scanned();
    }
    repo.submodule_status(name, SubmoduleIgnore::None).unwrap_or(SubmoduleStatus::empty())
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

/// The submodules of the repository at `workdir` (see `list`), each checked-out one scanned.
pub fn submodules_at(workdir: &Path) -> Result<Vec<SubmoduleInfo>> {
    let repo = Repository::open(workdir)?;
    Ok(list(&repo, None, None))
}

/// The submodules of the repository at `workdir` for a status reload: None without a
/// .gitmodules unless `known` (the last load found some, e.g. a gitlink without an entry there),
/// so a repository without submodules pays nothing. `whole`: the unstaged rows of the whole
/// status read this follows, if it was one, so the submodules it already scanned aren't scanned
/// again; `memo`: the runner's, keeping left-behind walks and counting scans.
pub fn submodules_if_any(
    workdir: &Path,
    known: bool,
    whole: Option<&[FileEntry]>,
    memo: Option<&SubmoduleMemo>,
) -> Result<Option<Vec<SubmoduleInfo>>> {
    if !known && !workdir.join(".gitmodules").exists() {
        return Ok(None);
    }
    let repo = Repository::open(workdir)?;
    Ok(Some(list(&repo, whole, memo)))
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

/// See `SubmoduleInfo::left_behind`; empty when in sync, uninitialized or unreadable. With
/// `memo`, walked again only when the commits or the submodule's refs changed.
fn left_behind(sm: &Submodule, recorded: Oid, checked_out: Option<Oid>, memo: Option<&SubmoduleMemo>) -> Vec<(Oid, String)> {
    let Some(head) = checked_out.filter(|&c| c != recorded) else { return Vec::new() };
    let Ok(repo) = sm.open() else { return Vec::new() };
    // Kept only when whole: a failed walk, a recorded commit not fetched yet, or a shallow
    // history can change without any ref changing.
    let walk = || match walk_unreferenced(&repo, head, recorded) {
        Ok(c) => (c, !repo.is_shallow() && repo.find_commit(recorded).is_ok()),
        Err(_) => (Vec::new(), false),
    };
    match memo {
        Some(m) => m.left_behind(&repo, recorded, head, walk),
        None => walk().0,
    }
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

/// The submodule's `submodule.<name>.ignore` as git reads it: the repository's config over
/// .gitmodules. libgit2's own rule can miss the config's value, which git's status read honours.
fn ignore_of(sm: &Submodule<'_>, name: &str, config: Option<&git2::Config>) -> SubmoduleIgnore {
    let set = config.and_then(|c| c.get_string(&format!("submodule.{name}.ignore")).ok());
    match set.as_deref().map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("none") => SubmoduleIgnore::None,
        Some("untracked") => SubmoduleIgnore::Untracked,
        Some("dirty") => SubmoduleIgnore::Dirty,
        Some("all") => SubmoduleIgnore::All,
        // Unset, or a value git would refuse: whatever libgit2 found, and anything but `none`
        // makes the list scan the submodule itself.
        Some(_) => SubmoduleIgnore::Unspecified,
        None => sm.ignore_rule(),
    }
}
