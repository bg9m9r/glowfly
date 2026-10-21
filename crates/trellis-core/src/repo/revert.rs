//! Reverting part of a commit or comparison diff (a hunk, some of its lines, or the whole file)
//! in the working tree only. The inverse patch is applied where the changed lines still are;
//! when the file was edited since, the revert is merged with those edits instead
//! (`revert_merge`), and conflicts are written as ordinary conflict markers. A hunk shown with
//! whitespace changes hidden reverts only the changes shown (`shown_ws`). A diff from a commit
//! to the working tree reverts hunks and lines only, giving the working file the commit's
//! lines; it never merges (its working side has no stored version), so a file edited since is
//! refused as stale.

use super::diff::{hunk_of, target_diff};
use super::lines::{Patched, build_patch, images};
use super::revert_merge::{Labels, merge_into_workdir, splice};
use super::shown_ws::{changed, exact_context, raw_lines, side_lines};
use super::{DiffTarget, Hunk, LoadDiffOptions};
use crate::error::{Error, Result};
use git2::{ApplyLocation, Delta, DiffDelta, ErrorCode, FileMode, Patch, Repository};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// What part of a target's change to a file to revert.
#[derive(Clone, Debug, PartialEq)]
pub enum RevertPart {
    /// Hunk `index`, which must still be `expected`.
    Hunk { index: usize, expected: Hunk },
    /// Some lines of hunk `hunk`: `lines` index into its lines (as `load_diff` returns them),
    /// and only `+`/`-` lines count.
    Lines { hunk: usize, expected: Hunk, lines: Vec<usize> },
    /// The target's whole change to the file (content only: renames and modes stay).
    File,
}

/// A revert of part of a commit or comparison diff.
#[derive(Clone, Debug, PartialEq)]
pub struct RevertRequest {
    /// A `Commit`, `Between` or (hunks and lines only) `ToWorkdir` target.
    pub target: DiffTarget,
    pub part: RevertPart,
    /// The file's name in the working tree when it differs from `target.path()` (File history
    /// after a rename).
    pub dest: Option<String>,
    /// `part`'s hunk and line indexes count the diff with whitespace changes hidden; only the
    /// shown changes are reverted. Ignored for File.
    pub ignore_whitespace: bool,
}

/// How a revert went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reverted {
    /// The inverse patch applied as it was.
    Applied,
    /// The commit added the file and nothing else is left in it, so the file was removed.
    Removed,
    /// The compared commit doesn't have the file and nothing was left in it, so it was removed
    /// from the working tree (a commit-to-working-tree diff of an untracked or new file).
    RemovedNew,
    /// Merged cleanly with edits made since.
    Merged,
    /// Merged with edits made since, with conflict markers left in the file.
    Conflicts,
}

/// The refusal when the working copy already has the change undone.
pub const ALREADY_REVERTED: &str = "your working copy already has this change undone; nothing was changed";

/// Reverts `req.part` of `req.target` in the working-tree file (`req.dest`, or the target's
/// path); the index and the commits are untouched. A hunk or lines whose `expected` hunk git no
/// longer produces is refused as stale with `NotFound`.
pub fn revert(workdir: &Path, req: &RevertRequest) -> Result<Reverted> {
    let target = &req.target;
    if !target.reverts_hunks() {
        return Err(Error::Unsupported("only hunks of a commit or comparison diff can be reverted".into()));
    }
    if matches!(req.part, RevertPart::File) && !target.reverts_files() {
        return Err(Error::Unsupported("use Restore to bring back the commit's version of the whole file".into()));
    }
    let repo = Repository::open(workdir)?;
    let path = target.path();
    let work = req.dest.as_deref().unwrap_or(path);
    if let Some(dest) = &req.dest
        && super::exact::index(&repo)?.get_path(Path::new(dest), 0).is_none()
    {
        return Err(Error::NotFound(format!("{dest} is not tracked in the working tree")));
    }
    super::collide::refuse(workdir, work, "revert")?;
    super::attrs::refuse_converted(&repo, work, "reverting")?;
    // The same diff, renames detected, that the viewer shows, so the hunks match.
    let ws = req.ignore_whitespace && !matches!(req.part, RevertPart::File);
    let diff = target_diff(&repo, target, LoadDiffOptions { ignore_whitespace: ws, ..Default::default() }, None)?;
    let no_changes = || Error::NotFound(format!("no changes in {path}"));
    let position = |d: &git2::Diff| d.deltas().position(|d| super::rpath::touches(&d, path)).ok_or_else(no_changes);
    let idx = position(&diff)?;
    let delta = diff.get_delta(idx).ok_or_else(no_changes)?;
    if req.dest.is_some() && delta.status() == Delta::Deleted {
        return Err(Error::Unsupported("a file the commit deleted can't be brought back under another name".into()));
    }
    for m in [delta.old_file().mode(), delta.new_file().mode()] {
        match m {
            FileMode::Link => return Err(Error::Unsupported("symbolic links can't be reverted".into())),
            FileMode::Commit => return Err(Error::Unsupported("submodules can't be reverted".into())),
            _ => {}
        }
    }
    // libgit2's own spelling of the working tree, so `blob_path` sees the file as inside it and
    // runs the clean filters.
    let root = repo.workdir().ok_or_else(|| Error::Unsupported("a bare repository has no working tree".into()))?;
    let file = root.join(work);
    if file.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::Unsupported("symbolic links can't be reverted".into()));
    }
    let patch = Patch::from_diff(&diff, idx)?;
    // With whitespace hidden, the exact bytes of the commit's side, for the unchanged lines.
    let side = if ws {
        let full = target_diff(&repo, target, LoadDiffOptions::default(), None)?;
        Patch::from_diff(&full, position(&full)?)?.map(|p| side_lines(&p, true)).transpose()?
    } else {
        None
    };
    let to_workdir = matches!(target, DiffTarget::ToWorkdir(..));
    let ctx = Ctx { repo: &repo, work, file: &file, labels: Labels::of(target), to_workdir };
    let side = side.as_ref();
    match &req.part {
        RevertPart::Hunk { index, expected } => lines_part(&ctx, &delta, patch, *index, expected, None, side),
        RevertPart::Lines { hunk, expected, lines } => lines_part(&ctx, &delta, patch, *hunk, expected, Some(lines), side),
        RevertPart::File => super::revert_file::file_part(&ctx, &delta, patch),
    }
}

