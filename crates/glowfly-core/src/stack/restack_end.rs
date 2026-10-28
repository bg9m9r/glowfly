//! The auto-stash of a restack and how a restack ends: `stash_first` stashes the uncommitted
//! changes once before the first step (`git stash push -m "Auto-stash before restacking onto
//! <lower>"`), and `come_back` checks the branch checked out before out again and puts the
//! stash back there with `stash_ops::pop_put_back` (`pop --index`, so what was staged comes
//! back staged). The stash only goes back onto that branch: when HEAD can't get there, it stays
//! in the stash list and the output says why. `end_stopped` ends a restack whose stopped step's
//! rebase is over (Abort, or finished outside Glowfly), from the step's top where git leaves
//! HEAD.

use super::plan::RestackPlan;
use super::restack::{RestackRun, git, head_branch, join};
use crate::error::{Error, Result};
use crate::gitcli;
use crate::rebase::{Outcome, stash_tip};
use crate::stash_ops::{AutoStashFor, PutBack, pop_put_back, prepare_put_back};
use git2::{Oid, Repository, RepositoryState};
use std::path::Path;

/// Why `end_stopped` refuses: the stopped step's rebase still runs.
pub const STILL_REBASING: &str = "The restack's rebase is still in progress.";

/// The operation `plan`'s auto-stash is made for, which names it: restacking onto the branch
/// its first step goes onto.
pub fn stash_op(plan: &RestackPlan) -> AutoStashFor {
    AutoStashFor::Restack { onto: plan.steps.first().map_or_else(String::new, |s| s.onto.clone()) }
}

/// Stashes the uncommitted changes for `plan`: the stash made (`None` when there was nothing
/// to stash), or `None` outright when `git stash push` failed (nothing ran).
pub(super) fn stash_first(workdir: &Path, plan: &RestackPlan, output: &mut String) -> Option<Option<Oid>> {
    let tip = || Repository::open(workdir).ok().and_then(|r| stash_tip(&r));
    let before = tip();
    if !git(workdir, &stash_op(plan).stash_command(), output) {
        return None;
    }
    let after = tip();
    Some(after.filter(|_| after != before))
}

/// Checks `plan.return_to` out again (when `checkout`) and puts `plan.stash` back once HEAD is
/// on it: the branch HEAD stayed on when the checkout was refused, and the stash with how
/// putting it back went.
pub(super) fn come_back(
    workdir: &Path,
    plan: &RestackPlan,
    checkout: bool,
    output: &mut String,
) -> (Option<String>, Option<(Oid, PutBack)>) {
    let head = head_branch(workdir);
    let mut stayed_on = None;
    if let Some(b) = plan.return_to.as_deref().filter(|b| checkout && head.as_deref() != Some(*b))
        && !git(workdir, &gitcli::args(&["checkout", b, "--"]), output)
    {
        stayed_on = Some(head.unwrap_or_else(|| "HEAD".to_owned()));
    }
    let Some(oid) = plan.stash else { return (stayed_on, None) };
    let now = head_branch(workdir);
    if plan.return_to.as_deref().is_none_or(|b| now.as_deref() != Some(b)) {
        let why = match plan.return_to.as_deref() {
            Some(b) => format!("The stashed changes stay in the stash list: HEAD is not on {b}, where they were stashed."),
            None => "The stashed changes stay in the stash list: the restack started on a detached HEAD.".to_owned(),
        };
        join(output, &why);
        return (stayed_on, Some((oid, PutBack::Failed)));
    }
    let outcome = match prepare_put_back(workdir, oid) {
        Ok(n) => {
            let (outcome, text) = pop_put_back(workdir, n, oid);
            join(output, &text);
            outcome
        }
        Err((outcome, text)) => {
            join(output, &text);
            outcome
        }
    };
    (stayed_on, Some((oid, outcome)))
}

/// Ends a restack that stopped, once its rebase is over (`STILL_REBASING` otherwise): `left` is
/// what the stop left (the steps not run and the stash), `stopped_top` the top of the step it
/// stopped in. When HEAD is still on that top (where git's abort, and its own finish, leave it),
/// the branch checked out before is checked out again; the stash goes back there.
pub fn end_stopped(workdir: &Path, left: &RestackPlan, stopped_top: Option<&str>) -> Result<RestackRun> {
    let repo = Repository::open(workdir)?;
    if repo.state() != RepositoryState::Clean {
        return Err(Error::Unsupported(STILL_REBASING.into()));
    }
    let head = head_branch(workdir);
    let checkout = stopped_top.is_some() && head.as_deref() == stopped_top;
    let mut output = String::new();
    let (stayed_on, put_back) = come_back(workdir, left, checkout, &mut output);
    let outcome = Outcome::Finished { output, stash_kept: false };
    Ok(RestackRun { outcome, left: RestackPlan { stash: None, ..left.clone() }, stayed_on, put_back })
}
