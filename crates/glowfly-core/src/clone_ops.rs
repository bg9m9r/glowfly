//! Cloning a repository: the `git clone` command line (a full clone, or a shallow one limited by
//! a commit count or a start date, optionally of one branch, and optionally sparse: only some
//! folders checked out, and with a partial clone only their file contents downloaded; optionally
//! with its submodules) and the checks for the Clone dialog's fields. The app runs a plain clone
//! through `gitcli`, so credentials, config and hooks apply; a sparse clone is one job here
//! ([`run_sparse`]): the clone, then `git sparse-checkout set` in the new folder, then the
//! submodules inside the chosen folders.

use crate::gitcli;
use crate::sparse_ops::{self, SparseCommand, SparseMode};
use chrono::{DateTime, NaiveDate};
use std::path::Path;

/// How much history a shallow clone downloads.
#[derive(Clone, Debug, PartialEq)]
pub enum HistoryLimit {
    /// The newest N commits of the branch.
    Depth(u32),
    /// Commits made on or after this day (local midnight).
    Since(NaiveDate),
}

/// A shallow clone: the history limit and, optionally, the one branch (or tag) to download.
#[derive(Clone, Debug, PartialEq)]
pub struct Shallow {
    pub limit: HistoryLimit,
    /// `None` is the remote's default branch.
    pub branch: Option<String>,
}

/// A sparse clone: the folders to check out (cone mode; empty is only the files at the root)
/// and whether to download file contents only for them (`--filter=blob:none`).
#[derive(Clone, Debug, PartialEq)]
pub struct Sparse {
    pub folders: Vec<String>,
    pub partial: bool,
}

/// How to clone. The default is a full clone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CloneOptions {
    pub shallow: Option<Shallow>,
    pub sparse: Option<Sparse>,
    /// Clone the submodules too (`--recurse-submodules`). They get git's defaults: their whole
    /// history and every file, whatever the shallow and partial options say for the parent.
    /// With a sparse clone see [`sparse_submodule_cmds`].
    pub submodules: bool,
}

/// `git clone` arguments (without the leading `git`) that clone `url` into the folder `name`,
/// run in the parent folder. `--` always comes before the URL so it is never read as an option.
pub fn clone_cmd(url: &str, name: &str, opts: &CloneOptions) -> Vec<String> {
    let mut cmd = vec!["clone".to_owned()];
    if let Some(s) = &opts.shallow {
        cmd.push(match &s.limit {
            HistoryLimit::Depth(n) => format!("--depth={n}"),
            // A bare date would mean the current time of day on that date; midnight keeps the
            // commits made on the day itself.
            HistoryLimit::Since(d) => format!("--shallow-since={}T00:00:00", d.format("%Y-%m-%d")),
        });
        cmd.push("--single-branch".to_owned());
        if let Some(b) = &s.branch {
            cmd.push(format!("--branch={b}"));
        }
    }
    let partial = opts.sparse.as_ref().is_some_and(|s| s.partial);
    if opts.sparse.is_some() {
        cmd.push("--sparse".to_owned());
        if partial {
            cmd.push("--filter=blob:none".to_owned());
        }
    }
    // git's sparse checkout leaves gitlinks out of its patterns, so `--recurse-submodules`
    // would check out every submodule; a sparse clone updates its folders' ones afterwards.
    if opts.submodules && opts.sparse.is_none() {
        cmd.push("--recurse-submodules".to_owned());
    }
    // git ignores --depth and --filter for a plain path ("local clone") unless the normal
    // transport is used.
    if (opts.shallow.is_some() || partial) && is_local_source(url) {
        cmd.push("--no-local".to_owned());
    }
    cmd.extend(["--".to_owned(), url.to_owned(), name.to_owned()]);
    cmd
}

/// What a sparse clone job did.
#[derive(Clone, Debug, PartialEq)]
pub struct CloneOutcome {
    /// The clone itself succeeded, so the new folder is a repository.
    pub cloned: bool,
    /// Its folders were set too, so only the submodule step can have failed after that.
    pub folders_set: bool,
    /// Every step succeeded.
    pub ok: bool,
    /// The job log: each command line, then git's output.
    pub output: String,
}

/// The commands a sparse clone with submodules runs in the new repository once its folders are
/// set: `git submodule update --init --recursive` for the submodules inside `folders` only, as
/// the sparse checkout leaves the others out (none for an empty list: only the root's files are
/// checked out). Without `--checkout`, as `git clone --recurse-submodules` updates them.
pub fn sparse_submodule_cmds(folders: &[String]) -> Vec<Vec<String>> {
    if folders.is_empty() { Vec::new() } else { crate::submodule_ops::batch_update_cmds(Some(folders), false) }
}

/// Runs the clone `cmd` (from [`clone_cmd`] with `--sparse`) in `parent`, then sets the folders
/// to check out in the new repository `dest` (cone mode, no sparse index), then runs
/// `submodules` there (from [`sparse_submodule_cmds`], empty for none). Credential prompts are
/// allowed, as for any clone the user starts. Each step runs only when the one before it
/// succeeded.
pub fn run_sparse(parent: &Path, cmd: &[String], dest: &Path, folders: &[String], submodules: &[Vec<String>]) -> CloneOutcome {
    let (ok, out) = gitcli::run_with_input(parent, cmd, "", &[]);
    let mut output = crate::text::redact_credentials(&format!("$ git {}", cmd.join(" ")));
    let out = out.trim();
    if !out.is_empty() {
        output.push('\n');
        output.push_str(out);
    }
    if !ok {
        return CloneOutcome { cloned: false, folders_set: false, ok: false, output };
    }
    let set = SparseCommand::Set { mode: SparseMode::Cone, paths: folders.to_vec() };
    let (set_ok, text) = sparse_ops::run(dest, &set);
    output.push('\n');
    output.push_str(&text);
    if !set_ok {
        return CloneOutcome { cloned: true, folders_set: false, ok: false, output };
    }
    for c in submodules {
        let (c_ok, out) = gitcli::run_with_input(dest, c, "", &[]);
        output.push('\n');
        output.push_str(&crate::text::redact_credentials(&format!("$ git {}", c.join(" "))));
        let out = out.trim();
        if !out.is_empty() {
            output.push('\n');
            output.push_str(out);
        }
        if !c_ok {
            return CloneOutcome { cloned: true, folders_set: true, ok: false, output };
        }
    }
    CloneOutcome { cloned: true, folders_set: true, ok: true, output }
}

