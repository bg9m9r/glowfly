//! The `Execute` impl of `JobEffect`: a job's git commands (`RunGit`), with the expected
//! revision's check (expect.rs), the undo record, the trust gate's re-run (trust.rs), a stop for
//! network commands (job_steps.rs), the journal note (journal.rs) and the saved record of an
//! auto-stash, written once its first command made the stash (auto_stash_made.rs), a failed job's
//! check through the gate too (none after the gate stopped the job); and undoing or
//! redoing an action (undo_run.rs). Apply and Unstash of a stash run `--index` unless something is
//! staged, and once more without it after git refuses to restore the staged state
//! (`stash_ops::retry_unstaged`).

use super::execute::{Cx, Execute};
use super::{auto_stash_made, expect, git_ops, job_steps, journal, op_stops, undo_run};
use glowfly_core::undo;
use glowfly_model::effect::{Effect, JobEffect, UndoResult};
use glowfly_model::msg::{JobMsg, Msg};
use std::path::Path;

impl Execute for JobEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        match self {
            JobEffect::RunGit {
                cwd,
                cmds,
                write_file,
                record,
                journal,
                prompts,
                expect,
                stoppable: _,
                on_fail_check,
                without_index,
                auto_stash,
            } => {
                let picked = expect.as_ref().map(|(_, oid)| *oid);
                if let Some(done) = refused(&cwd, &cmds, expect, record.is_some()) {
                    return Some(done);
                }
                // Git resets the index when it applies a stash with `--index` onto staged changes,
                // losing the selection, so the plain form runs then.
                let (cmds, without_index) = match without_index {
                    Some(plain) if glowfly_core::stash_ops::anything_staged(&cwd) => (vec![plain], None),
                    other => (cmds, other),
                };
                let before = record.as_ref().and_then(|k| undo::before(&cwd, k).ok());
                // A pop onto local changes mixes them with the stash's, so it can't be undone.
                let irreversible = before.as_ref().is_some_and(undo::irreversible);
                let (stops, aborts) = (op_stops::can_stop(&cmds, record.as_ref()), undo::is_abort(&cmds));
                // Held again before anything but a fetch ran: the job as it was asked for.
                let again = cx.gate.map(|(_, check)| {
                    let job = Effect::Job(JobEffect::RunGit {
                        cwd: cwd.clone(),
                        cmds: cmds.clone(),
                        write_file: write_file.clone(),
                        record: record.clone(),
                        journal: journal.clone(),
                        prompts,
                        expect: None,
                        stoppable: false,
                        on_fail_check: on_fail_check.clone(),
                        without_index: without_index.clone(),
                        auto_stash: auto_stash.clone(),
                    });
                    (job, check.clone())
                });
                // The journal note sees the refs before git runs and after the undo record is read.
                // The gate stopped a command after others ran: nothing more runs, its check neither.
                let mut halted = false;
                Some(journal::around(&cwd, journal, || {
                    let mut starting = auto_stash_made::starting(&cwd, auto_stash.as_deref(), cx.send);
                    // A job with a network command has a stop, and it writes no file (job_steps.rs).
                    let (ok, output, start, stopped) = match (cx.stop.filter(|_| write_file.is_none()), cx.gate) {
                        (Some(stop), None) => {
                            let r = job_steps::run(&cwd, &cmds, prompts, stop, cx.send, stops, &mut starting);
                            (r.ok, r.output, r.start, r.stopped)
                        }
                        (stop, Some((gate, _))) => {
                            let mut live = stop.map(|s| job_steps::Live::new(&cwd, prompts, s, cx.send));
                            let (gated, start) = match live.as_mut() {
                                Some(l) => {
                                    git_ops::run_git_gated_with(&cwd, &cmds, gate, stops, &mut |a, i, e| l.git(a, i, e), &mut starting)
                                }
                                None => git_ops::run_git_gated_noting(&cwd, &cmds, write_file, prompts, gate, stops, &mut starting),
                            };
                            let stopped = live.as_ref().is_some_and(|l| l.stopped);
                            match gated {
                                git_ops::Gated::Done(ok, output) => {
                                    let output = match &live {
                                        Some(l) => l.finish(output),
                                        None => output,
                                    };
                                    (ok && !stopped, output, start, stopped)
                                }
                                git_ops::Gated::Halted { halt, moved: false, at, .. } => {
                                    let (job, check) = again.expect("a gate");
                                    let now = git2::Repository::open(&cwd).map(|r| glowfly_core::trust::scan_now(&r)).unwrap_or_default();
                                    return match super::trust::halted(now, halt) {
                                        super::trust::Verdict::Stale(f) => super::trust::stale(check, job, f.0, f.1, None),
                                        super::trust::Verdict::Unchecked(f) => {
                                            let accepted = glowfly_model::effect::Accepted { at: Some(at), reason: f.1 };
                                            super::trust::stale(check, job, f.0, None, Some(accepted))
                                        }
                                        _ => unreachable!("halted is held or unchecked"),
                                    };
                                }
                                git_ops::Gated::Halted { output, halt, .. } => {
                                    halted = true;
                                    (false, format!("{output}\n{}", halt.describe(gate.lang)), None, false)
                                }
                            }
                        }
                        (None, None) if stops => {
                            let (ok, output, start) = op_stops::run_noting_start(&cwd, &cmds, write_file, prompts, &mut starting);
                            (ok, output, start, false)
                        }
                        (None, None) => {
                            let (ok, output) = git_ops::run_git_each(&cwd, &cmds, write_file, prompts, &mut starting);
                            (ok, output, None, false)
                        }
                    };
                    // Git refused the staged part of a stash: the same action once more without it.
                    let (ok, output) = match without_index.filter(|_| !ok && !stopped) {
                        Some(plain) => retry_unstaged(cx, &cwd, &plain, picked, ok, output),
                        None => (ok, output),
                    };
                    // A tag delete is recorded even when a later remote delete failed or was stopped.
                    let keep = ok || record.as_ref().is_some_and(undo::UndoKind::kept_when_failed);
                    let recorded = |k| if keep { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
                    let undo = if ok && irreversible { None } else { record.map(|k| UndoResult::Recorded(recorded(k))) };
                    let undo = op_stops::stopped(&cwd, ok, start).or(undo);
                    let undo = if ok && aborts { Some(UndoResult::Aborted(undo::op_start(&cwd))) } else { undo };
                    if stopped {
                        return Msg::Job(JobMsg::Stopped { output, undo });
                    }
                    let check = on_fail_check.filter(|_| !ok && !halted);
                    after_failure(cx, &cwd, check, prompts, cmds.len(), ok, output, undo)
                }))
            }
            JobEffect::UndoRedo { cwd, action, redo, opts } => {
                let what = action.describe();
                let op = if redo { glowfly_core::journal::Op::Redo { what } } else { glowfly_core::journal::Op::Undo { what } };
                let gate = cx.gate.map(|(g, _)| g);
                Some(journal::around(&cwd, Some(op), || undo_run::undo_redo(&cwd, action, redo, opts, gate, cx.seams.trashers)))
            }
        }
    }
}

