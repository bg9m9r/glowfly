//! Sparse checkout: the worktree's state (on or off, folders or patterns, the paths, how many
//! tracked files are left out), checks of the paths a user typed, and the `git sparse-checkout`
//! command lines. The app runs them through gitcli.

use crate::error::{Error, Result};
use crate::gitcli::{self, args};
use git2::{IndexEntryExtendedFlag, Repository};
use std::path::Path;

/// How git matches the sparse paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SparseMode {
    /// Whole folders (git's cone mode): fast, and files at the root are always checked out.
    #[default]
    Cone,
    /// gitignore-style lines.
    Patterns,
}

impl SparseMode {
    /// The `git sparse-checkout set` flag for this mode.
    pub fn flag(self) -> &'static str {
        match self {
            SparseMode::Cone => "--cone",
            SparseMode::Patterns => "--no-cone",
        }
    }
}

/// Sparse checkout in one worktree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SparseStatus {
    pub enabled: bool,
    pub mode: SparseMode,
    /// What `git sparse-checkout list` prints (unquoted); empty when off.
    pub paths: Vec<String>,
    /// Cone mode is configured but the file holds lines it can't read, so git matches patterns.
    pub fell_back: bool,
    /// A sparse index (`index.sparse`) is in use: the counts below miss collapsed folders.
    pub sparse_index: bool,
    /// Index entries.
    pub tracked: usize,
    /// Index entries left out of the working tree (skip-worktree).
    pub skipped: usize,
}

/// Reads the sparse checkout state of the worktree at `workdir`.
pub fn status(workdir: &Path) -> Result<SparseStatus> {
    let repo = Repository::open(workdir)?;
    // libgit2's config includes the worktree's config.worktree, where git keeps these.
    let config = repo.config()?;
    let enabled = config.get_bool("core.sparseCheckout").unwrap_or(false);
    // Unset means patterns: that is how git reads the file.
    let cone_cfg = config.get_bool("core.sparseCheckoutCone").unwrap_or(false);
    let sparse_index = config.get_bool("index.sparse").unwrap_or(false);
    let index = repo.index()?;
    let skip = IndexEntryExtendedFlag::SKIP_WORKTREE.bits();
    let tracked = index.len();
    let skipped = index.iter().filter(|e| e.flags_extended & skip != 0).count();
    let mut s = SparseStatus { enabled, sparse_index, tracked, skipped, ..SparseStatus::default() };
    if !enabled {
        return Ok(s);
    }
    let out = gitcli::read(workdir, &args(&["-c", "core.quotePath=false", "sparse-checkout", "list"]));
    if !out.ok {
        let line = out.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git sparse-checkout list failed");
        return Err(Error::Other(line.to_owned()));
    }
    s.fell_back = cone_cfg && out.stderr.contains("disabling cone pattern matching");
    s.mode = if cone_cfg && !s.fell_back { SparseMode::Cone } else { SparseMode::Patterns };
    let cone = s.mode == SparseMode::Cone;
    s.paths = out
        .stdout
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.is_empty())
        .map(|l| if cone { unquote(l) } else { l.to_owned() })
        .collect();
    Ok(s)
}

/// A `git sparse-checkout` job.
#[derive(Clone, Debug, PartialEq)]
pub enum SparseCommand {
    /// Turn it on, or replace the paths; the paths go on stdin.
    Set {
        mode: SparseMode,
        paths: Vec<String>,
    },
    Reapply,
    Disable,
}

impl SparseCommand {
    /// The arguments after `git`. Never creates a sparse index, which libgit2 can't read.
    pub fn args(&self) -> Vec<String> {
        match self {
            SparseCommand::Set { mode, .. } => args(&["sparse-checkout", "set", mode.flag(), "--no-sparse-index", "--stdin"]),
            SparseCommand::Reapply => args(&["sparse-checkout", "reapply"]),
            SparseCommand::Disable => args(&["sparse-checkout", "disable"]),
        }
    }

    /// What goes on stdin: one path per line for Set.
    pub fn input(&self) -> String {
        match self {
            SparseCommand::Set { paths, .. } => paths.iter().map(|p| format!("{p}\n")).collect(),
            SparseCommand::Reapply | SparseCommand::Disable => String::new(),
        }
    }
}

