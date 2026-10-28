//! What the runner's trust re-check compares for one effect, where hooks follow the working
//! tree (glowfly_core::trust::follow): the commits it brings in, named by the effect itself
//! (`targets`: a pick's commits, a rebase's base and steps, the steps left in the rebase in
//! progress, the commits an undo goes back to), and for a job of git commands, each command's
//! own just before it runs (`Gate`), so a command after a fetch in the same job is compared with
//! what that fetch brought. A pull is pinned there: its fetch, the comparison of the commits it
//! fetched, then a merge or rebase of exactly those (glowfly_core::trust::pull).

use crate::effect::{Effect, RebaseEffect};
use glowfly_core::pick_ops::Picking;
use glowfly_core::rebase::Upstream;
use glowfly_core::settings::SavedTrust;
use glowfly_core::trust::compare::{self, Refused};
use glowfly_core::trust::incoming::{self, Target};
use glowfly_core::trust::pull::{Pull, PullRun, run_pinned};
use glowfly_core::trust::{Follow, TrustScan};
use std::path::{Path, PathBuf};

/// What `effect` brings in by itself (a job of git commands is compared command by command
/// instead, by `Gate`).
pub(super) fn targets(effect: &Effect) -> Vec<Target> {
    let commit = Target::Commit;
    let replay = |onto: &str, upstream: git2::Oid, top: &str| Target::Replayed {
        onto: Some(format!("refs/heads/{onto}")),
        upstream: Some(upstream.to_string()),
        branch: Some(format!("refs/heads/{top}")),
        root: false,
        keep_base: false,
    };
    match effect {
        Effect::Pick(j) => {
            let revert = matches!(j.picking, Picking::Revert);
            vec![Target::Picked { revs: j.oids.iter().map(|o| o.to_string()).collect(), revert }]
        }
        Effect::Rebase(RebaseEffect::Start { request, .. }) => {
            let base = match request.upstream {
                Upstream::Commit(oid) => Some(commit(oid)),
                Upstream::Root => None,
            };
            base.into_iter().chain(request.items.iter().filter_map(|s| s.item()).map(|i| commit(i.oid))).collect()
        }
        Effect::Rebase(RebaseEffect::Resume { .. }) => vec![Target::Sequencer],
        Effect::Rebase(RebaseEffect::EditTodo { request, .. }) => {
            let steps = request.steps.iter().filter_map(|s| s.item()).map(|i| commit(i.oid));
            std::iter::once(Target::Sequencer).chain(steps).collect()
        }
        Effect::Rebase(
            RebaseEffect::Restack { job, .. } | RebaseEffect::ResumeRestack { job, .. } | RebaseEffect::EndRestack { job, .. },
        ) => {
            let resume = matches!(effect, Effect::Rebase(RebaseEffect::ResumeRestack { .. })).then_some(Target::Sequencer);
            // Ending one goes back to the branch it started on; the others replay their steps.
            let ends = matches!(effect, Effect::Rebase(RebaseEffect::EndRestack { .. }));
            let steps = job.plan.steps.iter().filter(|_| !ends).map(|s| replay(&s.onto, s.upstream, &s.top));
            let back = job.plan.return_to.iter().map(|b| Target::Switch { name: b.clone(), paths: false });
            resume.into_iter().chain(steps).chain(back).collect()
        }
        // Every commit the action names, from its fields: an undo can go back to any of them.
        Effect::UndoRedo { action, .. } => commits_in(&format!("{action:?}")).into_iter().map(commit).collect(),
        _ => Vec::new(),
    }
}

/// The full commit ids written in `text` (an action's `Debug` form, which prints every `Oid`
/// field), so no field of any action is missed.
fn commits_in(text: &str) -> Vec<git2::Oid> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !c.is_ascii_hexdigit()) {
        if (word.len() == 40 || word.len() == 64)
            && let Ok(oid) = git2::Oid::from_str(word)
            && !oid.is_zero()
            && !out.contains(&oid)
        {
            out.push(oid);
        }
    }
    out
}

