//! Local git operations run by the runner: staging and discarding ops, undo and redo (also of a
//! deleted file, put back or deleted again, and of an abort that kept its work), and
//! running a list of git commands.

use crate::effect::{LocalOp, UndoOpts, UndoResult};
use crate::msg::{JobMsg, Msg};
use crate::state::files::TRASH;
use glowfly_core::error::Error;
use glowfly_core::gitcli;
use glowfly_core::repo;
use glowfly_core::undo::{self, Action, Discarded, Plan, Refusal};
use std::path::Path;

/// Runs a local op. Discards and conflict saves save the file's contents first and return the
/// undo action. A hunk or lines shown with whitespace changes hidden go through
/// `repo::apply_shown`.
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
        LocalOp::ResolveWithSide { path, side, expected } => done(repo::resolve_with_side(workdir, &path, side, &expected)),
        LocalOp::MarkResolved(paths) => done(repo::mark_resolved(workdir, &paths)),
        LocalOp::MarkConflicted(paths) => done(repo::mark_conflicted(workdir, &paths)),
    }
}

/// Checks that `action` still applies, then runs the git commands that undo or redo it. With
/// `opts.stash_first`, local changes are stashed before anything is checked; the stash's output
/// leads every result.
pub(super) fn undo_redo(cwd: &Path, action: Action, redo: bool, opts: UndoOpts) -> Msg {
    let mut stashed = String::new();
    if opts.stash_first {
        let (ok, output) = run_git(cwd, &[undo::stash_first_command()], None, true);
        if !ok {
            return Msg::Job(JobMsg::JobDone { ok: false, output, undo: Some(UndoResult::Failed { redo }) });
        }
        stashed = format!(
            "{output}
Your changes were stashed as \"{}\".
",
            undo::UNDO_STASH_MESSAGE
        );
    }
    let done = |ok: bool, output: String, undo: UndoResult| {
        Msg::Job(JobMsg::JobDone { ok, output: format!("{stashed}{output}"), undo: Some(undo) })
    };
    let failed = |output: String| done(false, output, UndoResult::Failed { redo });
    match undo::plan_with(cwd, &action, redo, undo::Options { delete_created: opts.delete_created }) {
        Err(e) => failed(e.to_string()),
        Ok(Plan::Refuse(r)) => {
            let offer_stash = matches!(r, Refusal::Dirty(_)) && action.offers_stash(redo) && !opts.stash_first;
            done(false, r.message().into(), UndoResult::Refused { redo, stale: r.is_stale(), offer_stash })
        }
        Ok(Plan::Write { path, content }) => match undo::write_saved(cwd, &path, content) {
            Ok(()) => {
                let output = action.wrote(redo);
                done(true, output, UndoResult::Done { redo, before: action.clone(), after: Box::new(action) })
            }
            Err(e) => failed(format!("Could not write {path}: {e}")),
        },
        Ok(Plan::Run(cmds)) => {
            let (ok, output) = run_git(cwd, &cmds, None, true);
            if !ok {
                return failed(output);
            }
            let after = if redo { undo::redone(cwd, action.clone()).unwrap_or_else(|_| action.clone()) } else { action.clone() };
            done(ok, output, UndoResult::Done { redo, before: action, after: Box::new(after) })
        }
        Ok(Plan::RunThenWrite { cmds, path, content }) => {
            let (ok, output) = run_git(cwd, &cmds, None, true);
            if !ok {
                return failed(output);
            }
            match undo::write_saved(cwd, &path, content) {
                Ok(()) => {
                    let output = format!("{output}\n{}", action.wrote(redo));
                    done(true, output, UndoResult::Done { redo, before: action.clone(), after: Box::new(action) })
                }
                Err(e) => {
                    let kept = content.map(|s| format!(". Its contents are kept as blob {} (git cat-file -p {})", s.blob, s.blob));
                    failed(format!("{output}\nCould not write {path}: {e}{}", kept.unwrap_or_default()))
                }
            }
        }
        Ok(Plan::PutBack { path, content, staged, via }) => {
            match undo::put_back(cwd, &path, content, staged.as_ref(), via, super::files::TRASHERS) {
                Ok(how) => {
                    let output = how.wrote(&path, staged.is_some(), via);
                    done(true, output, UndoResult::Done { redo, before: action.clone(), after: Box::new(action) })
                }
                Err(e) => failed(format!("Could not put back {path}: {e}")),
            }
        }
        Ok(Plan::Restage { path, staged }) => match undo::restage(cwd, &path, &staged) {
            Ok(()) => done(true, action.wrote(redo), UndoResult::Done { redo, before: action.clone(), after: Box::new(action) }),
            Err(e) => failed(format!("Could not stage {path} again: {e}")),
        },
        Ok(Plan::UnstageAgain { path }) => match undo::unstage_again(cwd, &path) {
            Ok(()) => done(true, action.wrote(redo), UndoResult::Done { redo, before: action.clone(), after: Box::new(action) }),
            Err(e) => failed(format!("Could not unstage {path}: {e}")),
        },
        Ok(Plan::DeleteAgain { path, via, staged }) => match undo::delete_again(cwd, &path, via, staged, super::files::TRASHERS) {
            Ok(_) => done(true, action.wrote(redo), UndoResult::Done { redo, before: action.clone(), after: Box::new(action) }),
            Err(Error::TrashRefused(r)) => failed(format!("The {TRASH} couldn't take {path}: {}", r.detail)),
            Err(e) => failed(format!("Could not delete {path}: {e}")),
        },
        Ok(Plan::RestoreAbort) => match undo::restore_abort(cwd, &action) {
            Ok(output) => done(true, output, UndoResult::Done { redo, before: action.clone(), after: Box::new(action) }),
            Err(e) => failed(format!("Could not put the {} back: {e}", action.describe())),
        },
        Ok(Plan::AbortAgain) => match super::stashes::abort_keeping(cwd, None) {
            (true, output, Some(again)) => done(true, output, UndoResult::Done { redo, before: action, after: Box::new(again) }),
            (_, output, _) => failed(output),
        },
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
