//! Stash and carry on: one guard for checkout, pull, merge, rebase, cherry-pick, revert and hard
//! reset on a dirty work tree, and the stash a Git Flow Finish makes when its dialog's Stash
//! changes and finish asks for one (`wrap` directly, without the gate).
//!
//! Each entry point calls `gate` with the command it would run and a retry message. On a dirty
//! tree the gate opens the prompt (`Modal::AutoStash`) holding the retry; its answer sets
//! `AutoStashes::approved` for exactly the length of the retry, which reaches the gate again
//! and is let through. `wrap` puts `git stash push -m <message>` in front of the operation's
//! command (one RunGit job), and `JobSpec::auto_stash` marks the job (`making`). When it ends,
//! `job_finished` arms a pending entry with `from = load_gen + 1`, and `seen`, after each
//! snapshot of that tab, finds the stash and, once nothing is in progress, puts it back as a
//! quiet job (`StashEffect::PutBack`), whose result `handle` turns into a toast.
//!
//! A guarded job the gate lets run without stashing (the snapshot showed nothing to stash) is
//! recorded with its retry (`ran`, `AutoStashes::unstashed`); when it fails because changes were
//! in the way, its toast's Stash and <op> sends that retry with the stash approved
//! (update/job_errors.rs, `AutoStashMsg::Retry`).

use super::jobs::{JobSpec, Started, begin_background_job, start_job};
use crate::effect::{Effect, PutBackRequest, StashEffect};
use crate::msg::{AutoStashMsg, Msg};
use crate::state::auto_stash::{
    Approval, AutoStashAsk, AutoStashJob, PendingStash, Phase, StashRetry, Unstashed, back, blocks, changed_files, conflicted, failed,
    found, gone, made, ready,
};
use crate::state::toasts::ToastAction;
use crate::state::{AppState, Modal, RunningJob};
use glowfly_core::paths::same_spelling;
use glowfly_core::stash_ops::{AutoStashFor, PutBack};
use glowfly_core::undo::UndoKind;
use std::time::Instant;

/// The quiet job that puts an auto-stash back.
pub(super) const PUT_BACK_LABEL: &str = "Put back stashed changes";

/// What an entry point does with its command.
pub(super) enum Gate {
    /// Run it as it is: nothing to stash, nothing guarded, or Discard and reset. With the retry
    /// of a guarded command that runs without stashing, for its failure's Stash and <op>.
    Run(Option<StashRetry>),
    /// Stash first (`wrap`).
    Stash(AutoStashFor),
    /// The prompt opened, or couldn't (logged): run nothing now.
    Asked,
}

/// Whether `cmd` may run now on the active tab's tree; `retry` is the message that starts it
/// again once the prompt is answered.
pub(super) fn gate(st: &mut AppState, cmd: &[String], retry: impl FnOnce() -> Msg) -> Gate {
    let Some(snap) = st.snap.as_ref() else { return Gate::Run(None) };
    let Some(op) = AutoStashFor::of_command(cmd, snap.head_branch.as_deref()) else { return Gate::Run(None) };
    // The prompt's answer or a failure toast's Stash and <op>; also when the snapshot hasn't
    // caught up with the changes.
    if let Some(a) = st.auto_stash.approved.take_if(|a| a.op == op) {
        return if a.stash { Gate::Stash(op) } else { Gate::Run(None) };
    }
    if !blocks(snap) {
        return Gate::Run(Some(StashRetry { op, retry: Box::new(retry()) }));
    }
    let files = changed_files(snap);
    if st.settings.auto_stash && !op.is_hard_reset() {
        return Gate::Stash(op);
    }
    if super::jobs::busy(st, false) {
        return Gate::Asked;
    }
    if st.modal.is_some() {
        let t = st.settings.lang();
        st.log(false, t.tr("auto_stash.busy_dialog"));
        return Gate::Asked;
    }
    st.modal = Some(Modal::AutoStash(AutoStashAsk { op, files, always: false, retry: Box::new(retry()) }));
    Gate::Asked
}

/// Starts `spec` (one command) through the gate: as it is, stashing first, or not yet.
pub(super) fn start_guarded(st: &mut AppState, mut spec: JobSpec, retry: impl FnOnce() -> Msg, fx: &mut Vec<Effect>) -> Started {
    if spec.cmds.len() != 1 {
        return start_job(st, spec, fx);
    }
    let (cmd, label) = (spec.cmds[0].clone(), spec.label.clone());
    match gate(st, &cmd, retry) {
        Gate::Asked => Started::No,
        Gate::Run(r) => {
            let started = start_job(st, spec, fx);
            ran(st, &label, started, r);
            started
        }
        Gate::Stash(op) => {
            wrap(st, &mut spec, op);
            start_job(st, spec, fx)
        }
    }
}