/// Why a comparison stopped the work.
#[derive(Debug)]
pub(super) enum Halt {
    /// What a commit it brings in has, not covered by the decision.
    Held(Box<TrustScan>),
    /// It couldn't be compared.
    Unchecked(incoming::Unchecked),
}

impl Halt {
    /// Why the job stopped, for its output (a job held after some of its commands ran).
    pub(super) fn describe(&self) -> String {
        let why = match self {
            Halt::Held(found) => {
                let places: Vec<&str> = found.follows.iter().map(|f| f.path.as_str()).collect();
                format!("it brings other hooks into {}, which you haven't trusted", places.join(", "))
            }
            Halt::Unchecked(u) => format!("Glowfly couldn't compare the hooks it brings in ({u:?})"),
        };
        format!("Glowfly stopped the job before its next command: {why}. Review the repository's trust, then run it again.")
    }
}

impl From<Refused> for Halt {
    fn from(r: Refused) -> Self {
        match r {
            Refused::Held(s) => Halt::Held(s),
            Refused::Unchecked(u) => Halt::Unchecked(u),
        }
    }
}

/// Compares what `targets` bring into the repository at `workdir` (whose hooks follow the
/// working tree at `follows`) with `saved`. `accept_unchecked`: what can't be compared passes.
pub(super) fn compare(
    workdir: &Path,
    follows: &[Follow],
    saved: Option<&SavedTrust>,
    targets: &[Target],
    accept_unchecked: bool,
) -> Result<(), Halt> {
    if follows.is_empty() || targets.is_empty() {
        return Ok(());
    }
    let unchecked = |u| if accept_unchecked { Ok(()) } else { Err(Halt::Unchecked(u)) };
    let repo = match git2::Repository::open(workdir) {
        Ok(r) => r,
        Err(e) => return unchecked(incoming::Unchecked::Unreadable(e.message().to_owned())),
    };
    let mut oids = Vec::new();
    for t in targets {
        match incoming::commits(&repo, follows, t) {
            Ok(found) => oids.extend(found),
            Err(u) => return unchecked(u),
        }
    }
    match compare::check(&repo, follows, saved, &oids) {
        Ok(()) => Ok(()),
        Err(Refused::Unchecked(u)) => unchecked(u),
        Err(r) => Err(r.into()),
    }
}

/// The comparison a job of git commands gets before each of its commands.
#[derive(Debug)]
pub(crate) struct Gate {
    pub workdir: PathBuf,
    pub follows: Vec<Follow>,
    pub saved: Option<SavedTrust>,
    pub accept_unchecked: bool,
}

/// What the gate did with one command.
pub(super) enum Step {
    /// Compared (or nothing to compare): run it as it is.
    Run,
    /// It ran it already (a pinned pull): whether it succeeded and what git printed.
    Ran(bool, String),
    /// Not run: what it brings in isn't covered. The output of what ran first (a pull's fetch).
    Halt(Halt, String),
}

impl Gate {
    /// Compares what `cmd` brings in now; a pull runs here, pinned, through `git`.
    pub(super) fn before(&self, cmd: &[String], git: &mut glowfly_core::trust::pull::Git<'_>) -> Step {
        if let Some(pull) = Pull::of(cmd) {
            return match run_pinned(&self.workdir, &pull, &self.follows, self.saved.as_ref(), git) {
                PullRun::Ran { ok, output } => Step::Ran(ok, output),
                PullRun::Refused { refused: Refused::Unchecked(_), .. } if self.accept_unchecked => {
                    let (ok, output) = git(cmd, "");
                    Step::Ran(ok, format!("$ git {}\n{output}", cmd.join(" ")).trim_end().to_owned())
                }
                PullRun::Refused { refused, output } => Step::Halt(refused.into(), output),
            };
        }
        match compare(&self.workdir, &self.follows, self.saved.as_ref(), &incoming::targets(cmd), self.accept_unchecked) {
            Ok(()) => Step::Run,
            Err(h) => Step::Halt(h, String::new()),
        }
    }
}

#[cfg(test)]
#[path = "trust_gate_tests.rs"]
mod tests;
