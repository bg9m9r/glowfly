//! The three-way fallback of a revert: when the working file was edited since the commit, the
//! commit's version (base), the working file (ours) and the commit's version with the change
//! reversed (theirs) are merged, and the result is written to the working file only. Conflicts
//! become ordinary conflict markers in the file, never index conflicts: a revert leaves the
//! index alone, which is why this merges with libgit2 rather than `git apply --3way`.

use super::lines::{ImageLine, Patched, replace_patch};
use super::revert::{ALREADY_REVERTED, Ctx, Reverted};
use super::{DiffTarget, marker_size};
use crate::error::{Error, Result};
use git2::{ErrorCode, IndexEntry, IndexTime, MergeFileOptions, Oid};

/// The conflict-marker labels of a revert's merge.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Labels {
    pub base: String,
    pub theirs: String,
}

impl Labels {
    pub fn of(target: &DiffTarget) -> Self {
        let short = |o: &Oid| if super::is_empty_tree(*o) { "empty".to_owned() } else { o.to_string()[..7].to_owned() };
        match target {
            DiffTarget::Commit(oid, _) => Labels { base: short(oid), theirs: format!("revert of {}", short(oid)) },
            DiffTarget::Between(from, to, _) => Labels { base: short(to), theirs: format!("revert of {}..{}", short(from), short(to)) },
            // Never merged: `revert` refuses the working-tree diffs and FromWorkdir, and refuses
            // a ToWorkdir hunk as stale rather than merge it. Labelled to keep the match total.
            DiffTarget::Unstaged(_) | DiffTarget::Staged(_) | DiffTarget::ToWorkdir(..) | DiffTarget::FromWorkdir(..) => {
                Labels { base: "base".into(), theirs: "revert".into() }
            }
        }
    }
}

/// `base` with the `pre` lines at line `new_start` (1-based; for an empty `pre`, the line they
/// would follow) replaced by the `post` lines. `None` when `base` doesn't hold `pre` there,
/// line endings included.
pub(super) fn splice(base: &[u8], new_start: u32, pre: &[ImageLine], post: &[ImageLine]) -> Option<Vec<u8>> {
    let lines: Vec<&[u8]> = base.split_inclusive(|&b| b == b'\n').collect();
    let start = if pre.is_empty() { new_start as usize } else { (new_start as usize).checked_sub(1)? };
    let end = start + pre.len();
    if end > lines.len() {
        return None;
    }
    let same = |l: &[u8], &(text, eol): &ImageLine| l.len() == text.len() + eol.len() && l.starts_with(text) && l.ends_with(eol);
    if !lines[start..end].iter().zip(pre).all(|(l, p)| same(l, p)) {
        return None;
    }
    let mut out = lines[..start].concat();
    for (text, eol) in post {
        out.extend_from_slice(text);
        out.extend_from_slice(eol);
    }
    out.extend(lines[end..].concat());
    Some(out)
}

/// How far into a file a NUL byte makes the merge treat it as binary (as libgit2 does).
const BINARY_PROBE: usize = 8000;
/// The largest file libgit2's line merge takes; anything larger counts as binary.
const MERGE_MAX_BYTES: usize = 1024 * 1024 * 1023;

/// Whether libgit2 would merge `bytes` as a binary file. A binary merge yields no content at
/// all (not even conflict markers), so such inputs are refused before merging.
fn merges_as_binary(bytes: &[u8]) -> bool {
    bytes.len() > MERGE_MAX_BYTES || bytes[..bytes.len().min(BINARY_PROBE)].contains(&0)
}

