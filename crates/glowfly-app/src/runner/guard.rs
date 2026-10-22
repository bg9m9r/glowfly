//! Keeps a panicking effect from leaving the UI waiting forever: the panic is caught and turned
//! into the failure result the effect's sender is waiting for (a finished job clears `st.job`).

use crate::effect::partial::PartialEffect;
use crate::effect::{
    CloneEffect, Effect, FailedLineCounts, ForgeError, InitEffect, LoadKind, RebaseEffect, ReposEffect, StashEffect, SubmoduleEffect,
    UndoResult, UpdateEffect,
};
use crate::msg::BlameMsg;
use crate::msg::CloneMsg;
use crate::msg::JobMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::UpdateMsg;
use crate::msg::{ContentFailure, PartialMsg};
use crate::remote::{RemoteEffect, RemoteMsg};
use crate::ssh::SshFailure;
use crate::tools::ToolEffect;
use glowfly_core::forge::{Account, Target};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

/// What to report if an effect panics, worked out before it runs.
pub(super) struct Recovery {
    name: &'static str,
    scope: Option<PathBuf>,
    kind: Kind,
}

enum Kind {
    /// A job that holds `st.job`: it must finish, failed, with this undo outcome.
    Job(Option<UndoResult>),
    Rebase,
    /// Editing a running rebase's remaining steps: the panel waits for it.
    EditTodo,
    LocalOp,
    /// A repository load or status read: the UI waits for it before starting another.
    Load(u64, LoadKind),
    Status(u64),
    /// Preferences > SSH work (reading ~/.ssh or the agent, generating or adding a key, writing
    /// core.sshCommand): the page shows a spinner until its result arrives.
    Ssh(SshFailure),
    /// A fork dialog's request: the dialog shows a spinner until its answer arrives.
    Fork(FailedFork),
    /// The Clone dialog's list of the user's repositories on this account: a spinner until it ends.
    Repos(Account),
    /// The blame view shows "Loading…" until its result arrives.
    Blame {
        path: String,
        generation: u64,
    },
    /// An update check (`Some(manual)`) or download (None): the status shows a spinner until
    /// its result arrives.
    Update(Option<bool>),
    /// A scan of the project folders: the Find repositories dialog shows a spinner until it ends.
    RepoScan(u64, Vec<PathBuf>),
    /// One repository of a fetch or pull across repositories (generation, row): it fails, so
    /// the next one starts.
    RepoSync(u64, usize),
    /// A commit's or the working tree's line counts: pending until a result arrives.
    LineCounts(FailedLineCounts),
    /// Putting an auto-stash back (workdir, stash): it ends the active job.
    PutBack(PathBuf, git2::Oid),
    /// Fetch full history's size check (workdir, id): the status bar shows it until it ends.
    Sizing(PathBuf, u64),
    /// A background download of missing contents: its pane shows it until it ends.
    Content(u64),
    Log,
}

enum FailedFork {
    List(PathBuf, Target),
    Find(PathBuf, Target),
    Create(PathBuf, Target, String),
}

impl FailedFork {
    fn of(e: &RemoteEffect) -> Self {
        match e {
            RemoteEffect::ListForks(r) => FailedFork::List(r.workdir.clone(), r.target.clone()),
            RemoteEffect::FindMyFork(r) => FailedFork::Find(r.workdir.clone(), r.target.clone()),
            RemoteEffect::CreateFork { req, name } => FailedFork::Create(req.workdir.clone(), req.target.clone(), name.clone()),
        }
    }

    fn message(self, text: String) -> Msg {
        let err = ForgeError { message: text, auth: false };
        Msg::Remote(match self {
            FailedFork::List(workdir, target) => RemoteMsg::ForksLoaded { workdir, target, result: Box::new(Err(err)) },
            FailedFork::Find(workdir, target) => RemoteMsg::MyForkFound { workdir, target, result: Box::new(Err(err)) },
            FailedFork::Create(workdir, target, name) => RemoteMsg::ForkCreated { workdir, target, name, result: Box::new(Err(err)) },
        })
    }
}

