//! Local git operations run by the runner: staging and discarding ops, conflict changes such as
//! Mark resolved, and running a list of git commands (through the trust gate too). Undo and redo
//! are in `undo_run.rs`.

use crate::effect::LocalOp;
use crate::msg::{Msg, StageMsg};
use glowfly_core::gitcli;
use glowfly_core::repo;
use glowfly_core::undo::{self, Action, ConflictChange, ConflictsFailed, Discarded};
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
    }
}

/// Runs a local op and says how it went. A conflict change that failed partway carries the
/// undo action for the files it changed.
pub(super) fn local_done(workdir: &Path, op: LocalOp) -> Msg {
    let conflict = matches!(op, LocalOp::ResolveWithSide { .. } | LocalOp::MarkResolved(_) | LocalOp::MarkConflicted(_));
    let result = if conflict { conflict_change(workdir, op) } else { local_op(workdir, op).map_err(ConflictsFailed::from) };
    Msg::Stage(match result {
        Ok(undo) => StageMsg::LocalOpDone(Ok(undo)),
        Err(ConflictsFailed { error, undo: None }) => StageMsg::LocalOpDone(Err(error.to_string())),
        Err(ConflictsFailed { error, undo: Some(undo) }) => {
            let error = format!("{error}. Undo puts back the files it changed before it stopped.");
            StageMsg::LocalOpFailed { error, undo: *undo }
        }
    })
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

/// The caller environment for a job's git commands: none for one that may prompt, else
/// [`gitcli::BACKGROUND`] (no prompts, and no Git Credential Manager window either).
pub(super) fn job_env(prompts: bool) -> &'static [(&'static str, &'static str)] {
    if prompts { &[] } else { gitcli::BACKGROUND }
}

