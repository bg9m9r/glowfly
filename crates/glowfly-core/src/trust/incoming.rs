//! The commits a job brings into the working tree, so the hooks that follow the working tree
//! (follow.rs) can be compared with the trusted print before it runs: the branch a checkout or
//! switch moves to, the commits a merge takes, every commit a rebase, cherry-pick or revert
//! replays (and the branch a rebase checks out first), the tree or index a checkout or restore
//! of paths writes from, a stash it applies, the patches `git am` or `git apply` writes, and
//! the steps left in a rebase, cherry-pick or `am` in progress (sequencer.rs). The arguments
//! are read in targets.rs; revisions and rev-lists are resolved by git itself (plumbing.rs);
//! what merges leave is compared in merged.rs. A pull is pinned instead (pull.rs). Whatever
//! can't be read that way is `Unchecked`, and the caller holds the job rather than run it.

use super::{Follow, TrustScan, follow, plumbing};
use crate::settings::SavedTrust;
use git2::{Oid, Repository};
use std::path::Path;

#[path = "patches.rs"]
mod patches;
#[path = "sequencer.rs"]
pub(super) mod sequencer;
#[path = "targets.rs"]
mod targets;
pub use targets::targets;

/// How a merge, replay or pick is made, from its options.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct How {
    /// `-s`: ort or recursive (both made as ort), or a merge's `ours`; others are Unmodeled.
    pub strategy: Option<String>,
    /// `-X` options, given to `git merge-tree` as they are.
    pub xopts: Vec<String>,
    /// A pick's or revert's `-m`: the parent a merge commit is taken against.
    pub mainline: Option<u32>,
    pub unrelated: bool,
    /// Options a cherry-pick or revert gives its rev-list (`--not`, `--branches`).
    pub walk: Vec<String>,
    /// A rebase's `--fork-point` or `--no-fork-point`.
    pub fork_point: Option<bool>,
    /// A rebase's `--reapply-cherry-picks`.
    pub reapply: bool,
    pub autostash: bool,
}

/// One thing a command brings in.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// `checkout` or `switch <name>`: the branch `refs/heads/<name>` first, then any revision,
    /// then the remote branches of that name it would make a branch from. `paths`: a checkout
    /// whose one argument may name files instead (`git checkout a.txt`).
    Switch { name: String, paths: bool },
    /// A revision as the command names it (a new branch's start, `reset --hard <rev>`).
    Rev(String),
    /// The commits a merge takes (`FETCH_HEAD`: every one the last fetch marked for merging).
    Merged { revs: Vec<String>, how: How },
    /// The patch files `git am` (`mailbox`) or `git apply` writes, with the options that move
    /// or filter their paths.
    Patches { files: Vec<String>, apply: Vec<String>, mailbox: bool },
    /// A commit known already (the rebase panel's base, a todo step).
    Commit(Oid),
    /// What `git rebase` replays: the commits of `branch` (HEAD when None, checked out first
    /// otherwise) not in `upstream` (the branch's upstream when None; nothing hidden with
    /// `root`), and where it starts: `onto`, else the merge base (`keep_base`), else `upstream`.
    Replayed { onto: Option<String>, upstream: Option<String>, branch: Option<String>, root: bool, keep_base: bool, how: How },
    /// The commits a cherry-pick (or a revert: each one's parents' version) takes.
    Picked { revs: Vec<String>, revert: bool, how: How },
    /// `checkout <tree> -- <paths>`, `restore --source`: the tree's version, when one of
    /// `paths` reaches a place hooks run from. `maybe_paths`: written without `--`, so `tree`
    /// may be a file too.
    Paths { tree: String, paths: Vec<String>, maybe_paths: bool },
    /// `checkout -- <paths>`, `restore <paths>`: the index's version of the paths.
    Index(Vec<String>),
    /// `stash apply` or `pop`: the stash merged onto HEAD, its untracked files, and with
    /// `index` its index.
    Stash { spec: String, index: bool },
    /// A rebase made from a list Glowfly wrote (the rebase panel): the commits it picks in
    /// order onto `onto` (None: a new root); `picks` None when a step merges or resets.
    Steps { onto: Option<Oid>, picks: Option<Vec<Oid>> },
    /// The patches left in the `am` in progress.
    Applying,
    /// The steps left in the rebase, cherry-pick or revert in progress (`--continue`;
    /// `skip`: from HEAD, the current step dropped).
    Sequencer { skip: bool },
}

