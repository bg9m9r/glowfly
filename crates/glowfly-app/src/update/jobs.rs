//! Starting the single active job: busy check, the trust gate (update/trust.rs), and the
//! RunningJob plus effect it runs, sealed for the runner's re-check (update/trust_seal.rs);
//! `begin_tool_job` for the merge tool skips both.

use super::trust::Gate;
use crate::effect::{Effect, PickJob};
use crate::state::auto_stash::Making;
use crate::state::trust::Created;
use crate::state::{AppState, RunningJob};
use git2::RepositoryState;
use glowfly_core::journal::Op;
use glowfly_core::undo::UndoKind;
use std::path::PathBuf;
use std::time::Instant;

use crate::state::job_spec::Retry;
pub(super) use crate::state::job_spec::{JobSpec, PickPlan};

/// What became of a job that was asked to start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Started {
    /// It is the active job and its effect was pushed.
    Now,
    /// It waits for the trust prompt; no effect was pushed.
    Held,
    /// Busy, no repository, or dropped; already logged.
    No,
}

/// Whether a job is already running (only one runs at a time); tells the user unless `quiet`.
pub(super) fn busy(st: &mut AppState, quiet: bool) -> bool {
    if st.job.is_some() && !quiet {
        st.log(false, "Another operation is still running.");
    }
    st.job.is_some()
}

/// Runs `spec` as the active job. Holding it for the trust prompt pushes no effect.
pub(super) fn start_job(st: &mut AppState, spec: JobSpec, fx: &mut Vec<Effect>) -> Started {
    // Try again runs the same spec (update/job_retry.rs); a job with its own auto-stash offers
    // Stash and <op> instead, and a quiet one shows no toast.
    let again = (spec.auto_stash.is_none() && !spec.quiet).then(|| spec.clone());
    let auto_stash = spec.auto_stash;
    // A plain job outside the guard: its failure over local changes can still offer the stash.
    let plain = (auto_stash.is_none() && spec.cwd.is_none() && !spec.quiet && spec.cmds.len() == 1).then(|| spec.cmds[0].clone());
    let job = RunningJob {
        label: spec.label,
        workdir: None,
        quiet: spec.quiet,
        clear_commit: spec.clear_commit,
        then_push: spec.then_push,
        then_pr: spec.then_pr,
        then_link: spec.then_link,
        then_open: spec.then_open,
        keeps_undo: super::undo::keeps_history(&spec.cmds),
        // Only a plain fetch or pull refreshes the remote branches. Downloading a partial clone's
        // missing contents (`-c ... fetch <ids>`) writes no ref and no FETCH_HEAD, so it isn't one.
        fetches: spec.cmds.iter().any(|c| matches!(c.first().map(String::as_str), Some("fetch" | "pull"))),
    };
    // Background jobs fail rather than pop up a credential prompt.
    let prompts = !spec.quiet;
    let record = spec.record.or_else(|| match (spec.cmds.as_slice(), &spec.cwd) {
        ([c], None) => UndoKind::classify(c),
        _ => None,
    });
    // Jobs in other repositories (cwd set) get no note. Several commands recorded as one kind
    // (deleting a branch with its remote one, stage all and amend) are noted from the command
    // of that kind, else the last.
    let journal = spec.journal.or_else(|| {
        let k = record.as_ref().filter(|_| spec.cwd.is_none())?;
        let c = spec.cmds.iter().find(|c| UndoKind::classify(c).as_ref() == Some(k)).or(spec.cmds.last())?;
        Op::of_kind(k, c)
    });
    // A branch checkout, pull, merge, cherry-pick, revert, stash pop or reset may be followed by
    // updating the submodules; so may the commit that finishes a merge, cherry-pick or revert
    // that stopped for conflicts (stopping, it failed and updated nothing).
    // In a sequence that commit may finish only one pick: then only the sequence's end updates.
    let state = st.snap.as_ref().map(|s| s.state);
    let finishes = state.is_some_and(|s| glowfly_core::submodule_ops::finishes_operation(s, &spec.cmds));
    let in_sequence = finishes && matches!(state, Some(RepositoryState::CherryPickSequence | RepositoryState::RevertSequence));
    let moves = (glowfly_core::submodule_ops::moves_work_tree(&spec.cmds) || finishes) && spec.cwd.is_none();
    let (label, quiet) = (job.label.clone(), spec.quiet);
    // A fetch in the active repository can pause or resume its remotes' auto-fetch.
    let sign_in_cmds = if spec.cwd.is_none() { spec.cmds.clone() } else { Vec::new() };
    // `git lfs install` adds settings and hooks the trust decision didn't see (trust_inherit.rs).
    let lfs_install = spec.cwd.is_none() && spec.cmds.iter().any(|c| c.windows(2).any(|w| w[0] == "lfs" && w[1] == "install"));
    // A job with a network command can be cancelled while that command runs (update/job_stop.rs).
    let stoppable = spec.write_file.is_none() && crate::state::job_stop::Step::any_network(&spec.cmds);
    let (cmds, write_file, expect, pick, on_fail_check) = (spec.cmds, spec.write_file, spec.expect, spec.pick, spec.on_fail_check);
    let effect = move |cwd| match pick {
        Some(PickPlan { picking, oids, can_skip }) => {
            Effect::Pick(PickJob { cwd, cmds, record, journal, prompts, picking, oids, can_skip })
        }
        None => Effect::RunGit { cwd, cmds, write_file, record, journal, prompts, expect, stoppable, on_fail_check },
    };
    let started = launch(st, job, spec.cwd, effect, fx);
    if started != Started::No {
        st.logs.job_retry = again.and_then(|spec| Some(Retry { tab: st.tabs.get(st.active_tab)?.id, workdir: st.workdir(), spec }));
        super::submodule_batch::job_started(st, &label, moves);
        if in_sequence {
            super::submodule_batch::only_once_finished(st);
        }
        super::auto_stash::started_plain(st, &label, plain);
        // Also for a job held for the trust prompt; dropping it clears this.
        if let Some(job) = auto_stash {
            st.auto_stash.making = Some(Making { label: label.clone(), workdir: st.workdir(), job });
        }
    }
    if started == Started::Now {
        super::sign_in::job_started(st, quiet, &label, &sign_in_cmds);
        if lfs_install {
            super::trust_inherit::expect_lfs_install(st, &label);
        }
    }
    started
}

