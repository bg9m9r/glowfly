//! The commits a job brings into the working tree, read from its git arguments, so the hooks
//! that follow the working tree (follow.rs) can be compared with the trusted print before it
//! runs: the revision a checkout, switch, merge or rebase names, and the upstream a pull merges
//! once fetched. Reading the arguments and resolving them uses libgit2 only.

use super::{TrustScan, follow};
use crate::settings::SavedTrust;
use git2::{Oid, Repository};

/// One commit a command brings in.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A revision as the command names it.
    Rev(String),
    /// A commit known already (the rebase panel's base).
    Commit(Oid),
    /// The current branch's upstream (`git rebase` with no upstream named).
    Upstream,
    /// A pull: `fetch` (git's arguments) runs first, then the commit it merges is the
    /// upstream (`upstream`, a pull with no remote named) or the fetch's FETCH_HEAD.
    Fetched { fetch: Vec<String>, upstream: bool },
}

/// Options of the commands below that take the next argument as their value.
const VALUED: &[&str] = &["-b", "-B", "-c", "-C", "--orphan", "-m", "-F", "-s", "-X", "-x", "--exec", "--onto", "--depth", "-o"];

/// What the git command `cmd` (its arguments, without `git`) brings into the working tree.
/// Empty for a command that moves to no other commit (`checkout -- <file>`, `merge --abort`,
/// `checkout -b new`) and for every other command.
pub fn targets(cmd: &[String]) -> Vec<Target> {
    let Some(at) = sub_at(cmd) else { return Vec::new() };
    let (globals, sub, args) = (&cmd[..at], cmd[at].as_str(), &cmd[at + 1..]);
    let mut positional = Vec::new();
    let mut onto = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--" => break,
            "--abort" | "--continue" | "--quit" | "--skip" | "--edit-todo" | "--show-current-patch" => return Vec::new(),
            "-" => positional.push("@{-1}".to_owned()),
            "--onto" => onto = it.next().cloned(),
            v if VALUED.contains(&v) => {
                it.next();
            }
            v if v.starts_with("--onto=") => onto = Some(v["--onto=".len()..].to_owned()),
            v if v.starts_with('-') => {}
            v => positional.push(v.to_owned()),
        }
    }
    let rev = |s: &String| Target::Rev(s.clone());
    match sub {
        "checkout" | "switch" => positional.first().map(rev).into_iter().collect(),
        "merge" => positional.iter().map(rev).collect(),
        "rebase" => match (onto, positional.first()) {
            (Some(o), _) => vec![Target::Rev(o)],
            (None, Some(up)) => vec![rev(up)],
            (None, None) if args.iter().any(|a| a == "--root") => Vec::new(),
            (None, None) => vec![Target::Upstream],
        },
        "pull" => {
            let mut fetch = globals.to_vec();
            fetch.push("fetch".to_owned());
            fetch.extend(positional.iter().cloned());
            vec![Target::Fetched { fetch, upstream: positional.is_empty() }]
        }
        _ => Vec::new(),
    }
}

/// The index of `cmd`'s subcommand, past git's own options (`-c key=value`, `-C dir`).
fn sub_at(cmd: &[String]) -> Option<usize> {
    let mut i = 0;
    while i < cmd.len() {
        match cmd[i].as_str() {
            "-c" | "-C" => i += 2,
            s if s.starts_with('-') => i += 1,
            _ => return Some(i),
        }
    }
    None
}

/// The commit `target` names in `repo` now (a pull's after its fetch has run). None when it
/// names none: git refuses such a command too. A branch only a single remote has counts, as
/// `git checkout <branch>` makes it from that one.
pub fn resolve(repo: &Repository, target: &Target) -> Option<Oid> {
    let commit = |spec: &str| repo.revparse_single(spec).ok()?.peel_to_commit().ok().map(|c| c.id());
    match target {
        Target::Commit(oid) => Some(*oid),
        Target::Rev(spec) => commit(spec).or_else(|| {
            let refs = repo.references_glob(&format!("refs/remotes/*/{spec}")).ok()?;
            let mut found = refs.flatten().filter_map(|r| r.peel_to_commit().ok().map(|c| c.id()));
            match (found.next(), found.next()) {
                (Some(oid), None) => Some(oid),
                _ => None,
            }
        }),
        Target::Upstream | Target::Fetched { upstream: true, .. } => upstream(repo),
        Target::Fetched { upstream: false, .. } => {
            let mut merged = None;
            let _ = repo.fetchhead_foreach(|_, _, oid, is_merge| {
                if is_merge && merged.is_none() {
                    merged = Some(*oid);
                }
                merged.is_none()
            });
            merged
        }
    }
}

fn upstream(repo: &Repository) -> Option<Oid> {
    let head = repo.head().ok()?;
    let name = repo.branch_upstream_name(head.name()?).ok()?;
    repo.find_reference(name.as_str()?).ok()?.peel_to_commit().ok().map(|c| c.id())
}

/// Whether the decision `saved` covers what the commit read as `incoming` (`follow::incoming`)
/// would run: Trusted with a print made on every version it found (or a decision saved before
/// prints were kept); without a decision only when nothing follows the working tree.
pub fn allows(saved: Option<&SavedTrust>, incoming: &TrustScan) -> bool {
    match saved {
        None => incoming.follows.is_empty(),
        Some(s) if s.is_trusted() => s.print.as_ref().is_none_or(|p| follow::covered(p, incoming)),
        Some(_) => false,
    }
}

#[cfg(test)]
#[path = "incoming_tests.rs"]
mod tests;
