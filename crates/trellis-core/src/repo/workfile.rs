//! Creating, renaming and deleting one working-tree file, and checking a typed path.
//!
//! Working tree only: nothing here touches the index or HEAD, so a renamed or deleted tracked
//! file shows up as an unstaged change. The file operations use plain `std::fs`, because no git
//! config, hooks or filters apply to creating, moving or removing a working copy; libgit2 is
//! used only to ask whether a path is ignored and whether it has unresolved conflicts. A rename
//! that is also staged lives in `workfile_mv`, which shares the checks here.
//!
//! Every path is checked before anything is written: it must stay inside the working tree, never
//! enter `.git`, and never go through a symbolic link or junction in its parent folders, so the
//! repository can't be escaped. New names must also be valid on Windows, so the repository stays
//! checkout-able there.

use super::noclobber::rename_noclobber;
use crate::error::{Error, Result};
use git2::Repository;
use std::fs::{Metadata, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Why a typed path can't be used for a new file.
#[derive(Debug, Clone, PartialEq)]
pub enum PathProblem {
    Empty,
    Absolute,
    OutsideRepo,
    InsideGitDir,
    /// The path ends with a separator.
    Folder,
    /// This component isn't a valid file name (on any supported system).
    BadName(String),
}

impl PathProblem {
    pub fn message(&self) -> String {
        match self {
            PathProblem::Empty => "Enter a file name.".into(),
            PathProblem::Absolute => "Enter a path inside the repository.".into(),
            PathProblem::OutsideRepo => "That path leads outside the repository.".into(),
            PathProblem::InsideGitDir => "Files can't be created inside the .git folder.".into(),
            PathProblem::Folder => "Enter a file name, not a folder.".into(),
            PathProblem::BadName(c) => format!("{c} isn't a valid file name."),
        }
    }
}

const DEVICE_NAMES: &[&str] = &["CON", "PRN", "AUX", "NUL"];

/// Checks a typed repo-relative path for a new file and returns it normalized: '/' separators,
/// no empty or '.' components. Pure; looks at the text only.
pub fn check_new_path(input: &str) -> Result<String, PathProblem> {
    let text = input.trim().replace('\\', "/");
    let b = text.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/');
    // A leading '/' also covers UNC paths ("//server/share").
    if text.starts_with('/') || drive {
        return Err(PathProblem::Absolute);
    }
    let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() {
        return Err(PathProblem::Empty);
    }
    if parts.contains(&"..") {
        return Err(PathProblem::OutsideRepo);
    }
    if parts.iter().any(|p| is_git_dir(p)) {
        return Err(PathProblem::InsideGitDir);
    }
    if text.ends_with('/') {
        return Err(PathProblem::Folder);
    }
    if let Some(bad) = parts.iter().find(|p| !valid_name(p)) {
        return Err(PathProblem::BadName((*bad).to_owned()));
    }
    Ok(parts.join("/"))
}

/// `.git` as Windows would resolve it: trailing dots and spaces dropped, any case, or its short
/// (8.3) name.
fn is_git_dir(part: &str) -> bool {
    let trimmed = part.trim_end_matches(['.', ' ']);
    trimmed.eq_ignore_ascii_case(".git") || part.eq_ignore_ascii_case("git~1")
}

fn valid_name(part: &str) -> bool {
    if part.chars().any(|c| c.is_control() || "<>:\"|?*".contains(c)) || part.ends_with(['.', ' ']) {
        return false;
    }
    let stem = part.split('.').next().unwrap_or(part).trim_end();
    let numbered = stem.len() == 4
        && stem.is_ascii()
        && (stem[..3].eq_ignore_ascii_case("COM") || stem[..3].eq_ignore_ascii_case("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9');
    !(numbered || DEVICE_NAMES.iter().any(|d| stem.eq_ignore_ascii_case(d)))
}

/// An existing path to rename or delete, exactly as status lists it ('/' separators). Only the
/// containment rules apply, so a file with an odd name can still be removed, and nothing is
/// trimmed or rewritten: " a.txt" or (outside Windows) "a\b" is that file, never "a.txt" or
/// "a/b". A path the system would read as another file is refused instead.
pub(super) fn contained(path: &str) -> Result<&str> {
    let parts: Vec<&str> = path.split('/').collect();
    if path.starts_with('/') {
        return Err(problem(PathProblem::Absolute));
    }
    if parts.iter().any(|p| p.is_empty() || *p == ".") {
        return Err(Error::Other(format!("{path:?} isn't a working-tree file path")));
    }
    if parts.contains(&"..") {
        return Err(problem(PathProblem::OutsideRepo));
    }
    if parts.iter().any(|p| is_git_dir(p)) {
        return Err(problem(PathProblem::InsideGitDir));
    }
    // Windows reads '\' as a separator, ':' as a drive or stream, and drops trailing dots and
    // spaces, so such a path would reach a different file from the one listed.
    if cfg!(windows) && (path.contains(['\\', ':']) || parts.iter().any(|p| p.ends_with(['.', ' ']))) {
        return Err(Error::Unsupported(format!("{path:?} can't be reached by that name on Windows")));
    }
    Ok(path)
}

fn problem(p: PathProblem) -> Error {
    Error::Other(p.message())
}

fn open_root(workdir: &Path) -> Result<(Repository, PathBuf)> {
    let repo = Repository::open(workdir)?;
    let root = repo.workdir().ok_or_else(|| Error::Unsupported("a bare repository has no working tree".into()))?.to_path_buf();
    Ok((repo, root))
}

/// The parent folders of `rel` that already exist must be real folders, not files or links.
fn check_parents(root: &Path, rel: &str) -> Result<()> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut prefix = root.to_path_buf();
    for i in 0..parts.len() - 1 {
        prefix.push(parts[i]);
        let shown = parts[..=i].join("/");
        match prefix.symlink_metadata() {
            Err(_) => return Ok(()),
            // Junctions count as symbolic links too.
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::Unsupported(format!("{shown} is a symbolic link; Trellis won't write through it")));
            }
            Ok(m) if !m.is_dir() => return Err(Error::Other(format!("{shown} is a file, so {rel} can't be written"))),
            Ok(_) => {}
        }
    }
    Ok(())
}