/// Makes a job labelled `label` the active one in the active tab's repository, running the
/// effect `effect` builds for that folder, unless another job is running (said in the log).
/// Holding it for the trust prompt pushes no effect, so callers that close their dialog only
/// when an effect was pushed keep it open.
pub(super) fn begin_job(
    st: &mut AppState,
    label: &str,
    keeps_undo: bool,
    effect: impl FnOnce(PathBuf) -> Effect,
    fx: &mut Vec<Effect>,
) -> Started {
    let job = RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo,
        fetches: false,
    };
    launch(st, job, None, effect, fx)
}

/// `begin_job` for work Glowfly starts on its own (putting an auto-stash back): quiet (no toast,
/// a quiet log line, no credential prompts), skipped silently while another job runs, and
/// dropped by the trust gate rather than held. It keeps the undo history.
pub(super) fn begin_background_job(
    st: &mut AppState,
    label: &str,
    effect: impl FnOnce(PathBuf) -> Effect,
    fx: &mut Vec<Effect>,
) -> Started {
    let job = RunningJob {
        label: label.into(),
        workdir: None,
        quiet: true,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    };
    launch(st, job, None, effect, fx)
}

/// Makes a job labelled `label` the active one in the active tab's repository, running the
/// effect `effect` builds for that folder, unless another job is running (said in the log).
/// Unlike `begin_job`, the trust prompt never holds it: it is for a program the user chose in
/// their own settings (the merge tool, or a merge command from the user's own git config, never
/// the repository's), and its copies of the repository's files are made
/// without the repository's filters while the repository is not trusted, so nothing the
/// repository configures runs. It keeps the undo history: the tool edits (and at most stages)
/// a working file and moves no refs, like the conflict view's Save and Mark resolved.
pub(super) fn begin_tool_job(st: &mut AppState, label: &str, effect: impl FnOnce(PathBuf) -> Effect, fx: &mut Vec<Effect>) -> Started {
    if busy(st, false) {
        return Started::No;
    }
    let Some(cwd) = st.workdir() else { return Started::No };
    st.logs.job_retry = None;
    st.job = Some(RunningJob {
        label: label.into(),
        workdir: Some(cwd.clone()),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    st.job_stop = None;
    fx.push(effect(cwd));
    Started::Now
}

/// Makes a job labelled `label` that runs in `cwd` (outside the active tab's repository: a clone
/// or an init) the active one, running the effect `effect` builds for that folder; once it
/// succeeds, `then_open` opens in the active tab. The trust gate applies as for git jobs.
pub(super) fn start_effect_job(
    st: &mut AppState,
    label: &str,
    cwd: PathBuf,
    then_open: Option<(PathBuf, Created)>,
    effect: impl FnOnce(PathBuf) -> Effect,
    fx: &mut Vec<Effect>,
) -> Started {
    let job = RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open,
        keeps_undo: false,
        fetches: false,
    };
    launch(st, job, Some(cwd), effect, fx)
}

/// Ends a job that makes a repository (a sparse clone, an init): `made` says the repository
/// exists, `ok` that every step after it worked too. A repository made by a job that failed
/// later is still opened (and trusted as `then_open` says), under the failure's `headline`.
pub(super) fn finish_made(
    st: &mut AppState,
    made: bool,
    ok: bool,
    output: String,
    headline: Option<String>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let open = if made && !ok { st.job.as_mut().and_then(|j| j.then_open.take()) } else { None };
    super::ops::finish_job(st, ok, output.clone(), headline, None, now, fx);
    if let Some((p, how)) = open {
        super::trust::created(st, &p, how, &output);
        super::tabs::open(st, p, fx);
    }
}

/// Makes `job` the active job and pushes its effect for `cwd`, or the active tab's repository,
/// unless one is running or the trust gate of the repository it runs in holds or refuses it
/// (update/trust.rs). A job for another repository than the active tab's (not a clone or an
/// init) belongs to that repository (`RunningJob::workdir`).
fn launch(
    st: &mut AppState,
    mut job: RunningJob,
    cwd: Option<PathBuf>,
    effect: impl FnOnce(PathBuf) -> Effect,
    fx: &mut Vec<Effect>,
) -> Started {
    if busy(st, job.quiet) {
        return Started::No;
    }
    // Only start_job keeps a Try again, after this.
    st.logs.job_retry = None;
    let explicit = cwd.is_some();
    let Some(cwd) = cwd.or_else(|| st.workdir()) else { return Started::No };
    let makes = matches!(job.then_open, Some((_, Created::Clone | Created::Init)));
    let active = st.snap.as_ref().is_some_and(|s| crate::state::tabs::is_repo_at(s, &cwd));
    job.workdir = if explicit && !makes && !active { Some(cwd.clone()) } else { st.workdir() };
    match super::trust::gate(st, &job, &cwd) {
        Gate::Run => {}
        Gate::Hold(prompt, elsewhere) => return super::trust::hold(st, job, effect(cwd), *prompt, elsewhere),
        Gate::Refuse(why) => {
            if job.quiet {
                st.log_quiet(false, why);
            } else {
                st.log(false, why);
            }
            return Started::No;
        }
    }
    let effect = effect(cwd.clone());
    st.job_stop = super::job_stop::for_effect(&effect);
    let sealed = crate::effect::Sealed::Job { label: job.label.clone() };
    st.job = Some(job);
    // A clone or init runs in a folder that isn't a repository yet; anything else is checked
    // again just before it starts (update/trust_seal.rs).
    if makes {
        fx.push(effect);
    } else {
        super::trust_seal::push(st, effect, sealed, &cwd, fx);
    }
    Started::Now
}
