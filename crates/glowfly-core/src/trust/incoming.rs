//! The commits a job brings into the working tree, read from its git arguments, so the hooks
//! that follow the working tree (follow.rs) can be compared with the trusted print before it
//! runs: the branch a checkout or switch moves to, the commits a merge takes, every commit a
//! rebase, cherry-pick or revert replays (and the branch a rebase checks out first), the tree a
//! `checkout <tree-ish> -- <paths>` writes the hooks from, and the steps left in a rebase or
//! cherry-pick in progress. A pull is pinned instead (pull.rs). Reading the arguments and
//! resolving them uses libgit2 only, the way git resolves them; what can't be resolved that way
//! is `Unchecked`, and the caller holds the job rather than run it unchecked.

use super::{Follow, TrustScan, follow};
use crate::settings::SavedTrust;
use git2::{Oid, Repository, Sort};
use std::path::Path;

/// One thing a command brings in.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// `checkout` or `switch <name>`: the branch `refs/heads/<name>` first, then any revision,
    /// then the remote branches of that name it would make a branch from. `paths`: a checkout
    /// whose one argument may name files instead (`git checkout a.txt`).
    Switch { name: String, paths: bool },
    /// A revision as the command names it (a merge's commits, a new branch's start).
    Rev(String),
    /// A commit known already (the rebase panel's base, a todo step).
    Commit(Oid),
    /// What `git rebase` replays: the commits of `branch` (HEAD when None, checked out first
    /// otherwise) not in `upstream` (the branch's upstream when None; nothing hidden with
    /// `root`), and where it starts: `onto`, else the merge base (`keep_base`), else `upstream`.
    Replayed { onto: Option<String>, upstream: Option<String>, branch: Option<String>, root: bool, keep_base: bool },
    /// The commits a cherry-pick (or a revert: each one's parents' version) takes.
    Picked { revs: Vec<String>, revert: bool },
    /// `checkout <tree> -- <paths>`: the tree's version, when one of `paths` reaches a place
    /// hooks run from. `maybe_paths`: written without `--`, so `tree` may be a file too.
    Paths { tree: String, paths: Vec<String>, maybe_paths: bool },
    /// The steps left in the rebase, cherry-pick or revert in progress (`--continue`, `--skip`).
    Sequencer,
}

/// Why what a command brings in couldn't be compared: the job is held, never run unchecked.
#[derive(Clone, Debug, PartialEq)]
pub enum Unchecked {
    /// A revision libgit2 can't resolve the way git would (`@{push}`), or one git refuses too.
    Unresolved(String),
    /// More commits than `MAX_COMPARED` would be replayed.
    TooMany(usize),
    /// An object can't be read, as in a partial clone that fetched without it.
    Unreadable(String),
    /// A sequencer state Glowfly doesn't read (an `am`-style rebase).
    Unknown,
}

/// The most commits one command's comparison reads; a longer replay is held.
pub const MAX_COMPARED: usize = 500;

/// Whether `opt` of the subcommand `sub` takes the next argument as its value.
fn valued(sub: &str, opt: &str) -> bool {
    let of: &[&str] = match sub {
        "checkout" | "switch" => &["-b", "-B", "-c", "-C", "--orphan", "--create", "--force-create", "--conflict", "--pathspec-from-file"],
        "merge" => &["-m", "-F", "--file", "-s", "-X", "--strategy", "--strategy-option", "--into-name", "--cleanup"],
        "rebase" => &["--onto", "-s", "-X", "-x", "--exec", "--strategy", "--strategy-option", "--whitespace"],
        "cherry-pick" | "revert" => &["-m", "--mainline", "-s", "-X", "--strategy", "--strategy-option", "--cleanup"],
        _ => &[],
    };
    of.contains(&opt)
}