impl Recovery {
    pub(super) fn of(effect: &Effect, name: &'static str) -> Self {
        let kind = match effect {
            Effect::RunGit { record, .. } => Kind::Job(record.as_ref().map(|_| UndoResult::Recorded(None))),
            Effect::Pick(j) => Kind::Job(j.record.as_ref().map(|_| UndoResult::Recorded(None))),
            Effect::UndoRedo { redo, .. } => Kind::Job(Some(UndoResult::Failed { redo: *redo })),
            Effect::RemoveWorktree(_)
            | Effect::MoveWorktree(_)
            | Effect::Submodule(SubmoduleEffect::Remove(_))
            | Effect::Stash(StashEffect::Rename { .. })
            | Effect::SparseCheckout { .. }
            | Effect::TurnOnMonitor { .. }
            | Effect::Stack(_)
            | Effect::Clone(CloneEffect::Sparse { .. })
            | Effect::Init(InitEffect::Run { .. })
            | Effect::Tool(ToolEffect::RunMerge(_) | ToolEffect::RunGitMerge(_)) => Kind::Job(None),
            Effect::Clone(CloneEffect::ListRepos { account, .. }) => Kind::Repos(account.clone()),
            Effect::Rebase(RebaseEffect::Start { .. } | RebaseEffect::Resume { .. }) => Kind::Rebase,
            Effect::Rebase(RebaseEffect::EditTodo { .. }) => Kind::EditTodo,
            Effect::LocalOp { .. } => Kind::LocalOp,
            Effect::LoadRepo { generation, kind, .. } => Kind::Load(*generation, *kind),
            Effect::LoadStatus { generation, .. } | Effect::LoadIndexStatus { generation, .. } => Kind::Status(*generation),
            Effect::Ssh(e) => Kind::Ssh(e.failure()),
            Effect::Remote(e) => Kind::Fork(FailedFork::of(e)),
            Effect::LoadBlame(r) => Kind::Blame { path: r.path.clone(), generation: r.generation },
            Effect::Updates(UpdateEffect::Check { manual }) => Kind::Update(Some(*manual)),
            Effect::Updates(UpdateEffect::Download(_)) => Kind::Update(None),
            Effect::Repos(ReposEffect::Scan { generation, roots }) => Kind::RepoScan(*generation, roots.clone()),
            Effect::Repos(ReposEffect::Sync { generation, index, .. }) => Kind::RepoSync(*generation, *index),
            Effect::LineCounts(e) => Kind::LineCounts(e.failure()),
            Effect::Stash(StashEffect::PutBack(r)) => Kind::PutBack(r.cwd.clone(), r.oid),
            Effect::Partial(PartialEffect::SizeHistory(r)) => Kind::Sizing(r.workdir.clone(), r.id),
            Effect::Partial(PartialEffect::FetchContent(r)) => Kind::Content(r.id),
            _ => Kind::Log,
        };
        Self { name, scope: effect.scope().cloned(), kind }
    }

