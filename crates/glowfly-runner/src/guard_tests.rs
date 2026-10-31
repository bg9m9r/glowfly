//! Tests for panic recovery (runner/guard.rs): a panicking effect still sends the result that
//! ends its job or loading state.

use super::*;
use crate::state::{AppState, RunningJob};
use crate::update::update;
use glowfly_core::settings::Settings;
use glowfly_core::undo::UndoKind;
use glowfly_model::ssh::SshMsg;
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
        on_fail_check: None,
    };
    let msg = guarded(Recovery::of(&effect, "git"), || panic!("boom {}", 42)).expect("a failure result");
    let Msg::Job(JobMsg::JobDone { ok: false, output, undo: Some(UndoResult::Recorded(None)) }) = &msg else { panic!("not a failed job") };
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
    let effect = Effect::Stash(StashEffect::Rename { cwd: "C:/r".into(), index: 1, oid: git2::Oid::zero(), message: "On main: x".into() });
    assert!(effect.scope().is_none(), "unscoped: the job ends whichever tab is active");
    let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
    let Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }) = &msg else { panic!("not a failed job") };
    assert!(output.contains("boom") && output.contains("rename-stash"), "{output}");
}

#[test]
fn a_panicking_merge_tool_ends_its_job() {
    use glowfly_core::external_tools::git_config::{GitTool, GitToolRun};
    use glowfly_model::tools::{GitMergeRequest, MergeToolRequest};
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
fn a_panicking_sparse_monitor_or_worktree_job_ends_its_job() {
    use crate::effect::{MonitorJob, MonitorOff};
    use crate::worktree::{MoveRequest, RemoveRequest};
    let sparse = Effect::Sparse(SparseEffect::Checkout { cwd: "C:/r".into(), command: glowfly_core::sparse_ops::SparseCommand::Disable });
    let monitor = Effect::TurnOnMonitor(MonitorJob { cwd: "C:/r".into(), then: vec![] });
    let remove = Effect::Worktree(WorktreeEffect::Remove(RemoveRequest {
        repo: "C:/r".into(),
        path: "C:/wt".into(),
        name: "wt".into(),
        force: glowfly_core::worktree_ops::RemoveForce::None,
        delete_branch: None,
        delete_remote: None,
        stop_monitor: false,
    }));
    let moved = Effect::Worktree(WorktreeEffect::Move(MoveRequest {
        repo: "C:/r".into(),
        from: "C:/wt".into(),
        to: "C:/moved/wt".into(),
        name: "wt".into(),
        folder: None,
        stop_monitor: false,
    }));
    let off = Effect::TurnOffMonitor(MonitorOff { cwd: "C:/r".into() });
    for effect in [sparse, monitor, off, remove, moved] {
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
        Effect::Clone(CloneEffect::Sparse {
            parent: "C:/p".into(),
            cmd: vec![],
            dest: "C:/p/r".into(),
            folders: vec![],
            submodules: vec![],
        }),
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
        Effect::LoadRepo { path: "r".into(), opts: Default::default(), generation: 3, kind: LoadKind::More, known: None, prior: None };
    let msg = guarded(Recovery::of(&effect, "load-repo"), || panic!("boom")).unwrap();
    assert!(matches!(&msg, Msg::Repo(RepoMsg::RepoLoaded { generation: 3, result: Err(e), .. }) if e.contains("boom")));
    update(&mut st, msg, Instant::now());
    assert!(!st.loading, "the next refresh isn't left waiting for it");

    let effect = Effect::LoadStatus { workdir: "C:/r".into(), generation: 3, submodules: false, open: None, git_status: true, paths: None };
    let msg = guarded(Recovery::of(&effect, "load-status"), || panic!("boom")).unwrap();
    let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
    assert!(matches!(*msg, Msg::Load(LoadMsg::StatusLoaded { generation: 3, result: Err(_), .. })));

    let old = std::sync::Arc::new(glowfly_core::repo::IndexMemo::of(&git2::Index::new().unwrap()).unwrap());
    let read = crate::effect::IndexRequest { old, partners: Vec::new(), paths: Vec::new() };
    let effect = Effect::LoadIndexStatus { workdir: "C:/r".into(), generation: 3, submodules: false, open: None, git_status: true, read };
    let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
    let Msg::Scoped { msg, .. } = msg else { panic!("scoped to its repository") };
    assert!(matches!(*msg, Msg::Load(LoadMsg::StatusLoaded { generation: 3, result: Err(_), index: None, .. })));
}

#[test]
fn a_panicking_ssh_effect_ends_the_page_state() {
    use crate::state::{PrefPage, PrefsState};
    use glowfly_core::ssh_keys::NewKey;
    use glowfly_model::ssh::{GenerateFailure, SshEffect};
    let key = NewKey { path: "C:/h/.ssh/id".into(), comment: String::new(), passphrase: "s3cret".into() };
    let effect = Effect::Ssh(SshEffect::Generate { keygen: "C:/ssh-keygen.exe".into(), key, overwrite: false });
    assert!(effect.scope().is_none(), "unscoped: the page gets its answer whichever tab is active");
    let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).unwrap();
    assert!(matches!(&msg, Msg::Ssh(SshMsg::Generated(Err(GenerateFailure::Failed(e)))) if e.contains("boom") && e.contains("ssh-keygen")));
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
    use crate::state::ssh_service::{UploadForm, UploadState};
    use crate::state::{PrefPage, PrefsState};
    use glowfly_core::forge::{Account, Provider};
    use glowfly_core::ssh_keys::AgentReach;
    use glowfly_model::ssh::SshEffect;
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

    let effect = SshEffect::AddToService { account, public_key: "ssh-ed25519 AAAA".into(), title: "t".into(), key: "C:/h/.ssh/id".into() };
    let msg = panicked(Effect::Ssh(effect));
    update(&mut st, msg, Instant::now());
    let state = &st.prefs.as_ref().unwrap().ssh.upload.as_ref().expect("the form stays").state;
    assert!(matches!(state, UploadState::Failed { message, fallback: false } if message.contains("boom")), "{state:?}");
}

#[test]
fn a_panicking_restart_reports_a_failed_restart() {
    let job = crate::effect::RestartJob {
        file: glowfly_core::paths::temp_dir().join("glowfly.msi"),
        sha256: [0; 32],
        method: glowfly_core::app_update::InstallMethod::RunMsi,
        version: glowfly_core::app_update::Version::parse("0.3.0").unwrap(),
    };
    let effect = Effect::Updates(UpdateEffect::Restart(Box::new(job)));
    let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
    assert!(matches!(&msg, Msg::Updates(UpdateMsg::RestartReady(Err(e))) if e.contains("boom") && e.contains("update-restart")));
}
