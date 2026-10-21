//! Git LFS: whether git-lfs is installed, whether a repository is set up for it, the patterns
//! `.gitattributes` stores in LFS, checks for a new pattern and the LFS command lines.
//!
//! What a file row can track (the file, its extension or its folder) is in `track`; pointer
//! files are read in `pointer`.
//!
//! Reading the status never runs a git-lfs command that writes: `git lfs track` without
//! arguments, for example, quietly installs the hooks. Only `git lfs version` runs; the rest is
//! read from files and git config. The app runs the command lines through `gitcli` as jobs.

use crate::error::{Error, Result};
use crate::gitcli::{args, read};
use crate::i18n::Lang;
use std::path::{Path, PathBuf};

pub mod pointer;
mod track;
pub use track::{
    TrackTarget, check_target, extension, extension_pattern, folder_pattern, literal_glob, root_patterns, track_filename_args,
    track_targets,
};

/// A pattern `.gitattributes` stores in LFS.
#[derive(Clone, Debug, PartialEq)]
pub struct LfsPattern {
    /// As written in `.gitattributes`.
    pub raw: String,
    /// `raw` as typed: see [`unescape_pattern`].
    pub display: String,
    pub lockable: bool,
}

/// What the Git LFS page shows about a repository.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LfsStatus {
    /// git-lfs's version, None when it isn't installed.
    pub version: Option<String>,
    /// git-lfs is installed, its pre-push hook is in place and the LFS filters are configured.
    pub initialized: bool,
    pub patterns: Vec<LfsPattern>,
    pub has_gitattributes: bool,
    /// LFS hooks that exist but aren't git-lfs's, so setting up would stop at them.
    pub foreign_hooks: Vec<String>,
}

impl LfsStatus {
    pub fn installed(&self) -> bool {
        self.version.is_some()
    }
}

/// The version in `git lfs version` output (`git-lfs/3.7.1 (GitHub; ...)` gives `3.7.1`).
pub fn parse_version(out: &str) -> Option<String> {
    let rest = out.lines().next()?.trim().strip_prefix("git-lfs/")?;
    let version = rest.split_whitespace().next()?;
    (!version.is_empty()).then(|| version.to_owned())
}

/// git-lfs's version, or None when `git lfs` isn't available.
pub fn installed(cwd: &Path) -> Option<String> {
    let out = read(cwd, &args(&["lfs", "version"]));
    if out.ok { parse_version(&out.stdout) } else { None }
}

/// The hooks `git lfs install` writes.
pub const HOOKS: [&str; 4] = ["pre-push", "post-checkout", "post-commit", "post-merge"];

/// The repository's hooks folder (honours core.hooksPath).
fn hooks_dir(workdir: &Path) -> Result<PathBuf> {
    let out = read(workdir, &args(&["rev-parse", "--git-path", "hooks"]));
    if !out.ok {
        return Err(Error::Other(out.combined()));
    }
    let path = PathBuf::from(out.stdout.trim());
    Ok(if path.is_relative() { workdir.join(path) } else { path })
}

fn is_lfs_hook(text: &str) -> bool {
    text.contains("git lfs") || text.contains("git-lfs")
}

/// Whether `dir`'s pre-push hook runs git-lfs, and which of [`HOOKS`] exist without being
/// git-lfs's. Only reads files.
pub fn hook_state(dir: &Path) -> (bool, Vec<String>) {
    let mut pre_push = false;
    let mut foreign = Vec::new();
    for hook in HOOKS {
        let Ok(bytes) = std::fs::read(dir.join(hook)) else { continue };
        if is_lfs_hook(&String::from_utf8_lossy(&bytes)) {
            pre_push |= hook == "pre-push";
        } else {
            foreign.push(hook.to_owned());
        }
    }
    (pre_push, foreign)
}

/// Whether the LFS filter is configured (at any level: Git for Windows sets it system-wide).
fn filters_configured(workdir: &Path) -> bool {
    ["filter.lfs.process", "filter.lfs.smudge"].iter().any(|key| {
        let out = read(workdir, &args(&["config", "--get", key]));
        out.ok && !out.stdout.trim().is_empty()
    })
}

/// A typed pattern as git-lfs writes it to `.gitattributes`: a space becomes `[[:space:]]`
/// and `#` becomes `\#`.
pub fn escape_pattern(typed: &str) -> String {
    typed.replace(' ', "[[:space:]]").replace('#', r"\#")
}

/// The reverse of [`escape_pattern`]: the pattern as typed. git-lfs escapes the pattern it is
/// given before comparing, so this is also what `git lfs untrack` needs.
pub fn unescape_pattern(raw: &str) -> String {
    raw.replace("[[:space:]]", " ").replace(r"\#", "#")
}

/// Splits a `.gitattributes` line into its pattern and attribute tokens. A quoted pattern is
/// unescaped (backslash-quote and backslash-backslash).
fn split_line(line: &str) -> Option<(String, Vec<&str>)> {
    if let Some(quoted) = line.strip_prefix('"') {
        let mut pattern = String::new();
        let mut chars = quoted.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some((_, e @ ('"' | '\\'))) => pattern.push(e),
                    Some((_, e)) => {
                        pattern.push('\\');
                        pattern.push(e);
                    }
                    None => pattern.push('\\'),
                },
                '"' => return Some((pattern, quoted[i + 1..].split_whitespace().collect())),
                c => pattern.push(c),
            }
        }
        None
    } else {
        let mut tokens = line.split_whitespace();
        let pattern = tokens.next()?.to_owned();
        Some((pattern, tokens.collect()))
    }
}