    fn message(self, panic: String) -> Msg {
        let text = format!("Glowfly hit an internal error ({}): {panic}", self.name);
        let msg = match self.kind {
            Kind::Job(undo) => Msg::Job(JobMsg::JobDone { ok: false, output: text, undo }),
            Kind::Rebase => Msg::Rebase(RebaseMsg::RebaseFinished { result: Err(text), undo: None }),
            Kind::EditTodo => Msg::Rebase(RebaseMsg::RemainingEdited { result: Err(text) }),
            Kind::LocalOp => Msg::Stage(StageMsg::LocalOpDone(Err(text))),
            Kind::Load(generation, kind) => Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result: Err(text) }),
            Kind::Status(generation) => {
                let result = Err(text);
                Msg::Load(LoadMsg::StatusLoaded {
                    generation,
                    result,
                    submodules: None,
                    open_stamp: None,
                    slow: None,
                    lfs_patterns: None,
                    index: None,
                })
            }
            Kind::Ssh(f) => Msg::Ssh(f.message(text)),
            Kind::Fork(f) => f.message(text),
            Kind::Repos(account) => {
                let result = Box::new(Err(ForgeError { message: text, auth: false }));
                Msg::Clone(CloneMsg::ReposLoaded { account, result })
            }
            Kind::Blame { path, generation } => Msg::Blame(BlameMsg::Loaded { path, generation, result: Err(text) }),
            Kind::Update(Some(manual)) => Msg::Updates(UpdateMsg::Checked { manual, result: Err(text) }),
            Kind::Update(None) => Msg::Updates(UpdateMsg::Downloaded(Err(text))),
            Kind::RepoScan(generation, roots) => {
                let unreadable = roots.into_iter().map(|r| (r, text.clone())).collect();
                let report = Box::new(crate::msg::ScanFound { unreadable, ..Default::default() });
                Msg::Repos(crate::msg::ReposMsg::Scanned { generation, report })
            }
            Kind::RepoSync(generation, index) => {
                let outcome = glowfly_core::repo_sync::Outcome::Failed { reason: text, detail: String::new() };
                Msg::Repos(crate::msg::ReposMsg::Synced { generation, index, outcome })
            }
            Kind::LineCounts(FailedLineCounts::Commit(oid)) => Msg::Repo(RepoMsg::CommitLineCountsLoaded { oid, result: Err(text) }),
            Kind::LineCounts(FailedLineCounts::Wip(generation)) => {
                Msg::Repo(RepoMsg::WipLineCountsLoaded { generation, result: Err(text) })
            }
            Kind::PutBack(workdir, oid) => {
                let outcome = glowfly_core::stash_ops::PutBack::Failed;
                Msg::AutoStash(crate::msg::AutoStashMsg::PutBack { workdir, oid, outcome, output: text, undo: None })
            }
            Kind::Sizing(workdir, id) => {
                tracing::debug!("{text}");
                Msg::Partial(PartialMsg::HistorySized { workdir, id, size: glowfly_core::history_size::HistorySize::Unknown })
            }
            Kind::Content(id) => Msg::Partial(PartialMsg::ContentFetched { id, result: Err(ContentFailure::Failed(text)) }),
            Kind::Log => Msg::Logged { ok: false, text },
        };
        match self.scope {
            Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
            None => msg,
        }
    }
}

