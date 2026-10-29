//! What the runner's trust re-check compares for one effect, where hooks follow the working
//! tree (glowfly_core::trust::follow): the commits it brings in, named by the effect itself
//! (`targets`: a pick's commits, a rebase's base and steps, the steps left in the rebase in
//! progress, the commits an undo goes back to), and for a job of git commands, each command's
//! own just before it runs (`Gate`), so a command after a fetch in the same job is compared with
//! what that fetch brought, and after any command but a fetch the repository is scanned again
//! (a patch applied, a stash popped or a checkout made can write hooks the next command runs).
//! A pull is pinned there: its fetch, the comparison of the commits it fetched, then a merge or
//! rebase of exactly those (glowfly_core::trust::pull).

use crate::effect::{Accepted, Effect, RebaseEffect};
use glowfly_core::i18n::Lang;
use glowfly_core::pick_ops::Picking;
use glowfly_core::rebase::{MergeParent, MergeStep, RebaseAction, ResetTo, Resume, TodoStep, Upstream};
use glowfly_core::settings::SavedTrust;
use glowfly_core::trust::compare::{self, Refused};
use glowfly_core::trust::incoming::{self, How, Target};
use glowfly_core::trust::pull::{Pull, PullRun, run_pinned};
use glowfly_core::trust::{Follow, TrustScan};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

/// The commits one rebase step takes: its commit, a `reset` to a commit outside the range, and
/// a recreated merge's parents outside it.
fn step_commits(step: &TodoStep) -> Vec<git2::Oid> {
    let mut out: Vec<git2::Oid> = step.item().map(|i| i.oid).into_iter().collect();
    match step {
        TodoStep::Reset(ResetTo::Commit(oid)) => out.push(*oid),
        TodoStep::Merge(MergeStep { parents, .. }) => {
            out.extend(parents.iter().filter_map(|p| if let MergeParent::Commit(o) = p { Some(*o) } else { None }))
        }
        _ => {}
    }
    out
}