/// Runs `command` in `cwd` (credential prompts allowed: a partial clone may download). Returns
/// success and the job log text: the command line, the paths sent, then git's output.
pub fn run(cwd: &Path, command: &SparseCommand) -> (bool, String) {
    let a = command.args();
    let (ok, out) = gitcli::run_with_input(cwd, &a, &command.input(), &[]);
    let mut text = format!("$ git {}", a.join(" "));
    if let SparseCommand::Set { mode, paths } = command {
        if paths.is_empty() && *mode == SparseMode::Cone {
            text.push_str("\n  (no paths: only files at the root)");
        }
        for p in paths {
            text.push_str(&format!("\n  {p}"));
        }
    }
    let out = out.trim();
    if !out.is_empty() {
        text.push('\n');
        text.push_str(out);
    }
    (ok, text)
}

/// Why typed paths can't be used.
#[derive(Clone, Debug, PartialEq)]
pub enum PathProblem {
    /// A folders-mode line that looks like a pattern.
    Pattern(String),
    /// A line that names something outside the repository.
    Outside(String),
    /// Patterns mode with nothing to match.
    NoPatterns,
}

impl PathProblem {
    pub fn message(&self) -> String {
        match self {
            PathProblem::Pattern(l) => format!("\"{l}\" looks like a pattern; switch the mode to Patterns to use it."),
            PathProblem::Outside(l) => format!("\"{l}\" leaves the repository; use folders relative to its root."),
            PathProblem::NoPatterns => "With no patterns git would remove every file from the working tree. Add at least one.".into(),
        }
    }
}

/// The paths in the form's `text`, one per line, checked for `mode` (see [`PathProblem`]).
/// Folders are cleaned up (slashes, `./`, duplicates); patterns are kept as typed.
pub fn parse_paths(text: &str, mode: SparseMode) -> std::result::Result<Vec<String>, PathProblem> {
    match mode {
        SparseMode::Patterns => {
            let lines: Vec<String> = text.lines().map(|l| l.trim_end().to_owned()).filter(|l| !l.trim().is_empty()).collect();
            if lines.is_empty() { Err(PathProblem::NoPatterns) } else { Ok(lines) }
        }
        SparseMode::Cone => {
            let mut out: Vec<String> = Vec::new();
            for raw in text.lines() {
                let line = raw.trim();
                if line.is_empty() {
                    continue;
                }
                if line.starts_with('!') || line.contains(['*', '?', '[']) {
                    return Err(PathProblem::Pattern(line.to_owned()));
                }
                let slashed = line.replace('\\', "/");
                let bytes = slashed.as_bytes();
                if (bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()) || slashed.starts_with("//") {
                    return Err(PathProblem::Outside(line.to_owned()));
                }
                let mut p = slashed.as_str();
                loop {
                    let before = p;
                    p = p.trim_start_matches('/');
                    p = p.strip_prefix("./").unwrap_or(p);
                    if p == before {
                        break;
                    }
                }
                let p = p.trim_end_matches('/');
                if p.is_empty() || p.split('/').any(|c| c == "." || c == "..") {
                    return Err(PathProblem::Outside(line.to_owned()));
                }
                if !out.iter().any(|o| o == p) {
                    out.push(p.to_owned());
                }
            }
            Ok(out)
        }
    }
}

/// The changed files git kept outside the new paths: the tab-indented lines after its "were
/// left despite sparse patterns" warning.
pub fn left_behind(output: &str) -> Vec<String> {
    let mut lines = output.lines().skip_while(|l| !l.contains("were left despite sparse patterns"));
    if lines.next().is_none() {
        return Vec::new();
    }
    lines.map_while(|l| l.strip_prefix('\t')).map(|l| l.trim_end().to_owned()).filter(|l| !l.is_empty()).collect()
}

/// Decodes a C-quoted path from `git sparse-checkout list` (`"q\"z"`, octal bytes); other lines
/// come back unchanged.
fn unquote(line: &str) -> String {
    let Some(inner) = line.strip_prefix('"').and_then(|l| l.strip_suffix('"')) else { return line.to_owned() };
    let b = inner.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 == b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let c = b[i + 1];
        i += 2;
        let decoded = match c {
            b'a' => 0x07,
            b'b' => 0x08,
            b't' => b'\t',
            b'n' => b'\n',
            b'v' => 0x0b,
            b'f' => 0x0c,
            b'r' => b'\r',
            b'0'..=b'7' if b.get(i..i + 2).is_some_and(|d| d.iter().all(|x| (b'0'..=b'7').contains(x))) => {
                let v = (c - b'0') as u32 * 64 + (b[i] - b'0') as u32 * 8 + (b[i + 1] - b'0') as u32;
                i += 2;
                v as u8
            }
            other => other,
        };
        out.push(decoded);
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests;