/// Runs `f`, turning a panic into `recovery`'s failure message (and logging it).
pub(super) fn guarded(recovery: Recovery, f: impl FnOnce() -> Option<Msg>) -> Option<Msg> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(msg) => msg,
        Err(payload) => {
            let panic = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            tracing::error!(effect = recovery.name, "effect panicked: {panic}");
            Some(recovery.message(panic))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::SshMsg;
    use crate::state::{AppState, RunningJob};
    use crate::update::update;
    use glowfly_core::settings::Settings;
    use glowfly_core::undo::UndoKind;
    use std::time::Instant;

    #[test]
    fn a_panicking_job_finishes_failed_and_frees_the_job_slot() {
        let effect = Effect::RunGit {
            cwd: "C:/r".into(),
            cmds: vec![],
            write_file: None,
            record: Some(UndoKind::Checkout),
            journal: None,
            prompts: false,
            expect: None,
            stoppable: false,
        };
        let msg = guarded(Recovery::of(&effect, "git"), || panic!("boom {}", 42)).expect("a failure result");
        let Msg::Job(JobMsg::JobDone { ok: false, output, undo: Some(UndoResult::Recorded(None)) }) = &msg else {
            panic!("not a failed job")
        };
        assert!(output.contains("boom 42") && output.contains("git"), "{output}");

        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        st.job = Some(RunningJob {
            label: "Pull".into(),
            workdir: None,
            quiet: false,
            clear_commit: false,
            then_open: None,
            then_push: false,
            then_pr: None,
            then_link: false,
            keeps_undo: false,
            fetches: false,
        });
        update(&mut st, msg, Instant::now());
        assert!(st.job.is_none(), "the UI can start another operation");
    }

    #[test]
    fn a_panicking_fork_request_ends_the_dialogs_spinner() {
        use crate::remote::ForkRequest;
        let target = glowfly_core::forge::parse_remote("https://github.com/team/app", &[]).unwrap();
        let req = ForkRequest { workdir: "C:/r".into(), target: target.clone(), auth: None, connected: false };
        let effect = Effect::Remote(RemoteEffect::ListForks(req));
        assert!(effect.scope().is_none(), "unscoped: the dialog gets its answer whichever tab is active");
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        let Msg::Remote(RemoteMsg::ForksLoaded { workdir, target: t, result }) = msg else { panic!("not a fork list result") };
        let Err(e) = *result else { panic!("not a failure") };
        assert_eq!((workdir, t), (PathBuf::from("C:/r"), target));
        assert!(e.message.contains("boom") && e.message.contains("list-forks") && !e.auth, "{e:?}");
    }

    #[test]
    fn a_panicking_stash_rename_ends_its_job() {
        let effect =
            Effect::Stash(StashEffect::Rename { cwd: "C:/r".into(), index: 1, oid: git2::Oid::zero(), message: "On main: x".into() });
        assert!(effect.scope().is_none(), "unscoped: the job ends whichever tab is active");
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
        let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
        assert!(output.contains("boom") && output.contains("rename-stash"), "{output}");
    }

    #[test]
    fn a_panicking_merge_tool_ends_its_job() {
        use crate::tools::{GitMergeRequest, MergeToolRequest};
        use glowfly_core::external_tools::git_config::{GitTool, GitToolRun};
        let req =
            MergeToolRequest { program: "m".into(), template: String::new(), workdir: "C:/r".into(), path: "a.txt".into(), filters: false };
        let tool = GitTool { name: "m".into(), chosen_by: "merge.tool".into(), run: GitToolRun::BuiltIn { path: None } };
        let git = GitMergeRequest { tool, workdir: "C:/r".into(), path: "a.txt".into(), filters: false };
        for effect in [Effect::Tool(ToolEffect::RunMerge(req)), Effect::Tool(ToolEffect::RunGitMerge(git))] {
            let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
            let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
            assert!(output.contains("boom") && output.contains(effect.name()), "{output}");
        }
    }

    #[test]
    fn a_panicking_sparse_or_monitor_job_ends_its_job() {
        let sparse = Effect::SparseCheckout { cwd: "C:/r".into(), command: glowfly_core::sparse_ops::SparseCommand::Disable };
        let monitor = Effect::TurnOnMonitor { cwd: "C:/r".into(), then: vec![] };
        for effect in [sparse, monitor] {
            assert!(effect.scope().is_none(), "unscoped: the job ends whichever tab is active");
            let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
            let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
            assert!(output.contains("boom") && output.contains(effect.name()), "{output}");
        }
    }

    #[test]
    fn panicking_clone_and_init_effects_end_their_job_or_spinner() {
        use glowfly_core::forge::Provider;
        let jobs = [
            Effect::Clone(CloneEffect::Sparse { parent: "C:/p".into(), cmd: vec![], dest: "C:/p/r".into(), folders: vec![] }),
            Effect::Init(InitEffect::Run {
                folder: "C:/n".into(),
                plan: glowfly_core::init_ops::InitPlan { branch: None, files: vec![], commit: None },
            }),
        ];
        for effect in jobs {
            assert!(effect.scope().is_none(), "unscoped: the job ends whichever tab is active");
            let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
            let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
            assert!(output.contains("boom") && output.contains(effect.name()), "{output}");
        }
        let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
        let effect = Effect::Clone(CloneEffect::ListRepos { account: account.clone(), cwd: "C:/r".into() });
        assert!(effect.scope().is_none());
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        let Msg::Clone(CloneMsg::ReposLoaded { account: a, result }) = msg else { panic!("not a repository list result") };
        let Err(e) = *result else { panic!("not a failure") };
        assert_eq!(a, account);
        assert!(e.message.contains("boom") && e.message.contains("list-my-repos") && !e.auth, "{e:?}");
    }

    #[test]
    fn a_panicking_blame_ends_the_views_loading() {
        let r = crate::effect::BlameRequest {
            workdir: "C:/r".into(),
            path: "a.txt".into(),
            at: None,
            opts: Default::default(),
            highlight: true,
            generation: 4,
            follow: None,
        };
        let effect = Effect::LoadBlame(r);
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(
            matches!(*msg, Msg::Blame(BlameMsg::Loaded { generation: 4, ref path, result: Err(ref e) }) if path == "a.txt" && e.contains("boom"))
        );
    }

    #[test]
    fn a_panicking_line_count_reports_a_failed_result() {
        use crate::effect::LineCountsEffect;
        let effect = Effect::LineCounts(LineCountsEffect::Wip { workdir: "r".into(), generation: 3 });
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Repo(RepoMsg::WipLineCountsLoaded { generation: 3, result: Err(ref e) }) if e.contains("boom")));

        let oid = git2::Oid::from_bytes(&[7; 20]).unwrap();
        let effect = Effect::LineCounts(LineCountsEffect::Commit { workdir: "r".into(), oid });
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Repo(RepoMsg::CommitLineCountsLoaded { oid: o, result: Err(_) }) if o == oid));
    }

    #[test]
    fn a_panicking_local_op_reports_to_its_repository() {
        let effect = Effect::LocalOp { workdir: "C:/r".into(), op: crate::effect::LocalOp::StageAll };
        let msg = guarded(Recovery::of(&effect, "local-op"), || panic!("static")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Stage(StageMsg::LocalOpDone(Err(ref e))) if e.contains("static")));
        assert!(guarded(Recovery::of(&effect, "local-op"), || None).is_none(), "no panic, no message");
    }

    #[test]
    fn a_panicking_load_ends_the_load() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        st.load_gen = 3;
        st.loading = true;
        let effect =
            Effect::LoadRepo { path: "C:/r".into(), opts: Default::default(), generation: 3, kind: LoadKind::Refresh, known: None };
        let msg = guarded(Recovery::of(&effect, "load-repo"), || panic!("boom")).unwrap();
        assert!(matches!(&msg, Msg::Repo(RepoMsg::RepoLoaded { generation: 3, result: Err(e), .. }) if e.contains("boom")));
        update(&mut st, msg, Instant::now());
        assert!(!st.loading, "the next refresh isn't left waiting for it");

        let effect =
            Effect::LoadStatus { workdir: "C:/r".into(), generation: 3, submodules: false, open: None, git_status: true, paths: None };
        let msg = guarded(Recovery::of(&effect, "load-status"), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Load(LoadMsg::StatusLoaded { generation: 3, result: Err(_), .. })));

        let old = std::sync::Arc::new(glowfly_core::repo::IndexMemo::of(&git2::Index::new().unwrap()).unwrap());
        let read = crate::effect::IndexRequest { old, partners: Vec::new(), paths: Vec::new() };
        let effect =
            Effect::LoadIndexStatus { workdir: "C:/r".into(), generation: 3, submodules: false, open: None, git_status: true, read };
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Load(LoadMsg::StatusLoaded { generation: 3, result: Err(_), index: None, .. })));
    }

    #[test]
    fn a_panicking_ssh_effect_ends_the_page_state() {
        use crate::ssh::{GenerateFailure, SshEffect};
        use crate::state::{PrefPage, PrefsState};
        use glowfly_core::ssh_keys::NewKey;
        let key = NewKey { path: "C:/h/.ssh/id".into(), comment: String::new(), passphrase: "s3cret".into() };
        let effect = Effect::Ssh(SshEffect::Generate { keygen: "C:/ssh-keygen.exe".into(), key, overwrite: false });
        assert!(effect.scope().is_none(), "unscoped: the page gets its answer whichever tab is active");
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        assert!(
            matches!(&msg, Msg::Ssh(SshMsg::Generated(Err(GenerateFailure::Failed(e)))) if e.contains("boom") && e.contains("ssh-keygen"))
        );
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        let mut prefs = PrefsState::new(PrefPage::Ssh);
        prefs.ssh.generating = true;
        prefs.ssh.generation = 3;
        st.prefs = Some(prefs);
        update(&mut st, msg, Instant::now());
        assert!(!st.prefs.as_ref().unwrap().ssh.generating, "Generate can run again");

        let effect = Effect::Ssh(SshEffect::List { generation: 3, extra: Vec::new() });
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        assert!(matches!(&msg, Msg::Ssh(SshMsg::Loaded { generation: 3, result: Err(_) })));
        update(&mut st, msg, Instant::now());
        assert!(st.prefs.as_ref().unwrap().ssh.error.as_deref().is_some_and(|e| e.contains("boom")), "the spinner gives way to the error");
    }

    #[test]
    fn panicking_key_choice_agent_and_upload_effects_report_failures() {
        use crate::ssh::SshEffect;
        use crate::state::ssh_service::{UploadForm, UploadState};
        use crate::state::{PrefPage, PrefsState};
        use glowfly_core::forge::{Account, Provider};
        use glowfly_core::ssh_keys::AgentReach;
        let prefs_busy = |st: &mut AppState| st.prefs.as_mut().unwrap().ssh.choice_busy = true;
        let panicked = |effect: Effect| {
            assert!(effect.scope().is_none(), "unscoped");
            guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap()
        };
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        let mut prefs = PrefsState::new(PrefPage::Ssh);
        prefs.ssh.choice_busy = true;
        prefs.ssh.agent_generation = 4;
        let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
        let mut form = UploadForm::new("C:/h/.ssh/id".into(), account.clone(), "id".into());
        form.state = UploadState::Sending;
        prefs.ssh.upload = Some(form);
        st.prefs = Some(prefs);

        let msg = panicked(Effect::Ssh(SshEffect::SetKeyChoice { repo: None, key: Some("C:/h/.ssh/id".into()) }));
        assert!(matches!(&msg, Msg::Ssh(SshMsg::KeyChoiceSet { repo: false, result: Err(e) }) if e.contains("ssh-config")));
        update(&mut st, msg, Instant::now());
        let page = &st.prefs.as_ref().unwrap().ssh;
        assert!(!page.choice_busy && page.choice_error.as_deref().is_some_and(|e| e.contains("boom")));

        prefs_busy(&mut st);
        let msg = panicked(Effect::Ssh(SshEffect::SetWindowsSsh { program: Some("C:/Windows/System32/OpenSSH/ssh.exe".into()) }));
        assert!(matches!(&msg, Msg::Ssh(SshMsg::WindowsSshSet { on: true, result: Err(e) }) if e.contains("ssh-windows")));
        update(&mut st, msg, Instant::now());
        let page = &st.prefs.as_ref().unwrap().ssh;
        assert!(!page.choice_busy && page.windows_error.as_deref().is_some_and(|e| e.contains("boom")));
        let msg = panicked(Effect::Ssh(SshEffect::SetWindowsSsh { program: None }));
        assert!(matches!(&msg, Msg::Ssh(SshMsg::WindowsSshSet { on: false, result: Err(_) })));

        let msg = panicked(Effect::Ssh(SshEffect::Agent { generation: 4 }));
        update(&mut st, msg, Instant::now());
        let agent = st.prefs.as_ref().unwrap().ssh.agent.clone().expect("the check ended");
        assert!(matches!(agent.reach, Some(AgentReach::Failed(e)) if e.contains("boom")));

        let effect =
            SshEffect::AddToService { account, public_key: "ssh-ed25519 AAAA".into(), title: "t".into(), key: "C:/h/.ssh/id".into() };
        let msg = panicked(Effect::Ssh(effect));
        update(&mut st, msg, Instant::now());
        let state = &st.prefs.as_ref().unwrap().ssh.upload.as_ref().expect("the form stays").state;
        assert!(matches!(state, UploadState::Failed { message, fallback: false } if message.contains("boom")), "{state:?}");
    }
}