/// Why what a command brings in couldn't be compared: the job is held, never run unchecked.
#[derive(Clone, Debug, PartialEq)]
pub enum Unchecked {
    /// A revision git can't resolve now.
    Unresolved(String),
    /// An object can't be read, as in a partial clone that fetched without it.
    Unreadable(String),
    /// A patch writes into a place hooks run from (the path it names), from the patches with
    /// this digest.
    Patched { path: String, digest: String },
    /// Merging what it brings in conflicts in a place hooks run from (the path).
    Conflicted(String),
    /// A command, or a form of one, Glowfly doesn't read (what it is).
    Unmodeled(String),
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
/// (a checkout of files only, a checkout of paths no hook runs from). What its merges leave
/// is compared by merged.rs.
pub fn commits(repo: &Repository, follows: &[Follow], target: &Target) -> Result<Vec<Oid>, Unchecked> {
    let base = repo.workdir().unwrap_or_else(|| repo.path());
    match target {
        Target::Commit(oid) => Ok(vec![*oid]),
        Target::Rev(spec) => Ok(vec![commit(repo, spec)?]),
        Target::Merged { revs, .. } => merge_heads(repo, revs),
        Target::Patches { files, apply, mailbox } => {
            let three_way = plumbing::config(repo, "am.threeWay", Some("bool")).map_err(Unchecked::Unreadable)?;
            if *mailbox && three_way.as_deref() == Some("true") {
                return Err(Unchecked::Unmodeled("git am with am.threeWay".to_owned()));
            }
            for f in files {
                let check = if *mailbox { patches::check_mailbox } else { patches::check_patch };
                check(repo, &base.join(f), apply, follows)?;
            }
            Ok(Vec::new())
        }
        Target::Switch { name, paths } => {
            if let Ok(b) = plumbing::resolve(repo, &format!("refs/heads/{name}"), "commit") {
                return Ok(vec![b]);
            }
            if let Ok(c) = commit(repo, name) {
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
            match plumbing::resolve(repo, tree, "tree") {
                Ok(_) if !reaches => Ok(Vec::new()),
                Ok(t) => Ok(vec![t]),
                // `git checkout a.txt b.txt`: files from the index.
                Err(_) if *maybe_paths && is_path(repo, tree) => {
                    commits(repo, follows, &Target::Index([vec![tree.clone()], paths.clone()].concat()))
                }
                Err(e) => Err(e),
            }
        }
        Target::Index(paths) if !paths.iter().any(|p| reaches(p, follows)) => Ok(Vec::new()),
        // The index as a tree (objects only; an index with conflicts can't be one).
        Target::Index(_) => repo.index().and_then(|mut i| i.write_tree()).map(|t| vec![t]).map_err(|e| unreadable(&e)),
        Target::Stash { spec, index } => {
            let stash = commit(repo, spec)?;
            let c = repo.find_commit(stash).map_err(|e| unreadable(&e))?;
            // Its untracked files (the third parent), and with --index its index (the second).
            let mut out = vec![stash];
            out.extend(c.parent_id(2).ok());
            out.extend(c.parent_id(1).ok().filter(|_| *index));
            Ok(out)
        }
        Target::Steps { onto, picks } => Ok(onto.iter().chain(picks.iter().flatten()).copied().collect()),
        Target::Replayed { .. } => {
            let r = replay(repo, target)?;
            Ok([vec![r.tip], r.onto.into_iter().collect(), r.picks].concat())
        }
        Target::Picked { revs, revert, how } => {
            let picked = picks(repo, revs, *revert, how)?;
            if *revert { parents(repo, &picked) } else { Ok(picked) }
        }
        Target::Applying if repo.path().join("rebase-apply/applying").exists() => {
            patches::check_applying(repo, follows).map(|()| Vec::new())
        }
        Target::Applying => Ok(Vec::new()),
        Target::Sequencer { .. } => Ok(sequencer::read(repo)?.commits),
    }
}

/// The commits a merge of `revs` takes.
pub(super) fn merge_heads(repo: &Repository, revs: &[String]) -> Result<Vec<Oid>, Unchecked> {
    let mut out = Vec::new();
    for r in revs {
        if r == "FETCH_HEAD" {
            for h in super::pull::fetched(repo).map_err(Unchecked::Unreadable)? {
                out.push(commit(repo, &h.oid.to_string())?);
            }
        } else {
            out.push(commit(repo, r)?);
        }
    }
    Ok(out)
}

/// What a rebase replays and where.
pub(super) struct Replay {
    /// The branch it checks out first (HEAD when it names none).
    pub tip: Oid,
    /// Where it starts (None: a new root, `--root` without `--onto`).
    pub onto: Option<Oid>,
    /// The commits it picks, in order, as git lists them (merges dropped, and those upstream
    /// has already unless `--reapply-cherry-picks`).
    pub picks: Vec<Oid>,
}

/// `Target::Replayed`'s replay, listed by `git rev-list` the way git rebase lists it: the
/// commits in `upstream...branch` on the branch's side, without merges, oldest first, and
/// with the upstream named by default, after its fork point unless `rebase.forkPoint` is off.
pub(super) fn replay(repo: &Repository, target: &Target) -> Result<Replay, Unchecked> {
    let Target::Replayed { onto, upstream, branch, root, keep_base, how } = target else {
        return Err(Unchecked::Unmodeled("a replay".to_owned()));
    };
    if plumbing::config(repo, "rebase.backend", None).map_err(Unchecked::Unreadable)?.as_deref() == Some("apply") {
        return Err(Unchecked::Unmodeled("git rebase with rebase.backend=apply".to_owned()));
    }
    let branch_spec = branch.clone().unwrap_or_else(|| "HEAD".to_owned());
    let tip = commit(repo, &branch_spec)?;
    let named = upstream.clone().unwrap_or_else(|| "@{upstream}".to_owned());
    let up = if *root { None } else { Some(commit(repo, &named)?) };
    let start = match (onto, up) {
        (Some(o), _) => Some(commit(repo, o)?),
        (None, Some(u)) if *keep_base => Some(repo.merge_base(u, tip).map_err(|e| unreadable(&e))?),
        (None, Some(u)) => Some(u),
        (None, None) => None,
    };
    let mut list: Vec<String> = ["--reverse", "--topo-order", "--no-merges"].map(str::to_owned).to_vec();
    match up {
        Some(u) => {
            list.push("--right-only".to_owned());
            if !how.reapply {
                list.push("--cherry-pick".to_owned());
            }
            list.push(format!("{u}...{tip}"));
            let config = plumbing::config(repo, "rebase.forkPoint", Some("bool")).map_err(Unchecked::Unreadable)?;
            let fork = how.fork_point.unwrap_or(upstream.is_none() && !*keep_base && config.as_deref() != Some("false"));
            if fork && let Some(f) = plumbing::fork_point(repo, &named, &branch_spec) {
                list.push(format!("^{f}"));
            }
        }
        None => list.push(tip.to_string()),
    }
    Ok(Replay { tip, onto: start, picks: plumbing::rev_list(repo, &list)? })
}

/// The commits a cherry-pick or revert of `revs` takes, in the order it takes them: each named
/// one as given, or with a range or rev-list option the walk git makes (oldest first for a
/// cherry-pick, newest first for a revert).
pub(super) fn picks(repo: &Repository, revs: &[String], revert: bool, how: &How) -> Result<Vec<Oid>, Unchecked> {
    let walks = !how.walk.is_empty() || revs.iter().any(|r| r.contains("..") || r.starts_with('^'));
    if !walks {
        return revs.iter().map(|r| commit(repo, r)).collect();
    }
    let mut list: Vec<String> = if revert { Vec::new() } else { vec!["--reverse".to_owned()] };
    list.extend(how.walk.iter().cloned());
    list.extend(revs.iter().cloned());
    plumbing::rev_list(repo, &list)
}

fn unreadable(e: &git2::Error) -> Unchecked {
    Unchecked::Unreadable(e.message().to_owned())
}

/// The commit `spec` names, as git resolves it now.
pub(super) fn commit(repo: &Repository, spec: &str) -> Result<Oid, Unchecked> {
    plumbing::resolve(repo, spec, "commit")
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
    follow::touches(p, follows)
}

/// The parents of each of `commits`: the versions a revert brings back.
pub(super) fn parents(repo: &Repository, commits: &[Oid]) -> Result<Vec<Oid>, Unchecked> {
    let mut out = Vec::new();
    for c in commits {
        out.extend(repo.find_commit(*c).map_err(|e| unreadable(&e))?.parent_ids());
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
#[path = "incoming_more_tests.rs"]
mod more_tests;
#[cfg(test)]
#[path = "incoming_tests.rs"]
mod tests;
