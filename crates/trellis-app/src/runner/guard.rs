//! Keeps a panicking effect from leaving the UI waiting forever: the panic is caught and turned
//! into the failure result the effect's sender is waiting for (a finished job clears `st.job`).

use crate::effect::{CloneEffect, Effect, ForgeError, InitEffect, LoadKind, SubmoduleEffect, UndoResult};
use crate::msg::BlameMsg;
use crate::msg::CloneMsg;
use crate::msg::JobMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::remote::{RemoteEffect, RemoteMsg};
use crate::ssh::SshFailure;
use crate::tools::ToolEffect;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use trellis_core::forge::{Account, Target};

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
            Effect::UndoRedo { redo, .. } => Kind::Job(Some(UndoResult::Failed { redo: *redo })),
            Effect::RemoveWorktree(_)
            | Effect::Submodule(SubmoduleEffect::Remove(_))
            | Effect::RenameStash { .. }
            | Effect::SparseCheckout { .. }
            | Effect::Clone(CloneEffect::Sparse { .. })
            | Effect::Init(InitEffect::Run { .. })
            | Effect::Tool(ToolEffect::RunMerge(_)) => Kind::Job(None),
            Effect::Clone(CloneEffect::ListRepos { account, .. }) => Kind::Repos(account.clone()),
            Effect::InteractiveRebase { .. } => Kind::Rebase,
            Effect::LocalOp { .. } => Kind::LocalOp,
            Effect::LoadRepo { generation, kind, .. } => Kind::Load(*generation, *kind),
            Effect::LoadStatus { generation, .. } => Kind::Status(*generation),
            Effect::Ssh(e) => Kind::Ssh(e.failure()),
            Effect::Remote(e) => Kind::Fork(FailedFork::of(e)),
            Effect::LoadBlame(r) => Kind::Blame { path: r.path.clone(), generation: r.generation },
            _ => Kind::Log,
        };
        Self { name, scope: effect.scope().cloned(), kind }
    }

    fn message(self, panic: String) -> Msg {
        let text = format!("Trellis hit an internal error ({}): {panic}", self.name);
        let msg = match self.kind {
            Kind::Job(undo) => Msg::Job(JobMsg::JobDone { ok: false, output: text, undo }),
            Kind::Rebase => Msg::Rebase(RebaseMsg::RebaseFinished { result: Err(text) }),
            Kind::LocalOp => Msg::Stage(StageMsg::LocalOpDone(Err(text))),
            Kind::Load(generation, kind) => Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result: Err(text) }),
            Kind::Status(generation) => {
                Msg::Load(LoadMsg::StatusLoaded { generation, result: Err(text), submodules: None, open_stamp: None })
            }
            Kind::Ssh(f) => Msg::Ssh(f.message(text)),
            Kind::Fork(f) => f.message(text),
            Kind::Repos(account) => {
                let result = Box::new(Err(ForgeError { message: text, auth: false }));
                Msg::Clone(CloneMsg::ReposLoaded { account, result })
            }
            Kind::Blame { path, generation } => Msg::Blame(BlameMsg::Loaded { path, generation, result: Err(text) }),
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
    use std::time::Instant;
    use trellis_core::settings::Settings;
    use trellis_core::undo::UndoKind;

    #[test]
    fn a_panicking_job_finishes_failed_and_frees_the_job_slot() {
        let effect = Effect::RunGit {
            cwd: "C:/r".into(),
            cmds: vec![],
            write_file: None,
            record: Some(UndoKind::Checkout),
            prompts: false,
            expect: None,
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
            keeps_undo: false,
        });
        update(&mut st, msg, Instant::now());
        assert!(st.job.is_none(), "the UI can start another operation");
    }

    #[test]
    fn a_panicking_fork_request_ends_the_dialogs_spinner() {
        use crate::remote::ForkRequest;
        let target = trellis_core::forge::parse_remote("https://github.com/team/app", &[]).unwrap();
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
        let effect = Effect::RenameStash { cwd: "C:/r".into(), index: 1, oid: git2::Oid::zero(), message: "On main: x".into() };
        assert!(effect.scope().is_none(), "unscoped: the job ends whichever tab is active");
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
        let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
        assert!(output.contains("boom") && output.contains("rename-stash"), "{output}");
    }

    #[test]
    fn a_panicking_merge_tool_ends_its_job() {
        use crate::tools::MergeToolRequest;
        let req =
            MergeToolRequest { program: "m".into(), template: String::new(), workdir: "C:/r".into(), path: "a.txt".into(), filters: false };
        let effect = Effect::Tool(ToolEffect::RunMerge(req));
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
        let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
        assert!(output.contains("boom") && output.contains("run-merge-tool"), "{output}");
    }

    #[test]
    fn a_panicking_sparse_job_ends_its_job() {
        let effect = Effect::SparseCheckout { cwd: "C:/r".into(), command: trellis_core::sparse_ops::SparseCommand::Disable };
        assert!(effect.scope().is_none(), "unscoped: the job ends whichever tab is active");
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
        let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
        assert!(output.contains("boom") && output.contains("sparse-checkout"), "{output}");
    }

    #[test]
    fn panicking_clone_and_init_effects_end_their_job_or_spinner() {
        use trellis_core::forge::Provider;
        let jobs = [
            Effect::Clone(CloneEffect::Sparse { parent: "C:/p".into(), cmd: vec![], dest: "C:/p/r".into(), folders: vec![] }),
            Effect::Init(InitEffect::Run {
                folder: "C:/n".into(),
                plan: trellis_core::init_ops::InitPlan { branch: None, files: vec![], commit: None },
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
        let r = crate::effect::BlameRequest { workdir: "C:/r".into(), path: "a.txt".into(), at: None, highlight: true, generation: 4 };
        let effect = Effect::LoadBlame(r);
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(
            matches!(*msg, Msg::Blame(BlameMsg::Loaded { generation: 4, ref path, result: Err(ref e) }) if path == "a.txt" && e.contains("boom"))
        );
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

        let effect = Effect::LoadStatus { workdir: "C:/r".into(), generation: 3, submodules: false, open: None, git_status: true };
        let msg = guarded(Recovery::of(&effect, "load-status"), || panic!("boom")).unwrap();
        let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
        assert!(matches!(*msg, Msg::Load(LoadMsg::StatusLoaded { generation: 3, result: Err(_), .. })));
    }

    #[test]
    fn a_panicking_ssh_effect_ends_the_page_state() {
        use crate::ssh::{GenerateFailure, SshEffect};
        use crate::state::{PrefPage, PrefsState};
        use trellis_core::ssh_keys::NewKey;
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

        let effect = Effect::Ssh(SshEffect::List { generation: 3 });
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
        use trellis_core::forge::{Account, Provider};
        use trellis_core::ssh_keys::AgentReach;
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
