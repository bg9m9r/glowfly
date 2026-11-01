//! Headless tests for `update`: behaviour is checked through state changes and returned
//! effects, and end-to-end against real repositories by executing effects synchronously
//! (`drive`, `finish`); the fixtures are `crate::testing`'s.

use super::update;
use crate::effect::{Effect, ForgeReply, ForgeResult, LoadKind, LocalOp, UiEffect};
use crate::effect::{OsEffect, PrefsEffect};
use crate::msg::CommitMsg;
use crate::msg::ForgeMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::patch::PatchEffect;
use crate::runner;
use crate::ssh::SshEffect;
use crate::state::{AppState, ForgeState, Modal, Selection, Status};
use crate::testing::{git, scratch_repo, state};
use glowfly_core::forge::{CheckState, Checks, Provider, PullRequest, Target};
use glowfly_core::gitcli::args;
use std::time::{Duration, Instant};

/// Answers the open auto-stash prompt with its primary ("Stash and ...") and runs what follows:
/// the operation, then putting the changes back.
#[track_caller]
pub(crate) fn accept_stash(st: &mut AppState) {
    assert!(matches!(st.modal, Some(crate::state::Modal::AutoStash(_))), "the auto-stash prompt is open");
    drive(st, Msg::AutoStash(crate::msg::AutoStashMsg::Stash));
}

/// Runs `msg` and every resulting effect to completion, as the app would (minus the window).
pub(crate) fn drive(st: &mut AppState, msg: Msg) {
    let fx = update(st, msg, Instant::now());
    finish(st, fx);
}

/// Runs effects `update` returned, and everything they lead to, to completion (as `drive`).
pub(crate) fn finish(st: &mut AppState, fx: Vec<Effect>) {
    finish_keeping(st, fx);
}

/// `finish`, returning every effect it skipped (raised anywhere along the way), in order.
pub(crate) fn finish_keeping(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let mut queue = Vec::new();
    let mut kept = Vec::new();
    let mut fx = fx;
    loop {
        for effect in fx {
            if skipped(&effect) {
                kept.push(effect);
            } else {
                queue.extend(runner::testing::execute(effect));
            }
        }
        let Some(m) = queue.pop() else { return kept };
        fx = update(st, m, Instant::now());
    }
}

/// Effects `finish` doesn't run: they need a user, the network, a shell or the window.
pub(crate) fn skipped(effect: &Effect) -> bool {
    matches!(
        effect,
        // These need a user or the network; tests don't exercise them here.
        Effect::Ui(_)
            | Effect::Os(OsEffect::PickFolder { .. } | OsEffect::OpenUrl(_) | OsEffect::OpenTerminal(_) | OsEffect::OpenFolder(_))
            | Effect::Forge { .. } | Effect::Account(_) | Effect::SaveSettings(_) | Effect::SaveRecent(_)
            | Effect::SaveTabs { .. } | Effect::Watch(_) | Effect::SweepWatch { .. } | Effect::AnswerPrompt { .. }
            // Shells are never started by these tests.
            | Effect::Terminal(_)
            | Effect::Os(OsEffect::OpenPath { .. } | OsEffect::RevealPath(_))
            // OS dialogs would wait for a user.
            | Effect::Patch(PatchEffect::PickSave { .. } | PatchEffect::PickOpen { .. })
            // Never start programs or open dialogs from tests.
            | Effect::Tool(_)
            // Would write the developer's global git config.
            | Effect::Profile(_)
            | Effect::Onboarding(_)
            // Asks a hosting service.
            | Effect::Clone(crate::effect::CloneEffect::ListRepos { .. })
            // Would write the global git config, run ssh-keygen or ssh-add, call a service, or open a
            // file dialog.
            | Effect::Ssh(
                SshEffect::SetKeyChoice { repo: None, .. }
                    | SshEffect::SetWindowsSsh { .. }
                    | SshEffect::Agent { .. }
                    | SshEffect::AddToAgent { .. }
                    | SshEffect::AddToService { .. }
                    | SshEffect::Generate { .. }
                    | SshEffect::PickKey { .. }
            )
            // Never show real desktop notifications.
            | Effect::Notify(_)
            | Effect::PullRequest(crate::pull_request::PrEffect::Call(_))
            | Effect::Issue(_)
            // Would read and write the developer's real config folder.
            | Effect::Theme(_)
            // Would read the developer's own commands file, or run a shell.
            | Effect::CustomCommand(_)
            // The log worker owns the log files (never attached in tests).
            | Effect::Logs(_)
            // Would read or change which program opens links for the developer's account.
            | Effect::DeepLink(
                crate::deep_link::DeepLinkEffect::SchemeStatus
                    | crate::deep_link::DeepLinkEffect::Register
                    | crate::deep_link::DeepLinkEffect::Unregister
            )
            // Asks hosting services.
            | Effect::Dashboard(crate::effect::DashboardEffect::Fetch(_))
            // Index builds run only where a test asks (`history_index_tests`), so the load-more
            // fallback tests keep today's flow.
            | Effect::Index(crate::effect::IndexEffect::Build { .. })
    ) || matches!(effect, Effect::Partial(crate::effect::partial::PartialEffect::SizeHistory(r)) if r.service.is_some())
}

