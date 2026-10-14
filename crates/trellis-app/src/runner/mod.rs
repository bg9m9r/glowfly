//! Executes `Effect`s on worker threads and reports results as `Msg`s. This is the only place
//! the app starts background work.

use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult, LocalOp, UndoResult};
use crate::lfs::LfsMsg;
use crate::msg::Msg;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use trellis_core::avatars;
use trellis_core::forge::{self, Auth};
use trellis_core::gitcli::{self, args};
use trellis_core::repo;
use trellis_core::signing::{self, SigningFormat};
use trellis_core::undo::{self, Action, Plan};

mod askpass;
mod config;
mod expect;
pub(crate) mod files;
mod guard;
mod highlight;
mod names;
mod os;
mod queue;
mod rebase;
mod saves;
mod submodules;
mod terminal;
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
    /// Counts HighlightDiff jobs started: a job whose number is no longer the latest stops.
    highlight_epoch: Arc<AtomicU64>,
    /// The built-in terminal's shell sessions.
    terminals: terminal::Terminals,
    /// Keeps saves of each settings file in order.
    saves: saves::Saves,
    /// Effects that change a repository run one at a time per repository.
    queues: Arc<queue::RepoQueues>,
}

impl<W: Waker + Clone> Runner<W> {
    pub fn new(tx: Sender<Msg>, waker: W) -> Self {
        Self {
            tx,
            waker,
            watch: Default::default(),
            askpass: None,
            highlight_epoch: Default::default(),
            terminals: Default::default(),
            saves: Default::default(),
            queues: Default::default(),
        }
    }