/// What the git command `cmd` (its arguments, without `git`) brings into the working tree.
/// Empty for a command that moves to no other commit (`checkout -- <file>`, `merge --abort`,
/// `checkout -b new`), for a pull (pinned by pull.rs) and for every other command.
pub fn targets(cmd: &[String]) -> Vec<Target> {
    let Some(at) = sub_at(cmd) else { return Vec::new() };
    let (sub, args) = (cmd[at].as_str(), &cmd[at + 1..]);
    let mut positional = Vec::new();
    let mut after_dashes: Option<Vec<String>> = None;
    let (mut onto, mut root, mut keep_base, mut sequencer, mut creates) = (None, false, false, false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--" => {
                after_dashes = Some(it.by_ref().cloned().collect());
                break;
            }
            "--abort" | "--quit" | "--show-current-patch" => return Vec::new(),
            "--continue" | "--skip" | "--edit-todo" => sequencer = true,
            "-" => positional.push("@{-1}".to_owned()),
            "--onto" if sub == "rebase" => onto = it.next().cloned(),
            "--root" => root = true,
            "--keep-base" => keep_base = true,
            "-b" | "-B" | "-c" | "-C" | "--orphan" | "--create" | "--force-create" if matches!(sub, "checkout" | "switch") => {
                creates = true;
                it.next();
            }
            v if valued(sub, v) => {
                it.next();
            }
            v if v.starts_with("--onto=") => onto = Some(v["--onto=".len()..].to_owned()),
            v if v.starts_with('-') => {}
            v => positional.push(v.to_owned()),
        }
    }
    let rev = |s: &String| Target::Rev(s.clone());
    match sub {
        "rebase" | "cherry-pick" | "revert" if sequencer => vec![Target::Sequencer],
        _ if sequencer => Vec::new(),
        "checkout" | "switch" if creates => positional.first().map(rev).into_iter().collect(),
        "switch" => positional.first().map(|n| Target::Switch { name: n.clone(), paths: false }).into_iter().collect(),
        "checkout" => match (positional.as_slice(), after_dashes) {
            ([], _) => Vec::new(),
            ([tree, ..], Some(paths)) if !paths.is_empty() => vec![Target::Paths { tree: tree.clone(), paths, maybe_paths: false }],
            ([name], Some(_)) => vec![Target::Switch { name: name.clone(), paths: false }],
            ([name], None) => vec![Target::Switch { name: name.clone(), paths: true }],
            ([tree, rest @ ..], _) => vec![Target::Paths { tree: tree.clone(), paths: rest.to_vec(), maybe_paths: true }],
        },
        "merge" if positional.is_empty() => vec![Target::Rev("@{upstream}".to_owned())],
        "merge" => positional.iter().map(rev).collect(),
        "rebase" => {
            let (upstream, branch) = (positional.first().cloned(), positional.get(1).cloned());
            vec![Target::Replayed { onto, upstream: if root { None } else { upstream }, branch, root, keep_base }]
        }
        "cherry-pick" | "revert" => vec![Target::Picked { revs: positional, revert: sub == "revert" }],
        _ => Vec::new(),
    }
}

/// `cmd`'s subcommand (`fetch` in `-c x=y fetch origin`).
pub fn sub_name(cmd: &[String]) -> Option<&str> {
    sub_at(cmd).map(|i| cmd[i].as_str())
}

/// The index of `cmd`'s subcommand, past git's own options (`-c key=value`, `-C dir`).
pub(crate) fn sub_at(cmd: &[String]) -> Option<usize> {
    let mut i = 0;
    while i < cmd.len() {
        match cmd[i].as_str() {
            "-c" | "-C" | "--git-dir" | "--work-tree" | "--namespace" | "--config-env" => i += 2,
            s if s.starts_with('-') => i += 1,
            _ => return Some(i),
        }
    }
    None
}

