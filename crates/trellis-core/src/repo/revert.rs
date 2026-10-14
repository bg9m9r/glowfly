//! Reverting a hunk of a commit or comparison diff: its inverse applied to the working tree only.

use super::diff::{file_patch, hunk_of, target_diff};
use super::lines::{Patched, build_patch};
use super::{DiffTarget, Hunk, LoadDiffOptions};
use crate::error::{Error, Result};
use git2::{ApplyLocation, Delta, Diff, ErrorCode, FileMode, Repository};
use std::collections::BTreeSet;
use std::path::Path;

/// Applies the inverse of hunk `hunk_index` of `target` (a commit or comparison diff, as
/// `load_diff` shows it) to the working-tree file; the index and the commits are untouched.
/// `expected` is the hunk as the user saw it: if git produces a different one now, the revert
/// is refused as stale with `NotFound`. The hunk's lines are looked for in the working file
/// near their original position, so lines inserted or removed above it since don't matter;
/// if they are no longer there exactly, nothing is written.
pub fn revert_hunk(workdir: &Path, target: &DiffTarget, hunk_index: usize, expected: &Hunk) -> Result<()> {
    if target.is_working_tree() {
        return Err(Error::Unsupported("only hunks of a commit or comparison diff can be reverted".into()));
    }
    let repo = Repository::open(workdir)?;
    let path = target.path();
    super::attrs::refuse_converted(&repo, path, "reverting")?;
    // The same unfiltered diff, renames detected, that the viewer shows, so the index matches.
    let diff = target_diff(&repo, target, LoadDiffOptions::default(), None)?;
    let patch = file_patch(&diff, path)?.ok_or_else(|| Error::Unsupported("binary files can't be reverted by hunk".into()))?;
    let stale = || Error::NotFound("the diff changed; reopen it and try again".into());
    if hunk_index >= patch.num_hunks() || hunk_of(&patch, hunk_index)? != *expected {
        return Err(stale());
    }
    let (hunk, count) = patch.hunk(hunk_index)?;
    let mut content = Vec::with_capacity(count);
    for l in 0..count {
        let line = patch.line_in_hunk(hunk_index, l)?;
        content.push((line.origin(), line.content().to_vec()));
    }
    let selected: BTreeSet<usize> = (0..content.len()).filter(|&i| matches!(content[i].0, '+' | '-')).collect();

    let delta = patch.delta();
    if delta.new_file().mode() == FileMode::Link || delta.old_file().mode() == FileMode::Link {
        return Err(Error::Unsupported("symbolic links can't be reverted by hunk".into()));
    }
    let file = workdir.join(path);
    let exists = file.symlink_metadata().is_ok();
    let raw = |m: FileMode| i32::from(m) as u32;
    let target_kind = if delta.status() == Delta::Deleted {
        if exists {
            return Err(Error::Other(
                "the file exists in the working tree, so the version this commit deleted can't be restored over it".into(),
            ));
        }
        Patched::Create(raw(delta.old_file().mode()))
    } else {
        if !exists {
            return Err(Error::NotFound("the file is not in the working tree (deleted or renamed since)".into()));
        }
        // The mode the file has now, so a later chmod is not undone.
        let mode = repo.index()?.get_path(Path::new(path), 0).map(|e| e.mode).unwrap_or_else(|| raw(delta.new_file().mode()));
        Patched::Existing(mode)
    };

    let no_match = || {
        Error::Other(
            "the working copy no longer has this hunk's lines as the commit left them (edited since, or already reverted); \
             nothing was changed"
                .into(),
        )
    };
    // libgit2 only matches a hunk at its stated line, so find where the lines are now.
    let pre: Vec<&[u8]> = content.iter().filter(|(o, _)| matches!(o, ' ' | '+')).map(|(_, c)| strip_eol(c.as_slice())).collect();
    let start =
        if pre.is_empty() { hunk.new_start() } else { locate(&std::fs::read(&file)?, &pre, hunk.new_start()).ok_or_else(no_match)? };

    let borrowed: Vec<(char, &[u8])> = content.iter().map(|(o, c)| (*o, c.as_slice())).collect();
    let Some(bytes) = build_patch(path, target_kind, start, &borrowed, &selected, true) else { return Ok(()) };
    let inverse = Diff::from_buffer(&bytes)?;
    match repo.apply(&inverse, ApplyLocation::WorkDir, None) {
        Err(e) if e.code() == ErrorCode::ApplyFail => Err(no_match()),
        r => Ok(r?),
    }
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
