//! Executing one effect on a worker thread: each effect area's `Execute` impl (in this folder's
//! `<area>.rs`) and the arms of the flat variants, with the result message each sends back.

use super::{
    diff_gaps, expect, files, forge_calls, git_ops, highlight, history, journal, load, path_search, revert, saves, signatures, status,
};
use crate::effect::areas::{effect_areas, effect_match};
use crate::effect::{Effect, UndoResult};
use crate::msg::{AvatarMsg, DiffMsg, ForgeMsg, JobMsg, Msg, RecentMsg, RepoMsg, StageMsg};
use glowfly_core::{avatars, gitcli, repo, undo};

pub(super) fn err(e: glowfly_core::error::Error) -> String {
    e.to_string()
}

/// What an effect may need from the runner while it runs.
pub(super) struct Cx<'a> {
    /// True once a newer job replaces this one (only HighlightDiff checks it).
    pub cancelled: &'a dyn Fn() -> bool,
    /// The answers repository loads share.
    pub graph_cache: &'a repo::GraphCache,
    /// The stop of an effect that can be cancelled (`Effect::stop_key`).
    pub stop: Option<&'a gitcli::Stop>,
}

/// Runs one effect area's value to completion (blocking), returning the message to send back.
pub(super) trait Execute {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg>;
}

/// Runs one effect to completion (blocking) and returns the message to send back, if any.
/// Results that belong to one repository come back as `Msg::Scoped`.
#[cfg(test)]
pub fn execute(effect: Effect) -> Option<Msg> {
    execute_with(effect, &Cx { cancelled: &|| false, graph_cache: &repo::GraphCache::default(), stop: None })
}

/// `execute` with the runner's `Cx`.
pub(super) fn execute_with(effect: Effect, cx: &Cx<'_>) -> Option<Msg> {
    let scope = effect.scope().cloned();
    let msg = execute_unscoped(effect, cx)?;
    Some(match scope {
        Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
        None => msg,
    })
}

