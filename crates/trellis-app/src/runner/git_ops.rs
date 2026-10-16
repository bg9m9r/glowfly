//! Local git operations run by the runner: staging and discarding ops, undo and redo, and
//! running a list of git commands.

use crate::effect::{LocalOp, UndoResult};
use crate::msg::{JobMsg, Msg};
use std::path::Path;
use trellis_core::gitcli;
use trellis_core::repo;
use trellis_core::undo::{self, Action, Plan};

/// Runs a local op. Discards save the file's contents first and return the undo action.
pub(super) fn local_op(workdir: &Path, op: LocalOp) -> trellis_core::error::Result<Option<Action>> {
    use trellis_core::repo::HunkOp::Discard;
    let saving = |path: &str, f: &dyn Fn() -> trellis_core::error::Result<()>| undo::discard_saving(workdir, path, f);
    let done = |r: trellis_core::error::Result<()>| r.map(|()| None);
    match op {
        LocalOp::StageFile(p) => done(repo::stage_file(workdir, &p)),
        LocalOp::StageAll => done(repo::stage_all(workdir)),
        LocalOp::UnstagePaths(paths) => done(repo::unstage_paths(workdir, &paths)),
        LocalOp::DiscardFile(path) => saving(&path, &|| repo::discard_file(workdir, &path)),
        LocalOp::Hunk { op: Discard, path, index, expected } => {
            saving(&path, &|| repo::apply_hunk(workdir, &path, index, &expected, Discard))
        }
        LocalOp::Hunk { op, path, index, expected } => done(repo::apply_hunk(workdir, &path, index, &expected, op)),
        LocalOp::Lines { op: Discard, path, hunk, expected, lines } => {
            saving(&path, &|| repo::apply_lines(workdir, &path, hunk, &expected, &lines, Discard))
        }
        LocalOp::Lines { op, path, hunk, expected, lines } => done(repo::apply_lines(workdir, &path, hunk, &expected, &lines, op)),
        LocalOp::ResolveConflict { path, contents, expected } => done(repo::write_resolution(workdir, &path, &contents, &expected)),
        LocalOp::ResolveWithSide { path, side, expected } => done(repo::resolve_with_side(workdir, &path, side, &expected)),
        LocalOp::MarkResolved(paths) => done(repo::mark_resolved(workdir, &paths)),
        LocalOp::MarkConflicted(paths) => done(repo::mark_conflicted(workdir, &paths)),
    }
}

/// Checks that `action` still applies, then runs the git commands that undo or redo it.
pub(super) fn undo_redo(cwd: &Path, action: Action, redo: bool) -> Msg {
    let failed = |output: String| Msg::Job(JobMsg::JobDone { ok: false, output, undo: Some(UndoResult::Failed { redo }) });
    match undo::plan(cwd, &action, redo) {
        Err(e) => failed(e.to_string()),
        Ok(Plan::Refuse(r)) => Msg::Job(JobMsg::JobDone {
            ok: false,
            output: r.message().into(),
            undo: Some(UndoResult::Refused { redo, stale: r.is_stale() }),
        }),
        Ok(Plan::Write { path, content }) => match undo::write_saved(cwd, &path, content) {
            Ok(()) => {
                let output = action.wrote(redo);
                Msg::Job(JobMsg::JobDone { ok: true, output, undo: Some(UndoResult::Done { redo, before: action.clone(), after: action }) })
            }
            Err(e) => failed(format!("Could not write {path}: {e}")),
        },
        Ok(Plan::Run(cmds)) => {
            let (ok, output) = run_git(cwd, &cmds, None, true);
            if !ok {
                return failed(output);
            }
            let after = if redo { undo::redone(cwd, action.clone()).unwrap_or_else(|_| action.clone()) } else { action.clone() };
            Msg::Job(JobMsg::JobDone { ok, output, undo: Some(UndoResult::Done { redo, before: action, after }) })
        }
    }
}

/// Runs `cmds` in order, stopping at the first failure. Without `prompts`, git fails rather than
/// open a credential prompt.
pub(super) fn run_git(cwd: &Path, cmds: &[Vec<String>], write_file: Option<(std::path::PathBuf, String)>, prompts: bool) -> (bool, String) {
    if let Some((path, contents)) = write_file
        && let Err(e) = trellis_core::persist::atomic_write(&path, contents.as_bytes())
    {
        return (false, format!("Could not write {}: {e}", path.display()));
    }
    let mut output = String::new();
    for c in cmds {
        output.push_str(&format!("$ git {}\n", trellis_core::text::redact_credentials(&c.join(" "))));
        let (success, text) = gitcli::run_with_input(cwd, c, "", if prompts { &[] } else { gitcli::NO_PROMPTS });
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