#[test]
fn opening_ignores_stale_results() {
    let mut st = state();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open("C:/a".into())), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadRepo { generation: 1, kind: LoadKind::Open, .. }]));
    assert!(st.loading);
    // A newer load supersedes the first; the first's result is ignored.
    update(&mut st, Msg::Repo(RepoMsg::Open("C:/b".into())), Instant::now());
    let fx =
        update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation: 1, kind: LoadKind::Open, result: Err("boom".into()) }), Instant::now());
    assert!(fx.is_empty());
    assert!(st.log.is_empty(), "stale errors are not reported");
    assert!(st.loading);
}

#[test]
fn only_one_git_job_runs_at_a_time() {
    let mut st = state();
    st.snap = None;
    // Without a repository there is no working directory to run in.
    assert!(update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now()).is_empty());

    let dir = scratch_repo("jobs");
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let fx = update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]));
    assert!(update(&mut st, Msg::git("Pull", &["pull"]), Instant::now()).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("still running")));

    // Completion clears the job, toasts, and refreshes.
    let fx = update(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: "$ git fetch".into(), undo: None }), Instant::now());
    assert!(st.job.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Fetch succeeded"));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn auto_fetch_runs_quietly_on_its_interval() {
    let dir = scratch_repo("autofetch");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.settings.auto_fetch_minutes = 1;
    let start = st.last_fetch;

    assert!(update(&mut st, Msg::Tick(start + Duration::from_secs(30)), Instant::now()).is_empty(), "too early");
    let fx = update(&mut st, Msg::Tick(start + Duration::from_secs(61)), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, prompts, .. }] => {
            assert_eq!(cmds[0], args(&["fetch", "--all", "--prune"]));
            assert!(!prompts, "background jobs never open a credential prompt");
        }
        other => panic!("expected a fetch, got {other:?}"),
    }
    // A failed background fetch is logged but doesn't pop the log open or toast.
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output: "offline".into(), undo: None }), Instant::now());
    assert!(!st.view.show_log);
    assert!(st.toasts.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn confirmation_runs_the_wrapped_message() {
    let mut st = state();
    update(&mut st, Msg::confirm("Delete", "Sure?".into(), "Delete", Msg::Copy("x".into())), Instant::now());
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })));
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert!(st.modal.is_none());
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::CopyText(t))] if t == "x"));
    // Cancelling runs nothing.
    update(&mut st, Msg::confirm("Delete", "Sure?".into(), "Delete", Msg::Copy("y".into())), Instant::now());
    assert!(update(&mut st, Msg::CloseModal, Instant::now()).is_empty());
    assert!(update(&mut st, Msg::ConfirmAccepted, Instant::now()).is_empty());
}

#[test]
fn settings_are_saved_only_when_changed() {
    let mut st = state();
    assert!(update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now()).is_empty());
    st.settings.toolbar_labels = !st.settings.toolbar_labels;
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]));
    assert!(update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now()).is_empty(), "saved once");
}

#[test]
fn forge_replies_update_pull_requests_and_errors() {
    let mut st = state();
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    st.session.forge = Some(ForgeState::new(Some(target.clone()), "origin".into(), Some(target.clone())));
    let checks =
        Checks { state: CheckState::Pending, passed: 1, failed: 0, pending: 1, complete: true, url: "https://ci.example.com/1".into() };
    let pr = PullRequest {
        id: 7,
        title: "Add search".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        checks: Some(checks.clone()),
        url: "u".into(),
        facets: Default::default(),
    };
    let reply = ForgeReply { target: target.clone(), auth: None, results: vec![ForgeResult::Prs(Ok(vec![pr]))] };
    update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!(fs.prs.len(), 1);
    assert_eq!(fs.prs[0].checks, Some(checks));
    assert_eq!(fs.host_status, Status::Ok);
    assert_eq!(fs.pr_label(7), "#7");

    let reply = ForgeReply { target: target.clone(), auth: None, results: vec![ForgeResult::AuthFailed("not signed in".into())] };
    update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!(fs.host_status, Status::Error("not signed in".into()));

    // Replies for a different repository are ignored.
    let other = Target { repo: "elsewhere".into(), ..target };
    update(
        &mut st,
        Msg::Forge(ForgeMsg::ForgeReplied(ForgeReply { target: other, auth: None, results: vec![ForgeResult::Prs(Ok(Vec::new()))] })),
        Instant::now(),
    );
    assert_eq!(st.session.forge.as_ref().unwrap().prs.len(), 1);
}

