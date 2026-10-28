//! A pull pinned to the commits that were checked. `git pull` fetches and then merges or
//! rebases onto whatever that fetch brought, so when hooks follow the working tree the commit
//! it merges can't be compared before git runs its hooks. Instead the pull is split: its fetch
//! runs first (`Pull::fetch`), the commits it marked for merging are read from FETCH_HEAD
//! (`fetched`) and compared, and then exactly those are merged or rebased onto (`Pull::pinned`)
//! with the pull's own choice of merge or rebase (`--rebase`, `branch.<name>.rebase`,
//! `pull.rebase`) and fast-forward (`--ff`, `--ff-only`, `pull.ff`), with no second fetch.

use super::Follow;
use super::compare::Refused;
use super::incoming::Unchecked;
use crate::settings::SavedTrust;
use git2::{Oid, Repository};
use std::path::Path;

/// A `git pull` command split into its fetch and what follows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pull {
    /// git's own options before `pull` (`-c key=value`), for both commands.
    globals: Vec<String>,
    /// `fetch` with the pull's remote, refspecs and fetch options.
    pub fetch: Vec<String>,
    /// The options that go to the merge or rebase.
    merge: Vec<String>,
    rebase: Vec<String>,
    /// `--rebase[=<how>]` or `--no-rebase` as given.
    how: Option<String>,
    /// `--ff`, `--no-ff` or `--ff-only` as given.
    ff: Option<String>,
    /// `--dry-run`: only the fetch runs.
    dry_run: bool,
    /// A remote or refspec was named: the fork point isn't the branch's upstream.
    named: bool,
}

/// One commit FETCH_HEAD marks for merging, with its line as git wrote it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fetched {
    pub oid: Oid,
    pub line: String,
}

/// Options that go to both commands.
const BOTH: &[&str] = &["-q", "--quiet", "-v", "--verbose", "--progress", "--no-progress"];
/// Options that go to the merge only.
const MERGE: &[&str] = &[
    "--commit",
    "--no-commit",
    "-e",
    "--edit",
    "--no-edit",
    "--log",
    "--no-log",
    "--stat",
    "-n",
    "--no-stat",
    "--summary",
    "--no-summary",
    "--compact-summary",
    "--squash",
    "--no-squash",
    "--verify-signatures",
    "--no-verify-signatures",
    "--allow-unrelated-histories",
];
/// Options that go to the merge and to the rebase.
const SHARED: &[&str] = &["--signoff", "--no-signoff", "--verify", "--no-verify", "--autostash", "--no-autostash", "--no-gpg-sign"];
/// Options of the merge and the rebase that take the next argument as their value.
const SHARED_VALUED: &[&str] = &["-s", "--strategy", "-X", "--strategy-option"];
/// Fetch options that take the next argument as their value.
const FETCH_VALUED: &[&str] = &[
    "--depth",
    "--deepen",
    "--shallow-since",
    "--shallow-exclude",
    "-j",
    "--jobs",
    "--upload-pack",
    "-o",
    "--server-option",
    "--refmap",
    "--negotiation-tip",
    "--filter",
];

impl Pull {
    /// The pull `cmd` (its arguments, without `git`), or None when it isn't one.
    pub fn of(cmd: &[String]) -> Option<Pull> {
        let at = super::incoming::sub_at(cmd)?;
        if cmd[at] != "pull" {
            return None;
        }
        let globals = cmd[..at].to_vec();
        let mut p = Pull {
            globals: globals.clone(),
            fetch: [globals, vec!["fetch".to_owned(), "--update-head-ok".to_owned()]].concat(),
            merge: Vec::new(),
            rebase: Vec::new(),
            how: None,
            ff: None,
            dry_run: false,
            named: false,
        };
        let mut it = cmd[at + 1..].iter();
        while let Some(a) = it.next() {
            let s = a.as_str();
            match s {
                "-r" | "--rebase" => p.how = Some("true".to_owned()),
                "--no-rebase" => p.how = Some("false".to_owned()),
                _ if s.starts_with("--rebase=") => p.how = Some(s["--rebase=".len()..].to_owned()),
                "--ff" | "--no-ff" | "--ff-only" => p.ff = Some(s.to_owned()),
                "--dry-run" => {
                    p.dry_run = true;
                    p.fetch.push(a.clone());
                }
                _ if BOTH.contains(&s) => {
                    p.fetch.push(a.clone());
                    p.merge.push(a.clone());
                    p.rebase.push(a.clone());
                }
                _ if MERGE.contains(&s) || s.starts_with("--cleanup=") || s.starts_with("--log=") => p.merge.push(a.clone()),
                _ if SHARED.contains(&s) || s.starts_with("-S") || s.starts_with("--gpg-sign") || shared_joined(s) => {
                    p.merge.push(a.clone());
                    p.rebase.push(a.clone());
                }
                _ if SHARED_VALUED.contains(&s) => {
                    let v = it.next().cloned().unwrap_or_default();
                    p.merge.extend([a.clone(), v.clone()]);
                    p.rebase.extend([a.clone(), v]);
                }
                _ if FETCH_VALUED.contains(&s) => {
                    p.fetch.push(a.clone());
                    p.fetch.extend(it.next().cloned());
                }
                _ if s.starts_with('-') => p.fetch.push(a.clone()),
                _ => {
                    p.named = true;
                    p.fetch.push(a.clone());
                }
            }
        }
        Some(p)
    }

