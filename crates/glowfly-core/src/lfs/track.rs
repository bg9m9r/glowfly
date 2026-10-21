//! What a file row can track in Git LFS, and the `.gitattributes` pattern each choice writes:
//!
//! - the file itself, through `git lfs track --filename`, which escapes glob characters in the
//!   name with backslashes;
//! - every file with its extension (`*.psd`);
//! - everything in its folder, subfolders included (`assets/tex/**`).
//!
//! In the extension and folder patterns, glob characters taken from the name are made literal
//! with character classes (`[` becomes `[[]`, `*` becomes `[*]`, `?` becomes `[?]`). Backslashes
//! would not survive: in pattern mode git-lfs turns `\` into `/`. A character class is written
//! unchanged, matches in `git check-attr`, and `git lfs untrack` removes it again. Spaces and
//! `#` are left for git-lfs to escape, as for a typed pattern.

use super::{GIT_FILES, LfsPattern, PatternProblem, escape_pattern, parse_attributes, track_args};
use crate::gitcli::args;
use std::path::Path;

/// Tracks one file by name; `--` keeps a name starting with '-' from reading as an option.
pub fn track_filename_args(path: &str) -> Vec<String> {
    args(&["lfs", "track", "--filename", "--", path])
}

/// `s` with its glob characters made literal through character classes.
pub fn literal_glob(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '[' => out.push_str("[[]"),
            '*' => out.push_str("[*]"),
            '?' => out.push_str("[?]"),
            c => out.push(c),
        }
    }
    out
}

/// What `--filename` writes for `path` before git-lfs escapes spaces and '#': a backslash before
/// each glob character.
fn filename_raw_globs(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '*' | '?' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn base_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// The text after the last '.' of the file name, None without one (`README`, `.env`, `a.`).
pub fn extension(path: &str) -> Option<&str> {
    let (stem, ext) = base_name(path).rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then_some(ext)
}

/// `*.<ext>` for the file's extension.
pub fn extension_pattern(path: &str) -> Option<String> {
    extension(path).map(|ext| format!("*.{}", literal_glob(ext)))
}

/// `<folder>/**` for the file's folder; None at the repository root, where `**` would also catch
/// `.gitattributes`.
pub fn folder_pattern(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(folder, _)| format!("{}/**", literal_glob(folder)))
}

/// One way to track a file row in LFS.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackTarget {
    /// The repository-relative path (with '/').
    File(String),
    /// The extension's pattern, as typed (glob characters already literal).
    Extension(String),
    /// The folder's pattern, as typed (glob characters already literal).
    Folder(String),
}

impl TrackTarget {
    pub fn args(&self) -> Vec<String> {
        match self {
            Self::File(p) => track_filename_args(p),
            Self::Extension(p) | Self::Folder(p) => track_args(p),
        }
    }

    /// The path or the pattern.
    pub fn display(&self) -> &str {
        match self {
            Self::File(p) | Self::Extension(p) | Self::Folder(p) => p,
        }
    }

    /// The pattern as it will appear in `.gitattributes`.
    pub fn raw(&self) -> String {
        match self {
            Self::File(p) => escape_pattern(&filename_raw_globs(p)),
            Self::Extension(p) | Self::Folder(p) => escape_pattern(p),
        }
    }
}

/// The file, then its extension and its folder when it has them.
pub fn track_targets(path: &str) -> Vec<TrackTarget> {
    let mut out = vec![TrackTarget::File(path.to_owned())];
    out.extend(extension_pattern(path).map(TrackTarget::Extension));
    out.extend(folder_pattern(path).map(TrackTarget::Folder));
    out
}

/// Whether `t` can be tracked, given the patterns already tracked.
pub fn check_target(t: &TrackTarget, existing: &[LfsPattern]) -> Result<(), PatternProblem> {
    let text = t.display();
    if text.chars().any(char::is_control) {
        return Err(PatternProblem::Control);
    }
    let git_file = match t {
        TrackTarget::File(p) => GIT_FILES.contains(&base_name(p)),
        TrackTarget::Extension(p) => GIT_FILES.iter().any(|g| p.strip_prefix('*') == Some(g)),
        TrackTarget::Folder(_) => false,
    };
    if git_file {
        return Err(PatternProblem::GitFile);
    }
    if text.starts_with('!') {
        return Err(PatternProblem::Negated);
    }
    let raw = t.raw();
    if existing.iter().any(|e| e.raw == raw) {
        return Err(PatternProblem::AlreadyTracked(text.to_owned()));
    }
    Ok(())
}

/// The LFS patterns of the root `.gitattributes`; empty without one. Only reads the file.
pub fn root_patterns(workdir: &Path) -> Vec<LfsPattern> {
    std::fs::read(workdir.join(".gitattributes")).map(|b| parse_attributes(&String::from_utf8_lossy(&b))).unwrap_or_default()
}