/// Merges `theirs` (the blob `base` with the change reversed) into the working file and writes
/// the result through a whole-file patch, so the file keeps its line endings and `mode`. With
/// `remove_if_empty` (the commit added the file), a clean empty result removes the file.
pub(super) fn merge_into_workdir(ctx: &Ctx, mode: u32, base: Oid, theirs: &[u8], remove_if_empty: bool) -> Result<Reverted> {
    let repo = ctx.repo;
    // The working file in repository form (clean filters and EOL conversion applied).
    let ours_id = repo.blob_path(ctx.file)?;
    let ours_blob = repo.find_blob(ours_id)?;
    let ours = ours_blob.content();
    let base_blob = repo.find_blob(base)?;
    if [base_blob.content(), ours, theirs].into_iter().any(merges_as_binary) {
        return Err(Error::Unsupported(
            "the file is binary in the working tree or in the commit, so its changes can't be merged; use Restore instead".into(),
        ));
    }
    let theirs_id = repo.blob(theirs)?;
    let zero = IndexTime::new(0, 0);
    let entry = |id: Oid, size: usize| IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: u32::try_from(size).unwrap_or(u32::MAX),
        id,
        flags: 0,
        flags_extended: 0,
        path: ctx.work.as_bytes().to_vec(),
    };
    let texts = [base_blob.content(), ours, theirs].map(String::from_utf8_lossy);
    let n = marker_size(texts.iter().map(|t| t.as_ref()));
    let mut opts = MergeFileOptions::new();
    opts.style_standard(true)
        .marker_size(u16::try_from(n).unwrap_or(u16::MAX))
        .our_label("working copy")
        .their_label(ctx.labels.theirs.as_str())
        .ancestor_label(ctx.labels.base.as_str());
    let result = repo.merge_file_from_index(
        &entry(base, base_blob.size()),
        &entry(ours_id, ours.len()),
        &entry(theirs_id, theirs.len()),
        Some(&mut opts),
    )?;
    let merged = result.content();
    if merged == ours {
        return Err(Error::Other(ALREADY_REVERTED.into()));
    }
    let clean = result.is_automergeable();
    if clean && merged.is_empty() && remove_if_empty {
        crate::persist::remove_file(ctx.file)?;
        return Ok(Reverted::Removed);
    }
    let Some(bytes) = replace_patch(ctx.work, Patched::Existing(mode), ours, merged) else {
        return Err(Error::Other(ALREADY_REVERTED.into()));
    };
    match ctx.apply(&bytes) {
        Err(e) if e.code() == ErrorCode::ApplyFail => {
            return Err(Error::Other("the file changed while reverting; nothing was changed".into()));
        }
        r => r?,
    }
    Ok(if clean { Reverted::Merged } else { Reverted::Conflicts })
}

#[cfg(test)]
mod tests {
    use super::splice;
    use crate::repo::lines::ImageLine;

    fn img<'a>(lines: &[(&'a str, &'a str)]) -> Vec<ImageLine<'a>> {
        lines.iter().map(|(t, e)| (t.as_bytes(), e.as_bytes())).collect()
    }

    fn at(base: &str, start: u32, pre: &[(&str, &str)], post: &[(&str, &str)]) -> Option<String> {
        splice(base.as_bytes(), start, &img(pre), &img(post)).map(|b| String::from_utf8(b).unwrap())
    }

    #[test]
    fn at_the_start() {
        assert_eq!(at("a\nb\nc\n", 1, &[("a", "\n"), ("b", "\n")], &[("A", "\n")]).as_deref(), Some("A\nc\n"));
    }

    #[test]
    fn in_the_middle() {
        assert_eq!(at("a\nb\nc\nd\n", 2, &[("b", "\n")], &[("x", "\n"), ("y", "\n")]).as_deref(), Some("a\nx\ny\nc\nd\n"));
    }

    #[test]
    fn at_the_end_without_a_final_newline() {
        assert_eq!(at("a\nb", 2, &[("b", "")], &[("B", "")]).as_deref(), Some("a\nB"));
        assert_eq!(at("a\r\nb\r\n", 2, &[("b", "\r\n")], &[]).as_deref(), Some("a\r\n"));
    }

    #[test]
    fn empty_pre() {
        assert_eq!(at("", 0, &[], &[("a", "\n"), ("b", "\n")]).as_deref(), Some("a\nb\n"));
        assert_eq!(at("a\n", 1, &[], &[("b", "\n")]).as_deref(), Some("a\nb\n"), "after line 1");
    }

    #[test]
    fn a_mismatch_is_none() {
        assert_eq!(at("a\nb\n", 1, &[("x", "\n")], &[]), None);
        assert_eq!(at("a\r\nb\n", 1, &[("a", "\n")], &[]), None, "line endings count");
        assert_eq!(at("a\n", 1, &[("a", "\n"), ("b", "\n")], &[]), None, "past the end");
        assert_eq!(at("a\n", 0, &[("a", "\n")], &[]), None);
    }
}