/// Runs `cmds` in order, stopping at the first failure. Without `prompts`, git fails rather than
/// open a credential prompt.
pub(super) fn run_git(cwd: &Path, cmds: &[Vec<String>], write_file: Option<(std::path::PathBuf, String)>, prompts: bool) -> (bool, String) {
    if let Some((path, contents)) = write_file
        && let Err(e) = glowfly_core::persist::atomic_write(&path, contents.as_bytes())
    {
        return (false, format!("Could not write {}: {e}", path.display()));
    }
    let mut output = String::new();
    for c in cmds {
        output.push_str(&format!("$ git {}\n", glowfly_core::text::redact_credentials(&c.join(" "))));
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
    write_file: Option<(std::path::PathBuf, String)>,
    prompts: bool,
    gate: &super::trust_gate::Gate,
) -> Gated {
    run_git_gated_noting(cwd, cmds, write_file, prompts, gate, false).0
}

/// `run_git_gated`, noting where an operation starts (`undo::op_start`) just before the last
/// command when `note` is set (`op_stops.rs`).
pub(super) fn run_git_gated_noting(
    cwd: &Path,
    cmds: &[Vec<String>],
    write_file: Option<(std::path::PathBuf, String)>,
    prompts: bool,
    gate: &super::trust_gate::Gate,
    note: bool,
) -> (Gated, Option<undo::OpStart>) {
    let mut start = None;
    let gated = gated_steps(cwd, cmds, write_file, prompts, gate, &mut |at| {
        if note && at + 1 == cmds.len() {
            start = undo::op_start(cwd);
        }
    });
    (gated, start)
}

fn gated_steps(
    cwd: &Path,
    cmds: &[Vec<String>],
    write_file: Option<(std::path::PathBuf, String)>,
    prompts: bool,
    gate: &super::trust_gate::Gate,
    before: &mut dyn FnMut(usize),
) -> Gated {
    use super::trust_gate::Step;
    if let Some((path, contents)) = write_file
        && let Err(e) = glowfly_core::persist::atomic_write(&path, contents.as_bytes())
    {
        return Gated::Done(false, format!("Could not write {}: {e}", path.display()));
    }
    let mut output = String::new();
    let mut moved = false;
    let mut git = |c: &[String], input: &str, extra: &[(&str, &str)]| {
        let env: Vec<(&str, &str)> = job_env(prompts).iter().chain(extra).copied().collect();
        gitcli::run_with_input(cwd, c, input, &env)
    };
    for (at, c) in cmds.iter().enumerate() {
        before(at);
        let (success, text) = match gate.before(at, c, &mut git) {
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
                output.push_str(&format!("$ git {}\n", glowfly_core::text::redact_credentials(&c.join(" "))));
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
/// to the job's `output`; whether it passed.
pub(super) fn check_passes(cwd: &Path, check: &[String], prompts: bool, output: &mut String) -> bool {
    output.push_str(&format!("\n$ git {}", glowfly_core::text::redact_credentials(&check.join(" "))));
    let (passed, text) = gitcli::run_with_input(cwd, check, "", job_env(prompts));
    if !text.trim().is_empty() {
        output.push('\n');
        output.push_str(text.trim_end());
    }
    passed
}

/// `run_git` for a job that only fetches, stopped when `stop` is asked (`Effect::StopGit`):
/// git's process tree is ended, then the lock and temporary files it left in the repository are
/// removed. Returns (ok, output, stopped); a stopped run runs none of the commands after it.
pub(super) fn run_git_until(cwd: &Path, cmds: &[Vec<String>], prompts: bool, stop: &gitcli::Stop) -> (bool, String, bool) {
    let before = glowfly_core::stop_cleanup::scan(cwd);
    let mut output = String::new();
    for c in cmds {
        output.push_str(&format!("$ git {}\n", glowfly_core::text::redact_credentials(&c.join(" "))));
        let out = gitcli::output_until(cwd, c, "", job_env(prompts), stop);
        let text = out.combined();
        if !text.is_empty() {
            output.push_str(&text);
            output.push('\n');
        }
        if out.stopped {
            output.push_str("Cancelled.\n");
            let removed = before.as_ref().map(glowfly_core::stop_cleanup::clean).unwrap_or_default();
            match removed.len() {
                0 => {}
                1 => output.push_str("Removed 1 file the stopped fetch left behind.\n"),
                n => output.push_str(&format!("Removed {n} files the stopped fetch left behind.\n")),
            }
            return (false, output.trim_end().to_owned(), true);
        }
        if !out.ok {
            return (false, output.trim_end().to_owned(), false);
        }
    }
    (true, output.trim_end().to_owned(), false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn background_jobs_run_without_prompts_or_credential_manager_windows() {
        assert_eq!(super::job_env(false), glowfly_core::gitcli::BACKGROUND);
        assert!(super::job_env(false).contains(&("GCM_INTERACTIVE", "never")));
        assert!(super::job_env(true).is_empty(), "a job that may prompt keeps the askpass helper");
    }

    #[test]
    fn a_stopped_job_ends_its_git_and_removes_what_it_left_behind() {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-stop-cleanup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (ok, out) = glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["init", "-q"]));
        assert!(ok, "{out}");
        let lock = dir.join(".git/refs/heads/topic.lock");
        // Leaves a ref lock behind, as a fetch stopped while updating refs would.
        let alias = "alias.slow=!echo x > .git/refs/heads/topic.lock && sleep 20";
        let cmds = [glowfly_core::gitcli::args(&["-c", alias, "slow"]), glowfly_core::gitcli::args(&["status"])];
        let stop = glowfly_core::gitcli::Stop::new();
        let asker = stop.clone();
        let watched = lock.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while !watched.exists() && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            asker.stop();
        });
        let started = std::time::Instant::now();
        let (ok, out, stopped) = super::run_git_until(&dir, &cmds, false, &stop);
        assert!(stopped && !ok, "{out}");
        assert!(started.elapsed() < std::time::Duration::from_secs(15), "took {:?}", started.elapsed());
        assert!(out.contains("Cancelled.") && out.contains("Removed 1 file the stopped fetch left behind."), "{out}");
        assert!(!out.contains("$ git status"), "nothing after the stopped command runs");
        assert!(!lock.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
