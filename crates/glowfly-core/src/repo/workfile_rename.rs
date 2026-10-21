//! Renaming one working-tree file (working tree only; `workfile_mv` also moves it in the
//! index), and the checks both renames share.
//!
//! A rename onto a path that exists is allowed only when that path is the source itself under
//! another spelling: a change of case (`Ärger.txt` to `ärger.txt`) or of Unicode form on a file
//! system that ignores those. That is decided by file identity (see `workfile_disk`), so it
//! works the same on a case-sensitive Windows folder or macOS volume, and on a Linux FAT or
//! casefold mount. A change of case in a parent folder is refused: the file system would keep
//! the folder's old spelling, so the rename would change nothing.

use super::noclobber::rename_noclobber;
use super::workfile::{Written, create_parents, prune_empty_parents, remove_dirs};
use super::workfile::{already_exists, check_new_path, check_parents, conflicted, contained, existing, open_root, problem};
use super::workfile_disk::{DISK, Disk, check_parent_spelling, ensure_inside, listed_exactly, new_path_warnings};
use crate::error::{Error, Result};
use git2::Repository;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// A rename that passed every check, ready to move.
pub(super) struct RenamePlan {
    pub root: PathBuf,
    pub from: String,
    pub to: String,
    pub full_from: PathBuf,
    pub full_to: PathBuf,
    /// `to` is `from` under another spelling, so the move goes through a temporary name.
    pub case_only: bool,
    pub warnings: Vec<String>,
}

/// The checks shared by both renames: path rules, links in parent folders, parent folders
/// spelled as on disk, an existing, unconflicted source, and nothing at the target except the
/// source itself under another spelling.
pub(super) fn plan_rename_on(workdir: &Path, from: &str, to: &str, disk: &Disk) -> Result<(Repository, RenamePlan)> {
    let from = contained(from)?;
    let to = check_new_path(to).map_err(problem)?;
    if from == to {
        return Err(Error::Other("the new path is the same".into()));
    }
    let (repo, root) = open_root(workdir)?;
    check_parents(&root, from)?;
    check_parents(&root, &to)?;
    check_parent_spelling(&root, &to, disk)?;
    let (full_from, full_to) = (root.join(from), root.join(&to));
    existing(&root, from)?;
    conflicted(&repo, from)?;
    let case_only = (disk.exists)(&full_to);
    if case_only && !same_spelling_of(&full_from, &full_to, disk) {
        return Err(already_exists(&to, full_to.symlink_metadata().is_ok_and(|m| m.is_dir())));
    }
    let warnings = new_path_warnings(&repo, &to, Some(from));
    let plan = RenamePlan { root, from: from.to_owned(), to, full_from, full_to, case_only, warnings };
    Ok((repo, plan))
}

/// Whether the existing `to` is `from` under another spelling: one file by identity, and the
/// folder lists no entry spelled exactly like `to` (a hard link would have one).
fn same_spelling_of(from: &Path, to: &Path, disk: &Disk) -> bool {
    let listed = match (to.parent(), to.file_name()) {
        (Some(dir), Some(name)) => listed_exactly(dir, name),
        _ => true,
    };
    !listed && (disk.same_file)(from, to)
}

/// Moves the working-tree file `from` to the typed path `to`, creating missing folders and
/// removing the ones `from` leaves empty. Never overwrites a file; only a change of case or
/// Unicode form is allowed onto a path that exists, when both name the same file. Returns
/// whether git ignores the new path, and warnings about the new name.
pub fn rename_file(workdir: &Path, from: &str, to: &str) -> Result<Written> {
    rename_file_on(workdir, from, to, &DISK)
}

pub(super) fn rename_file_on(workdir: &Path, from: &str, to: &str, disk: &Disk) -> Result<Written> {
    let (repo, p) = plan_rename_on(workdir, from, to, disk)?;
    let created = create_parents(&p.root, &p.to)?;
    let moved = ensure_inside(&p.root, p.full_to.parent().unwrap_or(&p.root)).and_then(|()| {
        // The check above gives a clear message; the move itself still refuses a file that
        // appeared since, rather than replacing it.
        let r = if p.case_only { rename_via_temp(&p.full_from, &p.full_to) } else { rename_noclobber(&p.full_from, &p.full_to) };
        r.map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => already_exists(&p.to, p.full_to.is_dir()),
            _ => e.into(),
        })
    });
    if let Err(e) = moved {
        remove_dirs(&created);
        return Err(e);
    }
    prune_empty_parents(&p.root, &p.from);
    Ok(Written { ignored: repo.status_should_ignore(Path::new(&p.to))?, warnings: p.warnings })
}

/// The temporary name a change of spelling goes through, in the same folder as `from`.
pub(super) fn temp_name(from: &str) -> String {
    let (dir, name) = from.rsplit_once('/').map_or(("", from), |(d, n)| (d, n));
    let temp = format!(".{name}.glowfly-rename-{}", std::process::id());
    if dir.is_empty() { temp } else { format!("{dir}/{temp}") }
}

/// A change of case or Unicode form only: some file systems treat a direct rename as a no-op,
/// so go through a temporary name in the same folder. Neither step replaces an existing file.
fn rename_via_temp(from: &Path, to: &Path) -> std::io::Result<()> {
    let name = from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = from.with_file_name(temp_name(&name));
    rename_noclobber(from, &temp)?;
    if let Err(e) = rename_noclobber(&temp, to) {
        let _ = rename_noclobber(&temp, from);
        return Err(e);
    }
    Ok(())
}
