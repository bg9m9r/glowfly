//! Copy hunk as patch: one hunk of a diff, read again from the repository, as a patch `git
//! apply` takes on the diff's old side, byte for byte (line endings, encoding, the file's mode
//! and, for a renamed or copied file, both its names).

use super::diff::{file_patch, hunk_of, target_diff};
use super::lines::{Names, Patched, split_eol, write_patch};
use super::shown_ws::{changed, exact_context, raw_lines, side_lines};
use super::{DiffTarget, Hunk, LoadDiffOptions};
use crate::error::{Error, Result};
use git2::{Delta, Repository};
use std::path::Path;

/// Hunk `hunk_index` of `target`'s diff as patch text: the hunk's own `@@` line and its lines,
/// context included, so `git apply` places it where it was cut. `expected` is the hunk as the
/// user saw it: unless git produces exactly that hunk now, it is refused as stale with
/// `NotFound`. With `ignore_whitespace` the hunks are counted in the diff shown with whitespace
/// changes hidden, and its unchanged lines are taken from the old side as they are, so the
/// patch still applies there (refused with `Unsupported` in the rare case that one differs from
/// the file by more than whitespace). The bytes are the file's own, whatever its encoding.
pub fn hunk_patch(workdir: &Path, target: &DiffTarget, hunk_index: usize, expected: &Hunk, ignore_whitespace: bool) -> Result<Vec<u8>> {
    let repo = Repository::open(workdir)?;
    let attrs = super::attr_source::for_target(&repo, target);
    let path = target.path();
    let diff = target_diff(&attrs, target, LoadDiffOptions { ignore_whitespace, ..Default::default() }, None)?;
    let stale = || Error::NotFound("the diff changed; try again on the updated diff".into());
    let patch = file_patch(&diff, path)?.ok_or_else(stale)?;
    if hunk_index >= patch.num_hunks() || hunk_of(&patch, hunk_index)? != *expected {
        return Err(stale());
    }
    let (hunk, _) = patch.hunk(hunk_index)?;
    let at = match std::str::from_utf8(hunk.header()) {
        Ok(h) => h.trim_end().to_owned(),
        Err(_) => format!("@@ -{},{} +{},{} @@", hunk.old_start(), hunk.old_lines(), hunk.new_start(), hunk.new_lines()),
    };
    let raw = raw_lines(&patch, hunk_index)?;
    let content = if ignore_whitespace {
        let full = target_diff(&attrs, target, LoadDiffOptions::default(), None)?;
        let full = file_patch(&full, path)?.ok_or_else(stale)?;
        exact_context(&raw, &side_lines(&full, false)?, false, &changed(&raw))?.0
    } else {
        raw.into_iter().map(|r| (r.origin, r.bytes)).collect()
    };
    let marker = |i: usize| content.get(i).is_some_and(|(o, _)| matches!(o, '=' | '<' | '>'));
    let body = content.iter().enumerate().filter(|(i, _)| !marker(*i)).map(|(i, (origin, bytes))| {
        let sign = if matches!(origin, '+' | '-') { *origin as u8 } else { b' ' };
        let (text, eol) = split_eol(bytes);
        // A "No newline at end of file" marker after it: the line has no newline.
        (sign, (text, if marker(i + 1) { &b""[..] } else { eol }))
    });

    let delta = patch.delta();
    let mode = |new: bool| i32::from(if new { delta.new_file() } else { delta.old_file() }.mode()) as u32;
    // An intent-to-add file's old side is empty too: its patch creates it.
    let empty = hunk.old_start() == 0 && hunk.old_lines() == 0;
    let to = match delta.status() {
        Delta::Added | Delta::Untracked => Patched::Create(mode(true)),
        _ if empty => Patched::Create(mode(true)),
        _ => Patched::Existing(mode(false)),
    };
    let old = delta.old_file().path_bytes().and_then(|b| std::str::from_utf8(b).ok());
    let moved = match delta.status() {
        Delta::Renamed => Some("rename"),
        Delta::Copied => Some("copy"),
        _ => None,
    };
    let names = match (moved, old) {
        (Some(how), Some(old)) if old != path => Names { old, new: path, moved: Some(how) },
        _ => Names::same(path),
    };
    Ok(write_patch(names, to, &at, body))
}

#[cfg(test)]
#[path = "hunk_copy_tests.rs"]
mod tests;
