//! Executes `Effect`s on worker threads and reports results as `Msg`s. This is the only place
//! the app starts background work.
//!
//! `Runner::run` starts one named thread per effect. A few effects get special handling: the
//! file watcher (`Effect::Watch`), credential-prompt answers and the terminal's shell sessions
//! are run by the runner itself, the log files have one ordered worker (`logs`), effects are
//! timed for the performance log, and saves of Trellis's own files are kept in order (`saves`). Work longer than a line lives in this
//! folder's per-area files (`files`, `worktrees`, `rebase`, `highlight`, …); the git, file and
//! network code itself lives in `trellis-core`.

use crate::effect::{Effect, UndoResult};
use crate::lfs::LfsMsg;
use crate::msg::AvatarMsg;
use crate::msg::ConflictMsg;
use crate::msg::ForgeMsg;
use crate::msg::HistoryMsg;
use crate::msg::JobMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::PublishMsg;
use crate::msg::RecentMsg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::msg::StageMsg;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use trellis_core::avatars;
use trellis_core::forge;
use trellis_core::repo;
use trellis_core::undo;

mod askpass;
mod blame;
mod clone;
mod config;
mod conflict_check;
mod deep_link;
mod expect;
pub(crate) mod files;
mod forge_calls;
mod forge_filter_calls;
mod forks;
mod git_env;
mod git_ops;
mod guard;
mod highlight;
mod init;
mod instance;
mod keys;
mod logs;
mod notify;
mod onboarding;
mod os;
mod patches;
mod path_search;
mod profiles;
mod pull_request;
mod queue;
mod rebase;
mod saves;
mod signatures;
mod sparse;
mod ssh;
mod stashes;
mod stats;
mod status;
mod submodules;
mod terminal;
mod terminal_out;
mod themes;
mod tools;
mod worktrees;

#[cfg(test)]
pub(crate) use highlight::{decorate, decorate_in_context};

/// Wakes the UI when a result arrives (so it can process the message promptly).
pub trait Waker: Send + Sync + 'static {
    fn wake(&self);
}

impl Waker for eframe::egui::Context {
    fn wake(&self) {
        self.request_repaint();
    }
}

pub struct Runner<W: Waker + Clone> {
    tx: Sender<Msg>,
    waker: W,
    watch: crate::watcher::WatchSlot,
    /// The credential-prompt listener, once started (never in tests).
    askpass: Option<Arc<trellis_core::askpass::Server>>,
    /// The listener for links a second Trellis process hands over, once started (never in
    /// tests); dropping it removes the instance file.
    instance: Option<trellis_core::instance::Listener>,
    /// Counts HighlightDiff jobs started: a job whose number is no longer the latest stops.
    highlight_epoch: Arc<AtomicU64>,
    /// The built-in terminal's shell sessions.
    terminals: terminal::Terminals,
    /// Keeps saves of each settings file in order.
    saves: saves::Saves,
    /// Effects that change a repository run one at a time per repository.
    queues: Arc<queue::RepoQueues>,
    /// Works out the graph's line counts, newest request first.
    stats: stats::StatsWorker,
    /// Finds the commits touching the search's path terms, newest request first.
    path_search: path_search::PathSearchWorker,
    /// Ahead/behind and containment answers, kept between repository loads.
    graph_cache: Arc<repo::GraphCache>,
    /// The log files' worker (attached at startup, never in tests).
    logs: logs::LogWorker,
}

impl<W: Waker + Clone> Runner<W> {
    pub fn new(tx: Sender<Msg>, waker: W) -> Self {
        Self {
            tx,
            waker,
            watch: Default::default(),
            askpass: None,
            instance: None,
            highlight_epoch: Default::default(),
            terminals: Default::default(),
            saves: Default::default(),
            queues: Default::default(),
            graph_cache: Default::default(),
            stats: Default::default(),
            path_search: Default::default(),
            logs: Default::default(),
        }
    }

    /// Starts the log worker on the folder `logging::init` chose. Once, at startup.
    pub fn attach_logs(&mut self, setup: crate::logging::LogSetup) {
        self.logs.attach(setup, self.tx.clone(), self.waker.clone());
    }

