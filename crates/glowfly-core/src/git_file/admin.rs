//! The linked worktrees registered under a common dir, found as libgit2's worktree list finds
//! them but without following any link to a refused place: libgit2's own list and lookup stat
//! through the admin files (`HEAD`, `commondir`, `gitdir`, `locked`) and the place `commondir`
//! names.

use super::{Named, common_dir_of, read_small_in, worktree_folder};
use crate::net_path::may_follow;
use std::path::{Path, PathBuf};

/// One linked worktree's admin dir.
#[derive(Debug, PartialEq)]
pub(crate) struct WorktreeAdmin {
    /// Its name under `<common>/worktrees`.
    pub name: String,
    /// `<common>/worktrees/<name>`.
    pub dir: PathBuf,
    /// The worktree folder: the parent of the `.git` file its `gitdir` record names, as
    /// libgit2's `Worktree::path()` gives it.
    pub folder: PathBuf,
    /// Nothing it names leads to a refused place, so libgit2 may look it up, validate it and
    /// open its folder. Otherwise it is read from the admin dir alone.
    pub near: bool,
}

/// The linked worktrees under `common` (a common dir), in listing order: the folders in
/// `worktrees/` holding `commondir`, `gitdir` and `HEAD` files, as libgit2 lists them. One whose
/// files are links to a refused place, or whose `commondir` names one, is listed with `near`
/// false; one whose `gitdir` record can't be read, or whose admin dir is such a link, is left
/// out (there is no folder to show). Nothing past a refused place is touched.
pub(crate) fn worktree_admins(common: &Path) -> Vec<WorktreeAdmin> {
    let root = common.join("worktrees");
    if !may_follow(&root, common) {
        return Vec::new();
    }
    let Ok(read) = std::fs::read_dir(&root) else { return Vec::new() };
    let mut out = Vec::new();
    for e in read.flatten() {
        // libgit2's list drops a name that isn't UTF-8 too.
        let Ok(name) = e.file_name().into_string() else { continue };
        let dir = root.join(&name);
        if !may_follow(&dir, &root) || !dir.is_dir() {
            continue;
        }
        if let Some(a) = admin(name, dir, common) {
            out.push(a);
        }
    }
    out
}

/// The admin dir `dir` (allowed, a folder) as `worktree_admins` lists it.
fn admin(name: String, dir: PathBuf, common: &Path) -> Option<WorktreeAdmin> {
    let mut near = true;
    for f in ["commondir", "gitdir", "HEAD"] {
        let file = dir.join(f);
        if !may_follow(&file, &dir) {
            near = false;
        } else if !std::fs::metadata(&file).is_ok_and(|m| m.is_file()) {
            // Not a worktree admin dir to libgit2.
            return None;
        }
    }
    if !may_follow(&dir.join("locked"), &dir) || matches!(common_dir_of(&dir, common), Named::Refused(_)) {
        near = false;
    }
    let git_file = match worktree_folder(&dir, common) {
        Named::At(f) => f,
        Named::Refused(f) => {
            near = false;
            f
        }
        Named::None => return None,
    };
    let folder = git_file.parent().map_or_else(|| git_file.clone(), Path::to_path_buf);
    near &= may_follow(&folder, common);
    Some(WorktreeAdmin { name, dir, folder, near })
}

/// The `locked` reason of the admin dir `dir` read without libgit2: None when not locked, ""
/// without a reason, and "" for a `locked` that is a link to a refused place (never followed:
/// git counts the worktree as locked while the file is there).
pub(crate) fn admin_locked(dir: &Path) -> Option<String> {
    let file = dir.join("locked");
    if !may_follow(&file, dir) {
        return Some(String::new());
    }
    std::fs::symlink_metadata(&file).ok()?;
    let text = read_small_in(&file, dir).and_then(|b| String::from_utf8(b).ok());
    Some(text.map(|s| s.trim_end().to_owned()).unwrap_or_default())
}