/// The trees and commits whose versions of the places `follows` names `target` brings into
/// `repo`'s working tree, as git would resolve it now. Empty when it brings in none of them
/// (a checkout of files only, a checkout of paths no hook runs from).
pub fn commits(repo: &Repository, follows: &[Follow], target: &Target) -> Result<Vec<Oid>, Unchecked> {
    match target {
        Target::Commit(oid) => Ok(vec![*oid]),
        Target::Rev(spec) => Ok(vec![commit(repo, spec)?]),
        Target::Switch { name, paths } => {
            let branch = repo.find_reference(&format!("refs/heads/{name}")).ok().and_then(|r| r.peel_to_commit().ok());
            if let Some(c) = branch.map(|c| c.id()).or_else(|| commit(repo, name).ok()) {
                return Ok(vec![c]);
            }
            if *paths && is_path(repo, name) {
                return Ok(Vec::new());
            }
            let remote = remote_branches(repo, name);
            if remote.is_empty() { Err(Unchecked::Unresolved(name.clone())) } else { Ok(remote) }
        }
        Target::Paths { tree, paths, maybe_paths } => {
            let reaches = paths.iter().any(|p| reaches(p, follows));
            match repo.revparse_single(tree).ok().and_then(|o| o.peel_to_tree().ok()) {
                Some(_) if !reaches => Ok(Vec::new()),
                Some(t) => Ok(vec![t.id()]),
                // `git checkout a.txt b.txt`: files from the index.
                None if *maybe_paths && is_path(repo, tree) => Ok(Vec::new()),
                None => Err(Unchecked::Unresolved(tree.clone())),
            }
        }
        Target::Replayed { onto, upstream, branch, root, keep_base } => replayed(repo, onto, upstream, branch, *root, *keep_base),
        Target::Picked { revs, revert } => {
            let mut picked = Vec::new();
            for r in revs {
                if r.contains("..") || r.starts_with('^') {
                    let mut w = repo.revwalk().map_err(|e| unreadable(&e))?;
                    w.push_range(r).map_err(|_| Unchecked::Unresolved(r.clone()))?;
                    picked.extend(capped(w)?);
                } else {
                    picked.push(commit(repo, r)?);
                }
            }
            if *revert { parents(repo, &picked) } else { Ok(picked) }
        }
        Target::Sequencer => sequencer(repo),
    }
}

/// `Target::Replayed`'s commits: where it starts, the branch it checks out first, and every
/// commit it replays.
fn replayed(
    repo: &Repository,
    onto: &Option<String>,
    upstream: &Option<String>,
    branch: &Option<String>,
    root: bool,
    keep_base: bool,
) -> Result<Vec<Oid>, Unchecked> {
    let upstream = match upstream {
        Some(u) => Some(commit(repo, u)?),
        None if root => None,
        None => Some(commit(repo, "@{upstream}")?),
    };
    let tip = match branch {
        Some(b) => commit(repo, b)?,
        None => commit(repo, "HEAD")?,
    };
    let mut out = vec![tip];
    match (onto, upstream) {
        (Some(o), _) => out.push(commit(repo, o)?),
        (None, Some(u)) if keep_base => out.push(repo.merge_base(u, tip).map_err(|e| unreadable(&e))?),
        (None, Some(u)) => out.push(u),
        (None, None) => {}
    }
    out.extend(walk(repo, &[tip], upstream.as_slice())?);
    Ok(out)
}

fn unreadable(e: &git2::Error) -> Unchecked {
    Unchecked::Unreadable(e.message().to_owned())
}

/// The commit `spec` names, as libgit2 parses it.
fn commit(repo: &Repository, spec: &str) -> Result<Oid, Unchecked> {
    let found = repo.revparse_single(spec).ok().and_then(|o| o.peel_to_commit().ok());
    found.map(|c| c.id()).ok_or_else(|| Unchecked::Unresolved(spec.to_owned()))
}

/// Whether `name` is a file git would check out instead (in the index or the working tree).
fn is_path(repo: &Repository, name: &str) -> bool {
    let in_index = repo.index().ok().is_some_and(|i| i.get_path(Path::new(name), 0).is_some());
    in_index || repo.workdir().is_some_and(|w| std::fs::symlink_metadata(w.join(name)).is_ok())
}

/// The remote-tracking branches `git checkout <name>` would make `<name>` from: each remote's
/// `refs/heads/<name>` through its fetch refspecs; only `checkout.defaultRemote`'s when several
/// have it and that one does, else every one (git refuses to pick among several, so all of
/// them must be covered).
fn remote_branches(repo: &Repository, name: &str) -> Vec<Oid> {
    let source = format!("refs/heads/{name}");
    let names: Vec<String> = repo.remotes().map(|r| r.iter().flatten().map(str::to_owned).collect()).unwrap_or_default();
    let mut found: Vec<(String, Oid)> = Vec::new();
    for remote in names {
        let Ok(r) = repo.find_remote(&remote) else { continue };
        for spec in r.refspecs().filter(|s| s.direction() == git2::Direction::Fetch && s.src_matches(&source)) {
            let Some(dst) = spec.transform(&source).ok().and_then(|d| d.as_str().map(str::to_owned)) else { continue };
            if let Some(c) = repo.find_reference(&dst).ok().and_then(|r| r.peel_to_commit().ok()) {
                found.push((remote.clone(), c.id()));
            }
        }
    }
    let default = repo.config().ok().and_then(|c| c.get_string("checkout.defaultRemote").ok());
    match default.and_then(|d| found.iter().find(|(r, _)| *r == d).map(|(_, o)| *o)) {
        Some(oid) if found.len() > 1 => vec![oid],
        _ => found.into_iter().map(|(_, o)| o).collect(),
    }
}

