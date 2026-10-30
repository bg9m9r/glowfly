//! The `update-ref` steps git adds to an interactive rebase's list with `rebase.updateRefs`:
//! after each commit (or merge) of the range, one per other local branch pointing at it, so the
//! branches stacked inside the range move with it. The plan predicts them the way git does
//! (the config read through git, so includes and `-c` settings count; never HEAD's own branch,
//! and never a branch checked out, rebased, bisected or moved by a rebase in another worktree,
//! which git leaves as a comment), and the lists git and Glowfly write are compared by these
//! lines too.
//!
//! A branch alias (a symbolic ref under `refs/heads`) is listed by its own name, which git then
//! follows at the end, and its target isn't listed. git checks only the alias's name against the
//! other worktrees, so an alias whose target is checked out elsewhere (or isn't a local branch)
//! would move that ref: such a rebase is refused ([`HeldAlias`]).

use super::TodoStep;

use crate::error::{Error, Result};
use crate::git_version::Feature;
use crate::gitcli;
use git2::{Oid, Repository};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A branch alias inside the range whose target git would move, though it mustn't.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HeldAlias {
    /// The alias, without `refs/heads/`.
    pub alias: String,
    /// The ref it points at (without `refs/heads/` for a branch).
    pub target: String,
    /// The target is checked out in another worktree; otherwise it isn't a local branch.
    pub elsewhere: bool,
}

impl HeldAlias {
    /// Why the rebase is refused, in English (the panel shows its own words).
    pub fn text(&self) -> String {
        let (alias, target) = (&self.alias, &self.target);
        if self.elsewhere {
            format!(
                "{alias} points at {target}, which is checked out in another worktree; git would move {target} with this rebase (rebase.updateRefs). Delete {alias} or switch that worktree to another branch first."
            )
        } else {
            format!(
                "{alias} points at {target}, which isn't a local branch; git would move it with this rebase (rebase.updateRefs). Delete {alias} first."
            )
        }
    }
}

/// A branch name that isn't UTF-8 points at a commit of the range: git would list it, and the
/// list can't name it.
pub const NOT_UTF8: &str =
    "A branch whose name isn't valid UTF-8 points at a commit of this rebase, and git would move it (rebase.updateRefs). Rename it first.";

/// Whether git adds update-ref lines here: `rebase.updateRefs` is true and git is new enough to
/// know it.
fn enabled(workdir: &Path) -> bool {
    let out = gitcli::read(workdir, &gitcli::args(&["config", "--type=bool", "--get", "rebase.updateRefs"]));
    if !out.ok || out.stdout.trim() != "true" {
        return false;
    }
    gitcli::version(workdir).is_none_or(|v| (v.0, v.1) >= Feature::UpdateRefs.min())
}

/// The update-ref lines git would write, by commit, for a rebase of HEAD's branch `head_ref` (a
/// full ref name) over `range`, each commit's in git's order (reverse name order); and the
/// aliases that must hold the rebase back. Unreadable refs are passed over: should git list one,
/// its list no longer matches the plan's and the rebase is refused when it starts.
pub(super) fn predict(repo: &Repository, workdir: &Path, head_ref: &str, range: &HashSet<Oid>) -> Result<Tips> {
    let mut tips = Tips::default();
    if !enabled(workdir) {
        return Ok(tips);
    }
    let busy = checked_out(repo);
    let mut targets: HashSet<String> = HashSet::new();
    let mut listed: Vec<(String, Oid)> = Vec::new();
    for r in repo.references_glob("refs/heads/*")? {
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("rebase plan: skipping a ref that can't be read: {e}");
                continue;
            }
        };
        let Some(oid) = r.resolve().ok().and_then(|r| r.target()) else { continue };
        if !range.contains(&oid) {
            continue;
        }
        let Some(name) = r.name() else { return Err(Error::Unsupported(NOT_UTF8.into())) };
        if let Some(target) = r.symbolic_target() {
            targets.insert(target.to_owned());
            let end = final_target(repo, target);
            let short = |s: &str| s.strip_prefix("refs/heads/").unwrap_or(s).to_owned();
            let held = if !end.starts_with("refs/heads/") {
                Some(false)
            } else if end != head_ref && busy.contains(&end) {
                Some(true)
            } else {
                None
            };
            if let Some(elsewhere) = held {
                tips.held.push(HeldAlias { alias: short(name), target: short(&end), elsewhere });
            }
        }
        if name != head_ref && !busy.contains(name) {
            listed.push((name.to_owned(), oid));
        }
    }
    for (name, oid) in listed.into_iter().filter(|(n, _)| !targets.contains(n)) {
        tips.by_commit.entry(oid).or_default().push(name);
    }
    for names in tips.by_commit.values_mut() {
        names.sort_by(|a, b| b.cmp(a));
    }
    tips.held.sort_by(|a, b| a.alias.cmp(&b.alias));
    Ok(tips)
}

