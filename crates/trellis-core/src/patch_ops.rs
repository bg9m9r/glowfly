//! Patch files: saving commits (whole, or limited to some paths) as one mbox-style patch with
//! git format-patch, saving uncommitted changes with git diff (`wip.rs`), reading what a patch
//! file holds (`inspect.rs`), the command lines that apply one to the working tree with the
//! chosen options and reading git apply's errors (`apply.rs`), and applying a series as commits
//! with git am (`am.rs`).
//!
//! The commands run through `gitcli`, so the user's git config applies and messages are
//! untranslated (the error parsing relies on that).

mod am;
mod apply;
mod inspect;
mod wip;

pub use am::{AM_ABORT, AM_CONTINUE, AM_SKIP, AmProgress, AmStop, am_cmd, am_stop};
pub(crate) use am::{current_subject, progress_of};
pub use apply::{ApplyOptions, ApplyProblem, OnFail, apply_cmds, apply_problems, failure_summary, rej_name, rejects, three_way_conflicts};
pub use inspect::{MAX_INSPECT, PatchSummary, inspect, summarize};
pub use wip::{ChangesRequest, EMPTY_TREE, MAX_UNTRACKED, NO_CHANGES, WipScope, diff_cmd, untracked_cmd};

use crate::error::{Error, Result};
use crate::gitcli::{self, args};
use git2::Oid;
use std::path::Path;
use std::time::Duration;

/// How long `git format-patch` (or one `git diff`) may take.
pub const EXPORT_TIMEOUT: Duration = Duration::from_secs(120);

/// Why an export of commits that change nothing fails.
pub const NOTHING_TO_SAVE: &str = "None of the selected commits changes any file, so there is no patch to save.";
/// Why copying the patch of commits that change nothing fails.
pub const NOTHING_TO_COPY: &str = "None of the selected commits changes any file, so there is no patch to copy.";
/// Why saving one file of a commit fails when the commit leaves that file alone.
pub const FILE_UNCHANGED: &str = "The commit doesn't change that file, so there is no patch to save.";
/// The largest patch put on the clipboard.
pub const MAX_COPY: usize = 4 << 20;
pub const TOO_LARGE_TO_COPY: &str = "The patch is larger than 4 MB; save it to a file instead.";
pub const NOT_TEXT: &str = "The patch isn't UTF-8 text; save it to a file instead.";

/// The most characters of a commit subject kept in a suggested file name.
const MAX_SLUG: usize = 50;

/// The suggested file name for a patch of `count` commits whose oldest has `subject`:
/// `<slug>.patch`, or `<slug>-and-<count - 1>-more.patch`.
pub fn file_name(subject: &str, count: usize) -> String {
    let slug = slug(subject);
    if count > 1 { format!("{slug}-and-{}-more.patch", count - 1) } else { format!("{slug}.patch") }
}

/// The suggested file name for a patch of `paths` of the commit with `subject`:
/// `<subject>-<file>.patch` for one path (cut to [`MAX_SLUG`] characters), else
/// `<subject>-<n>-files.patch`.
pub fn files_file_name(subject: &str, paths: &[String]) -> String {
    match paths {
        [one] => format!("{}.patch", cut(&format!("{}-{}", slug(subject), slug(basename(one))))),
        _ => format!("{}-{}-files.patch", slug(subject), paths.len()),
    }
}

