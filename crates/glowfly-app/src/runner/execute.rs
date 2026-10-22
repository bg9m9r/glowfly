//! Executing one effect on a worker thread: the match from each `Effect` to the code that does
//! the work, and the result message it sends back.

use super::{
    blame, clone, config, conflict_check, conflict_files, credentials, custom_commands, dashboard, deep_link, diff_gaps, expect, files,
    forge_calls, forks, git_env, git_ops, highlight, history, index, init, issues, journal, line_counts, logs, notify, onboarding,
    op_preview, os, partial, patches, path_search, pick, profiles, pull_request, range_diff, rebase, repos, revert, saves, signatures,
    sparse, ssh, stack, stashes, status, submodules, themes, tools, trust, updates, worktrees,
};
use crate::effect::{Effect, UndoResult};
use crate::lfs::LfsMsg;
use crate::msg::{AvatarMsg, ConflictMsg, DiffMsg, ForgeMsg, JobMsg, LoadMsg, Msg, RecentMsg, RepoMsg, SelectionMsg, StageMsg};
use glowfly_core::{avatars, repo, undo};

pub(super) fn err(e: glowfly_core::error::Error) -> String {
    e.to_string()
}

/// Runs one effect to completion (blocking) and returns the message to send back, if any.
/// Results that belong to one repository come back as `Msg::Scoped`.
#[cfg(test)]
pub fn execute(effect: Effect) -> Option<Msg> {
    execute_with(effect, &|| false, &repo::GraphCache::default(), None)
}

/// `execute`, stopping early when `cancelled` returns true (only HighlightDiff checks it), with
/// the answers repository loads share; `stop` is the stop of an effect that can be cancelled
/// (`Effect::stop_key`).
pub(super) fn execute_with(
    effect: Effect,
    cancelled: &dyn Fn() -> bool,
    graph_cache: &repo::GraphCache,
    stop: Option<&glowfly_core::gitcli::Stop>,
) -> Option<Msg> {
    let scope = effect.scope().cloned();
    let msg = execute_unscoped(effect, cancelled, graph_cache, stop)?;
    Some(match scope {
        Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
        None => msg,
    })
}

