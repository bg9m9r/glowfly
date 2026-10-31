//! Deleting the local branch after a pull request is merged from the panel (the merge form's
//! "Delete local branch" choice): which local branch that is, why it can't go, the commands the
//! clean-up job runs for it after deleting the remote branch and fetching, and the job's headline
//! when that part stops. The branch goes only once the fetched target contains it
//! (`merge-base --is-ancestor`), and then through `git branch -d`, so git still refuses a branch
//! whose commits would be lost; a branch checked out here goes after checking out the target, and
//! only from a clean working tree; one checked out in another worktree stays. A refused delete
//! puts the branch's upstream back. Pure: the job is started by update/pull_request/cleanup.rs.

use super::branches::{checkout_local, pr_source_refused};
use super::worktree_branches::elsewhere;
use git2::RepositoryState;
use glowfly_core::forge::MergeMethod;
use glowfly_core::gitcli::args;
use glowfly_core::gitflow::failed_step;
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_core::repo::{BranchInfo, Snapshot};

/// The merge whose local branch is meant.
#[derive(Clone, Copy, Debug)]
pub struct Merging<'a> {
    pub remote: &'a str,
    pub source: &'a str,
    pub target: &'a str,
    /// None while the form has no method chosen.
    pub method: Option<MergeMethod>,
    /// The source branch is in this repository rather than on a fork.
    pub same_repo: bool,
    pub default_branch: Option<&'a str>,
}

/// What deleting the local branch takes: its name, the branch its upstream names on the remote
/// (the pull request's source), and the checkout of the target first when it is checked out here.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalDelete {
    pub branch: String,
    pub upstream: String,
    pub checkout: Option<Vec<String>>,
}

/// The local branch of the pull request's source: the one tracking `<remote>/<source>`. A branch
/// that merely has the source's name may be another branch, so it is never offered.
pub fn local_branch<'a>(snap: &'a Snapshot, remote: &str, source: &str) -> Option<&'a BranchInfo> {
    snap.locals.iter().find(|b| b.tracking.as_ref().is_some_and(|t| t.remote == remote && t.merge == source))
}

/// Whether nothing would be carried over or lost by checking out another branch: no staged,
/// changed or untracked files, and no operation in progress.
fn clean(snap: &Snapshot) -> bool {
    snap.staged.is_empty() && snap.unstaged.is_empty() && snap.state == RepositoryState::Clean
}

/// `git checkout` of the target: its local branch, else a new one tracking `<remote>/<target>`.
/// None when there is neither, or git would read the name as an option.
fn checkout_target(snap: &Snapshot, remote: &str, target: &str) -> Option<Vec<String>> {
    if pr_source_refused(target).is_some() {
        return None;
    }
    if snap.locals.iter().any(|b| b.name == target) {
        return Some(checkout_local(target));
    }
    let on_remote = snap.remotes.iter().any(|(r, bs)| r == remote && bs.iter().any(|(n, _)| n == target));
    on_remote.then(|| args(&["checkout", "--track", &format!("{remote}/{target}"), "--"]))
}

/// The local branch of `m`, with how it would be deleted or why it can't be; None when there is
/// no local branch to delete.
pub fn local_delete(t: Lang, snap: &Snapshot, m: &Merging) -> Option<(String, Result<LocalDelete, String>)> {
    let b = local_branch(snap, m.remote, m.source)?;
    Some((b.name.clone(), plan(t, snap, m, b)))
}

fn plan(t: Lang, snap: &Snapshot, m: &Merging, b: &BranchInfo) -> Result<LocalDelete, String> {
    let (branch, target) = (b.name.as_str(), m.target);
    let refused = if !m.same_repo {
        Some(t.tr("pr.merge.delete_local.fork").to_owned())
    } else if m.default_branch == Some(branch) || branch == target {
        Some(t.trf("pr.merge.delete_local.default", &[("branch", &branch)]))
    } else if snap.gitflow.base_role(branch).is_some() {
        Some(t.trf("pr.merge.delete_local.flow_base", &[("branch", &branch)]))
    } else if matches!(m.method, Some(MergeMethod::Squash | MergeMethod::Rebase)) {
        Some(t.trf("pr.merge.delete_local.rewritten", &[("branch", &branch)]))
    } else {
        elsewhere(snap, branch).map(|wt| t.trf("pr.merge.delete_local.elsewhere", &[("branch", &branch), ("path", &shown(&wt.path))]))
    };
    if let Some(why) = refused {
        return Err(why);
    }
    let upstream = m.source.to_owned();
    if !b.is_head {
        return Ok(LocalDelete { branch: branch.to_owned(), upstream, checkout: None });
    }
    let checkout = checkout_target(snap, m.remote, target).filter(|_| elsewhere(snap, target).is_none());
    match checkout {
        None => Err(t.trf("pr.merge.delete_local.no_target", &[("branch", &branch), ("target", &target)])),
        Some(_) if !clean(snap) => Err(t.trf("pr.merge.delete_local.dirty", &[("branch", &branch), ("target", &target)])),
        Some(c) => Ok(LocalDelete { branch: branch.to_owned(), upstream, checkout: Some(c) }),
    }
}

