//! The `update-ref` steps git adds to an interactive rebase's list with `rebase.updateRefs`:
//! after each commit (or merge) of the range, one per other local branch pointing at it, so the
//! branches stacked inside the range move with it. The plan predicts them the way git does
//! (the config read through git, so includes and `-c` settings count; never HEAD's own branch,
//! and never a branch checked out, rebased or bisected in another worktree, which git leaves as
//! a comment), and the lists git and Glowfly write are compared by these lines too.

use super::TodoStep;
use crate::git_version::Feature;
use crate::gitcli;
use git2::{Oid, Repository};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Whether git adds update-ref lines here: `rebase.updateRefs` is true and git is new enough to
/// know it.
fn enabled(workdir: &Path) -> bool {
    let out = gitcli::read(workdir, &gitcli::args(&["config", "--type=bool", "--get", "rebase.updateRefs"]));
    if !out.ok || out.stdout.trim() != "true" {
        return false;
    }
    gitcli::version(workdir).is_none_or(|v| (v.0, v.1) >= Feature::UpdateRefs.min())
}

/// Adds the update-ref steps git would write to `steps` (the plan for HEAD's branch `head_ref`,
/// a full ref name), each right after the commit or merge its branch points at.
pub(super) fn add(repo: &Repository, workdir: &Path, head_ref: &str, steps: Vec<TodoStep>) -> crate::error::Result<Vec<TodoStep>> {
    if !enabled(workdir) {
        return Ok(steps);
    }
    let busy = checked_out(repo);
    let mut tips: HashMap<Oid, Vec<String>> = HashMap::new();
    for r in repo.references_glob("refs/heads/*")? {
        let r = r?;
        let (Some(name), Some(oid)) = (r.name(), r.resolve().ok().and_then(|r| r.target())) else { continue };
        if name != head_ref && !busy.contains(name) {
            tips.entry(oid).or_default().push(name.to_owned());
        }
    }
    let mut out = Vec::with_capacity(steps.len());
    for step in steps {
        let at = step.oid();
        out.push(step);
        if let Some(names) = at.and_then(|o| tips.get_mut(&o)) {
            names.sort();
            out.extend(names.drain(..).map(TodoStep::UpdateRef));
        }
    }
    Ok(out)
}

/// The branches (full ref names) a worktree holds: checked out there, or being rebased or
/// bisected there.
fn checked_out(repo: &Repository) -> HashSet<String> {
    // Each linked worktree's own git folder, read even when its working folder is gone, as git
    // does.
    let common = repo.commondir();
    let mut dirs: Vec<PathBuf> = vec![common.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(common.join("worktrees")) {
        dirs.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    let mut out = HashSet::new();
    for dir in dirs {
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        if let Some(r) = read("HEAD").trim().strip_prefix("ref:") {
            out.insert(r.trim().to_owned());
        }
        for f in ["rebase-merge/head-name", "rebase-apply/head-name"] {
            let r = read(f);
            if r.trim().starts_with("refs/") {
                out.insert(r.trim().to_owned());
            }
        }
        let bisect = read("BISECT_START");
        if !bisect.trim().is_empty() {
            out.insert(format!("refs/heads/{}", bisect.trim()));
        }
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