#[test]
fn stage_and_commit_end_to_end() {
    let dir = scratch_repo("e2e");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let snap = st.snap.as_ref().expect("opened");
    assert_eq!(st.session.selection, Selection::Wip, "a dirty repo opens on the WIP");
    assert_eq!(snap.unstaged.len(), 1);

    let fx = update(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())), Instant::now());
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::StageFile(p), .. }] if p == "a.txt"));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    assert_eq!(st.snap.as_ref().unwrap().staged.len(), 1);

    st.session.draft.summary = "Second line".into();
    st.session.draft.description = "Adds a line.".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.commits[0].summary, "Second line");
    assert_eq!(snap.commits[0].body, "Adds a line.");
    assert!(!snap.is_dirty());
    assert!(st.session.draft.summary.is_empty(), "the draft clears after a successful commit");
    assert!(st.job.is_none());

    // Amend pre-fills from HEAD and rewrites it.
    st.session.draft.amend = true;
    drive(&mut st, Msg::Commit(CommitMsg::PrefillAmend));
    assert_eq!(st.session.draft.summary, "Second line");
    st.session.draft.summary = "Second line, amended".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.commits.len(), 2);
    assert_eq!(snap.commits[0].summary, "Second line, amended");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn push_sets_upstream_for_new_branches() {
    let dir = scratch_repo("push");
    let mut st = state();
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: false }), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, prompts, .. }] => {
            assert_eq!(cmds[0], args(&["push", "-u", "--", "origin", "main"]));
            assert!(prompts, "user-started jobs may ask for credentials");
        }
        other => panic!("expected a push, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_signing_flow() {
    use crate::state::PrefPage;
    use crate::state::prefs::GitConfigValues;
    use glowfly_core::signing::{SigningFormat, SigningKey};

    let mut st = state();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Signing)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Prefs(PrefsEffect::ReadGitConfig { .. }))));

    // Config arrives → the key list for the configured format is requested.
    let values = GitConfigValues {
        user_name: "Ada".into(),
        user_email: "ada@example.com".into(),
        signing_format: "openpgp".into(),
        ..Default::default()
    };
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values)), Instant::now());
    match &fx[..] {
        [Effect::Prefs(PrefsEffect::ListSigningKeys { format, program })] => {
            assert_eq!((*format, program.as_str()), (SigningFormat::OpenPgp, "gpg"))
        }
        other => panic!("expected a key listing, got {other:?}"),
    }
    let key = SigningKey { id: "ABCD".into(), label: "Ada <ada@example.com>".into(), expires: None, expired: false, revoked: false };
    update(&mut st, Msg::Prefs(PrefsMsg::SigningKeysLoaded(Ok(vec![key]))), Instant::now());
    assert_eq!(st.prefs.as_ref().unwrap().signing.keys.as_ref().map(Vec::len), Some(1));

    // Generating uses the profile identity and the typed passphrase (then forgets it).
    st.prefs.as_mut().unwrap().signing.passphrase = "s3cret".into();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::GenerateGpgKey), Instant::now());
    match &fx[..] {
        [Effect::Prefs(PrefsEffect::GenerateGpgKey { name, email, passphrase, .. })] => {
            assert_eq!((name.as_str(), email.as_str(), passphrase.as_str()), ("Ada", "ada@example.com", "s3cret"))
        }
        other => panic!("expected key generation, got {other:?}"),
    }
    assert!(st.prefs.as_ref().unwrap().signing.passphrase.is_empty());
    assert!(st.prefs.as_ref().unwrap().signing.generating);

    // The new key becomes the signing key.
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::GpgKeyGenerated(Ok("NEWKEY".into()))), Instant::now());
    assert!(fx.iter().any(
        |e| matches!(e, Effect::Prefs(PrefsEffect::SetGitConfig { key, value: Some(v), .. }) if key == "user.signingkey" && v == "NEWKEY")
    ));
    assert_eq!(st.prefs.as_ref().unwrap().git.as_ref().unwrap().signing_key, "NEWKEY");
    assert!(!st.prefs.as_ref().unwrap().signing.generating);
}

#[test]
fn job_output_hides_credentials_in_urls() {
    let dir = scratch_repo("redact");
    let cmds = vec![args(&["remote", "add", "--", "x", "https://ada:TOKEN123@example.com/o/r.git"])];
    let effect = Effect::RunGit {
        cwd: dir.clone(),
        cmds,
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable: false,
        on_fail_check: None,
    };
    let Some(Msg::Job(JobMsg::JobDone { ok, output, .. })) = runner::testing::execute(effect) else { panic!("expected JobDone") };
    assert!(ok, "{output}");
    assert!(!output.contains("TOKEN123") && output.contains("https://***@example.com/o/r.git"), "{output}");
    let _ = std::fs::remove_dir_all(&dir);
}