/// The suggested file name for a patch of uncommitted changes: `<file>-changes.patch` for one
/// path, `<branch>-changes.patch` on a branch, else `uncommitted-changes.patch`.
pub fn changes_file_name(branch: Option<&str>, paths: &[String]) -> String {
    match (paths, branch) {
        ([one], _) => format!("{}-changes.patch", slug(basename(one))),
        (_, Some(b)) => format!("{}-changes.patch", slug(b)),
        _ => "uncommitted-changes.patch".to_owned(),
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `subject` as a file name: letters, digits and '_' kept, every other run of characters one
/// '-', no '-' or '.' at either end, at most [`MAX_SLUG`] characters. Never empty, and never a
/// name Windows reserves for a device.
pub(crate) fn slug(subject: &str) -> String {
    let mut out = String::new();
    for c in subject.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let slug = cut(&out);
    if slug.is_empty() {
        "commit".to_owned()
    } else if crate::names::win_name_problem(&slug).is_some() {
        // Only a device name can get this far (CON, COM¹...): the rest was mapped to '-'.
        format!("{slug}-commit")
    } else {
        slug
    }
}

/// `s` without '-' or '.' at either end, cut to [`MAX_SLUG`] characters (on a char boundary).
fn cut(s: &str) -> String {
    let trim = |s: &str| s.trim_matches(['-', '.']).to_owned();
    let cut: String = trim(s).chars().take(MAX_SLUG).collect();
    trim(&cut)
}

/// The command that prints commit `oid` as a patch on stdout (binary diffs included, which
/// format-patch does by default), limited to `paths` when there are any. For a commit that
/// changes no file it prints the message alone, so such commits are left out before running
/// it. The explicit prefixes keep a `diff.noprefix` from making a patch git apply rejects.
pub fn format_patch_cmd(oid: Oid, paths: &[String]) -> Vec<String> {
    let sha = oid.to_string();
    let mut cmd =
        args(&["--literal-pathspecs", "format-patch", "-1", "--stdout", "--no-color", "--src-prefix=a/", "--dst-prefix=b/", &sha]);
    if !paths.is_empty() {
        cmd.push("--".into());
        cmd.extend(paths.iter().cloned());
    }
    cmd
}

/// What a patch file is made from.
#[derive(Clone, Debug, PartialEq)]
pub enum PatchSource {
    /// Commits (oldest first); `paths` empty means the whole of each commit.
    Commits { oids: Vec<Oid>, paths: Vec<String> },
    /// Uncommitted changes.
    Changes(ChangesRequest),
}

/// What a save wrote.
#[derive(Clone, Debug, PartialEq)]
pub enum SaveReport {
    Commits(ExportReport),
    /// Uncommitted changes to `files` files.
    Changes {
        files: usize,
    },
}

/// What an export of commits wrote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportReport {
    /// Commits in the file.
    pub written: usize,
    /// Commits left out because they change no file.
    pub skipped: Vec<Oid>,
}

/// The bytes of the patch `src` describes, and what they hold.
pub fn build(workdir: &Path, src: &PatchSource) -> Result<(Vec<u8>, SaveReport)> {
    match src {
        PatchSource::Commits { oids, paths } => {
            let (bytes, report) = commit_parts(workdir, oids, paths)?;
            if report.written == 0 {
                return Err(Error::Other(NOTHING_TO_SAVE.to_owned()));
            }
            Ok((bytes, SaveReport::Commits(report)))
        }
        PatchSource::Changes(req) => {
            let (bytes, files) = wip::build_changes(workdir, req)?;
            Ok((bytes, SaveReport::Changes { files }))
        }
    }
}

/// Saves the patch `src` describes at `dest`, written atomically; nothing is written when it
/// can't be built.
pub fn save(workdir: &Path, src: &PatchSource, dest: &Path) -> Result<SaveReport> {
    let (bytes, report) = build(workdir, src)?;
    crate::persist::atomic_write(dest, &bytes)?;
    Ok(report)
}

/// Saves `oids` (oldest first) as one patch file at `dest`, whole commits.
pub fn export(workdir: &Path, oids: &[Oid], dest: &Path) -> Result<ExportReport> {
    match save(workdir, &PatchSource::Commits { oids: oids.to_vec(), paths: Vec::new() }, dest)? {
        SaveReport::Commits(r) => Ok(r),
        SaveReport::Changes { .. } => unreachable!("a commit export reports commits"),
    }
}

/// The patch of `oids` (oldest first) as text for the clipboard, refused when it is larger
/// than [`MAX_COPY`] or isn't UTF-8.
pub fn patch_text(workdir: &Path, oids: &[Oid]) -> Result<(String, ExportReport)> {
    let (bytes, report) = commit_parts(workdir, oids, &[])?;
    if report.written == 0 {
        return Err(Error::Other(NOTHING_TO_COPY.to_owned()));
    }
    if !fits_clipboard(bytes.len()) {
        return Err(Error::Other(TOO_LARGE_TO_COPY.to_owned()));
    }
    let text = String::from_utf8(bytes).map_err(|_| Error::Other(NOT_TEXT.to_owned()))?;
    Ok((text, report))
}

/// Whether a patch of `len` bytes may go on the clipboard.
fn fits_clipboard(len: usize) -> bool {
    len <= MAX_COPY
}

/// Each commit's `git format-patch` output in turn (limited to `paths` when there are any), as
/// raw bytes, so any text encoding and binary diffs survive. Both `git apply` and `git am` read
/// the result. Commits that change nothing (their tree is their parent's) are left out; with
/// `paths`, a commit that leaves them all alone is an error ([`FILE_UNCHANGED`]).
fn commit_parts(workdir: &Path, oids: &[Oid], paths: &[String]) -> Result<(Vec<u8>, ExportReport)> {
    let repo = git2::Repository::open(workdir)?;
    let mut bytes = Vec::new();
    let mut report = ExportReport::default();
    for &oid in oids {
        if !paths.is_empty() && !changes_paths(&repo, oid, paths)? {
            return Err(Error::Other(FILE_UNCHANGED.to_owned()));
        }
        if changes_nothing(&repo, oid)? {
            report.skipped.push(oid);
            continue;
        }
        let (out, part) = gitcli::output_bytes(workdir, &format_patch_cmd(oid, paths), gitcli::NO_PROMPTS, Some(EXPORT_TIMEOUT));
        if !out.ok {
            let short = &oid.to_string()[..7];
            return Err(Error::Other(format!("git format-patch failed for {short}: {}", out.stderr.trim())));
        }
        // Nothing at all: a merge, or an older git that leaves empty commits out itself.
        if part.is_empty() {
            report.skipped.push(oid);
            continue;
        }
        // With paths, git's walk could stop at an older commit that touches them instead.
        if !paths.is_empty() && (!part.starts_with(format!("From {oid} ").as_bytes()) || !has_diff(&part)) {
            return Err(Error::Other(FILE_UNCHANGED.to_owned()));
        }
        // Each part starts its own "From <sha>" line, which must begin a line.
        if bytes.last().is_some_and(|b| *b != b'\n') {
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(&part);
        report.written += 1;
    }
    Ok((bytes, report))
}

/// Whether `part` holds a `diff --git` line.
fn has_diff(part: &[u8]) -> bool {
    const LINE: &[u8] = b"\ndiff --git ";
    part.windows(LINE.len()).any(|w| w == LINE)
}

/// Whether commit `oid` leaves its first parent's tree as it is (a root commit: has no files).
fn changes_nothing(repo: &git2::Repository, oid: Oid) -> Result<bool> {
    let commit = repo.find_commit(oid)?;
    Ok(match commit.parent_ids().next() {
        Some(parent) => repo.find_commit(parent)?.tree_id() == commit.tree_id(),
        None => commit.tree()?.is_empty(),
    })
}

/// Whether commit `oid` changes any of `paths` (taken literally) against its first parent.
fn changes_paths(repo: &git2::Repository, oid: Oid, paths: &[String]) -> Result<bool> {
    let commit = repo.find_commit(oid)?;
    let parent = match commit.parent_ids().next() {
        Some(p) => Some(repo.find_commit(p)?.tree()?),
        None => None,
    };
    let mut opts = git2::DiffOptions::new();
    opts.disable_pathspec_match(true);
    for p in paths {
        opts.pathspec(p);
    }
    let diff = repo.diff_tree_to_tree(parent.as_ref(), Some(&commit.tree()?), Some(&mut opts))?;
    Ok(diff.deltas().len() > 0)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod files_tests;
