//! Executing one effect on a worker thread: the match from each `Effect` to the code that does
//! the work, and the result message it sends back.

use super::{
    blame, clone, config, conflict_check, conflict_files, credentials, dashboard, deep_link, diff_gaps, expect, files, forge_calls, forks, git_env,
    git_ops, highlight, history, init, issues, keys, notify, onboarding, os, patches, path_search, profiles, pull_request, rebase, repos,
    revert, saves, signatures, sparse, ssh, stashes, status, submodules, themes, tools, updates, worktrees,
};
use crate::effect::{Effect, UndoResult};
use crate::lfs::LfsMsg;
use crate::msg::{
    AvatarMsg, ConflictMsg, ForgeMsg, JobMsg, LoadMsg, Msg, PrefsMsg, PublishMsg, RecentMsg, RepoMsg, SelectionMsg, StageMsg,
};
use trellis_core::{avatars, forge, repo, undo};

pub(super) fn err(e: trellis_core::error::Error) -> String {
    e.to_string()
}

/// Runs one effect to completion (blocking) and returns the message to send back, if any.
/// Results that belong to one repository come back as `Msg::Scoped`.
#[cfg(test)]
pub fn execute(effect: Effect) -> Option<Msg> {
    execute_with(effect, &|| false, &repo::GraphCache::default())
}

/// `execute`, stopping early when `cancelled` returns true (only HighlightDiff checks it), with
/// the answers repository loads share.
pub(super) fn execute_with(effect: Effect, cancelled: &dyn Fn() -> bool, graph_cache: &repo::GraphCache) -> Option<Msg> {
    let scope = effect.scope().cloned();
    let msg = execute_unscoped(effect, cancelled, graph_cache)?;
    Some(match scope {
        Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
        None => msg,
    })
}