/// A guarded job labelled `label` was asked to start without an auto-stash: records `retry`
/// for its failure toast while it runs or waits for the trust prompt (any older record is
/// stale by now).
pub(super) fn ran(st: &mut AppState, label: &str, started: Started, retry: Option<StashRetry>) {
    if started != Started::No {
        let workdir = st.workdir();
        st.auto_stash.unstashed = retry.map(|retry| Unstashed { label: label.to_owned(), workdir, retry });
    }
}

/// The retry recorded for `job`, which just ended (always cleared: only the job it was recorded
/// for can use it).
pub(super) fn take_retry(st: &mut AppState, job: &RunningJob) -> Option<StashRetry> {
    let u = st.auto_stash.unstashed.take()?;
    let same = u.label == job.label && matches!((&u.workdir, &job.workdir), (Some(a), Some(b)) if same_spelling(a, b));
    same.then_some(u.retry)
}

/// Puts the auto-stash for `op` in front of `spec`'s one command, keeping the job recorded for
/// undo, and noted in the journal, as that command.
pub(super) fn wrap(st: &AppState, spec: &mut JobSpec, op: AutoStashFor) {
    if spec.cwd.is_none()
        && let [c] = spec.cmds.as_slice()
    {
        if spec.record.is_none() {
            spec.record = UndoKind::classify(c);
        }
        if spec.journal.is_none() {
            spec.journal = spec.record.as_ref().and_then(|k| glowfly_core::journal::Op::of_kind(k, c));
        }
    }
    spec.cmds.insert(0, op.stash_command());
    let before = st.snap.as_ref().map(|s| s.stashes.iter().map(|x| x.oid).collect()).unwrap_or_default();
    spec.auto_stash = Some(AutoStashJob { message: op.message(), before, op });
}

/// `job` ended (ops::finish_job, before its refresh): when it made an auto-stash, its tab's
/// next snapshots look for the stash and put it back.
pub(super) fn job_finished(st: &mut AppState, job: &RunningJob) {
    let Some(m) = st.auto_stash.making.take() else { return };
    let same = m.label == job.label && matches!((&m.workdir, &job.workdir), (Some(a), Some(b)) if same_spelling(a, b));
    let (true, Some(workdir)) = (same, m.workdir) else { return };
    if let Some(i) = st.auto_stash.index_of(&workdir) {
        let old = st.auto_stash.pending.remove(i);
        st.log_quiet(true, format!("Stopped waiting to put back \"{}\"; it stays in the stash list.", old.job.message));
    }
    // A load that started before the job ended can still arrive with the current generation.
    let from = st.load_gen + 1;
    st.auto_stash.pending.push(PendingStash { workdir, job: m.job, oid: None, phase: Phase::Ended { from }, told: false });
}

/// After the active tab's snapshot changed (`full`: a repository load, whose stash list is
/// current): finds its auto-stash and puts it back once nothing is in progress.
pub(super) fn seen(st: &mut AppState, full: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(i) = st.auto_stash.index_of(&snap.workdir) else { return };
    let p = &mut st.auto_stash.pending[i];
    if p.oid.is_none() {
        p.oid = found(snap, &p.job);
    }
    let Phase::Ended { from } = p.phase else { return };
    if st.load_gen < from {
        return;
    }
    let Some(oid) = p.oid else {
        // git said "No local changes to save", or the push failed.
        if full {
            let gone = st.auto_stash.pending.remove(i);
            st.log_quiet(true, format!("Nothing was stashed for \"{}\", so there is nothing to put back.", gone.job.message));
        }
        return;
    };
    if st.job.is_some() || st.trust.held.is_some() {
        return;
    }
    if !ready(snap) {
        if !p.told {
            p.told = true;
            let (t, workdir) = (st.settings.lang(), p.workdir.clone());
            let text = made(t, &p.job.message);
            st.toast_action(true, text, show_action(t, oid, workdir), now);
        }
        return;
    }
    // A stopped Git Flow finish still has steps that need a clean tree (a checkout): its
    // changes wait for Continue finish, or for its steps to be forgotten or dropped. Only that
    // branch's stash waits; one from finishing another flow branch goes back as usual.
    if let AutoStashFor::GitFlowFinish { branch } = &p.job.op
        && st.gitflow_resume.as_ref().is_some_and(|r| r.resume.plan.branch == *branch)
    {
        return;
    }
    let effect = move |cwd| Effect::Stash(StashEffect::PutBack(PutBackRequest { cwd, oid }));
    if begin_background_job(st, PUT_BACK_LABEL, effect, fx) == Started::Now {
        st.auto_stash.pending[i].phase = Phase::PuttingBack;
    }
}

