//! Executes `Effect`s on worker threads and reports results as `Msg`s. This is the only place
//! the app starts background work.

use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult, LocalOp, UndoResult};
use crate::msg::Msg;
use crate::state::prefs::GitConfigValues;
use std::path::Path;
use std::sync::mpsc::Sender;
use trellis_core::avatars;
use trellis_core::forge::{self, Auth};
use trellis_core::gitcli::{self, args};
use trellis_core::highlight;
use trellis_core::intraline;
use trellis_core::repo;
use trellis_core::signing::{self, SigningFormat};
use trellis_core::undo::{self, Action, Plan};

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
}

impl<W: Waker + Clone> Runner<W> {
    pub fn new(tx: Sender<Msg>, waker: W) -> Self {
        Self { tx, waker }
    }

    /// Starts `effect` on a worker thread. UI effects must be handled by the caller instead.
    pub fn run(&self, effect: Effect) {
        let tx = self.tx.clone();
        let waker = self.waker.clone();
        let name = effect_name(&effect);
        let spawned = std::thread::Builder::new().name(format!("trellis-{name}")).spawn(move || {
            let _span = tracing::debug_span!("effect", name).entered();
            if let Some(msg) = execute(effect)
                && tx.send(msg).is_ok()
            {
                waker.wake();
            }
        });
        if let Err(e) = spawned {
            tracing::error!("could not start a worker thread for {name}: {e}");
        }
    }
}

fn effect_name(e: &Effect) -> &'static str {
    match e {
        Effect::LoadRepo { .. } => "load-repo",
        Effect::LoadStatus { .. } => "load-status",
        Effect::LoadDetail { .. } => "load-detail",
        Effect::LoadDiff { .. } => "load-diff",
        Effect::LoadConflict { .. } => "load-conflict",
        Effect::LocalOp { .. } => "local-op",
        Effect::RunGit { .. } => "git",
        Effect::UndoRedo { .. } => "undo-redo",
        Effect::Forge { .. } => "forge",
        Effect::Connect { .. } => "connect",
        Effect::RepoOwners { .. } => "repo-owners",
        Effect::CreateRepo { .. } => "create-repo",
        Effect::SaveToken { .. } => "save-token",
        Effect::DeleteToken(_) => "delete-token",
        Effect::ForgetCredentialManager { .. } => "forget-gcm",
        Effect::Stats { .. } => "stats",
        Effect::FetchAvatars(_) => "avatars",
        Effect::ReadGitConfig { .. } => "read-config",
        Effect::SetGitConfig { .. } => "set-config",
        Effect::SaveSettings(_) => "save-settings",
        Effect::SaveRecent(_) => "save-recent",
        Effect::SaveTabs { .. } => "save-tabs",
        Effect::OpenUrl(_) => "open-url",
        Effect::OpenTerminal(_) => "open-terminal",
        Effect::OpenFolder(_) => "open-folder",
        Effect::PickFolder { .. } => "pick-folder",
        Effect::PickFile { .. } => "pick-file",
        Effect::ListSigningKeys { .. } => "list-keys",
        Effect::GenerateGpgKey { .. } => "generate-key",
        Effect::ExportPublicKey { .. } => "export-key",
        Effect::Ui(_) => "ui",
    }
}

fn err(e: trellis_core::error::Error) -> String {
    e.to_string()
}

/// Runs one effect to completion (blocking) and returns the message to send back, if any.
/// Results that belong to one repository come back as `Msg::Scoped`.
pub fn execute(effect: Effect) -> Option<Msg> {
    let scope = effect.scope().cloned();
    let msg = execute_unscoped(effect)?;
    Some(match scope {
        Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
        None => msg,
    })
}

