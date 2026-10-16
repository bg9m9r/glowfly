//! The versions an external diff or merge tool is given: for a diff, where its old and new
//! sides come from (a blob, the working file, or nothing); for a conflict, the blobs of its
//! three index stages. `external_tools::compare` writes them out.

use super::rpath;
use super::{DiffTarget, LoadDiffOptions};
use crate::error::{Error, Result};
use crate::gitcli;
use git2::{DiffFile, ErrorCode, FileMode, Oid, Repository};
use std::path::Path;
use std::time::Duration;

/// How long `git cat-file --filters` may take for one version (a filter driver such as Git LFS
/// may download it).
pub const TOOL_FILE_TIMEOUT: Duration = Duration::from_secs(120);

/// Where one side of a diff comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideSource {
    /// A stored blob, with the path it has on that side (a rename's old side has the old path).
    Blob { id: Oid, path: String },
    /// The working-tree file at this repo-relative path, itself.
    Working(String),
    /// Nothing: the file doesn't exist on this side.
    Empty,
}

/// Both sides of a diff, and what to call each (a short commit id, "index", "empty").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffSides {
    pub old: SideSource,
    pub new: SideSource,
    pub old_label: String,
    pub new_label: String,
}

fn short(oid: Oid) -> String {
    crate::rebase::short(oid)
}

/// One side of `delta` as a source: Empty when it has no blob. A submodule can't be compared.
fn side(f: &DiffFile<'_>, fallback_path: &str) -> Result<SideSource> {
    let path = f.path_bytes().and_then(rpath::text).unwrap_or_else(|| fallback_path.to_owned());
    if f.mode() == FileMode::Commit {
        return Err(Error::Unsupported(format!("{path} is a submodule; it can't be opened in a diff tool")));
    }
    Ok(if f.id().is_zero() { SideSource::Empty } else { SideSource::Blob { id: f.id(), path } })
}

/// Where the old and new sides of `target`'s file come from, as its diff in the viewer has
/// them (renames detected for commits). `NotFound` when the file has no changes there.
pub fn diff_sides(workdir: &Path, target: &DiffTarget) -> Result<DiffSides> {
    let repo = Repository::open(workdir)?;
    let diff = super::diff::target_diff(&repo, target, LoadDiffOptions::default(), None)?;
    let path = target.path();
    let delta = diff.deltas().find(|d| rpath::touches(d, path)).ok_or_else(|| Error::NotFound(format!("no changes in {path}")))?;
    let old = side(&delta.old_file(), path)?;
    let mut new = side(&delta.new_file(), path)?;
    if let DiffTarget::Unstaged(p) = target
        && delta.status() != git2::Delta::Deleted
    {
        new = SideSource::Working(p.clone());
    }
    let (old_label, new_label) = match target {
        DiffTarget::Commit(oid, _) => {
            let parent = repo.find_commit(*oid)?.parent_id(0).ok();
            (parent.map(short).unwrap_or_else(|| "empty".into()), short(*oid))
        }
        DiffTarget::Between(from, to, _) => (short(*from), short(*to)),
        DiffTarget::Staged(_) => ("HEAD".into(), "index".into()),
        DiffTarget::Unstaged(_) => ("index".into(), "working tree".into()),
    };
    let label = |s: &SideSource, l: String| if *s == SideSource::Empty { "empty".to_owned() } else { l };
    Ok(DiffSides { old_label: label(&old, old_label), new_label: label(&new, new_label), old, new })
}

/// The blobs of a conflict's stages: the common ancestor (None when both sides added the
/// file), ours and theirs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeSides {
    pub base: Option<Oid>,
    pub ours: Oid,
    pub theirs: Oid,
}

/// The stages of the conflict at `path`. `NotFound` when it isn't conflicted; `Unsupported`
/// when one side deleted it, or it is a symbolic link or a submodule on any side.
pub fn merge_sides(workdir: &Path, path: &str) -> Result<MergeSides> {
    let repo = Repository::open(workdir)?;
    let index = repo.index()?;
    let c = match index.conflict_get(Path::new(path)) {
        Ok(c) => c,
        Err(e) if e.code() == ErrorCode::NotFound => return Err(Error::NotFound(format!("{path} is not conflicted"))),
        Err(e) => return Err(e.into()),
    };
    let (Some(ours), Some(theirs)) = (&c.our, &c.their) else {
        return Err(Error::Unsupported(format!("one side deleted {path}; keep or delete it in the conflict view")));
    };
    for e in c.ancestor.iter().chain([ours, theirs]) {
        let what = match e.mode & 0o170000 {
            0o120000 => "a symbolic link",
            0o160000 => "a submodule",
            _ => continue,
        };
        return Err(Error::Unsupported(format!("{path} is {what}; it can't be merged in a tool")));
    }
    Ok(MergeSides { base: c.ancestor.as_ref().map(|e| e.id), ours: ours.id, theirs: theirs.id })
}

/// Whether `path` is conflicted in the index as it is on disk now.
pub fn is_conflicted(workdir: &Path, path: &str) -> Result<bool> {
    let repo = Repository::open(workdir)?;
    match repo.index()?.conflict_get(Path::new(path)) {
        Ok(_) => Ok(true),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// The bytes of blob `id` for a copy of `path`. With `filters`, as a checkout would write
/// them (`git cat-file --filters`: line endings, ident, working-tree-encoding and filter
/// drivers such as Git LFS, from the user's and the repository's config); without, as stored.
pub fn blob_bytes(workdir: &Path, id: Oid, path: &str, filters: bool) -> Result<Vec<u8>> {
    if !filters {
        let repo = Repository::open(workdir)?;
        return Ok(repo.find_blob(id)?.content().to_vec());
    }
    let args = gitcli::args(&["cat-file", "--filters", &format!("--path={path}"), &id.to_string()]);
    let (out, bytes) = gitcli::output_bytes(workdir, &args, gitcli::NO_PROMPTS, Some(TOOL_FILE_TIMEOUT));
    if out.timed_out {
        return Err(Error::Other(format!("reading {path} from git took too long")));
    }
    if !out.ok {
        let first = out.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message");
        return Err(Error::Other(format!("git couldn't read {path}: {}", crate::text::printable(first))));
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "tool_sides_tests.rs"]
mod tests;