fn show_action(t: glowfly_core::i18n::Lang, oid: git2::Oid, workdir: std::path::PathBuf) -> ToastAction {
    ToastAction {
        label: t.tr("auto_stash.show.button").into(),
        msg: Box::new(Msg::AutoStash(AutoStashMsg::Show(oid))),
        workdir: Some(workdir),
    }
}

pub(super) fn handle(st: &mut AppState, msg: AutoStashMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        AutoStashMsg::Stash => answer(st, true, now, fx),
        AutoStashMsg::Retry(r) => {
            let StashRetry { op, retry } = *r;
            approve_and_run(st, op, true, retry, now, fx);
        }
        AutoStashMsg::Discard => answer(st, false, now, fx),
        AutoStashMsg::PutBack { workdir, oid, outcome, output, undo } => {
            let undo = undo.map(|u| *u);
            let i = st.auto_stash.pending.iter().position(|p| same_spelling(&p.workdir, &workdir) && p.oid == Some(oid));
            if outcome == PutBack::NotReady {
                // Two loads on: the one this job's end asks for can't start it again, so a
                // snapshot that keeps disagreeing with the repository never loops.
                let from = st.load_gen + 2;
                if let Some(i) = i {
                    st.auto_stash.pending[i].phase = Phase::Ended { from };
                }
                return super::ops::finish_job(st, true, output, None, undo, now, fx);
            }
            let entry = i.map(|i| st.auto_stash.pending.remove(i));
            let ok = matches!(outcome, PutBack::Restored | PutBack::Gone);
            super::ops::finish_job(st, ok, output.clone(), None, undo, now, fx);
            let Some(entry) = entry else { return };
            told(st, &entry.job.message, outcome, &output, oid, workdir, now);
        }
        AutoStashMsg::Show(oid) => {
            if st.snap.as_ref().is_some_and(|s| s.stashes.iter().any(|x| x.oid == oid)) {
                st.view.sections.stashes = true;
                super::repo::select_commit(st, oid, false, fx);
            } else {
                let t = st.settings.lang();
                st.toast(true, t.tr("auto_stash.show.gone"), now);
            }
        }
    }
}

/// The prompt's primary (`stash`) or Discard and reset: runs the retry with the answer.
fn answer(st: &mut AppState, stash: bool, now: Instant, fx: &mut Vec<Effect>) {
    let Some(Modal::AutoStash(ask)) = st.modal.take() else { return };
    if !stash && !ask.op.is_hard_reset() {
        st.modal = Some(Modal::AutoStash(ask));
        return;
    }
    if stash && ask.always && !st.settings.auto_stash {
        st.settings.auto_stash = true;
        fx.push(super::prefs::save_settings(st));
    }
    approve_and_run(st, ask.op, stash, ask.retry, now, fx);
}

/// Runs `retry` with the answer `stash` for `op`, which the gate takes when the retry reaches
/// it.
fn approve_and_run(st: &mut AppState, op: AutoStashFor, stash: bool, retry: Box<Msg>, now: Instant, fx: &mut Vec<Effect>) {
    st.auto_stash.approved = Some(Approval { op, stash });
    fx.extend(super::update(st, *retry, now));
    // The answer never outlives its retry.
    st.auto_stash.approved = None;
}

/// Tells how putting back the stash `name` went, in a toast and the log.
fn told(st: &mut AppState, name: &str, outcome: PutBack, output: &str, oid: git2::Oid, workdir: std::path::PathBuf, now: Instant) {
    let t = st.settings.lang();
    match outcome {
        PutBack::Restored | PutBack::Gone => {
            let text = if outcome == PutBack::Restored { back(t, name) } else { gone(t, name) };
            st.toast(true, text.clone(), now);
            st.log(true, text);
        }
        PutBack::Conflicted { paths } => {
            let text = conflicted(t, name, paths.len());
            st.toast_action(false, text.clone(), show_action(t, oid, workdir), now);
            st.log(false, text);
        }
        PutBack::Failed | PutBack::NotReady => {
            let text = failed(t, name, super::ops::last_line(output));
            st.toast_action(false, text.clone(), show_action(t, oid, workdir), now);
            st.log(false, text);
        }
    }
}