fn execute_unscoped(
    effect: Effect,
    cancelled: &dyn Fn() -> bool,
    graph_cache: &repo::GraphCache,
    stop: Option<&glowfly_core::gitcli::Stop>,
) -> Option<Msg> {
    match effect {
        Effect::LoadRepo { path, opts, generation, kind, known } => {
            if let Some(k) = known
                && repo::refs_key(&path).is_ok_and(|key| key == k.refs_key)
            {
                let open_stamp = k.open.and_then(|p| repo::open_file_stamp(&path, &p));
                let started = std::time::Instant::now();
                let result = repo::status_at(&path, k.git_status).map_err(err);
                let slow = repo::slow(started.elapsed(), &path);
                let submodules = repo::submodules_if_any(&path, k.submodules).ok().flatten();
                let lfs_patterns = Some(glowfly_core::lfs::root_patterns(&path));
                return Some(Msg::Load(LoadMsg::RepoUnchanged { generation, result, submodules, open_stamp, slow, lfs_patterns }));
            }
            let result = match repo::load_cached(&path, &opts, graph_cache) {
                Err(glowfly_core::error::Error::NotOwned(root)) => return Some(git_env::not_owned(generation, kind, root)),
                r => r.map(Box::new).map_err(|e| format!("Could not open {}: {e}", glowfly_core::native_path::shown(&path))),
            };
            Some(Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result }))
        }
        Effect::LoadStatus { workdir, generation, submodules, open, git_status, paths } => {
            Some(status::load(&workdir, generation, submodules, open, git_status, paths))
        }
        Effect::LoadDetail { workdir, oid } => {
            Some(Msg::Repo(RepoMsg::DetailLoaded { oid, result: repo::commit_detail(&workdir, oid).map_err(err) }))
        }
        Effect::LoadCompare { workdir, from, to } => {
            Some(Msg::Selection(SelectionMsg::CompareLoaded { from, to, result: repo::compare(&workdir, from, to).map_err(err) }))
        }
        Effect::LoadStacked(r) => Some(Msg::Selection(SelectionMsg::StackedLoaded {
            result: repo::stacked_changes(&r.workdir, &r.oids).map_err(err),
            oids: r.oids,
        })),
        Effect::LoadWorkdirCompare { workdir, oid, generation, swapped } => Some(Msg::Selection(SelectionMsg::WorkdirCompareLoaded {
            oid,
            generation,
            result: if swapped { repo::compare_from_workdir(&workdir, oid) } else { repo::compare_workdir(&workdir, oid) }.map_err(err),
        })),
        Effect::LoadDiff { workdir, target, reload, generation, previous, ignore_whitespace, whole_file } => Some(highlight::load_job(
            workdir,
            target,
            reload,
            generation,
            previous,
            repo::LoadDiffOptions { ignore_whitespace, whole_file },
        )),
        Effect::HighlightDiff { target, generation, diff, .. } => highlight::finish_job(target, generation, &diff, cancelled),
        Effect::LoadDiffGaps { workdir, target, generation, diff } => Some(diff_gaps::load_job(workdir, target, generation, diff)),
        Effect::LoadBinarySides { workdir, target, generation } => {
            let result = repo::binary_sides(&workdir, &target).map(std::sync::Arc::new).map_err(err);
            Some(Msg::Diff(DiffMsg::BinarySidesLoaded { target, generation, result }))
        }
        Effect::LoadConflict { workdir, path, generation } => {
            let result = repo::load_conflict(&workdir, &path).map_err(err);
            // The picks a file saved with markers holds come back with it.
            let saved = result.as_ref().ok().and_then(|f| repo::saved_picks(&workdir, f));
            Some(Msg::Conflict(ConflictMsg::ConflictLoaded { path: path.clone(), generation, result, saved }))
        }
        Effect::LoadSparse { workdir, generation, run_git } => Some(sparse::load(&workdir, generation, run_git)),
        Effect::SparseCheckout { cwd, command } => Some(sparse::run(&cwd, &command)),
        Effect::LoadFileHistory { workdir, path, start, limit, folder, generation } => {
            Some(history::walk(&workdir, path, &start, limit, folder, generation))
        }
        Effect::LoadFileText(r) => Some(history::file_text(r)),
        Effect::LoadBlame(r) => Some(blame::load(r)),
        Effect::LocalOp { workdir, op } => Some(Msg::Stage(StageMsg::LocalOpDone(git_ops::local_op(&workdir, op).map_err(err)))),
        Effect::ReadConflictFiles { workdir, paths, purpose } => Some(conflict_files::read(&workdir, paths, purpose)),
        Effect::Revert { workdir, req } => Some(revert::run(&workdir, req)),
        Effect::RestoreFile { workdir, oid, path, overwrite } => Some(files::restore(&workdir, oid, path, overwrite)),
        Effect::WorkFile { workdir, op } => Some(files::work_file(&workdir, op)),
        Effect::LoadTrackedPaths { workdir, generation } => Some(files::tracked_paths(&workdir, generation)),
        Effect::Submodule(e) => Some(submodules::run(e)),
        Effect::CheckWorktreeFolder(r) => Some(worktrees::check(r)),
        Effect::CheckCloneParent { parent, delay } => Some(files::check_clone_parent(parent, delay)),
        Effect::DetectLfs { cwd } => Some(Msg::Lfs(LfsMsg::Detected(glowfly_core::lfs::installed(&cwd)))),
        Effect::RemoveWorktree(r) => Some(worktrees::remove(r)),
        Effect::MoveWorktree(r) => Some(worktrees::move_(r)),
        Effect::CountWorktreeChanges(r) => Some(worktrees::count(r)),
        Effect::Stash(e) => Some(stashes::run(e)),
        Effect::LoadLfs { workdir, generation } => {
            Some(Msg::Lfs(LfsMsg::Loaded { generation, result: glowfly_core::lfs::status(&workdir).map_err(err) }))
        }
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
                let (ok, output) = match stop.filter(|_| write_file.is_none()) {
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
        Effect::Pick(j) => Some(pick::run(j)),
        Effect::UndoRedo { cwd, action, redo, opts } => {
            let what = action.describe();
            let op = if redo { glowfly_core::journal::Op::Redo { what } } else { glowfly_core::journal::Op::Undo { what } };
            Some(journal::around(&cwd, Some(op), || git_ops::undo_redo(&cwd, action, redo, opts)))
        }
        Effect::Rebase(e) => Some(rebase::run(e)),
        Effect::Stack(e) => Some(stack::run(e)),
        Effect::Forge { target, cwd, auth, interactive, connected, call } => {
            Some(Msg::Forge(ForgeMsg::ForgeReplied(forge_calls::call(target, &cwd, auth, interactive, connected, call))))
        }
        Effect::Account(e) => forge_calls::account(e),
        Effect::Stats { workdir, oids } => Some(Msg::Repo(RepoMsg::StatsLoaded(repo::commit_stats(&workdir, &oids)))),
        Effect::LineCounts(e) => Some(line_counts::run(e)),
        Effect::SearchPaths { workdir, request, terms, oids } => {
            Some(path_search::once(path_search::Request { workdir, request, terms, oids }))
        }
        Effect::Index(e) => index::once(e),
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
        Effect::Prefs(e) => config::run(e),
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
        Effect::Os(e) => os::run(e),
        Effect::Ssh(e) => Some(ssh::run(e)),
        Effect::Credential(e) => Some(credentials::run(e)),
        Effect::Profile(e) => Some(profiles::run(e)),
        Effect::Patch(p) => Some(patches::run(p)),
        Effect::Tool(t) => tools::run(t),
        Effect::CustomCommand(e) => Some(custom_commands::run(e)),
        Effect::GitEnv(e) => Some(git_env::run(e)),
        Effect::Onboarding(e) => Some(onboarding::run(e)),
        Effect::Theme(e) => themes::run(e),
        Effect::CheckConflicts(job) => Some(conflict_check::run(job)),
        Effect::OpPreview(job) => Some(op_preview::run(job)),
        Effect::Journal(e) => Some(journal::run(e)),
        Effect::RangeDiff(job) => Some(range_diff::run(job)),
        Effect::Remote(e) => Some(forks::run(e)),
        Effect::Clone(e) => Some(clone::run(e)),
        Effect::Init(e) => Some(init::run(e)),
        Effect::Notify(note) => notify::run(note),
        Effect::PullRequest(e) => Some(pull_request::run(e)),
        Effect::DeepLink(e) => Some(deep_link::run(e)),
        Effect::Dashboard(e) => Some(dashboard::run(e)),
        Effect::Issue(e) => Some(issues::run(e)),
        Effect::Updates(e) => updates::run(e),
        Effect::Repos(e) => Some(repos::run(e)),
        Effect::Trust(e) => Some(trust::run(e)),
        Effect::Partial(e) => Some(partial::run(e, stop)),
        // `Runner::run` stops it; there is nothing running here to stop.
        Effect::StopGit(_) => None,
        Effect::Ui(_) => {
            tracing::warn!("UI effect reached the runner; it should be handled by the UI shell");
            None
        }
        // The save dialog waits on the user, so it runs here rather than on the log worker.
        Effect::Logs(crate::logs::LogsEffect::PickExport { name }) => Some(logs::pick_export(&name)),
        Effect::Watch(_) | Effect::AnswerPrompt { .. } | Effect::Terminal(_) | Effect::Logs(_) => {
            tracing::warn!("watch, prompt, terminal or log effect reached a worker; it is handled by Runner::run");
            None
        }
    }
}