/// Guards against a folder swapped for a link between the checks and the write.
pub(super) fn ensure_inside(root: &Path, dir: &Path) -> Result<()> {
    if dir.canonicalize()?.starts_with(root.canonicalize()?) {
        Ok(())
    } else {
        Err(Error::Unsupported(format!("{} is outside the repository", dir.display())))
    }
}

/// Creates the missing parent folders of `rel`, returning the ones it made (outermost first).
pub(super) fn create_parents(root: &Path, rel: &str) -> Result<Vec<PathBuf>> {
    let mut created = Vec::new();
    let mut dir = root.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        dir.push(part);
        if dir.symlink_metadata().is_err() {
            if let Err(e) = std::fs::create_dir(&dir) {
                remove_dirs(&created);
                return Err(e.into());
            }
            created.push(dir.clone());
        }
    }
    Ok(created)
}

/// Removes folders `create_parents` made, innermost first, ignoring errors.
pub(super) fn remove_dirs(created: &[PathBuf]) {
    for d in created.iter().rev() {
        let _ = std::fs::remove_dir(d);
    }
}

/// Removes the parent folders of `rel` left empty, stopping at the first that isn't (or at the
/// repository root), as `git rm` does.
pub(super) fn prune_empty_parents(root: &Path, rel: &str) {
    let parts: Vec<&str> = rel.split('/').collect();
    for depth in (1..parts.len()).rev() {
        if std::fs::remove_dir(root.join(parts[..depth].join("/"))).is_err() {
            break;
        }
    }
}

fn already_exists(rel: &str, dir: bool) -> Error {
    Error::Other(if dir { format!("a folder named {rel} already exists") } else { format!("{rel} already exists") })
}

fn conflicted(repo: &Repository, rel: &str) -> Result<()> {
    let index = repo.index()?;
    if (1..=3).any(|stage| index.get_path(Path::new(rel), stage).is_some()) {
        return Err(Error::Other(format!("{rel} has unresolved conflicts; resolve them first")));
    }
    Ok(())
}

/// The existing file at `rel`: not missing and not a folder (a symbolic link is fine).
fn existing(root: &Path, rel: &str) -> Result<Metadata> {
    let m = root.join(rel).symlink_metadata().map_err(|_| Error::NotFound(format!("{rel} is no longer in the working tree")))?;
    if m.is_dir() {
        return Err(Error::Unsupported(format!("{rel} is a folder")));
    }
    Ok(m)
}

/// Creates an empty file at the typed `path`, with any missing folders. Never overwrites
/// anything. Returns whether git ignores the new file.
pub fn create_file(workdir: &Path, path: &str) -> Result<bool> {
    let rel = check_new_path(path).map_err(problem)?;
    let (repo, root) = open_root(workdir)?;
    check_parents(&root, &rel)?;
    let full = root.join(&rel);
    if let Ok(m) = full.symlink_metadata() {
        return Err(already_exists(&rel, m.is_dir()));
    }
    let created = create_parents(&root, &rel)?;
    let written = ensure_inside(&root, full.parent().unwrap_or(&root)).and_then(|()| {
        // create_new also refuses a name that differs only in case on a case-insensitive file
        // system, and a file that appeared since the check above.
        OpenOptions::new().write(true).create_new(true).open(&full).map(drop).map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => already_exists(&rel, full.is_dir()),
            _ => e.into(),
        })
    });
    if let Err(e) = written {
        remove_dirs(&created);
        return Err(e);
    }
    Ok(repo.status_should_ignore(Path::new(&rel))?)
}

