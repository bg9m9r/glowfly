//! Blame: which commit last changed each line of a file, as of one commit.
//!
//! The file is checked with libgit2 first (it must be a text blob in that commit, small enough
//! to show), then `git blame --porcelain` does the work, so `.mailmap` and
//! `blame.ignoreRevsFile` apply as they do on the command line. A listed ignore-revs file that
//! doesn't exist would make git fail, so such a run drops it and keeps the others. Each run
//! also records where its lines came from (the commit's path and the parent to blame for what
//! came before), so the view can step back past a commit.

use crate::commit_text;
use crate::error::{Error, Result};
use crate::gitcli;
use crate::settings::BlameMoves;
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
    /// The message after the summary paragraph, trailing whitespace trimmed; empty for one-line
    /// messages.
    pub body: String,
}

/// Where a run's lines came from in its commit.
#[derive(Clone, Debug, PartialEq)]
pub struct BlameOrigin {
    /// Index into `Blame::commits`.
    pub commit: usize,
    /// The file's path in that commit.
    pub path: String,
    /// The parent and the path there, to blame for what came before; None when the commit added
    /// the lines with the file (or the parent is outside a shallow clone).
    pub previous: Option<(Oid, String)>,
}

/// Consecutive lines blamed to one commit and origin. `start` is a 0-based line index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlameRun {
    /// Index into `Blame::commits`.
    pub commit: usize,
    /// Index into `Blame::origins`.
    pub origin: usize,
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
    /// Each (commit, path) lines came from, in order of first appearance.
    pub origins: Vec<BlameOrigin>,
    /// The 0-based number of each line in its origin's version of the file.
    pub orig_lines: Vec<usize>,
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

    /// Where line `line` (0-based) came from.
    pub fn origin_of_line(&self, line: usize) -> Option<&BlameOrigin> {
        self.origins.get(self.runs[self.run_of(line)?].origin)
    }

    /// Per commit, how new it is among the blame's commits: the rank of its author time among
    /// the distinct times, scaled to 0 (oldest) ..= 1 (newest). 1 when there is only one time,
    /// and for lines not committed yet.
    pub fn ages(&self) -> Vec<f32> {
        let mut times: Vec<i64> = self.commits.iter().filter(|c| !c.oid.is_zero()).map(|c| c.time).collect();
        times.sort_unstable();
        times.dedup();
        let top = times.len().saturating_sub(1);
        self.commits
            .iter()
            .map(|c| {
                if c.oid.is_zero() || top == 0 {
                    return 1.0;
                }
                let rank = times.partition_point(|t| *t < c.time);
                rank as f32 / top as f32
            })
            .collect()
    }
}

/// What a blame run asks git for beyond its defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlameOptions {
    /// Ignore whitespace changes (`-w`).
    pub ignore_whitespace: bool,
    pub moves: BlameMoves,
    /// Commits to look past (`--ignore-rev`), after any configured ignore-revs files.
    pub ignore_revs: Vec<Oid>,
}

/// Files larger than this (bytes) aren't blamed.
pub const BLAME_MAX_BYTES: usize = 8 * 1024 * 1024;
/// How long `git blame` may run.
pub const BLAME_TIMEOUT: Duration = Duration::from_secs(120);

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// Blames `path` (repo-relative, '/'-separated) as of commit `at`, or HEAD.
pub fn blame_file(workdir: &Path, path: &str, at: Option<Oid>, opts: &BlameOptions) -> Result<Blame> {
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
    // A partial clone downloads the versions blame reads first (git blame would fetch them one
    // at a time).
    let mut ids = vec![entry.id()];
    if crate::partial::looks_partial(&repo) {
        ids.extend(crate::partial::history_blobs(workdir, commit.id(), path));
    }
    crate::partial::require(&repo, workdir, path, ids)?;
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
    if opts.ignore_whitespace {
        args.push("-w".into());
    }
    args.extend(opts.moves.args().iter().map(|a| (*a).to_owned()));
    args.extend(opts.ignore_revs.iter().map(|oid| format!("--ignore-rev={oid}")));
    args.extend([commit.id().to_string(), "--".into(), path.to_owned()]);
    let (out, stdout) = gitcli::output_bytes(workdir, &args, gitcli::NO_PROMPTS, Some(BLAME_TIMEOUT));
    if out.timed_out {
        return Err(Error::Other("git blame took too long".into()));
    }
    if !out.ok {
        let first = out.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message");
        return Err(Error::Other(format!("git blame failed: {first}")));
    }
    let parsed = super::blame_porcelain::parse(&stdout)?;
    let mut commits = parsed.commits;
    for c in commits.iter_mut().filter(|c| !c.oid.is_zero()) {
        if let Ok(found) = repo.find_commit(c.oid) {
            c.body = commit_text::body(&found).trim_end().to_owned();
        }
    }
    Ok(Blame {
        path: path.to_owned(),
        at: commit.id(),
        commits,
        lines: parsed.lines,
        runs: parsed.runs,
        origins: parsed.origins,
        orig_lines: parsed.orig_lines,
        language,
    })
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

#[cfg(test)]
#[path = "blame_step_tests.rs"]
mod blame_step_tests;
