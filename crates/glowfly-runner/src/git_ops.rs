//! Local git operations run by the runner: staging and discarding ops, conflict changes such as
//! Mark resolved and taking the trivial picks in many files, and running a list of git commands (through the trust gate too). A stage,
//! unstage or discard that failed on a lock file reports `LockMsg::LocalLocked` with the op, for
//! Try again and Delete lock file. Undo and redo are in `undo_run.rs`.

use glowfly_core::error::WriteFailed;
use glowfly_core::gitcli;
use glowfly_core::lock_errors::locked_write;
use glowfly_core::repo;
use glowfly_core::undo::{self, Action, ConflictChange, ConflictsFailed, Discarded};
use glowfly_model::effect::LocalOp;
use glowfly_model::msg::{ConflictMsg, LockMsg, Msg, StageMsg};
use std::path::Path;

/// Runs a local op. Discards, conflict saves and conflict changes (Mark resolved, Mark
/// conflicted, a whole side taken) save the file's contents first and return the undo action.
/// A hunk or lines shown with whitespace changes hidden go through `repo::apply_shown`.
pub(super) fn local_op(workdir: &Path, op: LocalOp) -> glowfly_core::error::Result<Option<Action>> {
    use glowfly_core::repo::HunkOp::Discard;
    let saving =
        |path: &str, what: Discarded, f: &dyn Fn() -> glowfly_core::error::Result<()>| undo::discard_saving(workdir, path, what, f);
    let done = |r: glowfly_core::error::Result<()>| r.map(|()| None);
    match op {
        LocalOp::StageFile(p) => done(repo::stage_file(workdir, &p)),
        LocalOp::StagePaths(paths) => done(repo::stage_paths(workdir, &paths)),
        LocalOp::StageAll => done(repo::stage_all(workdir)),
        LocalOp::UnstagePaths(paths) => done(repo::unstage_paths(workdir, &paths)),
        LocalOp::DiscardFile(path) => saving(&path, Discarded::File, &|| repo::discard_file(workdir, &path)),
        LocalOp::Hunk { op, path, index, expected, ignore_whitespace } => {
            let apply = || {
                if ignore_whitespace {
                    repo::apply_shown(workdir, &path, index, &expected, None, op)
                } else {
                    repo::apply_hunk(workdir, &path, index, &expected, op)
                }
            };
            if op == Discard { saving(&path, Discarded::Hunk, &apply) } else { done(apply()) }
        }
        LocalOp::Lines { op, path, hunk, expected, lines, ignore_whitespace } => {
            let apply = || {
                if ignore_whitespace {
                    repo::apply_shown(workdir, &path, hunk, &expected, Some(&lines), op)
                } else {
                    repo::apply_lines(workdir, &path, hunk, &expected, &lines, op)
                }
            };
            if op == Discard { saving(&path, Discarded::Lines(lines.len()), &apply) } else { done(apply()) }
        }
        LocalOp::ResolveConflict { path, contents, expected } => {
            undo::resolve_saving(workdir, &path, &expected, || repo::write_resolution(workdir, &path, &contents, &expected))
        }
        LocalOp::SavePartialResolution { path, contents, expected } => {
            undo::resolve_saving(workdir, &path, &expected, || repo::write_partial_resolution(workdir, &path, &contents, &expected))
        }
        LocalOp::ResolveWithSide { .. } | LocalOp::MarkResolved(_) | LocalOp::MarkConflicted(_) => {
            conflict_change(workdir, op).map_err(|f| f.error)
        }
        LocalOp::TakeTrivialPicks(paths) => take_trivial(workdir, &paths).map(|(_, undo)| undo).map_err(|f| f.error),
    }
}