fn execute_unscoped(effect: Effect) -> Option<Msg> {
    match effect {
        Effect::LoadRepo { path, opts, generation, kind } => {
            let result = repo::load(&path, &opts).map(Box::new).map_err(|e| format!("Could not open {}: {e}", path.display()));
            Some(Msg::RepoLoaded { generation, kind, result })
        }
        Effect::LoadStatus { workdir, generation } => {
            Some(Msg::StatusLoaded { generation, result: repo::status_at(&workdir).map_err(err) })
        }
        Effect::LoadDetail { workdir, oid } => Some(Msg::DetailLoaded { oid, result: repo::commit_detail(&workdir, oid).map_err(err) }),
        Effect::LoadDiff { workdir, target, reload, generation, previous } => {
            let result = repo::load_diff(&workdir, &target)
                .map(|mut d| {
                    d.highlight = highlight::highlight_diff(target.path(), &d, previous.as_ref());
                    d.intraline = intraline::intraline(&d);
                    let syntax = d.highlight.as_ref().map_or("plain", |h| h.syntax.as_str());
                    let marked = (0..d.hunks.len())
                        .map(|h| (0..d.hunks[h].lines.len()).filter(|&l| !d.intraline.line(h, l).is_empty()).count())
                        .sum::<usize>();
                    tracing::debug!(path = target.path(), syntax, marked, "diff highlighted");
                    d
                })
                .map_err(err);
            Some(Msg::DiffLoaded { target, reload, generation, result })
        }
        Effect::LoadConflict { workdir, path, generation } => {
            Some(Msg::ConflictLoaded { path: path.clone(), generation, result: repo::load_conflict(&workdir, &path).map_err(err) })
        }
        Effect::LocalOp { workdir, op } => Some(Msg::LocalOpDone(local_op(&workdir, op).map_err(err))),
        Effect::RunGit { cwd, cmds, write_file, record } => {
            let before = record.as_ref().and_then(|k| undo::before(&cwd, k).ok());
            let (ok, output) = run_git(&cwd, &cmds, write_file);
            let recorded = |k| if ok { before.and_then(|b| undo::after(&cwd, k, b).ok().flatten()) } else { None };
            let undo = record.map(|k| UndoResult::Recorded(recorded(k)));
            Some(Msg::JobDone { ok, output, undo })
        }
        Effect::UndoRedo { cwd, action, redo } => Some(undo_redo(&cwd, action, redo)),
        Effect::Forge { target, cwd, auth, interactive, call } => {
            Some(Msg::ForgeReplied(forge_call(target, &cwd, auth, interactive, call)))
        }
        Effect::RepoOwners { account, cwd } => {
            let result = forge::resolve_auth(&account, &cwd, true).and_then(|a| forge::repo_owners(&account, &a)).map_err(err);
            Some(Msg::PublishOwners { account, result })
        }
        Effect::CreateRepo { account, cwd, user, repo, branch } => {
            let result =
                forge::resolve_auth(&account, &cwd, true).and_then(|a| forge::create_repo(&account, &a, &user, &repo)).map_err(err);
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
        Effect::ReadGitConfig { cwd, has_repo } => Some(Msg::GitConfigLoaded(read_git_config(&cwd, has_repo))),
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
        Effect::SaveSettings(settings) => {
            settings.save();
            None
        }
        Effect::SaveRecent(list) => {
            crate::platform::save_recent(&list);
            None
        }
        Effect::SaveTabs { paths, active } => {
            crate::platform::save_tabs(&paths, active);
            None
        }
        Effect::OpenUrl(url) => {
            open::that_detached(&url).err().map(|e| Msg::Logged { ok: false, text: format!("Could not open {url}: {e}") })
        }
        Effect::OpenTerminal(dir) => {
            crate::platform::open_terminal(&dir);
            None
        }
        Effect::OpenFolder(dir) => {
            crate::platform::open_in_file_manager(&dir);
            None
        }
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
    }
}

fn local_op(workdir: &Path, op: LocalOp) -> trellis_core::error::Result<()> {
    match op {
        LocalOp::StageFile(p) => repo::stage_file(workdir, &p),
        LocalOp::StageAll => repo::stage_all(workdir),
        LocalOp::UnstagePaths(paths) => repo::unstage_paths(workdir, &paths),
        LocalOp::DiscardFile { path, untracked } => repo::discard_file(workdir, &path, untracked),
        LocalOp::Hunk { op, path, index } => repo::apply_hunk(workdir, &path, index, op),
        LocalOp::Lines { op, path, hunk, expected, lines } => repo::apply_lines(workdir, &path, hunk, &expected, &lines, op),
        LocalOp::ResolveConflict { path, contents, expected } => repo::write_resolution(workdir, &path, &contents, &expected),
        LocalOp::ResolveWithSide { path, side, expected } => repo::resolve_with_side(workdir, &path, side, &expected),
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
        Ok(Plan::Run(cmds)) => {
            let (ok, output) = run_git(cwd, &cmds, None);
            if !ok {
                return failed(output);
            }
            let after = if redo { undo::redone(cwd, action.clone()).unwrap_or_else(|_| action.clone()) } else { action.clone() };
            Msg::JobDone { ok, output, undo: Some(UndoResult::Done { redo, before: action, after }) }
        }
    }
}

fn run_git(cwd: &Path, cmds: &[Vec<String>], write_file: Option<(std::path::PathBuf, String)>) -> (bool, String) {
    if let Some((path, contents)) = write_file
        && let Err(e) = std::fs::write(&path, contents)
    {
        return (false, format!("Could not write {}: {e}", path.display()));
    }
    let mut output = String::new();
    for c in cmds {
        output.push_str(&format!("$ git {}\n", c.join(" ")));
        let (success, text) = gitcli::run(cwd, c);
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

fn forge_call(target: trellis_core::forge::Target, cwd: &Path, auth: Option<Auth>, interactive: bool, call: ForgeCall) -> ForgeReply {
    let auth = match auth {
        Some(a) => a,
        None => match forge::resolve_auth(&target.account(), cwd, interactive) {
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

fn read_git_config(cwd: &Path, has_repo: bool) -> GitConfigValues {
    let read = |global: bool, key: &str| {
        if !global && !has_repo {
            return String::new();
        }
        let scope = if global { "--global" } else { "--local" };
        let (ok, out) = gitcli::run(cwd, &args(&["config", scope, "--get", key]));
        if ok { out.trim().to_owned() } else { String::new() }
    };
    let format = read(true, "gpg.format");
    GitConfigValues {
        longpaths: read(true, "core.longpaths") == "true",
        autocrlf: read(true, "core.autocrlf") == "true",
        user_name: read(true, "user.name"),
        user_email: read(true, "user.email"),
        gpgsign: read(true, "commit.gpgsign") == "true",
        signing_format: if format.is_empty() { "openpgp".into() } else { format },
        signing_key: read(true, "user.signingkey"),
        repo_name: read(false, "user.name"),
        repo_email: read(false, "user.email"),
        hooks_path: read(false, "core.hooksPath"),
        gpg_program: read(true, "gpg.program"),
        ssh_program: read(true, "gpg.ssh.program"),
        x509_program: read(true, "gpg.x509.program"),
        tag_gpgsign: read(true, "tag.gpgsign") == "true",
        ssh_allowed_signers: read(true, "gpg.ssh.allowedSignersFile"),
    }
}
