//! Whether git can add a submodule at a repo path: what the index already has there, what is
//! on disk, its parent folders, .gitignore, and a stored module an earlier submodule left. The
//! typed text is checked first by `submodule_ops::parse_add_path`.

use super::workfile::open_root;
use super::workfile_disk::{DISK, check_parent_spelling, new_path_warnings};
use crate::submodule_ops::{AddPathProblem, module_dir};
use std::io::ErrorKind;
use std::path::Path;

/// Checks the repo path `rel` ('/'-separated, from `parse_add_path`) of the repository at
/// `workdir` for a new submodule. Ok holds warnings (a name Windows can't use, where it isn't
/// refused; a tracked path that differs only in case or Unicode form).
pub fn check_submodule_path(workdir: &Path, rel: &str) -> Result<Vec<String>, AddPathProblem> {
    let unreadable = |e: &dyn std::fmt::Display| AddPathProblem::Unreadable(e.to_string());
    let (repo, root) = open_root(workdir).map_err(|e| unreadable(&e))?;
    let index = repo.index().map_err(|e| unreadable(&e))?;
    let want = rel.as_bytes();
    for e in index.iter() {
        let p = e.path.as_slice();
        if p == want {
            return Err(AddPathProblem::InIndex { gitlink: e.mode == 0o160000 });
        }
        if p.len() > want.len() && p.starts_with(want) && p[want.len()] == b'/' {
            return Err(AddPathProblem::TrackedInside);
        }
        if want.len() > p.len() && want.starts_with(p) && want[p.len()] == b'/' {
            return Err(AddPathProblem::UnderTrackedFile(String::from_utf8_lossy(p).into_owned()));
        }
    }
    check_parents(&root, rel)?;
    check_parent_spelling(&root, rel, &DISK).map_err(|e| AddPathProblem::ParentSpelling(e.to_string()))?;
    let full = rel.split('/').fold(root.clone(), |p, c| p.join(c));
    match full.symlink_metadata() {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(unreadable(&e)),
        Ok(m) if !m.is_dir() || m.file_type().is_symlink() => return Err(AddPathProblem::Exists { file: true }),
        Ok(_) => {
            let mut entries = std::fs::read_dir(&full).map_err(|e| unreadable(&e))?;
            if entries.next().is_some() {
                return Err(AddPathProblem::Exists { file: false });
            }
        }
    }
    if repo.is_path_ignored(rel).unwrap_or(false) {
        return Err(AddPathProblem::Ignored);
    }
    // git keeps stored modules in the repository's own git dir, per worktree.
    let git_dir = crate::native_path::native(repo.path());
    if let Ok(dir) = module_dir(&git_dir, rel)
        && dir.symlink_metadata().is_ok()
    {
        return Err(AddPathProblem::ModuleExists(dir));
    }
    Ok(new_path_warnings(&repo, rel, None))
}

/// The parent folders of `rel` that already exist must be real folders, not files or links
/// (junctions count as links).
fn check_parents(root: &Path, rel: &str) -> Result<(), AddPathProblem> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut at = root.to_path_buf();
    for i in 0..parts.len() - 1 {
        at.push(parts[i]);
        let shown = parts[..=i].join("/");
        match at.symlink_metadata() {
            Err(_) => return Ok(()),
            Ok(m) if m.file_type().is_symlink() => return Err(AddPathProblem::ParentLink(shown)),
            Ok(m) if !m.is_dir() => return Err(AddPathProblem::ParentFile(shown)),
            Ok(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "submodule_add_tests.rs"]
mod tests;