    /// Starts `effect` on a worker thread, or queues it behind the earlier effects that change the
    /// same repository. UI effects must be handled by the caller instead. A panic in the effect
    /// is reported as its failure, so a job it was running ends.
    pub fn run(&self, effect: Effect) {
        let effect = match effect {
            Effect::Watch(target) => return self.watch.set(target, self.tx.clone(), self.waker.clone()),
            Effect::AnswerPrompt { id, reply } => return self.answer_prompt(id, reply),
            Effect::Terminal(t) => return self.terminal(t),
            other => match self.save(other) {
                Some(other) => other,
                None => return,
            },
        };
        let tx = self.tx.clone();
        let waker = self.waker.clone();
        let name = names::effect_name(&effect);
        let epoch = &self.highlight_epoch;
        let ticket = matches!(effect, Effect::HighlightDiff { .. }).then(|| (epoch.clone(), epoch.fetch_add(1, Ordering::SeqCst) + 1));
        let repo = queue::repo_of(&effect).map(Path::to_owned);
        let recovery = guard::Recovery::of(&effect, name);
        let job: queue::Job = Box::new(move || {
            let _span = tracing::debug_span!("effect", name).entered();
            let cancelled = || ticket.as_ref().is_some_and(|(e, mine)| e.load(Ordering::Relaxed) != *mine);
            if let Some(msg) = guard::guarded(recovery, || execute_with(effect, &cancelled))
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
    execute_with(effect, &|| false)
}

/// `execute`, stopping early when `cancelled` returns true (only HighlightDiff checks it).
fn execute_with(effect: Effect, cancelled: &dyn Fn() -> bool) -> Option<Msg> {
    let scope = effect.scope().cloned();
    let msg = execute_unscoped(effect, cancelled)?;
    Some(match scope {
        Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
        None => msg,
    })
}

fn execute_unscoped(effect: Effect, cancelled: &dyn Fn() -> bool) -> Option<Msg> {
    match effect {
        Effect::LoadRepo { path, opts, generation, kind } => {
            let result = repo::load(&path, &opts).map(Box::new).map_err(|e| format!("Could not open {}: {e}", path.display()));
            Some(Msg::RepoLoaded { generation, kind, result })
        }
        Effect::LoadStatus { workdir, generation, submodules } => {
            let result = repo::status_at(&workdir).map_err(err);
            let submodules = repo::submodules_if_any(&workdir, submodules).ok().flatten();
            Some(Msg::StatusLoaded { generation, result, submodules })
        }
        Effect::LoadDetail { workdir, oid } => Some(Msg::DetailLoaded { oid, result: repo::commit_detail(&workdir, oid).map_err(err) }),
        Effect::LoadCompare { workdir, from, to } => {
            Some(Msg::CompareLoaded { from, to, result: repo::compare(&workdir, from, to).map_err(err) })
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
        Effect::LoadConflict { workdir, path, generation } => {
            Some(Msg::ConflictLoaded { path: path.clone(), generation, result: repo::load_conflict(&workdir, &path).map_err(err) })
        }
        Effect::LoadFileHistory { workdir, path, start, limit, generation } => {
            let result =
                repo::file_history(&workdir, &path, &start, limit).map_err(|e| format!("Could not load the history of {path}: {e}"));
            Some(Msg::FileHistoryLoaded { path, generation, result })
        }
        Effect::LocalOp { workdir, op } => Some(Msg::LocalOpDone(local_op(&workdir, op).map_err(err))),
        Effect::RevertHunk { workdir, target, index, expected } => {
            let result = repo::revert_hunk(&workdir, &target, index, &expected).map_err(err);
            Some(Msg::HunkReverted { path: target.path().to_owned(), result })
        }
        Effect::RestoreFile { workdir, oid, path, overwrite } => Some(match repo::restore_file(&workdir, oid, &path, overwrite) {
            Err(trellis_core::error::Error::WouldOverwrite(reason)) => Msg::RestoreWouldOverwrite { path, oid, reason },
            result => Msg::FileRestored { result: result.map_err(err), path, oid },
        }),
        Effect::WorkFile { workdir, op } => Some(files::work_file(&workdir, op)),
        Effect::LoadTrackedPaths { workdir, generation } => Some(files::tracked_paths(&workdir, generation)),
        Effect::CheckSubmodule { workdir, path } => Some(submodules::check(&workdir, path)),
        Effect::CheckWorktreeFolder(r) => Some(worktrees::check(r)),
        Effect::CheckCloneParent { parent, delay } => Some(files::check_clone_parent(parent, delay)),
        Effect::DetectLfs { cwd } => Some(Msg::Lfs(LfsMsg::Detected(trellis_core::lfs::installed(&cwd)))),
        Effect::RemoveWorktree(r) => Some(worktrees::remove(r)),
        Effect::LoadLfs { workdir, generation } => {
            Some(Msg::Lfs(LfsMsg::Loaded { generation, result: trellis_core::lfs::status(&workdir).map_err(err) }))
        }
        Effect::RunGit { cwd, cmds, write_file, record, prompts, expect } => {
            if let Some(refused) = expect.and_then(|(rev, oid)| expect::check(&cwd, &rev, oid).err()) {
                return Some(Msg::JobDone { ok: false, output: refused, undo: record.map(|_| UndoResult::Recorded(None)) });
            }
            let before = record.as_ref().and_then(|k| undo::before(&cwd, k).ok());
            let (ok, output) = run_git(&cwd, &cmds, write_file, prompts);
            let recorded = |k| if ok { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
            let undo = record.map(|k| UndoResult::Recorded(recorded(k)));
            Some(Msg::JobDone { ok, output, undo })
        }
        Effect::UndoRedo { cwd, action, redo } => Some(undo_redo(&cwd, action, redo)),
        Effect::LoadRebasePlan { workdir, base, generation } => Some(rebase::load_plan(&workdir, &base, generation)),
        Effect::InteractiveRebase { cwd, request } => Some(rebase::start(&cwd, &request)),
        Effect::Forge { target, cwd, auth, interactive, connected, call } => {
            Some(Msg::ForgeReplied(forge_call(target, &cwd, auth, interactive, connected, call)))
        }
        Effect::RepoOwners { account, cwd } => {
            let result = forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::repo_owners(&account, &a)).map_err(err);
            Some(Msg::PublishOwners { account, result })
        }
        Effect::CreateRepo { account, cwd, user, repo, branch } => {
            let result =
                forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::create_repo(&account, &a, &user, &repo)).map_err(err);
            Some(Msg::Published { workdir: cwd, branch, result })
        }
        Effect::Connect { account, user, token, cwd } => {
            let (method, auth, saved) = match token {
                Some(t) => {
                    let auth = Auth::from_token(account.provider, user.as_deref(), &t, "saved token");
                    ("access token".to_owned(), Ok(auth), Some((user, t)))
                }
                None => ("Git Credential Manager".to_owned(), forge::from_credential_manager(&account, &cwd, true), None),
            };
            let result = auth.and_then(|a| forge::verify(&account, &a)).map_err(err);
            Some(Msg::Connected { account, method, token: saved, result })
        }
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
        Effect::Stats { workdir, oids } => Some(Msg::StatsLoaded(repo::commit_stats(&workdir, &oids))),
        Effect::FetchAvatars(emails) => Some(Msg::AvatarsLoaded(
            emails
                .into_iter()
                .map(|e| {
                    let img = avatars::fetch(&e);
                    (e, img)
                })
                .collect(),
        )),
        Effect::ReadGitConfig { cwd, has_repo } => Some(Msg::GitConfigLoaded(config::read_git_config(&cwd, has_repo))),
        Effect::SetGitConfig { cwd, global, key, value } => {
            let scope = if global { "--global" } else { "--local" };
            let a = match &value {
                Some(v) => args(&["config", scope, &key, v]),
                None => args(&["config", scope, "--unset", &key]),
            };
            let (ok, out) = gitcli::run(&cwd, &a);
            // `--unset` of a missing key exits 5; that's fine.
            (!ok && value.is_some()).then(|| Msg::Logged { ok: false, text: format!("git config {key} failed: {out}") })
        }
        save @ (Effect::SaveSettings(_) | Effect::SaveRecent(_) | Effect::SaveTabs { .. }) => saves::save_now(save),
        Effect::OpenUrl(url) => os::open_url(&url),
        Effect::OpenTerminal(dir) => os::open_terminal(&dir),
        Effect::OpenFolder(dir) => os::open_folder(&dir),
        Effect::OpenPath(p) => os::open_path(&p),
        Effect::RevealPath(p) => os::reveal(&p),
        Effect::PickFolder { title, purpose } => {
            let path = rfd::FileDialog::new().set_title(title).pick_folder();
            Some(Msg::FolderPicked { purpose, path })
        }
        Effect::PickFile { title, purpose } => {
            let path = rfd::FileDialog::new().set_title(title).pick_file();
            Some(Msg::FilePicked { purpose, path })
        }
        Effect::ListSigningKeys { format, program } => {
            let now = chrono::Utc::now().timestamp();
            let result = match format {
                SigningFormat::Ssh => Ok(signing::home_dir().map(|h| signing::list_ssh_keys(&h)).unwrap_or_default()),
                SigningFormat::OpenPgp => signing::list_gpg_keys(&program, now).map_err(err),
                // X.509 certificates are chosen by id; listing them through gpgsm isn't supported yet.
                SigningFormat::X509 => Ok(Vec::new()),
            };
            Some(Msg::SigningKeysLoaded(result))
        }
        Effect::GenerateGpgKey { program, name, email, passphrase } => {
            let now = chrono::Utc::now().timestamp();
            Some(Msg::GpgKeyGenerated(signing::generate_gpg_key(&program, &name, &email, &passphrase, now).map_err(err)))
        }
        Effect::ExportPublicKey { format, program, key } => {
            Some(Msg::PublicKeyExported(signing::export_public_key(format, &program, &key).map_err(err)))
        }
        Effect::Ui(_) => {
            tracing::warn!("UI effect reached the runner; it should be handled by the UI shell");
            None
        }
        Effect::Watch(_) | Effect::AnswerPrompt { .. } | Effect::Terminal(_) => {
            tracing::warn!("watch, prompt or terminal effect reached a worker; it is handled by Runner::run");
            None
        }
    }
}

/// Runs a local op. Discards save the file's contents first and return the undo action.
fn local_op(workdir: &Path, op: LocalOp) -> trellis_core::error::Result<Option<Action>> {
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
    }
}

/// Checks that `action` still applies, then runs the git commands that undo or redo it.
fn undo_redo(cwd: &Path, action: Action, redo: bool) -> Msg {
    let failed = |output: String| Msg::JobDone { ok: false, output, undo: Some(UndoResult::Failed { redo }) };
    match undo::plan(cwd, &action, redo) {
        Err(e) => failed(e.to_string()),
        Ok(Plan::Refuse(r)) => {
            Msg::JobDone { ok: false, output: r.message().into(), undo: Some(UndoResult::Refused { redo, stale: r.is_stale() }) }
        }
        Ok(Plan::Write { path, content }) => match undo::write_saved(cwd, &path, content) {
            Ok(()) => {
                let output = format!("{} {path}", if redo { "Discarded the changes in" } else { "Restored" });
                Msg::JobDone { ok: true, output, undo: Some(UndoResult::Done { redo, before: action.clone(), after: action }) }
            }
            Err(e) => failed(format!("Could not write {path}: {e}")),
        },
        Ok(Plan::Run(cmds)) => {
            let (ok, output) = run_git(cwd, &cmds, None, true);
            if !ok {
                return failed(output);
            }
            let after = if redo { undo::redone(cwd, action.clone()).unwrap_or_else(|_| action.clone()) } else { action.clone() };
            Msg::JobDone { ok, output, undo: Some(UndoResult::Done { redo, before: action, after }) }
        }
    }
}

/// Runs `cmds` in order, stopping at the first failure. Without `prompts`, git fails rather than
/// open a credential prompt.
fn run_git(cwd: &Path, cmds: &[Vec<String>], write_file: Option<(std::path::PathBuf, String)>, prompts: bool) -> (bool, String) {
    if let Some((path, contents)) = write_file
        && let Err(e) = std::fs::write(&path, contents)
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

fn forge_call(
    target: trellis_core::forge::Target,
    cwd: &Path,
    auth: Option<Auth>,
    interactive: bool,
    connected: bool,
    call: ForgeCall,
) -> ForgeReply {
    let auth = match auth {
        Some(a) => a,
        None => match forge::resolve_auth(&target.account(), cwd, interactive, connected) {
            Ok(a) => a,
            Err(e) => return ForgeReply { target, auth: None, results: vec![ForgeResult::AuthFailed(err(e))] },
        },
    };
    let t = &target;
    let results = match call {
        ForgeCall::Refresh { need_default, with_issues } => {
            let mut r = vec![ForgeResult::Prs(forge::pull_requests(t, &auth).map_err(ForgeError::from))];
            if need_default {
                r.push(ForgeResult::DefaultBranch(forge::default_branch(t, &auth).ok().flatten()));
            }
            if with_issues {
                r.push(ForgeResult::MyIssues(forge::my_issues(t, &auth).map_err(ForgeError::from)));
            }
            r
        }
        ForgeCall::MyIssues => vec![ForgeResult::MyIssues(forge::my_issues(t, &auth).map_err(ForgeError::from))],
        ForgeCall::Issues(keys) => {
            let r = forge::issues(t, &auth, &keys).map_err(ForgeError::from);
            vec![ForgeResult::Issues(keys, r)]
        }
        ForgeCall::Create(pr) => vec![ForgeResult::Created(forge::create_pull_request(t, &auth, &pr).map_err(ForgeError::from))],
    };
    ForgeReply { target, auth: Some(auth), results }
}
