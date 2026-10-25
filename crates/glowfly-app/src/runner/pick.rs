//! Running a cherry-pick or revert job (`Effect::Pick`): like `RunGit` (the undo record and the
//! journal note around it), then carried past the commits that change nothing
//! (`glowfly_core::pick_ops::skip_empty`).

use super::execute::{Cx, Execute};
use super::git_ops::run_git;
use super::journal;
use crate::effect::{PickJob, UndoResult};
use crate::msg::{JobMsg, Msg};
use glowfly_core::pick_ops::{self, PickResult};
use glowfly_core::undo;
use std::path::Path;

fn head(cwd: &Path) -> Option<git2::Oid> {
    git2::Repository::open(cwd).ok()?.head().ok()?.target()
}

pub(super) fn run(j: PickJob) -> Msg {
    let PickJob { cwd, mut cmds, record, journal, prompts, picking, oids, can_skip } = j;
    let before = record.as_ref().and_then(|k| undo::before(&cwd, k).ok());
    let irreversible = before.as_ref().is_some_and(undo::irreversible);
    let total = oids.len();
    let (ok, output, undo, result) = journal::around(&cwd, journal, || {
        let Some(last) = cmds.pop() else {
            return (false, String::new(), None, PickResult { picking, total, ..PickResult::default() });
        };
        // The auto-stash push, when the job stashes first.
        let (stashed, mut output) = if cmds.is_empty() { (true, String::new()) } else { run_git(&cwd, &cmds, None, prompts) };
        if !stashed {
            let undo = record.as_ref().map(|_| UndoResult::Recorded(None));
            return (false, output, undo, PickResult { picking, total, ..PickResult::default() });
        }
        let start = head(&cwd);
        let first = run_git(&cwd, &[last], None, prompts);
        let mut git = |c: &[String]| run_git(&cwd, &[c.to_vec()], None, prompts);
        let (ok, picked, result) = pick_ops::skip_empty(&cwd, picking, &oids, start, first, can_skip, &mut git);
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&picked);
        // A run that made no commit records nothing new.
        let moved = head(&cwd) != start;
        let recorded = |k| if ok && moved { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
        let undo = if ok && irreversible { None } else { record.map(|k| UndoResult::Recorded(recorded(k))) };
        (ok, output, undo, result)
    });
    Msg::Job(JobMsg::PickDone { ok, output, undo, result })
}

impl Execute for PickJob {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