/// Whether the pathspec `p` may reach a place hooks run from: inside one or around one, or
/// written with pathspec magic or wildcards Glowfly doesn't expand.
fn reaches(p: &str, follows: &[Follow]) -> bool {
    let p = p.trim_start_matches("./").trim_end_matches('/');
    if p.is_empty() || p == "." || p.starts_with(':') || p.contains(['*', '?', '[', '\\']) {
        return true;
    }
    let lower = p.to_lowercase();
    follows.iter().any(|f| {
        let f = f.path.to_lowercase();
        follow::within(&lower, &f) || follow::within(&f, &lower)
    })
}

/// The commits reachable from `push` and not from `hide`, at most `MAX_COMPARED`.
fn walk(repo: &Repository, push: &[Oid], hide: &[Oid]) -> Result<Vec<Oid>, Unchecked> {
    let mut w = repo.revwalk().map_err(|e| unreadable(&e))?;
    for p in push {
        w.push(*p).map_err(|e| unreadable(&e))?;
    }
    for h in hide {
        w.hide(*h).map_err(|e| unreadable(&e))?;
    }
    capped(w)
}

fn capped(mut w: git2::Revwalk<'_>) -> Result<Vec<Oid>, Unchecked> {
    let _ = w.set_sorting(Sort::TOPOLOGICAL);
    let mut out = Vec::new();
    for oid in w {
        out.push(oid.map_err(|e| unreadable(&e))?);
        if out.len() > MAX_COMPARED {
            return Err(Unchecked::TooMany(out.len()));
        }
    }
    Ok(out)
}

/// The parents of each of `commits`: the versions a revert brings back.
fn parents(repo: &Repository, commits: &[Oid]) -> Result<Vec<Oid>, Unchecked> {
    let mut out = Vec::new();
    for c in commits {
        out.extend(repo.find_commit(*c).map_err(|e| unreadable(&e))?.parent_ids());
    }
    Ok(out)
}

/// The commits the steps left in the sequencer in progress take: a rebase's todo (with its
/// `onto`) or a cherry-pick's or revert's (each revert step's parents). An `am`-style rebase
/// (`rebase-apply`) is `Unknown`.
fn sequencer(repo: &Repository) -> Result<Vec<Oid>, Unchecked> {
    let git = repo.path();
    if git.join("rebase-apply").exists() {
        return Err(Unchecked::Unknown);
    }
    let mut out = Vec::new();
    let mut reverts = Vec::new();
    if let Ok(onto) = std::fs::read_to_string(git.join("rebase-merge/onto")) {
        out.push(commit(repo, onto.trim())?);
    }
    for todo in ["rebase-merge/git-rebase-todo", "sequencer/todo"] {
        let Ok(text) = std::fs::read_to_string(git.join(todo)) else { continue };
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let mut words = line.split_whitespace();
            let verb = words.next().unwrap_or_default();
            // A merge step names its commit after -C or -c; the others right after the verb.
            let named = words.find(|w| !w.starts_with('-')).filter(|w| w.len() >= 4 && w.bytes().all(|b| b.is_ascii_hexdigit()));
            let Some(oid) = named.and_then(|w| commit(repo, w).ok()) else { continue };
            if matches!(verb, "revert") { reverts.push(oid) } else { out.push(oid) }
        }
    }
    out.extend(parents(repo, &reverts)?);
    if out.len() > MAX_COMPARED {
        return Err(Unchecked::TooMany(out.len()));
    }
    Ok(out)
}

/// Whether the decision `saved` covers what the commit read as `incoming` (`follow::incoming`)
/// would run: Trusted with a print made on every version it found; without a decision (or
/// without a print) only when nothing follows the working tree.
pub fn allows(saved: Option<&SavedTrust>, incoming: &TrustScan) -> bool {
    match saved {
        None => incoming.follows.is_empty(),
        Some(s) if s.is_trusted() => s.print.as_ref().map_or_else(|| incoming.follows.is_empty(), |p| follow::covered(p, incoming)),
        Some(_) => false,
    }
}

#[cfg(test)]
#[path = "incoming_tests.rs"]
mod tests;