/// What every part of a revert works with.
pub(super) struct Ctx<'a> {
    pub repo: &'a Repository,
    /// The file's repository path in the working tree.
    pub work: &'a str,
    /// The working file.
    pub file: &'a Path,
    pub labels: Labels,
    /// The target is a commit compared with the working tree (`DiffTarget::ToWorkdir`).
    pub to_workdir: bool,
}

impl Ctx<'_> {
    /// The mode the file has now (so a later chmod is not undone), else the delta's new mode.
    pub fn mode(&self, delta: &DiffDelta<'_>) -> Result<u32> {
        let indexed = super::exact::index(self.repo)?.get_path(Path::new(self.work), 0).map(|e| e.mode);
        Ok(indexed.unwrap_or_else(|| raw(delta.new_file().mode())))
    }

    pub fn exists(&self) -> bool {
        self.file.symlink_metadata().is_ok()
    }

    /// Applies a patch built for the working file.
    pub fn apply(&self, bytes: &[u8]) -> std::result::Result<(), git2::Error> {
        self.repo.apply(&git2::Diff::from_buffer(bytes)?, ApplyLocation::WorkDir, None)
    }
}

pub(super) fn raw(m: FileMode) -> u32 {
    i32::from(m) as u32
}

pub(super) fn missing() -> Error {
    Error::NotFound("the file is not in the working tree (deleted or renamed since)".into())
}

pub(super) fn deleted_exists() -> Error {
    Error::Other("the file exists in the working tree, so the version this commit deleted can't be restored over it".into())
}