/// A step of the job's local part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LocalStep {
    /// The fetched target contains the branch.
    Check,
    Checkout,
    Delete,
}

/// The commands that delete the local branch, after the fetch. `git branch -d` judges a branch
/// by its upstream (else HEAD), which the remote delete and the prune have usually removed and a
/// stale checked-out target doesn't contain, so the branch's upstream names the target just
/// before the delete (`branch.<name>.merge`; the remote is already the pull request's): git's own
/// check then is the one that matters. That happens only once the target is known to contain the
/// branch, and `-d` removes the branch's config with it (Undo restores the upstream it had); if
/// git refuses, the job's `restore_upstream` puts it back.
pub fn local_delete_cmds(d: &LocalDelete, remote: &str, target: &str) -> Vec<(LocalStep, Vec<String>)> {
    let onto = format!("refs/remotes/{remote}/{target}");
    let mut cmds = vec![(LocalStep::Check, args(&["merge-base", "--is-ancestor", &format!("refs/heads/{}", d.branch), &onto]))];
    if let Some(c) = &d.checkout {
        cmds.push((LocalStep::Checkout, c.clone()));
    }
    cmds.push((LocalStep::Delete, merge_config(&d.branch, target)));
    cmds.push((LocalStep::Delete, args(&["branch", "-d", &d.branch])));
    cmds
}

/// `git config branch.<branch>.merge refs/heads/<onto>`.
fn merge_config(branch: &str, onto: &str) -> Vec<String> {
    args(&["config", &format!("branch.{branch}.merge"), &format!("refs/heads/{onto}")])
}

/// The command that puts the branch's upstream back, run when the job fails (its
/// `on_fail_check`). It writes the value the branch had, so after a failure before the repoint it
/// changes nothing, and it prints nothing.
pub fn restore_upstream(d: &LocalDelete) -> Vec<String> {
    merge_config(&d.branch, &d.upstream)
}

/// The running clean-up job that deletes a local branch, for its headline.
#[derive(Clone, Debug, PartialEq)]
pub struct CleanupJob {
    pub label: String,
    pub branch: String,
    pub remote: String,
    pub target: String,
    /// Each command's step, in order; None for the remote part (the remote delete, the fetch).
    pub steps: Vec<Option<LocalStep>>,
    /// `restore_upstream`, which the runner echoes after the job's own output when it fails.
    pub restore: Vec<String>,
}

impl CleanupJob {
    /// The failed job's headline when it stopped in the local part, which keeps the branch; None
    /// when it stopped before (git's cause in plain words says that better).
    pub fn stopped(&self, t: Lang, output: &str) -> Option<String> {
        let restore = format!("$ git {}", self.restore.join(" "));
        let output = output.rfind(&restore).map_or(output, |at| &output[..at]);
        let step = (*self.steps.get(failed_step(output, self.steps.len()))?)?;
        // git's own error line, rather than the hints it prints after it.
        let error = output.lines().rfind(|l| l.starts_with("error: ") || l.starts_with("fatal: "));
        let error = error.or_else(|| output.lines().rfind(|l| !l.starts_with("$ "))).unwrap_or("").trim();
        let (branch, target) = (self.branch.as_str(), self.target.as_str());
        Some(match step {
            LocalStep::Check => t.trf("pr.cleanup.unmerged", &[("branch", &branch), ("remote", &self.remote), ("target", &target)]),
            LocalStep::Checkout => t.trf("pr.cleanup.checkout_failed", &[("branch", &branch), ("target", &target), ("error", &error)]),
            LocalStep::Delete => t.trf("pr.cleanup.refused", &[("branch", &branch), ("error", &error)]),
        })
    }
}

#[cfg(test)]
mod tests;