/// Runs a local op and says how it went. A conflict change that failed partway carries the
/// undo action for the files it changed. A failure on a lock file is worded as one
/// (`lock_errors::locked_write`); for an op that may run again it is `LockMsg::LocalLocked`.
pub(super) fn local_done(workdir: &Path, op: LocalOp) -> Msg {
    if let LocalOp::TakeTrivialPicks(paths) = &op {
        return match take_trivial(workdir, paths) {
            Ok((taken, undo)) => Msg::Conflict(ConflictMsg::TrivialPicksTaken { taken, undo }),
            Err(ConflictsFailed { error, undo: None }) => lock_or_done(workdir, None, locked_write(workdir, error)),
            Err(ConflictsFailed { error, undo: Some(undo) }) => {
                Msg::Stage(StageMsg::LocalOpFailed { error: WriteFailed::from(error), undo: *undo })
            }
        };
    }
    let again = op.retryable().then(|| op.clone());
    let conflict = matches!(op, LocalOp::ResolveWithSide { .. } | LocalOp::MarkResolved(_) | LocalOp::MarkConflicted(_));
    let result = if conflict { conflict_change(workdir, op) } else { local_op(workdir, op).map_err(ConflictsFailed::from) };
    match result {
        Ok(undo) => Msg::Stage(StageMsg::LocalOpDone(Ok(undo))),
        Err(ConflictsFailed { error, undo: None }) => lock_or_done(workdir, again, locked_write(workdir, error)),
        Err(ConflictsFailed { error, undo: Some(undo) }) => {
            Msg::Stage(StageMsg::LocalOpFailed { error: WriteFailed::from(error), undo: *undo })
        }
    }
}

/// `LockMsg::LocalLocked` for a lock failure of an op that may run `again`, else the plain
/// failed result.
fn lock_or_done(workdir: &Path, again: Option<LocalOp>, error: WriteFailed) -> Msg {
    match (&error, again) {
        (WriteFailed::Locked { lock, .. }, Some(op)) => {
            Msg::Lock(LockMsg::LocalLocked { workdir: workdir.to_path_buf(), op: Box::new(op), lock: lock.clone(), error })
        }
        _ => Msg::Stage(StageMsg::LocalOpDone(Err(error))),
    }
}

/// Mark resolved, Mark conflicted or a whole side taken, recorded for undo.
fn conflict_change(workdir: &Path, op: LocalOp) -> Result<Option<Action>, ConflictsFailed> {
    match op {
        LocalOp::ResolveWithSide { path, side, expected } => {
            let paths = [path];
            undo::conflicts_saving(workdir, ConflictChange::Took(side), &paths, || {
                repo::resolve_with_side(workdir, &paths[0], side, &expected)
            })
        }
        LocalOp::MarkResolved(paths) => {
            undo::conflicts_saving(workdir, ConflictChange::MarkResolved, &paths, || repo::mark_resolved(workdir, &paths))
        }
        LocalOp::MarkConflicted(paths) => {
            undo::conflicts_saving(workdir, ConflictChange::MarkConflicted, &paths, || repo::mark_conflicted(workdir, &paths))
        }
        other => unreachable!("not a conflict change: {other:?}"),
    }
}

/// Takes the trivial picks in the conflicted `paths` (`repo::trivial_plan`): the files it
/// resolves are one undo entry, and the files it left or skipped come back by name.
fn take_trivial(workdir: &Path, paths: &[String]) -> Result<(repo::TrivialTaken, Option<Action>), ConflictsFailed> {
    let plan = repo::trivial_plan(workdir, paths);
    let resolve = plan.paths();
    let undo = if resolve.is_empty() {
        None
    } else {
        undo::conflicts_saving(workdir, ConflictChange::TrivialPicks, &resolve, || plan.apply(workdir))?
    };
    Ok((plan.into_taken(), undo))
}

/// The caller environment for a job's git commands: none for one that may prompt, else
/// [`gitcli::BACKGROUND`] (no prompts, and no Git Credential Manager window either).
pub(super) fn job_env(prompts: bool) -> &'static [(&'static str, &'static str)] {
    if prompts { &[] } else { gitcli::BACKGROUND }
}

