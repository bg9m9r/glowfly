//! A repository's common git dir: the one its main worktree and every linked worktree share (the
//! main folder's `.git`, or a bare repository's own folder). Tabs of one repository's worktrees
//! are grouped by it.

use crate::paths::{self, FolderId};
use std::path::{Path, PathBuf};

/// The git dir every worktree of a repository shares, as found on disk.
#[derive(Clone, Debug, PartialEq)]
pub struct CommonDir {
    /// libgit2's `commondir()`, resolved on disk (links and short 8.3 names followed).
    pub dir: PathBuf,
    /// `dir`'s identity on disk: what matches any other spelling of it. None on a volume that
    /// gives no file ids.
    pub folder: Option<FolderId>,
    /// The repository's main folder name (`repo_name_of`).
    pub name: String,
}

impl CommonDir {
    /// The common dir of the open repository `repo`.
    pub(crate) fn of(repo: &git2::Repository) -> CommonDir {
        let dir = paths::resolve(repo.commondir());
        let folder = FolderId::of(&dir);
        let name = repo_name_of(&dir);
        CommonDir { dir, folder, name }
    }

    /// The common dir of the repository at (or above) `path`, without loading it. None for a
    /// relative path (never resolved against the working folder) or where no repository opens.
    /// Reads the disk: for worker threads.
    pub fn at(path: &Path) -> Option<CommonDir> {
        if !path.is_absolute() {
            return None;
        }
        git2::Repository::discover(path).ok().map(|repo| Self::of(&repo))
    }

    /// Whether `other` is the same common dir: the same folder on disk, or (without an identity
    /// on either side) the same spelling. Pure.
    pub fn same(&self, other: &CommonDir) -> bool {
        (self.folder.is_some() && self.folder == other.folder) || paths::same_spelling(&self.dir, &other.dir)
    }
}

/// The name of the repository whose common dir is `dir`: the folder holding it for
/// `.../app/.git`, the folder's own name less `.git` for a bare `app.git`. Pure.
pub fn repo_name_of(dir: &Path) -> String {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    let found = match name(dir) {
        Some(n) if n == ".git" => dir.parent().and_then(name),
        Some(n) => Some(n.strip_suffix(".git").filter(|s| !s.is_empty()).map(str::to_owned).unwrap_or(n)),
        None => None,
    };
    found.unwrap_or_else(|| dir.display().to_string())
}