/// A rename that passed every check, ready to move.
pub(super) struct RenamePlan {
    pub root: PathBuf,
    pub from: String,
    pub to: String,
    pub full_from: PathBuf,
    pub full_to: PathBuf,
    pub case_only: bool,
}

/// The checks shared by both renames: path rules, links in parent folders, an existing,
/// unconflicted source, and nothing at the target except the same file under another case.
pub(super) fn plan_rename(workdir: &Path, from: &str, to: &str) -> Result<(Repository, RenamePlan)> {
    let from = contained(from)?;
    let to = check_new_path(to).map_err(problem)?;
    if from == to {
        return Err(Error::Other("the new path is the same".into()));
    }
    let (repo, root) = open_root(workdir)?;
    check_parents(&root, from)?;
    check_parents(&root, &to)?;
    let (full_from, full_to) = (root.join(from), root.join(&to));
    existing(&root, from)?;
    conflicted(&repo, from)?;
    let case_only = from.eq_ignore_ascii_case(&to);
    if let Ok(m) = full_to.symlink_metadata()
        && !(case_only && same_file_by_case(&full_from, &full_to))
    {
        return Err(already_exists(&to, m.is_dir()));
    }
    let plan = RenamePlan { root, from: from.to_owned(), to, full_from, full_to, case_only };
    Ok((repo, plan))
}

/// Moves the working-tree file `from` to the typed path `to`, creating missing folders and
/// removing the ones `from` leaves empty. Never overwrites a file; only a change of case is
/// allowed onto a path that exists, when both name the same file on a case-insensitive file
/// system. Returns whether git ignores the new path.
pub fn rename_file(workdir: &Path, from: &str, to: &str) -> Result<bool> {
    let (repo, p) = plan_rename(workdir, from, to)?;
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
    Ok(repo.status_should_ignore(Path::new(&p.to))?)
}

/// Whether `to` is `from` under another case on a case-insensitive file system: both resolve to
/// the same file and the folder has no entry spelled exactly like `to`.
fn same_file_by_case(from: &Path, to: &Path) -> bool {
    let same = matches!((from.canonicalize(), to.canonicalize()), (Ok(a), Ok(b)) if a == b);
    let name = to.file_name();
    let spelled = to
        .parent()
        .and_then(|p| std::fs::read_dir(p).ok())
        .is_some_and(|mut d| d.any(|e| e.is_ok_and(|e| Some(e.file_name().as_os_str()) == name)));
    same && !spelled
}

/// A change of case only: some file systems treat a direct rename as a no-op, so go through a
/// temporary name in the same folder.
/// Neither step replaces an existing file.
fn rename_via_temp(from: &Path, to: &Path) -> std::io::Result<()> {
    let name = from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = from.with_file_name(format!(".{name}.trellis-rename-{}", std::process::id()));
    rename_noclobber(from, &temp)?;
    if let Err(e) = rename_noclobber(&temp, to) {
        let _ = rename_noclobber(&temp, from);
        return Err(e);
    }
    Ok(())
}

/// Deletes the working-tree file (or symbolic link) `path` and the folders it leaves empty. The
/// index is untouched, so a tracked file shows up as an unstaged deletion.
pub fn delete_file(workdir: &Path, path: &str) -> Result<()> {
    let rel = contained(path)?;
    let (repo, root) = open_root(workdir)?;
    check_parents(&root, rel)?;
    let m = existing(&root, rel)?;
    conflicted(&repo, rel)?;
    remove(&root.join(rel), &m)?;
    prune_empty_parents(&root, rel);
    Ok(())
}

fn remove(full: &Path, m: &Metadata) -> std::io::Result<()> {
    match std::fs::remove_file(full) {
        // A link to a folder (or a junction) is removed as a folder on Windows; its target stays.
        Err(e) if cfg!(windows) && m.file_type().is_symlink() => std::fs::remove_dir(full).map_err(|_| e),
        Err(e) if cfg!(windows) && e.kind() == ErrorKind::PermissionDenied && m.permissions().readonly() => {
            let mut p = m.permissions();
            // Only on Windows, where this just clears the read-only attribute.
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            std::fs::set_permissions(full, p)?;
            std::fs::remove_file(full)
        }
        r => r,
    }
}