/// Runs `cmds` in order, stopping at the first failure. Without `prompts`, git fails rather than
/// open a credential prompt.
pub(super) fn run_git(
    cwd: &Path,
    cmds: &[Vec<String>],
    write_file: Option<(std::path::PathBuf, Vec<u8>)>,
    prompts: bool,
) -> (bool, String) {
    run_git_each(cwd, cmds, write_file, prompts, &mut |_| {})
}

/// `run_git`, calling `starting(i)` before command `i` runs (so `starting(1)` means the first
/// one succeeded).
pub(super) fn run_git_each(
    cwd: &Path,
    cmds: &[Vec<String>],
    write_file: Option<(std::path::PathBuf, Vec<u8>)>,
    prompts: bool,
    starting: &mut dyn FnMut(usize),
) -> (bool, String) {
    if let Some((path, contents)) = write_file
        && let Err(e) = glowfly_core::persist::atomic_write(&path, &contents)
    {
        return (false, format!("Could not write {}: {e}", path.display()));
    }
    let mut output = String::new();
    for (at, c) in cmds.iter().enumerate() {
        starting(at);
        output.push_str(&format!("{}\n", glowfly_core::text::command_echo(c)));
        let (success, text) = gitcli::run_with_input(cwd, c, "", job_env(prompts));
        if !text.is_empty() {
            output.push_str(&text);
            output.push('\n');
        }
        if !success {
            return (false, output.trim_end().to_owned());
        }
    }
    (true, output.trim_end().to_owned())
}

/// How a job's commands went through the trust gate (`run_git_gated`).
pub(super) enum Gated {
    /// They ran to the end or to the first failure: whether all succeeded, and the output.
    Done(bool, String),
    /// The gate stopped one: why, the output so far, and whether a command before it could have
    /// changed the repository (anything but a fetch ran).
    /// `at`: the index of the command it stopped before.
    Halted { halt: super::trust_gate::Halt, output: String, moved: bool, at: usize },
}

/// `run_git` with each command compared by `gate` first (runner/trust_gate.rs): a pull runs
/// there, pinned to the commits its fetch brought, and a command whose commits aren't covered
/// stops the job before it runs.
pub(super) fn run_git_gated(
    cwd: &Path,
    cmds: &[Vec<String>],
    write_file: Option<(std::path::PathBuf, Vec<u8>)>,
    prompts: bool,
    gate: &super::trust_gate::Gate,
) -> Gated {
    run_git_gated_noting(cwd, cmds, write_file, prompts, gate, false, &mut |_| {}).0
}

/// `run_git_gated`, noting where an operation starts (`undo::op_start`) just before the last
/// command when `note` is set (`op_stops.rs`), and calling `starting(i)` before command `i`.
pub(super) fn run_git_gated_noting(
    cwd: &Path,
    cmds: &[Vec<String>],
    write_file: Option<(std::path::PathBuf, Vec<u8>)>,
    prompts: bool,
    gate: &super::trust_gate::Gate,
    note: bool,
    starting: &mut dyn FnMut(usize),
) -> (Gated, Option<undo::OpStart>) {
    if let Some((path, contents)) = write_file
        && let Err(e) = glowfly_core::persist::atomic_write(&path, &contents)
    {
        return (Gated::Done(false, format!("Could not write {}: {e}", path.display())), None);
    }
    let mut git = |c: &[String], input: &str, extra: &[(&str, &str)]| {
        let env: Vec<(&str, &str)> = job_env(prompts).iter().chain(extra).copied().collect();
        gitcli::run_with_input(cwd, c, input, &env)
    };
    run_git_gated_with(cwd, cmds, gate, note, &mut git, starting)
}

/// `run_git_gated_noting` with each command (a pull's fetch and merge too) run by `git`: a
/// stoppable job's (runner/job_steps.rs). Writes no file.
pub(super) fn run_git_gated_with(
    cwd: &Path,
    cmds: &[Vec<String>],
    gate: &super::trust_gate::Gate,
    note: bool,
    git: &mut glowfly_core::trust::pull::Git<'_>,
    starting: &mut dyn FnMut(usize),
) -> (Gated, Option<undo::OpStart>) {
    let mut start = None;
    let gated = gated_steps(cmds, gate, git, &mut |at| {
        starting(at);
        if note && at + 1 == cmds.len() {
            start = undo::op_start(cwd);
        }
    });
    (gated, start)
}

