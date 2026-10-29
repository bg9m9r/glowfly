//! Where a repository's LFS patterns come from: the root `.gitattributes` (read from the working
//! tree, so a pattern just tracked shows before it is committed), every nested `.gitattributes`
//! the index tracks (read as blobs with libgit2, never by walking the working tree), and
//! `info/attributes` in the common git dir (shared by linked worktrees, as git reads it).
//!
//! A nested file's patterns apply inside its folder, so each pattern keeps its file
//! ([`PatternSource`]); `git lfs track` and `untrack` edit only the root file.
//!
//! The work is bounded for huge repositories: at most [`MAX_NESTED_FILES`] nested files are
//! read, and the nested files and `info/attributes` together at most [`MAX_BYTES`] bytes. A file
//! that would pass the byte budget is skipped (its size is read from the object header first),
//! and the rest are ignored once either cap is reached. The root file is always read.

use super::{LfsPattern, parse_attributes};
use git2::{Index, ObjectType, Repository};
use std::path::Path;

/// The most nested `.gitattributes` files read.
pub const MAX_NESTED_FILES: usize = 256;
/// The most bytes read from nested `.gitattributes` files and `info/attributes` together.
pub const MAX_BYTES: usize = 1 << 20;

/// The attributes file a pattern was read from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PatternSource {
    /// The root `.gitattributes`, which `git lfs track` and `untrack` edit.
    #[default]
    Root,
    /// A tracked `.gitattributes` in this folder ('/'-separated, as shown: see `repo::shown_path`).
    Nested(String),
    /// `info/attributes` in the (common) git dir.
    Info,
}

impl PatternSource {
    pub fn is_root(&self) -> bool {
        *self == Self::Root
    }

    /// The file as shown to the user, relative to the work tree.
    pub fn file(&self) -> String {
        match self {
            Self::Root => ".gitattributes".into(),
            Self::Nested(folder) => format!("{folder}/.gitattributes"),
            Self::Info => ".git/info/attributes".into(),
        }
    }
}

/// The LFS patterns of the root `.gitattributes`; empty without one. Only reads the file.
pub fn root_patterns(workdir: &Path) -> Vec<LfsPattern> {
    std::fs::read(workdir.join(".gitattributes")).map(|b| parse_attributes(&String::from_utf8_lossy(&b))).unwrap_or_default()
}

/// Every LFS pattern of the repository at `workdir`: the root file's, then the nested files' in
/// index order, then `info/attributes`'. Only the root file's when the repository can't be opened.
pub fn repo_patterns(workdir: &Path) -> Vec<LfsPattern> {
    match crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir) {
        Ok(repo) => patterns_in(&repo, workdir),
        Err(_) => root_patterns(workdir),
    }
}

/// [`repo_patterns`] with an open repository.
pub fn patterns_in(repo: &Repository, workdir: &Path) -> Vec<LfsPattern> {
    let mut out = root_patterns(workdir);
    let mut budget = MAX_BYTES;
    // info/attributes first, so many nested files can't use up its share of the budget.
    let info = info_patterns(repo, &mut budget);
    out.extend(nested_patterns(repo, &mut budget));
    out.extend(info);
    out
}

fn tagged(text: &[u8], source: &PatternSource) -> Vec<LfsPattern> {
    let mut patterns = parse_attributes(&String::from_utf8_lossy(text));
    for p in &mut patterns {
        p.source = source.clone();
    }
    patterns
}

fn info_patterns(repo: &Repository, budget: &mut usize) -> Vec<LfsPattern> {
    let file = repo.commondir().join("info").join("attributes");
    let Ok(meta) = std::fs::metadata(&file) else { return Vec::new() };
    if !meta.is_file() || meta.len() > *budget as u64 {
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(&file) else { return Vec::new() };
    *budget = budget.saturating_sub(bytes.len());
    tagged(&bytes, &PatternSource::Info)
}

/// The folder of a nested `.gitattributes` index path, None for any other path (the root file
/// included). Matched byte for byte, as git names the file.
fn nested_folder(path: &[u8]) -> Option<&[u8]> {
    let folder = path.strip_suffix(b"/.gitattributes")?;
    (!folder.is_empty()).then_some(folder)
}

/// A regular file's mode: git ignores a `.gitattributes` that is a symlink or a submodule.
fn regular(mode: u32) -> bool {
    mode == 0o100644 || mode == 0o100755
}

fn nested_patterns(repo: &Repository, budget: &mut usize) -> Vec<LfsPattern> {
    // The index file on its own: the repository's shared index stays as the caller left it.
    let Ok(index) = Index::open(&repo.path().join("index")) else { return Vec::new() };
    let Ok(odb) = repo.odb() else { return Vec::new() };
    let mut out = Vec::new();
    let mut files = 0;
    for entry in index.iter() {
        // Stage 0 only: a conflicted file has no settled content.
        let stage = (entry.flags >> 12) & 0x3;
        let Some(folder) = nested_folder(&entry.path).filter(|_| stage == 0 && regular(entry.mode)) else { continue };
        if files == MAX_NESTED_FILES || *budget == 0 {
            break;
        }
        files += 1;
        let Ok((size, ObjectType::Blob)) = odb.read_header(entry.id) else { continue };
        if size > *budget {
            continue;
        }
        let Ok(object) = odb.read(entry.id) else { continue };
        *budget -= object.data().len().min(*budget);
        let source = PatternSource::Nested(crate::repo::shown_path(folder));
        out.extend(tagged(object.data(), &source));
    }
    out
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