    /// Whether only the fetch runs (`--dry-run`).
    pub fn fetch_only(&self) -> bool {
        self.dry_run
    }

    /// Whether this pull rebases (the option, else `branch.<current>.rebase`, else
    /// `pull.rebase`), and how: None merges, Some("") rebases, Some("-i") or
    /// Some("--rebase-merges") with that option.
    fn rebases(&self, repo: &Repository) -> Option<&'static str> {
        let config = repo.config().ok()?.snapshot().ok()?;
        let branch = repo.head().ok().filter(|h| h.is_branch()).and_then(|h| h.shorthand().map(str::to_owned));
        let how = self.how.clone().or_else(|| {
            let key = branch.map(|b| format!("branch.{b}.rebase"));
            key.and_then(|k| config.get_string(&k).ok()).or_else(|| config.get_string("pull.rebase").ok())
        })?;
        match how.to_ascii_lowercase().as_str() {
            "false" | "no" | "off" | "0" => None,
            "merges" | "m" | "preserve" | "p" => Some("--rebase-merges"),
            "interactive" | "i" => Some("-i"),
            _ => Some(""),
        }
    }

    /// The fast-forward option the merge gets: the one given, else `pull.ff`'s, else, when
    /// neither that nor a rebase choice was made anywhere, `--ff-only` (git refuses to pull
    /// divergent branches until told how).
    fn ff(&self, repo: &Repository, rebase_chosen: bool) -> Option<String> {
        if self.ff.is_some() {
            return self.ff.clone();
        }
        let config = repo.config().ok()?.snapshot().ok()?;
        match config.get_string("pull.ff").ok().map(|v| v.to_ascii_lowercase()) {
            Some(v) if v == "only" => Some("--ff-only".to_owned()),
            Some(v) if matches!(v.as_str(), "false" | "no" | "off" | "0") => Some("--no-ff".to_owned()),
            Some(_) => Some("--ff".to_owned()),
            None if !rebase_chosen => Some("--ff-only".to_owned()),
            None => None,
        }
    }

    /// The command that takes exactly `heads` (FETCH_HEAD's commits for merging, as checked):
    /// a merge of them with `message` (git's own, from `git fmt-merge-msg`), or a rebase onto
    /// the one of them, from `fork_point` when there is one. `Err` with git's reason when the
    /// pull can't go on.
    pub fn pinned(&self, repo: &Repository, heads: &[Fetched], message: &str, fork_point: Option<Oid>) -> Result<Vec<String>, String> {
        if heads.is_empty() {
            return Err("There is no candidate for merging among the refs that you just fetched.".to_owned());
        }
        let chosen = self.how.is_some() || self.config_rebase_set(repo);
        let unborn = repo.head().is_err();
        match self.rebases(repo).filter(|_| !unborn && self.ff.as_deref() != Some("--ff-only")) {
            Some(how) => {
                let [head] = heads else { return Err("Cannot rebase onto multiple branches.".to_owned()) };
                let mut cmd = [self.globals.clone(), vec!["rebase".to_owned()], self.rebase.clone()].concat();
                if !how.is_empty() {
                    cmd.push(how.to_owned());
                }
                let onto = head.oid.to_string();
                cmd.extend(["--onto".to_owned(), onto.clone(), fork_point.map_or(onto, |f| f.to_string())]);
                Ok(cmd)
            }
            None => {
                let mut cmd = [self.globals.clone(), vec!["merge".to_owned()], self.merge.clone()].concat();
                cmd.extend(self.ff(repo, chosen));
                cmd.extend(["-m".to_owned(), message.trim_end().to_owned()]);
                cmd.extend(heads.iter().map(|h| h.oid.to_string()));
                Ok(cmd)
            }
        }
    }

    fn config_rebase_set(&self, repo: &Repository) -> bool {
        let Some(config) = repo.config().ok().and_then(|mut c| c.snapshot().ok()) else { return false };
        let branch = repo.head().ok().filter(|h| h.is_branch()).and_then(|h| h.shorthand().map(str::to_owned));
        branch.is_some_and(|b| config.get_string(&format!("branch.{b}.rebase")).is_ok()) || config.get_string("pull.rebase").is_ok()
    }

    /// The remote-tracking branch whose reflog gives the fork point a rebasing pull starts
    /// from: the current branch's upstream, when the pull named no remote.
    pub fn fork_ref(&self, repo: &Repository) -> Option<String> {
        if self.named {
            return None;
        }
        let head = repo.head().ok()?;
        repo.branch_upstream_name(head.name()?).ok()?.as_str().map(str::to_owned)
    }
}

/// `-s<x>`, `-X<x>` or `--strategy=<x>` written as one argument.
fn shared_joined(s: &str) -> bool {
    (s.len() > 2 && (s.starts_with("-s") || s.starts_with("-X"))) || s.starts_with("--strategy=") || s.starts_with("--strategy-option=")
}