/// What [`predict`] found.
#[derive(Default)]
pub(super) struct Tips {
    pub by_commit: HashMap<Oid, Vec<String>>,
    pub held: Vec<HeldAlias>,
}

/// The ref a chain of symbolic refs from `target` ends at.
fn final_target(repo: &Repository, target: &str) -> String {
    let mut at = target.to_owned();
    for _ in 0..5 {
        match repo.find_reference(&at).ok().and_then(|r| r.symbolic_target().map(str::to_owned)) {
            Some(next) => at = next,
            None => break,
        }
    }
    at
}

/// Adds the update-ref steps of `tips` to `steps`, each commit's right after its row.
pub(super) fn add(steps: Vec<TodoStep>, tips: &Tips) -> Vec<TodoStep> {
    let mut out = Vec::with_capacity(steps.len());
    for step in steps {
        let names = step.oid().and_then(|o| tips.by_commit.get(&o));
        out.push(step);
        out.extend(names.into_iter().flatten().cloned().map(TodoStep::UpdateRef));
    }
    out
}

/// The branches (full ref names) a worktree holds: checked out there (the main worktree's HEAD
/// only when it isn't bare), being rebased or bisected there, or moved by a rebase there.
fn checked_out(repo: &Repository) -> HashSet<String> {
    // Each linked worktree's own git folder, read even when its working folder is gone, as git
    // does.
    let common = repo.commondir();
    let bare = Repository::open(common).is_ok_and(|r| r.is_bare());
    let mut dirs: Vec<(PathBuf, bool)> = vec![(common.to_path_buf(), !bare)];
    if let Ok(entries) = std::fs::read_dir(common.join("worktrees")) {
        dirs.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).map(|p| (p, true)));
    }
    let mut out = HashSet::new();
    for (dir, head) in dirs {
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        if head && let Some(r) = read("HEAD").trim().strip_prefix("ref:") {
            out.insert(r.trim().to_owned());
        }
        // Being rebased or bisected there, read as the worktree list reads it.
        if let Some(held) = crate::repo::held_branch(&dir) {
            out.insert(format!("refs/heads/{}", held.name));
        }
        // Three lines a ref: its name, where it was, where it goes.
        out.extend(
            read("rebase-merge/update-refs").lines().step_by(3).map(str::trim).filter(|l| l.starts_with("refs/")).map(str::to_owned),
        );
    }
    out
}

/// The refs a todo text's `update-ref` (or `u`) lines name, in order; comments are ignored.
pub fn listed_refs(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            matches!(words.next(), Some("update-ref" | "u")).then(|| words.next().unwrap_or_default().to_owned())
        })
        .collect()
}

/// Whether two todo texts move the same branches, in any order.
pub(super) fn same_refs(a: &str, b: &str) -> bool {
    let sorted = |t: &str| {
        let mut v = listed_refs(t);
        v.sort();
        v
    };
    sorted(a) == sorted(b)
}

/// Why git's list `theirs` and the prepared `ours` can't be swapped by their update-ref lines,
/// if they can't: git moving a branch the list doesn't is `REFS_CHANGED` (reload); git leaving
/// out branches the list moves names them, with the reason git's comment gives.
pub(super) fn refs_problem(theirs: &str, ours: &str) -> Option<String> {
    if same_refs(theirs, ours) {
        return None;
    }
    let (t, o) = (listed_refs(theirs), listed_refs(ours));
    let extra: Vec<&String> = t.iter().filter(|r| !o.contains(r)).collect();
    // A name read with a replacement character wasn't UTF-8.
    if extra.iter().any(|r| r.contains('\u{FFFD}')) {
        return Some(NOT_UTF8.into());
    }
    if !extra.is_empty() || t.len() > o.len() {
        return Some(super::REFS_CHANGED.into());
    }
    let left: Vec<String> = o.iter().filter(|r| !t.contains(r)).cloned().collect();
    let why = |r: &str| {
        theirs.lines().find_map(|l| {
            let rest = l.trim().strip_prefix("# Ref ")?.strip_prefix(r)?;
            rest.trim().strip_prefix("checked out at ").map(|p| format!("checked out at {}", p.trim()))
        })
    };
    let mut elsewhere = false;
    let named: Vec<String> = left
        .iter()
        .map(|r| {
            let short = r.strip_prefix("refs/heads/").unwrap_or(r);
            match why(r) {
                Some(w) => {
                    elsewhere = true;
                    format!("{short} ({w})")
                }
                None => short.to_owned(),
            }
        })
        .collect();
    let what = named.join(", ");
    Some(if elsewhere {
        format!(
            "git won't move {what} with this rebase (rebase.updateRefs), though the list does. Switch that worktree to another branch, or turn rebase.updateRefs off, and try again."
        )
    } else {
        format!("git won't move {what} with this rebase, though the list does: check rebase.updateRefs and the git you run, and try again.")
    })
}