/// The job's result: its `check` (`on_fail_check`, after it failed, unless the gate stopped it)
/// runs first, through the gate like the job's own commands, as the one after its `count`.
#[allow(clippy::too_many_arguments)]
fn after_failure(
    cx: &Cx<'_>,
    cwd: &Path,
    check: Option<Vec<String>>,
    prompts: bool,
    count: usize,
    ok: bool,
    mut output: String,
    undo: Option<UndoResult>,
) -> Msg {
    let gate = cx.gate.map(|(g, _)| (g, count));
    match check {
        Some(check) if git_ops::check_passes(cwd, &check, prompts, gate, &mut output) => Msg::Job(JobMsg::FailCheckPassed { output, undo }),
        _ => Msg::Job(JobMsg::JobDone { ok, output, undo }),
    }
}

/// After a failed `--index` run of a stash, the plain command once more when git refused the
/// staged part (`stash_ops::retry_unstaged`), through the trust gate when there is one: a command
/// it holds fails the job with the gate's text. Anything else keeps the failure as it is.
fn retry_unstaged(cx: &Cx<'_>, cwd: &Path, plain: &[String], picked: Option<git2::Oid>, ok: bool, output: String) -> (bool, String) {
    let git = undo_run::UndoGit { gate: cx.gate.map(|(g, _)| g), trashers: cx.seams.trashers };
    glowfly_core::stash_ops::retry_unstaged(cwd, &output, plain, picked, &mut |c| {
        let r = git.run(cwd, &[c.to_vec()]);
        (r.ok, r.output)
    })
    .unwrap_or((ok, output))
}

/// The job's result when it must not run: the expected revision moved, or something sits where a
/// stash of a staged rename would put the old file back (the dialog may be older than the disk).
fn refused(cwd: &Path, cmds: &[Vec<String>], expect: Option<(String, git2::Oid)>, recorded: bool) -> Option<Msg> {
    let output = match expect.and_then(|(rev, oid)| expect::check(cwd, &rev, oid).err()) {
        Some(output) => output,
        None => glowfly_core::stash_ops::restore_blocked_output(&glowfly_core::stash_ops::restore_blocked(cwd, cmds)?),
    };
    Some(Msg::Job(JobMsg::JobDone { ok: false, output, undo: recorded.then_some(UndoResult::Recorded(None)) }))
}
