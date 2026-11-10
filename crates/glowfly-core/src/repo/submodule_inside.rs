//! Opening a submodule's own repository from its parent, safely: only a folder whose `.git` and
//! what it names stay on this machine's allowed places (`net_path::may_follow`,
//! `git_file::other_repo`), opened without searching upwards, so an empty folder never finds
//! the parent. Never through `repo_pool`: a pooled handle would keep packs open (locked on
//! Windows) while a fetch or pull runs inside. Also the submodule's checked-out branch and its
//! upstream, which Pull inside checks first.

use crate::error::Result;
use git2::{Repository, RepositoryOpenFlags};
use std::ffi::OsStr;
use std::path::Path;

/// What opening a submodule's repository found.
pub(crate) enum Opened {
    Repo(Repository),
    /// No folder, no `.git`, or one libgit2 can't open: never initialized, deinitialized, or
    /// broken.
    NotCheckedOut,
    /// It, or a file it names, is on another network place than the parent: never opened.
    Refused,
}

/// Opens the repository of the submodule at `path` (repo-relative, with '/') in the parent
/// working tree `workdir`.
pub(crate) fn open_at(workdir: &Path, path: &str) -> Opened {
    // A name that isn't a plain path inside the work tree has no folder there.
    let Ok(folder) = super::work_path(workdir, path) else { return Opened::NotCheckedOut };
    if !crate::net_path::may_follow(&folder, workdir) {
        return Opened::Refused;
    }
    match crate::git_file::other_repo(&folder, workdir) {
        crate::git_file::Found::Refused(_) => Opened::Refused,
        crate::git_file::Found::None => Opened::NotCheckedOut,
        crate::git_file::Found::Repo(_) => match Repository::open_ext(&folder, RepositoryOpenFlags::NO_SEARCH, &[] as &[&OsStr]) {
            Ok(r) => Opened::Repo(r),
            Err(_) => Opened::NotCheckedOut,
        },
    }
}

/// A submodule's HEAD as Pull inside needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsideHead {
    /// The branch checked out (its short name); None on a detached HEAD, as `git submodule
    /// update` leaves it.
    pub branch: Option<String>,
    /// That branch's upstream as git's config names it (`origin/main`), whether or not it has
    /// been fetched yet; None without one.
    pub upstream: Option<String>,
}

/// The HEAD of the submodule at `path` in the parent working tree `workdir`; None when it isn't
/// checked out or may not be opened (`open_at`).
pub fn submodule_head(workdir: &Path, path: &str) -> Result<Option<InsideHead>> {
    let Opened::Repo(repo) = open_at(workdir, path) else { return Ok(None) };
    let head = repo.head()?;
    if !head.is_branch() {
        return Ok(Some(InsideHead { branch: None, upstream: None }));
    }
    let branch = head.shorthand().map(str::to_owned);
    // From the config alone, so an upstream not fetched yet still counts (git pull fetches it).
    let upstream = head.name().and_then(|n| repo.branch_upstream_name(n).ok()).and_then(|u| u.as_str().map(short_ref));
    Ok(Some(InsideHead { branch, upstream }))
}

/// `refs/remotes/origin/main` as `origin/main`, `refs/heads/main` (an upstream in the same
/// repository) as `main`.
fn short_ref(full: &str) -> String {
    full.strip_prefix("refs/remotes/").or_else(|| full.strip_prefix("refs/heads/")).unwrap_or(full).to_owned()
}

#[cfg(test)]
#[path = "submodule_inside_tests.rs"]
mod tests;