/// Reverts hunk `index`: every changed line, or those of `lines`. With `side` (the commit side's
/// exact lines, when the hunk was shown with whitespace changes hidden), unchanged lines take
/// their bytes from it.
fn lines_part(
    ctx: &Ctx,
    delta: &DiffDelta<'_>,
    patch: Option<Patch<'_>>,
    index: usize,
    expected: &Hunk,
    lines: Option<&Vec<usize>>,
    side: Option<&HashMap<u32, Vec<u8>>>,
) -> Result<Reverted> {
    let patch = patch.ok_or_else(|| Error::Unsupported("binary files can't be reverted by hunk".into()))?;
    let stale = || Error::NotFound("the diff changed; reopen it and try again".into());
    if index >= patch.num_hunks() || hunk_of(&patch, index)? != *expected {
        return Err(stale());
    }
    let (hunk, _) = patch.hunk(index)?;
    let shown = raw_lines(&patch, index)?;
    let selected: BTreeSet<usize> = match lines {
        None => changed(&shown),
        Some(l) => l.iter().copied().filter(|&i| shown.get(i).is_some_and(|r| matches!(r.origin, '+' | '-'))).collect(),
    };
    if selected.is_empty() {
        return Err(Error::NotFound("no changed lines selected".into()));
    }
    let (content, selected) = match side {
        Some(side) => exact_context(&shown, side, true, &selected)?,
        None => (shown.into_iter().map(|r| (r.origin, r.bytes)).collect(), selected),
    };
    let borrowed: Vec<(char, &[u8])> = content.iter().map(|(o, c)| (*o, c.as_slice())).collect();

    if delta.status() == Delta::Deleted {
        if ctx.exists() {
            return Err(deleted_exists());
        }
        let target = Patched::Create(raw(delta.old_file().mode()));
        if let Some(bytes) = build_patch(ctx.work, target, hunk.new_start(), &borrowed, &selected, true) {
            ctx.apply(&bytes)?;
        }
        return Ok(Reverted::Applied);
    }
    if !ctx.exists() {
        return Err(missing());
    }
    let mode = ctx.mode(delta)?;
    let added = matches!(delta.status(), Delta::Added | Delta::Untracked);

    // libgit2 only matches a hunk at its stated line, so find where the lines are now.
    let pre: Vec<&[u8]> = content.iter().filter(|(o, _)| matches!(o, ' ' | '+')).map(|(_, c)| strip_eol(c.as_slice())).collect();
    let start = if pre.is_empty() { Some(hunk.new_start()) } else { locate(&std::fs::read(ctx.file)?, &pre, hunk.new_start()) };
    if let Some(start) = start {
        let Some(bytes) = build_patch(ctx.work, Patched::Existing(mode), start, &borrowed, &selected, true) else {
            return Ok(Reverted::Applied);
        };
        match ctx.apply(&bytes) {
            Ok(()) if added && std::fs::metadata(ctx.file).is_ok_and(|m| m.len() == 0) => {
                crate::persist::remove_file(ctx.file)?;
                return Ok(if ctx.to_workdir { Reverted::RemovedNew } else { Reverted::Removed });
            }
            Ok(()) => return Ok(Reverted::Applied),
            Err(e) if e.code() == ErrorCode::ApplyFail => {}
            Err(e) => return Err(e.into()),
        }
    }

    // The working side of a working-tree comparison has no stored version to merge from.
    if ctx.to_workdir {
        return Err(stale());
    }
    // Edited since: merge the commit's version, reverted, with the working file.
    let base_id = delta.new_file().id();
    let base = ctx.repo.find_blob(base_id)?;
    let (pre, post) = images(&borrowed, &selected, true);
    let theirs = splice(base.content(), hunk.new_start(), &pre, &post).ok_or_else(stale)?;
    merge_into_workdir(ctx, mode, base_id, &theirs, added)
}

fn strip_eol(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\n").map(|l| l.strip_suffix(b"\r").unwrap_or(l)).unwrap_or(line)
}

/// The 1-based line where `pre` occurs in `file`, line for line and ignoring line endings; the
/// occurrence nearest `hint` wins, the earlier one on a tie.
fn locate(file: &[u8], pre: &[&[u8]], hint: u32) -> Option<u32> {
    let lines: Vec<&[u8]> = file.split_inclusive(|&b| b == b'\n').map(strip_eol).collect();
    if pre.len() > lines.len() {
        return None;
    }
    (0..=lines.len() - pre.len())
        .filter(|&i| lines[i..i + pre.len()] == *pre)
        .map(|i| i as u32 + 1)
        .min_by_key(|&start| start.abs_diff(hint))
}

#[cfg(test)]
mod tests {
    use super::locate;

    fn at(file: &str, pre: &[&str], hint: u32) -> Option<u32> {
        let pre: Vec<&[u8]> = pre.iter().map(|l| l.as_bytes()).collect();
        locate(file.as_bytes(), &pre, hint)
    }

    #[test]
    fn single_match() {
        assert_eq!(at("a\nb\nc\nd\n", &["b", "c"], 1), Some(2));
        assert_eq!(at("a\nb\nc", &["c"], 9), Some(3), "last line without a newline");
    }

    #[test]
    fn no_match() {
        assert_eq!(at("a\nb\nc\n", &["b", "d"], 2), None);
        assert_eq!(at("", &["a"], 1), None);
    }

    #[test]
    fn nearest_to_the_hint_wins() {
        let file = "x\ny\n-\n-\n-\n-\nx\ny\n";
        assert_eq!(at(file, &["x", "y"], 6), Some(7));
        assert_eq!(at(file, &["x", "y"], 2), Some(1));
        assert_eq!(at(file, &["x", "y"], 4), Some(1), "a tie goes to the earlier one");
    }

    #[test]
    fn crlf_file_matches_lf_lines() {
        assert_eq!(at("a\r\nb\r\nc\r\n", &["b", "c"], 1), Some(2));
    }

    #[test]
    fn pre_longer_than_the_file() {
        assert_eq!(at("a\n", &["a", "b"], 1), None);
    }
}