fn execute_unscoped(effect: Effect, cx: &Cx<'_>) -> Option<Msg> {
    effect_areas! { effect_match! { effect, |e| Execute::execute(e, cx), {
        Effect::LoadRepo { path, opts, generation, kind, known, prior } => {
            Some(load::load_repo(path, opts, generation, kind, known, prior, cx.graph_cache))
        }
        Effect::LoadStatus { workdir, generation, submodules, open, git_status, paths } => {
            Some(status::load(&workdir, generation, submodules, open, git_status, paths))
        }
        Effect::LoadIndexStatus { workdir, generation, submodules, open, git_status, read } => {
            Some(status::load_index(&workdir, generation, submodules, open, git_status, read))
        }
        Effect::LoadDetail { workdir, oid } => {
            Some(Msg::Repo(RepoMsg::DetailLoaded { oid, result: repo::commit_detail(&workdir, oid).map_err(err) }))
        }
        Effect::LoadDiff { workdir, target, reload, generation, previous, ignore_whitespace, whole_file } => Some(highlight::load_job(
            workdir,
            target,
            reload,
            generation,
            previous,
            repo::LoadDiffOptions { ignore_whitespace, whole_file },
        )),
        Effect::HighlightDiff { target, generation, diff, .. } => highlight::finish_job(target, generation, &diff, cx.cancelled),
        Effect::LoadDiffGaps { workdir, target, generation, diff } => Some(diff_gaps::load_job(workdir, target, generation, diff)),
        Effect::LoadBinarySides { workdir, target, generation } => {
            let result = repo::binary_sides(&workdir, &target).map(std::sync::Arc::new).map_err(err);
            Some(Msg::Diff(DiffMsg::BinarySidesLoaded { target, generation, result }))
        }
        Effect::LoadFileHistory { workdir, path, start, limit, folder, generation } => {
            Some(history::walk(&workdir, path, &start, limit, folder, generation))
        }
        Effect::LocalOp { workdir, op } => Some(Msg::Stage(StageMsg::LocalOpDone(git_ops::local_op(&workdir, op).map_err(err)))),
        Effect::Revert { workdir, req } => Some(revert::run(&workdir, req)),
        Effect::RestoreFile { workdir, oid, path, overwrite } => Some(files::restore(&workdir, oid, path, overwrite)),
        Effect::WorkFile { workdir, op } => Some(files::work_file(&workdir, op)),
        Effect::LoadTrackedPaths { workdir, generation } => Some(files::tracked_paths(&workdir, generation)),
        Effect::RunGit { cwd, cmds, write_file, record, journal, prompts, expect, stoppable: _ } => {
            if let Some(refused) = expect.and_then(|(rev, oid)| expect::check(&cwd, &rev, oid).err()) {
                return Some(Msg::Job(JobMsg::JobDone { ok: false, output: refused, undo: record.map(|_| UndoResult::Recorded(None)) }));
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
            // The journal note sees the refs before git runs and after the undo record is read.
            Some(journal::around(&cwd, journal, || {
                // Only a job that just fetches has a stop, and it writes no file.
                let (ok, output) = match cx.stop.filter(|_| write_file.is_none()) {
                    Some(stop) => match git_ops::run_git_until(&cwd, &cmds, prompts, stop) {
                        (_, output, true) => return Msg::Job(JobMsg::Stopped { output }),
                        (ok, output, false) => (ok, output),
                    },
                    None => git_ops::run_git(&cwd, &cmds, write_file, prompts),
                };
                let recorded = |k| if ok { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
                let undo = if ok && irreversible { None } else { record.map(|k| UndoResult::Recorded(recorded(k))) };
                Msg::Job(JobMsg::JobDone { ok, output, undo })
            }))
        }
        Effect::UndoRedo { cwd, action, redo, opts } => {
            let what = action.describe();
            let op = if redo { glowfly_core::journal::Op::Redo { what } } else { glowfly_core::journal::Op::Undo { what } };
            Some(journal::around(&cwd, Some(op), || git_ops::undo_redo(&cwd, action, redo, opts)))
        }
        Effect::Forge { target, cwd, auth, interactive, connected, call } => {
            Some(Msg::Forge(ForgeMsg::ForgeReplied(forge_calls::call(target, &cwd, auth, interactive, connected, call))))
        }
        Effect::Stats { workdir, oids } => Some(Msg::Repo(RepoMsg::StatsLoaded(repo::commit_stats(&workdir, &oids)))),
        Effect::SearchPaths { workdir, request, terms, oids } => {
            Some(path_search::once(path_search::Request { workdir, request, terms, oids }))
        }
        Effect::VerifySignature { workdir, oid, format, generation } => Some(signatures::verify(&workdir, oid, format, generation)),
        Effect::FetchAvatars(emails) => Some(Msg::Avatars(AvatarMsg::AvatarsLoaded(
            emails
                .into_iter()
                .map(|e| {
                    let img = avatars::fetch(&e);
                    (e, img)
                })
                .collect(),
        ))),
        save @ (Effect::SaveSettings(_) | Effect::SaveRecent(_) | Effect::SaveTabs { .. }) => saves::save_now(save),
        Effect::CheckRepoFolders(paths) => Some(Msg::Recent(RecentMsg::FoldersChecked(crate::platform::check_folders(&paths)))),
        Effect::ReadTabGroups(paths) => {
            let read = paths
                .into_iter()
                .map(|p| {
                    let common = repo::CommonDir::at(&p);
                    (p, common)
                })
                .collect();
            Some(Msg::Tabs(crate::msg::TabsMsg::GroupsRead(read)))
        }
        // `Runner::run` stops it; there is nothing running here to stop.
        Effect::StopGit(_) => None,
        Effect::Watch(_) | Effect::SweepWatch { .. } | Effect::AnswerPrompt { .. } => {
            tracing::warn!("watch or prompt effect reached a worker; it is handled by Runner::run");
            None
        }
    } } }
}