    /// Starts `effect` on a worker thread, or queues it behind the earlier effects that change the
    /// same repository. UI effects must be handled by the caller instead. A panic in the effect
    /// is reported as its failure, so a job it was running ends.
    pub fn run(&self, effect: Effect) {
        let effect = match effect {
            Effect::Watch(target) => return self.watch.set(target, self.tx.clone(), self.waker.clone()),
            Effect::AnswerPrompt { id, reply } => return self.answer_prompt(id, reply),
            Effect::Terminal(t) => return self.terminal(t),
            Effect::Stats { workdir, oids } => return self.stats.submit(workdir, oids, self.tx.clone(), self.waker.clone()),
            Effect::Logs(e) => return self.logs.submit(e),
            Effect::SearchPaths { workdir, request, terms, oids } => {
                let r = path_search::Request { workdir, request, terms, oids };
                return self.path_search.submit(r, self.tx.clone(), self.waker.clone());
            }
            other => match self.save(other) {
                Some(other) => other,
                None => return,
            },
        };
        let tx = self.tx.clone();
        let waker = self.waker.clone();
        let name = effect.name();
        let epoch = &self.highlight_epoch;
        let ticket = matches!(effect, Effect::HighlightDiff { .. }).then(|| (epoch.clone(), epoch.fetch_add(1, Ordering::SeqCst) + 1));
        let repo = queue::repo_of(&effect).map(Path::to_owned);
        let recovery = guard::Recovery::of(&effect, name);
        let graph_cache = Arc::clone(&self.graph_cache);
        // Effects that don't wait on the user are timed for the performance log.
        let perf = if effect.timed() { self.logs.perf_probe().map(|p| (p, logs::folder_of(&effect))) } else { None };
        let job: queue::Job = Box::new(move || {
            let _span = tracing::debug_span!("effect", name).entered();
            let cancelled = || ticket.as_ref().is_some_and(|(e, mine)| e.load(Ordering::Relaxed) != *mine);
            let started = std::time::Instant::now();
            let msg = guard::guarded(recovery, || execute_with(effect, &cancelled, &graph_cache));
            let took = started.elapsed();
            if let Some((probe, detail)) = perf
                && took >= trellis_core::logs::SLOW
            {
                let _ = probe.send(logs::Job::Perf { took, what: name.to_owned(), detail });
            }
            if let Some(msg) = msg
                && tx.send(msg).is_ok()
            {
                waker.wake();
            }
        });
        let job = match repo {
            Some(repo) => match self.queues.submit(&repo, job) {
                Ok(()) => return,
                Err(job) => job,
            },
            None => job,
        };
        if let Err(e) = std::thread::Builder::new().name(format!("trellis-{name}")).spawn(job) {
            tracing::error!("could not start a worker thread for {name}: {e}");
        }
    }
}

fn err(e: trellis_core::error::Error) -> String {
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
fn execute_with(effect: Effect, cancelled: &dyn Fn() -> bool, graph_cache: &repo::GraphCache) -> Option<Msg> {
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
        Effect::LoadDiff { workdir, target, reload, generation, previous, ignore_whitespace, whole_file } => Some(highlight::load_job(
            workdir,
            target,
            reload,
            generation,
            previous,
            repo::LoadDiffOptions { ignore_whitespace, whole_file },
        )),
        Effect::HighlightDiff { target, generation, diff, .. } => highlight::finish_job(target, generation, &diff, cancelled),
        Effect::LoadConflict { workdir, path, generation } => Some(Msg::Conflict(ConflictMsg::ConflictLoaded {
            path: path.clone(),
            generation,
            result: repo::load_conflict(&workdir, &path).map_err(err),
        })),
        Effect::LoadSparse { workdir, generation } => Some(sparse::load(&workdir, generation)),
        Effect::SparseCheckout { cwd, command } => Some(sparse::run(&cwd, &command)),
        Effect::LoadFileHistory { workdir, path, start, limit, generation } => {
            let result =
                repo::file_history(&workdir, &path, &start, limit).map_err(|e| format!("Could not load the history of {path}: {e}"));
            Some(Msg::History(HistoryMsg::FileHistoryLoaded { path, generation, result }))
        }
        Effect::LoadBlame(r) => Some(blame::load(r)),
        Effect::LocalOp { workdir, op } => Some(Msg::Stage(StageMsg::LocalOpDone(git_ops::local_op(&workdir, op).map_err(err)))),
        Effect::RevertHunk { workdir, target, index, expected } => {
            let result = repo::revert_hunk(&workdir, &target, index, &expected).map_err(err);
            Some(Msg::Stage(StageMsg::HunkReverted { path: target.path().to_owned(), result }))
        }
        Effect::RestoreFile { workdir, oid, path, overwrite } => Some(files::restore(&workdir, oid, path, overwrite)),
        Effect::WorkFile { workdir, op } => Some(files::work_file(&workdir, op)),
        Effect::LoadTrackedPaths { workdir, generation } => Some(files::tracked_paths(&workdir, generation)),
        Effect::Submodule(e) => Some(submodules::run(e)),
        Effect::CheckWorktreeFolder(r) => Some(worktrees::check(r)),
        Effect::CheckCloneParent { parent, delay } => Some(files::check_clone_parent(parent, delay)),
        Effect::DetectLfs { cwd } => Some(Msg::Lfs(LfsMsg::Detected(trellis_core::lfs::installed(&cwd)))),
        Effect::RemoveWorktree(r) => Some(worktrees::remove(r)),
        Effect::RenameStash { cwd, index, oid, message } => Some(stashes::rename(&cwd, index, oid, &message)),
        Effect::LoadLfs { workdir, generation } => {
            Some(Msg::Lfs(LfsMsg::Loaded { generation, result: trellis_core::lfs::status(&workdir).map_err(err) }))
        }
        Effect::RunGit { cwd, cmds, write_file, record, prompts, expect } => {
            if let Some(refused) = expect.and_then(|(rev, oid)| expect::check(&cwd, &rev, oid).err()) {
                return Some(Msg::Job(JobMsg::JobDone { ok: false, output: refused, undo: record.map(|_| UndoResult::Recorded(None)) }));
            }
            let before = record.as_ref().and_then(|k| undo::before(&cwd, k).ok());
            let (ok, output) = git_ops::run_git(&cwd, &cmds, write_file, prompts);
            let recorded = |k| if ok { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
            let undo = record.map(|k| UndoResult::Recorded(recorded(k)));
            Some(Msg::Job(JobMsg::JobDone { ok, output, undo }))
        }
        Effect::UndoRedo { cwd, action, redo } => Some(git_ops::undo_redo(&cwd, action, redo)),
        Effect::LoadRebasePlan { workdir, base, generation } => Some(rebase::load_plan(&workdir, &base, generation)),
        Effect::InteractiveRebase { cwd, request } => Some(rebase::start(&cwd, &request)),
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