fn gated_steps(
    cmds: &[Vec<String>],
    gate: &super::trust_gate::Gate,
    git: &mut glowfly_core::trust::pull::Git<'_>,
    before: &mut dyn FnMut(usize),
) -> Gated {
    use super::trust_gate::Step;
    let mut output = String::new();
    let mut moved = false;
    for (at, c) in cmds.iter().enumerate() {
        before(at);
        let (success, text) = match gate.before(at, c, git) {
            Step::Halt(halt, ran) => {
                output.push_str(&ran);
                return Gated::Halted { halt, output: output.trim_end().to_owned(), moved, at };
            }
            Step::Ran(ok, text) => {
                output.push_str(&text);
                output.push('\n');
                (ok, String::new())
            }
            Step::Run => {
                output.push_str(&format!("{}\n", glowfly_core::text::command_echo(c)));
                git(c, "", &[])
            }
        };
        moved |= !matches!(glowfly_core::trust::incoming::sub_name(c), Some("fetch"));
        if !text.is_empty() {
            output.push_str(&text);
            output.push('\n');
        }
        if !success {
            return Gated::Done(false, output.trim_end().to_owned());
        }
    }
    Gated::Done(true, output.trim_end().to_owned())
}

/// Runs `check` (a job's `on_fail_check`) after the job failed, adding it and what it printed
/// to the job's `output`; whether it passed. With the job's `gate` (and the number of its
/// commands, `at`), the check is compared first like one more of them: one the gate holds
/// doesn't run, and fails with the gate's text.
pub(super) fn check_passes(
    cwd: &Path,
    check: &[String],
    prompts: bool,
    gate: Option<(&super::trust_gate::Gate, usize)>,
    output: &mut String,
) -> bool {
    use super::trust_gate::Step;
    output.push_str(&format!("\n{}", glowfly_core::text::command_echo(check)));
    let mut git = |c: &[String], input: &str, extra: &[(&str, &str)]| {
        let env: Vec<(&str, &str)> = job_env(prompts).iter().chain(extra).copied().collect();
        gitcli::run_with_input(cwd, c, input, &env)
    };
    let (passed, text) = match gate.map(|(g, at)| (g, g.before(at, check, &mut git))) {
        None | Some((_, Step::Run)) => git(check, "", &[]),
        Some((_, Step::Ran(ok, text))) => (ok, text),
        Some((g, Step::Halt(halt, ran))) => (false, format!("{ran}\n{}", halt.describe(g.lang)).trim().to_owned()),
    };
    if !text.trim().is_empty() {
        output.push('\n');
        output.push_str(text.trim_end());
    }
    passed
}

#[cfg(test)]
#[path = "git_ops_lock_tests.rs"]
mod lock_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn background_jobs_run_without_prompts_or_credential_manager_windows() {
        assert_eq!(super::job_env(false), glowfly_core::gitcli::BACKGROUND);
        assert!(super::job_env(false).contains(&("GCM_INTERACTIVE", "never")));
        assert!(super::job_env(true).is_empty(), "a job that may prompt keeps the askpass helper");
    }

    #[test]
    fn starting_comes_before_each_command_and_none_after_a_failure() {
        use glowfly_core::gitcli::args;
        let dir = glowfly_model::testing::scratch_repo("git-ops-starting");
        let mut seen = Vec::new();
        let (ok, _) = super::run_git_each(&dir, &[args(&["status"]), args(&["status"])], None, false, &mut |i| seen.push(i));
        assert!(ok);
        assert_eq!(seen, [0, 1]);
        seen.clear();
        let failing = [args(&["checkout", "nosuch"]), args(&["status"])];
        let (ok, _) = super::run_git_each(&dir, &failing, None, false, &mut |i| seen.push(i));
        assert!(!ok);
        assert_eq!(seen, [0], "the command after the failure never starts");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