/// The commits the last fetch in `repo` marked for merging, from FETCH_HEAD, with their lines
/// (for `git fmt-merge-msg`). `Err` when it can't be read or a line doesn't parse.
pub fn fetched(repo: &Repository) -> Result<Vec<Fetched>, String> {
    let path = repo.path().join("FETCH_HEAD");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.splitn(3, '\t');
        let (oid, mark) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let oid = Oid::from_str(oid).map_err(|_| format!("Couldn't read {}: {line}", path.display()))?;
        if mark.is_empty() {
            out.push(Fetched { oid, line: line.to_owned() });
        }
    }
    Ok(out)
}

/// What a pinned pull did.
#[derive(Debug, PartialEq)]
pub enum PullRun {
    /// It ran to the end or to git's failure: whether it succeeded, and what git printed.
    Ran { ok: bool, output: String },
    /// Only the fetch ran: what it brought isn't covered, or couldn't be compared.
    Refused { refused: Refused, output: String },
}

/// Runs one git command (its arguments and standard input): whether it succeeded, and what it
/// printed.
pub type Git<'a> = dyn FnMut(&[String], &str) -> (bool, String) + 'a;

/// Runs `pull` in the repository at `path`, pinned: its fetch, then a comparison of the commits
/// FETCH_HEAD marks for merging (and, for a rebase, the local commits it replays) with `saved`
/// where `follows` points, then the merge or rebase of exactly those commits. `git` runs one
/// git command (its arguments and standard input) and says whether it succeeded and what it
/// printed. The output lists the fetch and the merge or rebase as `$ git ...` lines.
pub fn run_pinned(path: &Path, pull: &Pull, follows: &[Follow], saved: Option<&SavedTrust>, git: &mut Git<'_>) -> PullRun {
    let mut output = String::new();
    let shown = |cmd: &[String], output: &mut String, git: &mut Git<'_>| {
        output.push_str(&format!("$ git {}\n", crate::text::redact_credentials(&cmd.join(" "))));
        let (ok, text) = git(cmd, "");
        if !text.is_empty() {
            output.push_str(text.trim_end());
            output.push('\n');
        }
        ok
    };
    let done = |ok, output: String| PullRun::Ran { ok, output: output.trim_end().to_owned() };
    if !shown(&pull.fetch, &mut output, git) {
        return done(false, output);
    }
    if pull.fetch_only() {
        return done(true, output);
    }
    let refused = |r, output: String| PullRun::Refused { refused: r, output: output.trim_end().to_owned() };
    let unreadable = |e: String| Refused::Unchecked(Unchecked::Unreadable(e));
    // Reopened: the fetch added objects and references this process hasn't seen.
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) => return refused(unreadable(e.message().to_owned()), output),
    };
    let heads = match fetched(&repo) {
        Ok(h) => h,
        Err(e) => return refused(unreadable(e), output),
    };
    let mut oids: Vec<Oid> = heads.iter().map(|h| h.oid).collect();
    let rebasing = pull.rebases(&repo).is_some() && pull.ff.as_deref() != Some("--ff-only");
    if let (true, Ok(head)) = (rebasing, repo.head().and_then(|h| h.peel_to_commit())) {
        match local_commits(&repo, head.id(), &oids) {
            Ok(local) => oids.extend(local),
            Err(u) => return refused(Refused::Unchecked(u), output),
        }
    }
    if let Err(r) = super::compare::check(&repo, follows, saved, &oids) {
        return refused(r, output);
    }
    let lines: String = heads.iter().map(|h| format!("{}\n", h.line)).collect();
    let (_, message) = git(&[pull.globals.clone(), crate::gitcli::args(&["fmt-merge-msg", "--no-log"])].concat(), &lines);
    let fork_point = pull.fork_ref(&repo).filter(|_| rebasing).and_then(|r| {
        let (ok, out) = git(&crate::gitcli::args(&["merge-base", "--fork-point", &r, "HEAD"]), "");
        ok.then(|| Oid::from_str(out.trim()).ok()).flatten()
    });
    match pull.pinned(&repo, &heads, &message, fork_point) {
        Ok(cmd) => {
            let ok = shown(&cmd, &mut output, git);
            done(ok, output)
        }
        Err(why) => {
            output.push_str(&why);
            done(false, output)
        }
    }
}

/// The local commits a rebase onto `onto` replays: HEAD's, not in any of them.
fn local_commits(repo: &Repository, head: Oid, onto: &[Oid]) -> Result<Vec<Oid>, Unchecked> {
    let err = |e: git2::Error| Unchecked::Unreadable(e.message().to_owned());
    let mut w = repo.revwalk().map_err(err)?;
    w.push(head).map_err(err)?;
    for o in onto {
        w.hide(*o).map_err(err)?;
    }
    let mut out = Vec::new();
    for oid in w {
        out.push(oid.map_err(err)?);
        if out.len() > super::incoming::MAX_COMPARED {
            return Err(Unchecked::TooMany(out.len()));
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "pull_tests.rs"]
mod tests;
