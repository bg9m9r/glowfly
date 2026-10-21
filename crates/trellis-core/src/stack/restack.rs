//! Running a restack through the git CLI, so hooks, signing and config apply: each step is
//! `git rebase --update-refs [--autostash] --onto refs/heads/<onto> <upstream> <top>`, and once
//! every step is done the branch checked out before is checked out again. HEAD on a middle
//! layer is detached for the step, since `--update-refs` skips a branch checked out here (git
//! only switches to the top in its own bookkeeping). A step that stops (a
//! conflict) leaves the rest of the plan for `resume`, after the rebase banner's Continue or
//! Skip finished that step.

use super::STALE_PLAN;
use super::plan::{RestackPlan, RestackStep};
use crate::error::{Error, Result};
use crate::gitcli;
use crate::rebase::{self, Outcome, Resume, outcome, stash_tip};
use git2::{BranchType, Repository};
use std::path::Path;

/// Where a restack run ended: `outcome` as for a rebase, the steps still to run (after a stop
/// or a failure), and the branch HEAD stayed on when checking the original one out again was
/// refused.
#[derive(Debug)]
pub struct RestackRun {
    pub outcome: Outcome,
    pub left: RestackPlan,
    pub stayed_on: Option<String>,
}

/// Runs `plan`. `Err` means nothing ran: the stack no longer matches the plan.
pub fn restack(workdir: &Path, plan: &RestackPlan) -> Result<RestackRun> {
    let repo = Repository::open(workdir)?;
    if !plan.steps.iter().all(|s| valid(&repo, s)) {
        return Err(Error::Unsupported(STALE_PLAN.into()));
    }
    run_steps(workdir, plan, String::new(), false)
}

/// Continues or skips the step that stopped, then runs the steps `left`.
pub fn resume(workdir: &Path, how: Resume, left: &RestackPlan) -> Result<RestackRun> {
    match rebase::resume(workdir, how)? {
        Outcome::Finished { output, stash_kept } => run_steps(workdir, left, output, stash_kept),
        outcome => Ok(RestackRun { outcome, left: left.clone(), stayed_on: None }),
    }
}

/// Whether `step` can still run as planned: both branches exist and the top still contains the
/// old base it is replayed from.
fn valid(repo: &Repository, step: &RestackStep) -> bool {
    let tip = |name: &str| repo.find_branch(name, BranchType::Local).ok().and_then(|b| b.get().target());
    let (Some(_), Some(top)) = (tip(&step.onto), tip(&step.top)) else { return false };
    top == step.upstream || repo.graph_descendant_of(top, step.upstream).unwrap_or(false)
}

/// Runs the steps of `plan` after `output` (what ran before), then checks `return_to` out.
fn run_steps(workdir: &Path, plan: &RestackPlan, mut output: String, mut kept: bool) -> Result<RestackRun> {
    let rest = |from: usize| RestackPlan { steps: plan.steps[from..].to_vec(), ..plan.clone() };
    let stop = |outcome, left| Ok(RestackRun { outcome, left, stayed_on: None });
    for (i, step) in plan.steps.iter().enumerate() {
        let repo = Repository::open(workdir)?;
        if !valid(&repo, step) {
            join(&mut output, STALE_PLAN);
            return stop(Outcome::Failed { output }, rest(i));
        }
        let detached = detach_inside(workdir, &repo, step, &mut output);
        let mut args = gitcli::args(&["rebase", "--update-refs"]);
        if plan.autostash {
            args.push("--autostash".into());
        }
        let onto = format!("refs/heads/{}", step.onto);
        args.extend(gitcli::args(&["--onto", &onto, &step.upstream.to_string(), &step.top]));
        let before = stash_tip(&repo);
        let ok = git(workdir, &args, &mut output);
        match outcome(workdir, ok, String::new(), plan.autostash, before)? {
            Outcome::Finished { stash_kept, .. } => kept |= stash_kept,
            Outcome::Stopped { ok, progress, .. } => return stop(Outcome::Stopped { ok, output, progress }, rest(i + 1)),
            Outcome::Failed { .. } => {
                // Nothing moved: back onto the branch that was detached for the step.
                if let Some(b) = detached {
                    git(workdir, &gitcli::args(&["checkout", &b, "--"]), &mut output);
                }
                return stop(Outcome::Failed { output }, rest(i + 1));
            }
        }
    }
    let mut stayed_on = None;
    let head = head_branch(workdir);
    if let Some(b) = plan.return_to.as_deref().filter(|b| head.as_deref() != Some(*b))
        && !git(workdir, &gitcli::args(&["checkout", b, "--"]), &mut output)
    {
        stayed_on = Some(head.unwrap_or_else(|| "HEAD".to_owned()));
    }
    let outcome = Outcome::Finished { output, stash_kept: kept };
    Ok(RestackRun { outcome, left: rest(plan.steps.len()), stayed_on })
}

/// `--update-refs` leaves a branch checked out here where it is, so when HEAD is on a layer
/// `step` moves (other than its top, which git rebases itself), HEAD is detached first, at the
/// same commit. Returns that branch.
fn detach_inside(workdir: &Path, repo: &Repository, step: &RestackStep, output: &mut String) -> Option<String> {
    let head = repo.head().ok()?;
    let name = head.shorthand().filter(|n| head.is_branch() && *n != step.top)?.to_owned();
    let at = head.target()?;
    let top = repo.find_branch(&step.top, BranchType::Local).ok()?.get().target()?;
    let below_top = at == top || repo.graph_descendant_of(top, at).unwrap_or(false);
    let inside = at != step.upstream && below_top && repo.graph_descendant_of(at, step.upstream).unwrap_or(false);
    (inside && git(workdir, &gitcli::args(&["checkout", "--detach"]), output)).then_some(name)
}

/// Runs `git <args>`, adding the command and what it printed to `output`; whether it worked.
fn git(workdir: &Path, args: &[String], output: &mut String) -> bool {
    let (ok, out) = gitcli::run(workdir, args);
    join(
        output,
        &format!(
            "$ git {}
{out}",
            args.join(" ")
        ),
    );
    ok
}

/// HEAD's branch, when it is on one.
fn head_branch(workdir: &Path) -> Option<String> {
    let repo = Repository::open(workdir).ok()?;
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    head.shorthand().map(str::to_owned)
}

fn join(output: &mut String, more: &str) {
    let more = more.trim_end();
    if more.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(more);
}
