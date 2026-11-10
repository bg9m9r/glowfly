//! The `Execute` impl of `JobEffect`: a job's git commands (`RunGit`), with the expected
//! revision's check (expect.rs), the undo record, the trust gate's re-run (trust.rs), a stop for
//! network commands (job_steps.rs), the journal note (journal.rs) and the saved record of an
//! auto-stash, written once its first command made the stash (auto_stash_made.rs); and undoing or
//! redoing an action (undo_run.rs).

use super::execute::{Cx, Execute};
use super::{auto_stash_made, expect, git_ops, job_steps, journal, op_stops, undo_run};
use glowfly_core::undo;
use glowfly_model::effect::{Effect, JobEffect, UndoResult};
use glowfly_model::msg::{JobMsg, Msg};

impl Execute for JobEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        match self {
            JobEffect::RunGit { cwd, cmds, write_file, record, journal, prompts, expect, stoppable: _, on_fail_check, auto_stash } => {
                if let Some(refused) = expect.and_then(|(rev, oid)| expect::check(&cwd, &rev, oid).err()) {
                    return Some(Msg::Job(JobMsg::JobDone {
                        ok: false,
                        output: refused,
                        undo: record.map(|_| UndoResult::Recorded(None)),
                    }));
                }
                // A stash that puts a staged rename's old file back stops before stashing when
                // something has appeared at that path since the dialog was drawn.
                if let Some(p) = glowfly_core::stash_ops::restore_blocked(&cwd, &cmds) {
                    let output = glowfly_core::stash_ops::restore_blocked_output(&p);
                    return Some(Msg::Job(JobMsg::JobDone { ok: false, output, undo: record.map(|_| UndoResult::Recorded(None)) }));
                }
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
                        auto_stash: auto_stash.clone(),
                    });
                    (job, check.clone())
                });
                // The journal note sees the refs before git runs and after the undo record is read.
                Some(journal::around(&cwd, journal, || {
                    let mut starting = auto_stash_made::starting(&cwd, auto_stash.as_ref(), cx.send);
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
                    // A tag delete is recorded even when a later remote delete failed or was stopped.
                    let keep = ok || record.as_ref().is_some_and(undo::UndoKind::kept_when_failed);
                    let recorded = |k| if keep { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
                    let undo = if ok && irreversible { None } else { record.map(|k| UndoResult::Recorded(recorded(k))) };
                    let undo = op_stops::stopped(&cwd, ok, start).or(undo);
                    let undo = if ok && aborts { Some(UndoResult::Aborted(undo::op_start(&cwd))) } else { undo };
                    if stopped {
                        return Msg::Job(JobMsg::Stopped { output, undo });
                    }
                    let mut output = output;
                    match on_fail_check.filter(|_| !ok) {
                        Some(check) if git_ops::check_passes(&cwd, &check, prompts, &mut output) => {
                            Msg::Job(JobMsg::FailCheckPassed { output, undo })
                        }
                        _ => Msg::Job(JobMsg::JobDone { ok, output, undo }),
                    }
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
