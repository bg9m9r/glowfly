//! Patch files: saving commits as one mbox-style patch with git format-patch, the command lines
//! that check and apply a patch to the working tree, and reading git apply's errors.
//!
//! The commands run through `gitcli`, so the user's git config applies and messages are
//! untranslated (the error parsing below relies on that).

use crate::error::{Error, Result};
use crate::gitcli::{self, args};
use git2::Oid;
use std::path::Path;
use std::time::Duration;

/// How long `git format-patch` may take for one commit.
pub const EXPORT_TIMEOUT: Duration = Duration::from_secs(120);

/// Why an export of commits that change nothing fails.
pub const NOTHING_TO_SAVE: &str = "None of the selected commits changes any file, so there is no patch to save.";

/// The most characters of a commit subject kept in a suggested file name.
const MAX_SLUG: usize = 50;

/// The suggested file name for a patch of `count` commits whose oldest has `subject`:
/// `<slug>.patch`, or `<slug>-and-<count - 1>-more.patch`.
pub fn file_name(subject: &str, count: usize) -> String {
    let slug = slug(subject);
    if count > 1 { format!("{slug}-and-{}-more.patch", count - 1) } else { format!("{slug}.patch") }
}

/// `subject` as a file name: letters, digits and '_' kept, every other run of characters one
/// '-', no '-' or '.' at either end, at most [`MAX_SLUG`] characters. Never empty, and never a
/// name Windows reserves for a device.
fn slug(subject: &str) -> String {
    let mut out = String::new();
    for c in subject.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trim = |s: &str| s.trim_matches(['-', '.']).to_owned();
    let cut: String = trim(&out).chars().take(MAX_SLUG).collect();
    let slug = trim(&cut);
    if slug.is_empty() {
        "commit".to_owned()
    } else if crate::names::win_name_problem(&slug).is_some() {
        // Only a device name can get this far (CON, COM¹...): the rest was mapped to '-'.
        format!("{slug}-commit")
    } else {
        slug
    }
}

/// The command that prints commit `oid` as a patch on stdout (binary diffs included, which
/// format-patch does by default). For a commit that changes no file it prints the message
/// alone, so [`export`] leaves such commits out before running it.
pub fn format_patch_cmd(oid: Oid) -> Vec<String> {
    args(&["format-patch", "-1", "--stdout", "--no-color", &oid.to_string()])
}

/// What an export wrote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportReport {
    /// Commits in the file.
    pub written: usize,
    /// Commits left out because they change no file.
    pub skipped: Vec<Oid>,
}