/// The commits `steps` pick in order, onto what the rebase starts from; None when one merges
/// or resets (what it leaves isn't made in memory).
fn picks(steps: &[TodoStep]) -> Option<Vec<git2::Oid>> {
    let mut out = Vec::new();
    for s in steps {
        match s {
            TodoStep::Commit(i) if i.action == RebaseAction::Drop => {}
            TodoStep::Commit(i) => out.push(i.oid),
            TodoStep::Exec(_) | TodoStep::Break | TodoStep::UpdateRef(_) => {}
            TodoStep::Label { .. } | TodoStep::Reset(_) | TodoStep::Merge(_) => return None,
        }
    }
    Some(out)
}

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
        how: How::default(),
    };
    match effect {
        Effect::Pick(j) => {
            let revert = matches!(j.picking, Picking::Revert);
            vec![Target::Picked { revs: j.oids.iter().map(|o| o.to_string()).collect(), revert, how: How::default() }]
        }
        Effect::Rebase(RebaseEffect::Start { request, .. }) => {
            let base = match request.upstream {
                Upstream::Commit(oid) => Some(oid),
                Upstream::Root => None,
            };
            let made = Target::Steps { onto: base, picks: picks(&request.items) };
            std::iter::once(made).chain(request.items.iter().flat_map(step_commits).map(commit)).collect()
        }
        Effect::Rebase(RebaseEffect::Resume { how, .. }) => vec![Target::Sequencer { skip: matches!(how, Resume::Skip) }],
        Effect::Rebase(RebaseEffect::EditTodo { request, .. }) => {
            let steps = request.steps.iter().flat_map(step_commits).map(commit);
            std::iter::once(Target::Sequencer { skip: false }).chain(steps).collect()
        }
        Effect::Rebase(
            RebaseEffect::Restack { job, .. } | RebaseEffect::ResumeRestack { job, .. } | RebaseEffect::EndRestack { job, .. },
        ) => {
            let resume = matches!(effect, Effect::Rebase(RebaseEffect::ResumeRestack { .. })).then_some(Target::Sequencer { skip: false });
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
    /// A command before it changed the repository's own settings or hooks: its scan now.
    Changed(Box<TrustScan>),
}

impl Halt {
    /// Why the job stopped, for its output (a job held after some of its commands ran).
    pub(super) fn describe(&self, t: Lang) -> String {
        let why = match self {
            Halt::Held(found) => {
                let places: Vec<&str> = found.follows.iter().map(|f| f.path.as_str()).collect();
                t.trf("trust.halt.held", &[("places", &places.join(", "))])
            }
            Halt::Unchecked(u) => crate::state::trust::unchecked_text(t, u),
            Halt::Changed(_) => t.tr("trust.halt.changed").to_owned(),
        };
        t.trf("trust.halt.stopped", &[("why", &why)])
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
/// working tree at `follows`) with `saved`: the commits, and what the merges they make leave.
/// `accept`: the one reason an item couldn't be compared that passes (that item only).
pub(super) fn compare(
    workdir: &Path,
    follows: &[Follow],
    saved: Option<&SavedTrust>,
    targets: &[Target],
    accept: Option<&incoming::Unchecked>,
) -> Result<(), Halt> {
    if follows.is_empty() || targets.is_empty() {
        return Ok(());
    }
    let repo = git2::Repository::open(workdir).map_err(|e| Halt::Unchecked(incoming::Unchecked::Unreadable(e.message().to_owned())))?;
    compare::check_targets(&repo, follows, saved, targets, accept).map_err(Halt::from)
}

/// The comparison a job of git commands gets before each of its commands.
#[derive(Debug)]
pub(crate) struct Gate {
    pub workdir: PathBuf,
    /// Where hooks follow the working tree, as the last scan found.
    pub follows: RefCell<Vec<Follow>>,
    pub saved: Option<SavedTrust>,
    pub accepted: Option<Accepted>,
    pub lang: Lang,
    /// A command that may have changed the repository ran: scan it again before the next.
    pub moved: Cell<bool>,
}

impl Gate {
    pub(super) fn new(workdir: PathBuf, follows: Vec<Follow>, saved: Option<SavedTrust>, accepted: Option<Accepted>, lang: Lang) -> Gate {
        Gate { workdir, follows: RefCell::new(follows), saved, accepted, lang, moved: Cell::new(false) }
    }
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
    /// Compares what `cmd`, the job's command number `at`, brings in now; a pull runs here,
    /// pinned, through `git`. After a command that may have changed the repository, its own
    /// settings and hooks are scanned again first.
    pub(super) fn before(&self, at: usize, cmd: &[String], git: &mut glowfly_core::trust::pull::Git<'_>) -> Step {
        if self.moved.get() {
            let now = match git2::Repository::open(&self.workdir) {
                Ok(r) => glowfly_core::trust::scan_now(&r),
                Err(e) => return Step::Halt(Halt::Unchecked(incoming::Unchecked::Unreadable(e.message().to_owned())), String::new()),
            };
            if !glowfly_core::trust::allows(self.saved.as_ref(), &now) {
                return Step::Halt(Halt::Changed(Box::new(now)), String::new());
            }
            *self.follows.borrow_mut() = now.follows;
        }
        self.moved.set(incoming::sub_name(cmd) != Some("fetch"));
        let follows = self.follows.borrow().clone();
        let accept = self.accepted.as_ref().filter(|a| a.at == Some(at)).map(|a| &a.reason);
        if let Some(pull) = Pull::of(cmd).filter(|_| !follows.is_empty()) {
            return match run_pinned(&self.workdir, &pull, &follows, self.saved.as_ref(), accept, git) {
                PullRun::Ran { ok, output } => Step::Ran(ok, output),
                PullRun::Refused { refused, output } => Step::Halt(refused.into(), output),
            };
        }
        // Where nothing follows the working tree there is nothing to compare; otherwise a
        // command Glowfly doesn't read is held.
        let targets = if follows.is_empty() { Ok(Vec::new()) } else { compare::skip_accepted(incoming::targets(cmd), accept) };
        match targets.map_err(Halt::from).and_then(|t| compare(&self.workdir, &follows, self.saved.as_ref(), &t, accept)) {
            Ok(()) => Step::Run,
            Err(h) => Step::Halt(h, String::new()),
        }
    }
}

#[cfg(test)]
#[path = "trust_gate_tests.rs"]
mod tests;