/// Whether git reads `url` as a path on this computer rather than a URL (git's own rules:
/// `scheme://` is a URL, `host:path` before any slash is scp-like ssh, a drive letter is a path).
/// `file://` counts as a URL: git already honours --depth for it.
pub fn is_local_source(url: &str) -> bool {
    let url = url.trim();
    if url.contains("://") {
        return false;
    }
    if url.starts_with(['/', '\\', '.', '~']) {
        return true;
    }
    let mut chars = url.chars();
    if matches!((chars.next(), chars.next()), (Some(a), Some(':')) if a.is_ascii_alphabetic()) {
        return true;
    }
    match (url.find(':'), url.find('/')) {
        (Some(colon), Some(slash)) => colon > slash,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// The folder name `git clone <url>` would create, made usable on Windows too (see
/// `names::portable`): a repository called `con` or `aux` clones into `con-repo`.
pub fn repo_name(url: &str) -> String {
    let raw = url.trim().trim_end_matches('/').rsplit(['/', ':', '\\']).next().unwrap_or_default().trim_end_matches(".git");
    crate::names::portable(raw, "-repo")
}

/// Why the commit count of a shallow clone can't be used.
#[derive(Debug, PartialEq)]
pub enum DepthProblem {
    Empty,
    NotANumber,
    Zero,
    /// More than git accepts (it reads the depth as an int).
    TooLarge,
}

impl DepthProblem {
    pub fn message(&self) -> &'static str {
        match self {
            DepthProblem::Empty => "Enter how many commits to download.",
            DepthProblem::NotANumber => "Enter a whole number of commits, like 1 or 50.",
            DepthProblem::Zero => "Download at least one commit.",
            DepthProblem::TooLarge => "That's more commits than git accepts.",
        }
    }
}

/// The commit count typed in the Clone dialog.
pub fn parse_depth(text: &str) -> Result<u32, DepthProblem> {
    let t = text.trim();
    if t.is_empty() {
        return Err(DepthProblem::Empty);
    }
    if !t.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DepthProblem::NotANumber);
    }
    // Only digits, so the parse fails only on overflow.
    let n = t.parse::<u64>().unwrap_or(u64::MAX);
    match n {
        0 => Err(DepthProblem::Zero),
        n if n > i32::MAX as u64 => Err(DepthProblem::TooLarge),
        n => Ok(n as u32),
    }
}

/// Why the start date of a shallow clone can't be used.
#[derive(Debug, PartialEq)]
pub enum SinceProblem {
    Empty,
    Invalid,
    BeforeEpoch,
    Future,
}

impl SinceProblem {
    pub fn message(&self) -> &'static str {
        match self {
            SinceProblem::Empty => "Enter a date as YYYY-MM-DD.",
            SinceProblem::Invalid => "Not a date. Use YYYY-MM-DD, like 2026-01-31.",
            SinceProblem::BeforeEpoch => "Choose a date in 1970 or later.",
            SinceProblem::Future => "Choose today or an earlier date.",
        }
    }
}

/// The start date typed in the Clone dialog, as YYYY-MM-DD; `today` (local) is the latest day
/// allowed.
pub fn parse_since(text: &str, today: NaiveDate) -> Result<NaiveDate, SinceProblem> {
    let t = text.trim();
    if t.is_empty() {
        return Err(SinceProblem::Empty);
    }
    // chrono alone would accept "2026-1-5".
    let shaped = t.len() == 10 && t.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
    if !shaped {
        return Err(SinceProblem::Invalid);
    }
    let d = NaiveDate::parse_from_str(t, "%Y-%m-%d").map_err(|_| SinceProblem::Invalid)?;
    if d < NaiveDate::from_ymd_opt(1970, 1, 1).expect("a valid date") {
        Err(SinceProblem::BeforeEpoch)
    } else if d > today {
        Err(SinceProblem::Future)
    } else {
        Ok(d)
    }
}

/// The message for a branch field that fails [`check_clone_branch`].
pub const BRANCH_INVALID: &str = "Not a valid branch or tag name.";

/// The branch (or tag) typed in the Clone dialog: empty is the remote's default branch.
#[allow(clippy::result_unit_err)]
pub fn check_clone_branch(text: &str) -> Result<Option<String>, ()> {
    let t = text.trim();
    if t.is_empty() {
        Ok(None)
    } else if crate::branch_ops::is_valid_branch_name(t) {
        Ok(Some(t.to_owned()))
    } else {
        Err(())
    }
}

/// The local calendar date at `now_secs` (Unix time) with the UTC offset `utc_offset` seconds.
pub fn local_date(now_secs: i64, utc_offset: i32) -> NaiveDate {
    now_secs
        .checked_add(utc_offset as i64)
        .and_then(|s| DateTime::from_timestamp(s, 0))
        .map(|d| d.date_naive())
        .unwrap_or(NaiveDate::from_ymd_opt(1970, 1, 1).expect("a valid date"))
}

#[cfg(test)]
mod tests;