/// The patterns stored in LFS by a `.gitattributes` file's text. A later line for the same
/// pattern that unsets or changes the filter removes it; a later `filter=lfs` line replaces the
/// earlier entry in place.
pub fn parse_attributes(text: &str) -> Vec<LfsPattern> {
    let mut patterns: Vec<LfsPattern> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("[attr]") {
            continue;
        }
        let Some((raw, attrs)) = split_line(line) else { continue };
        let mut lfs = None;
        for a in &attrs {
            if *a == "filter=lfs" {
                lfs = Some(true);
            } else if *a == "-filter" || *a == "!filter" || a.starts_with("filter=") {
                lfs = Some(false);
            }
        }
        let existing = patterns.iter().position(|p| p.raw == raw);
        match (lfs, existing) {
            (Some(true), at) => {
                let p = LfsPattern { display: unescape_pattern(&raw), lockable: attrs.contains(&"lockable"), raw };
                match at {
                    Some(i) => patterns[i] = p,
                    None => patterns.push(p),
                }
            }
            (Some(false), Some(i)) => {
                patterns.remove(i);
            }
            _ => {}
        }
    }
    patterns
}

/// The repository's Git LFS status. Writes nothing and runs no git-lfs command except
/// `git lfs version`.
pub fn status(workdir: &Path) -> Result<LfsStatus> {
    let (has_gitattributes, patterns) = match std::fs::read(workdir.join(".gitattributes")) {
        Ok(bytes) => (true, parse_attributes(&String::from_utf8_lossy(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (false, Vec::new()),
        Err(e) => return Err(Error::Io(e)),
    };
    let version = installed(workdir);
    let (pre_push, foreign_hooks) = hook_state(&hooks_dir(workdir)?);
    let initialized = version.is_some() && pre_push && filters_configured(workdir);
    Ok(LfsStatus { version, initialized, patterns, has_gitattributes, foreign_hooks })
}

/// An LFS command on the whole repository.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfsCommand {
    Fetch,
    Pull,
    Push,
    Prune,
    Checkout,
}

impl LfsCommand {
    pub const ALL: [Self; 5] = [Self::Fetch, Self::Pull, Self::Push, Self::Prune, Self::Checkout];

    /// The button text.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fetch => "Fetch",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::Prune => "Prune",
            Self::Checkout => "Checkout",
        }
    }

    /// The job label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Fetch => "LFS fetch",
            Self::Pull => "LFS pull",
            Self::Push => "LFS push",
            Self::Prune => "LFS prune",
            Self::Checkout => "LFS checkout",
        }
    }

    /// Whether the command talks to a remote.
    pub fn needs_remote(self) -> bool {
        matches!(self, Self::Fetch | Self::Pull | Self::Push)
    }

    /// The command line. Push needs `remote`; Fetch and Pull leave the choice to git-lfs.
    pub fn args(self, remote: Option<&str>) -> Vec<String> {
        match self {
            Self::Fetch => args(&["lfs", "fetch"]),
            Self::Pull => args(&["lfs", "pull"]),
            Self::Push => {
                let mut a = args(&["lfs", "push", "--all"]);
                a.extend(remote.map(str::to_owned));
                a
            }
            Self::Prune => args(&["lfs", "prune"]),
            Self::Checkout => args(&["lfs", "checkout"]),
        }
    }
}

/// Sets up LFS's hooks and filters for this repository only.
pub fn install_args() -> Vec<String> {
    args(&["lfs", "install", "--local"])
}

/// Tracks a typed pattern (git-lfs escapes spaces and '#' itself).
pub fn track_args(pattern: &str) -> Vec<String> {
    args(&["lfs", "track", "--", pattern])
}

/// Untracks a pattern as written in `.gitattributes` (git-lfs escapes it again itself).
pub fn untrack_args(raw: &str) -> Vec<String> {
    args(&["lfs", "untrack", "--", &unescape_pattern(raw)])
}

/// Git's own files, which LFS must never store.
pub(crate) const GIT_FILES: [&str; 3] = [".gitattributes", ".gitignore", ".lfsconfig"];

/// Why a new pattern can't be tracked.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternProblem {
    Empty,
    Control,
    Negated,
    /// Ends in a slash; holds the pattern.
    Folder(String),
    GitFile,
    AlreadyTracked(String),
}

impl PatternProblem {
    pub fn message(&self, t: Lang) -> String {
        match self {
            Self::Empty => t.tr("lfs.pattern.empty").to_owned(),
            Self::Control => t.tr("lfs.pattern.control").to_owned(),
            Self::Negated => t.tr("lfs.pattern.negated").to_owned(),
            Self::Folder(f) => t.trf("lfs.pattern.folder", &[("pattern", f)]),
            Self::GitFile => t.tr("lfs.pattern.git_file").to_owned(),
            Self::AlreadyTracked(p) => t.trf("lfs.pattern.already_tracked", &[("pattern", p)]),
        }
    }
}

/// Checks a typed pattern against the tracked ones; returns it trimmed.
pub fn check_pattern(input: &str, existing: &[LfsPattern]) -> Result<String, PatternProblem> {
    let p = input.trim();
    if p.is_empty() {
        return Err(PatternProblem::Empty);
    }
    if p.chars().any(char::is_control) {
        return Err(PatternProblem::Control);
    }
    if p.starts_with('!') {
        return Err(PatternProblem::Negated);
    }
    if p.ends_with('/') {
        return Err(PatternProblem::Folder(p.to_owned()));
    }
    if GIT_FILES.contains(&p) {
        return Err(PatternProblem::GitFile);
    }
    let escaped = escape_pattern(p);
    if existing.iter().any(|e| e.raw == escaped) {
        return Err(PatternProblem::AlreadyTracked(p.to_owned()));
    }
    Ok(p.to_owned())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod track_tests;