/// Saves `oids` (oldest first) as one patch file at `dest`, written atomically: each commit's
/// `git format-patch` output in turn, as raw bytes, so any text encoding and binary diffs
/// survive. Both `git apply` and `git am` read the result. Commits that change nothing (their
/// tree is their parent's) are left out; when that is all of them, nothing is written.
pub fn export(workdir: &Path, oids: &[Oid], dest: &Path) -> Result<ExportReport> {
    let repo = git2::Repository::open(workdir)?;
    let mut bytes = Vec::new();
    let mut report = ExportReport::default();
    for &oid in oids {
        if changes_nothing(&repo, oid)? {
            report.skipped.push(oid);
            continue;
        }
        let (out, part) = gitcli::output_bytes(workdir, &format_patch_cmd(oid), gitcli::NO_PROMPTS, Some(EXPORT_TIMEOUT));
        if !out.ok {
            let short = &oid.to_string()[..7];
            return Err(Error::Other(format!("git format-patch failed for {short}: {}", out.stderr.trim())));
        }
        // Nothing at all: a merge, or an older git that leaves empty commits out itself.
        if part.is_empty() {
            report.skipped.push(oid);
            continue;
        }
        // Each part starts its own "From <sha>" line, which must begin a line.
        if bytes.last().is_some_and(|b| *b != b'\n') {
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(&part);
        report.written += 1;
    }
    if report.written == 0 {
        return Err(Error::Other(NOTHING_TO_SAVE.to_owned()));
    }
    crate::persist::atomic_write(dest, &bytes)?;
    Ok(report)
}

/// Whether commit `oid` leaves its first parent's tree as it is (a root commit: has no files).
fn changes_nothing(repo: &git2::Repository, oid: Oid) -> Result<bool> {
    let commit = repo.find_commit(oid)?;
    Ok(match commit.parent_ids().next() {
        Some(parent) => repo.find_commit(parent)?.tree_id() == commit.tree_id(),
        None => commit.tree()?.is_empty(),
    })
}

/// The commands that apply the patch file at `patch` to the working tree (not the index):
/// a check first, so nothing is written when any part doesn't apply. `git apply` without
/// `--reject` is all-or-nothing anyway; the check makes the failure explicit.
pub fn apply_cmds(patch: &str) -> Vec<Vec<String>> {
    vec![args(&["apply", "--check", patch]), args(&["apply", patch])]
}

/// One reason `git apply` refused a patch.
#[derive(Clone, Debug, PartialEq)]
pub enum ApplyProblem {
    /// The file's lines around `line` no longer match a hunk.
    Hunk { file: String, line: u32 },
    /// The file doesn't match the patch (git's summary after the hunk problems).
    Mismatch { file: String },
    /// The patch changes a file the working tree doesn't have.
    Missing { file: String },
    /// The patch creates a file that already exists.
    Exists { file: String },
    /// The input holds no diff at all (empty, or not a patch).
    NoPatch,
    /// The patch file is malformed at `line`.
    Corrupt { line: u32 },
    /// Any other error, as git wrote it.
    Other(String),
}

impl ApplyProblem {
    /// A phrase for a sentence such as "x.patch doesn't apply: <phrase>."
    pub fn describe(&self) -> String {
        match self {
            ApplyProblem::Hunk { file, line } => format!("{file} has changed where the hunk at line {line} goes"),
            ApplyProblem::Mismatch { file } => format!("{file} doesn't match the patch"),
            ApplyProblem::Missing { file } => format!("{file} isn't in the working tree"),
            ApplyProblem::Exists { file } => format!("{file} already exists"),
            ApplyProblem::NoPatch => "the file holds no changes git can apply".to_owned(),
            ApplyProblem::Corrupt { line } => format!("the patch is damaged at line {line}"),
            ApplyProblem::Other(text) => text.clone(),
        }
    }
}

/// The problems in `git apply`'s output, in order and without duplicates: only its `error:`
/// lines are read (command echoes and warnings are skipped). A file's "patch does not apply" is
/// dropped when a hunk problem already names that file.
pub fn apply_problems(output: &str) -> Vec<ApplyProblem> {
    let mut found: Vec<ApplyProblem> = Vec::new();
    for problem in output.lines().filter_map(|l| l.trim_end().strip_prefix("error: ")).map(parse_problem) {
        if !found.contains(&problem) {
            found.push(problem);
        }
    }
    let hunk_files: Vec<String> =
        found.iter().filter_map(|p| if let ApplyProblem::Hunk { file, .. } = p { Some(file.clone()) } else { None }).collect();
    found.retain(|p| !matches!(p, ApplyProblem::Mismatch { file } if hunk_files.contains(file)));
    found
}

/// One `error:` line's text (after the prefix) as a problem.
fn parse_problem(text: &str) -> ApplyProblem {
    let other = || ApplyProblem::Other(text.to_owned());
    if let Some(rest) = text.strip_prefix("patch failed: ") {
        return match rest.rsplit_once(':').map(|(f, l)| (f, l.trim().parse::<u32>())) {
            Some((file, Ok(line))) => ApplyProblem::Hunk { file: file.to_owned(), line },
            _ => other(),
        };
    }
    if let Some(file) = text.strip_suffix(": patch does not apply") {
        return ApplyProblem::Mismatch { file: file.to_owned() };
    }
    if let Some(file) = text.strip_suffix(": No such file or directory") {
        return ApplyProblem::Missing { file: file.to_owned() };
    }
    if let Some(file) = text.strip_suffix(": already exists in working directory") {
        return ApplyProblem::Exists { file: file.to_owned() };
    }
    if text.starts_with("No valid patches in input") {
        return ApplyProblem::NoPatch;
    }
    if let Some(line) = text.strip_prefix("corrupt patch at line ").and_then(|l| l.trim().parse().ok()) {
        return ApplyProblem::Corrupt { line };
    }
    other()
}

/// The first problem in `output`, plus how many more there are; None when it has no `error:`
/// line.
pub fn failure_summary(output: &str) -> Option<String> {
    let problems = apply_problems(output);
    let first = problems.first()?.describe();
    Some(match problems.len() - 1 {
        0 => first,
        more => format!("{first} (and {more} more; see the log)"),
    })
}

#[cfg(test)]
mod tests;