fn execute_unscoped(effect: Effect, cancelled: &dyn Fn() -> bool, graph_cache: &repo::GraphCache) -> Option<Msg> {
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
                return Some(Msg::Load(LoadMsg::RepoUnchanged { generation, result, submodules, open_stamp, slow }));
            }
            let result = match repo::load_cached(&path, &opts, graph_cache) {
                Err(trellis_core::error::Error::NotOwned(root)) => return Some(git_env::not_owned(generation, kind, root)),
                r => r.map(Box::new).map_err(|e| format!("Could not open {}: {e}", trellis_core::native_path::shown(&path))),
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
        Effect::LoadWorkdirCompare { workdir, oid, generation } => Some(Msg::Selection(SelectionMsg::WorkdirCompareLoaded {
            oid,
            generation,
            result: repo::compare_workdir(&workdir, oid).map_err(err),
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
        Effect::LoadConflict { workdir, path, generation } => {
            let result = repo::load_conflict(&workdir, &path).map_err(err);
            // The picks a file saved with markers holds come back with it.
            let saved = result.as_ref().ok().and_then(|f| repo::saved_picks(&workdir, f));
            Some(Msg::Conflict(ConflictMsg::ConflictLoaded { path: path.clone(), generation, result, saved }))
        }
        Effect::LoadSparse { workdir, generation } => Some(sparse::load(&workdir, generation)),
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
        Effect::DetectLfs { cwd } => Some(Msg::Lfs(LfsMsg::Detected(trellis_core::lfs::installed(&cwd)))),
        Effect::RemoveWorktree(r) => Some(worktrees::remove(r)),
        Effect::MoveWorktree(r) => Some(worktrees::move_(r)),
        Effect::RenameStash { cwd, index, oid, message } => Some(stashes::rename(&cwd, index, oid, &message)),
        Effect::LoadLfs { workdir, generation } => {
            Some(Msg::Lfs(LfsMsg::Loaded { generation, result: trellis_core::lfs::status(&workdir).map_err(err) }))
        }
        Effect::RunGit { cwd, cmds, write_file, record, prompts, expect } => {
            if let Some(refused) = expect.and_then(|(rev, oid)| expect::check(&cwd, &rev, oid).err()) {
                return Some(Msg::Job(JobMsg::JobDone { ok: false, output: refused, undo: record.map(|_| UndoResult::Recorded(None)) }));
            }
            let before = record.as_ref().and_then(|k| undo::before(&cwd, k).ok());
            // A pop onto local changes mixes them with the stash's, so it can't be undone.
            let irreversible = before.as_ref().is_some_and(undo::irreversible);
            let (ok, output) = git_ops::run_git(&cwd, &cmds, write_file, prompts);
            let recorded = |k| if ok { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
            let undo = if ok && irreversible { None } else { record.map(|k| UndoResult::Recorded(recorded(k))) };
            Some(Msg::Job(JobMsg::JobDone { ok, output, undo }))
        }
        Effect::UndoRedo { cwd, action, redo, opts } => Some(git_ops::undo_redo(&cwd, action, redo, opts)),
        Effect::Rebase(e) => Some(rebase::run(e)),
        Effect::Forge { target, cwd, auth, interactive, connected, call } => {
            Some(Msg::Forge(ForgeMsg::ForgeReplied(forge_calls::call(target, &cwd, auth, interactive, connected, call))))
        }
        Effect::RepoOwners { account, cwd } => {
            let result = forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::repo_owners(&account, &a)).map_err(err);
            Some(Msg::Publish(PublishMsg::PublishOwners { account, result }))
        }
        Effect::CreateRepo { account, cwd, user, repo, branch } => {
            let result =
                forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::create_repo(&account, &a, &user, &repo)).map_err(err);
            Some(Msg::Publish(PublishMsg::Published { workdir: cwd, branch, result }))
        }
        Effect::Connect { account, user, token, cwd } => Some(forge_calls::connect(account, user, token, &cwd)),
        Effect::TrelloBoards { account, cwd } => Some(forge_calls::trello_boards(account, &cwd)),
        Effect::SaveToken { account, user, token } => forge::save_token(&account, user.as_deref(), &token)
            .err()
            .map(|e| Msg::Logged { ok: false, text: format!("Connected, but the token could not be saved: {e}") }),
        Effect::DeleteToken(account) => forge::delete_token(&account).err().map(|e| Msg::Logged { ok: false, text: e.to_string() }),
        Effect::ForgetCredentialManager { accounts, cwd } => {
            let failures: Vec<String> = accounts
                .iter()
                .filter_map(|a| forge::forget_credential_manager(a, &cwd).err().map(|e| format!("{}: {e}", a.display())))
                .collect();
            (!failures.is_empty()).then(|| Msg::Logged { ok: false, text: failures.join("\n") })
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
        Effect::ReadGitConfig { cwd, has_repo } => Some(Msg::Prefs(PrefsMsg::GitConfigLoaded(config::read_git_config(&cwd, has_repo)))),
        Effect::SetGitConfig { cwd, global, key, value } => config::set(&cwd, global, &key, value.as_deref()),
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
        Effect::OpenUrl(url) => os::open_url(&url),
        Effect::OpenTerminal(dir) => os::open_terminal(&dir),
        Effect::OpenFolder(dir) => os::open_folder(&dir),
        Effect::OpenPath { workdir, path, confirmed } => os::open_path(&workdir, &path, confirmed),
        Effect::RevealPath(p) => os::reveal(&p),
        Effect::PickFolder { title, purpose } => {
            let path = rfd::FileDialog::new().set_title(title).pick_folder();
            Some(Msg::Prefs(PrefsMsg::FolderPicked { purpose, path }))
        }
        Effect::PickFile { title, purpose } => {
            let path = rfd::FileDialog::new().set_title(title).pick_file();
            Some(Msg::Prefs(PrefsMsg::FilePicked { purpose, path }))
        }
        Effect::ListSigningKeys { format, program } => Some(keys::list(format, &program)),
        Effect::GenerateGpgKey { program, name, email, passphrase } => Some(keys::generate_gpg(&program, &name, &email, &passphrase)),
        Effect::ExportPublicKey { format, program, key } => Some(keys::export_public(format, &program, &key)),
        Effect::Ssh(e) => Some(ssh::run(e)),
        Effect::Credential(e) => Some(credentials::run(e)),
        Effect::Profile(e) => Some(profiles::run(e)),
        Effect::Patch(p) => Some(patches::run(p)),
        Effect::Tool(t) => tools::run(t),
        Effect::GitEnv(e) => Some(git_env::run(e)),
        Effect::Onboarding(e) => Some(onboarding::run(e)),
        Effect::Theme(e) => themes::run(e),
        Effect::CheckConflicts(job) => Some(conflict_check::run(job)),
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
        Effect::Ui(_) => {
            tracing::warn!("UI effect reached the runner; it should be handled by the UI shell");
            None
        }
        Effect::Watch(_) | Effect::AnswerPrompt { .. } | Effect::Terminal(_) | Effect::Logs(_) => {
            tracing::warn!("watch, prompt, terminal or log effect reached a worker; it is handled by Runner::run");
            None
        }
    }
}
