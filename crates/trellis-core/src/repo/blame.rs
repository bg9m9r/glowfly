//! Blame: which commit last changed each line of a file, as of one commit.
//!
//! The file is checked with libgit2 first (it must be a text blob in that commit, small enough
//! to show), then `git blame --porcelain` does the work, so `.mailmap` and
//! `blame.ignoreRevsFile` apply as they do on the command line. A listed ignore-revs file that
//! doesn't exist would make git fail, so such a run drops it and keeps the others.

use crate::error::{Error, Result};
use crate::gitcli;
use git2::{ObjectType, Oid, Repository};
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct BlameCommit {
    pub oid: Oid,
    /// May be empty (the view shows "Unknown").
    pub author: String,
    pub email: String,
    /// Author time (Unix seconds).
    pub time: i64,
    pub summary: String,
}

/// Consecutive lines blamed to one commit. `start` is a 0-based line index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlameRun {
    /// Index into `Blame::commits`.
    pub commit: usize,
    pub start: usize,
    pub len: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Blame {
    pub path: String,
    /// The commit blamed (HEAD resolved when asked for HEAD).
    pub at: Oid,
    /// In order of first appearance.
    pub commits: Vec<BlameCommit>,
    /// Without terminators; one trailing '\r' stripped.
    pub lines: Vec<String>,
    /// Cover every line in order.
    pub runs: Vec<BlameRun>,
    /// The file's `linguist-language` attribute, for highlighting.
    pub language: Option<String>,
}

impl Blame {
    /// The index of the run holding line `line` (0-based).
    pub fn run_of(&self, line: usize) -> Option<usize> {
        let i = self.runs.partition_point(|r| r.start + r.len <= line);
        self.runs.get(i).filter(|r| r.start <= line).map(|_| i)
    }

    /// The commit line `line` (0-based) is blamed to.
    pub fn commit_of_line(&self, line: usize) -> Option<&BlameCommit> {
        self.commits.get(self.runs[self.run_of(line)?].commit)
    }
}

/// Files larger than this (bytes) aren't blamed.
pub const BLAME_MAX_BYTES: usize = 8 * 1024 * 1024;
/// How long `git blame` may run.
pub const BLAME_TIMEOUT: Duration = Duration::from_secs(120);

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Blames `path` (repo-relative, '/'-separated) as of commit `at`, or HEAD.
pub fn blame_file(workdir: &Path, path: &str, at: Option<Oid>) -> Result<Blame> {
    let repo = Repository::open(workdir)?;
    let commit = match at {
        Some(oid) => repo.find_commit(oid)?,
        None => match repo.head() {
            Ok(h) => h.peel_to_commit()?,
            Err(e) if matches!(e.code(), git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound) => {
                return Err(Error::Other("This repository has no commits yet".into()));
            }
            Err(e) => return Err(e.into()),
        },
    };
    let tree = commit.tree()?;
    let entry = match tree.get_path(Path::new(path)) {
        Ok(e) => e,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(Error::NotFound(format!("{path} is not in commit {}", short(commit.id()))));
        }
        Err(e) => return Err(e.into()),
    };
    match entry.kind() {
        Some(ObjectType::Tree) => return Err(Error::Unsupported(format!("{path} is a folder"))),
        Some(ObjectType::Commit) => return Err(Error::Unsupported(format!("{path} is a submodule"))),
        _ => {}
    }
    let blob = repo.find_blob(entry.id())?;
    if blob.is_binary() {
        return Err(Error::Unsupported(format!("{path} is a binary file; blame shows text files only")));
    }
    if blob.size() > BLAME_MAX_BYTES {
        let mb = blob.size().div_ceil(1024 * 1024);
        return Err(Error::Unsupported(format!("{path} is too large to blame ({mb} MB)")));
    }
    let language = super::attrs::linguist_language(&repo, path);

    let mut args: Vec<String> = ["--literal-pathspecs", "blame", "--porcelain", "--no-textconv"].map(String::from).to_vec();
    args.extend(ignore_revs_args(&repo, workdir));
    args.extend([commit.id().to_string(), "--".into(), path.to_owned()]);
    let (out, stdout) = gitcli::output_bytes(workdir, &args, gitcli::NO_PROMPTS, Some(BLAME_TIMEOUT));
    if out.timed_out {
        return Err(Error::Other("git blame took too long".into()));
    }
    if !out.ok {
        let first = out.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message");
        return Err(Error::Other(format!("git blame failed: {first}")));
    }
    let (commits, lines, runs) = super::blame_porcelain::parse(&stdout)?;
    Ok(Blame { path: path.to_owned(), at: commit.id(), commits, lines, runs, language })
}

/// Options for `git blame` (after the subcommand) that keep a missing `blame.ignoreRevsFile`
/// from failing it: when some listed file doesn't exist, the list is cleared
/// (`--no-ignore-revs-file`) and the files that do exist are given again. Empty when every
/// listed file exists (or none is listed), so git reads its config as usual. Relative values
/// are resolved against the working folder and `~` against the home folder, as git does.
pub(super) fn ignore_revs_args(repo: &Repository, workdir: &Path) -> Vec<String> {
    let mut values = Vec::new();
    if let Ok(cfg) = repo.config()
        && let Ok(mut entries) = cfg.multivar("blame.ignorerevsfile", None)
    {
        while let Some(Ok(e)) = entries.next() {
            if let Some(v) = e.value().filter(|v| !v.is_empty()) {
                values.push(v.to_owned());
            }
        }
    }
    let home = crate::settings::home_dir();
    // Each value as git would open it, and as it is given back on the command line (where git
    // expands no '~').
    let files: Vec<(bool, String)> = values
        .into_iter()
        .map(|v| {
            let p = crate::paths::expand_home(&v, home.as_deref());
            let exists = if p.is_absolute() { p.is_file() } else { workdir.join(&p).is_file() };
            (exists, if v.starts_with('~') { p.display().to_string() } else { v })
        })
        .collect();
    if files.iter().all(|(exists, _)| *exists) {
        return Vec::new();
    }
    let mut args = vec!["--no-ignore-revs-file".to_owned()];
    args.extend(files.into_iter().filter(|(exists, _)| *exists).map(|(_, v)| format!("--ignore-revs-file={v}")));
    args
}

#[cfg(test)]
#[path = "blame_tests.rs"]
mod blame_tests;
