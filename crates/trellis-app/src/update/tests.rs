//! Headless tests for `update`: behaviour is checked through state changes and returned
//! effects, and end-to-end against real repositories by executing effects synchronously.

use super::update;
use crate::effect::{Effect, ForgeReply, ForgeResult, LoadKind, LocalOp, UiEffect};
use crate::msg::Msg;
use crate::runner;
use crate::state::{AppState, ForgeState, Modal, Selection, Status};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::forge::{Provider, PullRequest, Target};
use trellis_core::gitcli::{args, run};
use trellis_core::settings::Settings;

pub(super) fn state() -> AppState {
    AppState::new(Settings::default(), Vec::new(), Instant::now())
}

pub(super) fn git(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
}

/// A repository with one commit and one modified file.
pub(super) fn scratch_repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trellis-app-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    dir
}

/// Runs `msg` and every resulting effect to completion, as the app would (minus the window).
pub(super) fn drive(st: &mut AppState, msg: Msg) {
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for effect in update(st, m, Instant::now()) {
            match effect {
                // These need a user or the network; tests don't exercise them here.
                Effect::Ui(_) | Effect::PickFolder { .. } | Effect::OpenUrl(_) | Effect::OpenTerminal(_) | Effect::OpenFolder(_) => {}
                Effect::Forge { .. } | Effect::Connect { .. } | Effect::SaveToken { .. } | Effect::DeleteToken(_) => {}
                Effect::RepoOwners { .. } | Effect::CreateRepo { .. } => {}
                Effect::ForgetCredentialManager { .. } | Effect::SaveSettings(_) | Effect::SaveRecent(_) => {}
                Effect::SaveTabs { .. } | Effect::Watch(_) | Effect::AnswerPrompt { .. } => {}
                other => queue.extend(runner::execute(other)),
            }
        }
    }
}

#[test]
fn opening_ignores_stale_results() {
    let mut st = state();
    let fx = update(&mut st, Msg::Open("C:/a".into()), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadRepo { generation: 1, kind: LoadKind::Open, .. }]));
    assert!(st.loading);
    // A newer load supersedes the first; the first's result is ignored.
    update(&mut st, Msg::Open("C:/b".into()), Instant::now());
    let fx = update(&mut st, Msg::RepoLoaded { generation: 1, kind: LoadKind::Open, result: Err("boom".into()) }, Instant::now());
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
    drive(&mut st, Msg::Open(dir.clone()));
    let fx = update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]));
    assert!(update(&mut st, Msg::git("Pull", &["pull"]), Instant::now()).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("still running")));

    // Completion clears the job, toasts, and refreshes.
    let fx = update(&mut st, Msg::JobDone { ok: true, output: "$ git fetch".into(), undo: None }, Instant::now());
    assert!(st.job.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Fetch succeeded"));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn auto_fetch_runs_quietly_on_its_interval() {
    let dir = scratch_repo("autofetch");
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    drive(&mut st, Msg::Refresh);
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
    update(&mut st, Msg::JobDone { ok: false, output: "offline".into(), undo: None }, Instant::now());
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
    assert!(update(&mut st, Msg::SettingsEdited, Instant::now()).is_empty());
    st.settings.toolbar_labels = !st.settings.toolbar_labels;
    let fx = update(&mut st, Msg::SettingsEdited, Instant::now());
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]));
    assert!(update(&mut st, Msg::SettingsEdited, Instant::now()).is_empty(), "saved once");
}

#[test]
fn forge_replies_update_pull_requests_and_errors() {
    let mut st = state();
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    st.forge = Some(ForgeState::new(Some(target.clone()), "origin".into(), Some(target.clone())));
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
        merge_status: None,
        url: "u".into(),
    };
    let reply = ForgeReply { target: target.clone(), auth: None, results: vec![ForgeResult::Prs(Ok(vec![pr]))] };
    update(&mut st, Msg::ForgeReplied(reply), Instant::now());
    let fs = st.forge.as_ref().unwrap();
    assert_eq!(fs.prs.len(), 1);
    assert_eq!(fs.host_status, Status::Ok);
    assert_eq!(fs.pr_label(7), "#7");

    let reply = ForgeReply { target: target.clone(), auth: None, results: vec![ForgeResult::AuthFailed("not signed in".into())] };
    update(&mut st, Msg::ForgeReplied(reply), Instant::now());
    let fs = st.forge.as_ref().unwrap();
    assert_eq!(fs.host_status, Status::Error("not signed in".into()));

    // Replies for a different repository are ignored.
    let other = Target { repo: "elsewhere".into(), ..target };
    update(
        &mut st,
        Msg::ForgeReplied(ForgeReply { target: other, auth: None, results: vec![ForgeResult::Prs(Ok(Vec::new()))] }),
        Instant::now(),
    );
    assert_eq!(st.forge.as_ref().unwrap().prs.len(), 1);
}

#[test]
fn stage_and_commit_end_to_end() {
    let dir = scratch_repo("e2e");
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    let snap = st.snap.as_ref().expect("opened");
    assert_eq!(st.selection, Selection::Wip, "a dirty repo opens on the WIP");
    assert_eq!(snap.unstaged.len(), 1);

    let fx = update(&mut st, Msg::StageFile("a.txt".into()), Instant::now());
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::StageFile(p), .. }] if p == "a.txt"));
    drive(&mut st, Msg::StageFile("a.txt".into()));
    assert_eq!(st.snap.as_ref().unwrap().staged.len(), 1);

    st.draft.summary = "Second line".into();
    st.draft.description = "Adds a line.".into();
    drive(&mut st, Msg::Commit);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.commits[0].summary, "Second line");
    assert_eq!(snap.commits[0].body, "Adds a line.");
    assert!(!snap.is_dirty());
    assert!(st.draft.summary.is_empty(), "the draft clears after a successful commit");
    assert!(st.job.is_none());

    // Amend pre-fills from HEAD and rewrites it.
    st.draft.amend = true;
    drive(&mut st, Msg::PrefillAmend);
    assert_eq!(st.draft.summary, "Second line");
    st.draft.summary = "Second line, amended".into();
    drive(&mut st, Msg::Commit);
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
    drive(&mut st, Msg::Open(dir.clone()));
    let fx = update(&mut st, Msg::Push { force: false }, Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, prompts, .. }] => {
            assert_eq!(cmds[0], args(&["push", "-u", "origin", "main"]));
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
    use trellis_core::signing::{SigningFormat, SigningKey};

    let mut st = state();
    let fx = update(&mut st, Msg::OpenPrefs(PrefPage::Signing), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::ReadGitConfig { .. })));

    // Config arrives → the key list for the configured format is requested.
    let values = GitConfigValues {
        user_name: "Ada".into(),
        user_email: "ada@example.com".into(),
        signing_format: "openpgp".into(),
        ..Default::default()
    };
    let fx = update(&mut st, Msg::GitConfigLoaded(values), Instant::now());
    match &fx[..] {
        [Effect::ListSigningKeys { format, program }] => assert_eq!((*format, program.as_str()), (SigningFormat::OpenPgp, "gpg")),
        other => panic!("expected a key listing, got {other:?}"),
    }
    let key = SigningKey { id: "ABCD".into(), label: "Ada <ada@example.com>".into(), expires: None, expired: false };
    update(&mut st, Msg::SigningKeysLoaded(Ok(vec![key])), Instant::now());
    assert_eq!(st.prefs.as_ref().unwrap().signing.keys.as_ref().map(Vec::len), Some(1));

    // Generating uses the profile identity and the typed passphrase (then forgets it).
    st.prefs.as_mut().unwrap().signing.passphrase = "s3cret".into();
    let fx = update(&mut st, Msg::GenerateGpgKey, Instant::now());
    match &fx[..] {
        [Effect::GenerateGpgKey { name, email, passphrase, .. }] => {
            assert_eq!((name.as_str(), email.as_str(), passphrase.as_str()), ("Ada", "ada@example.com", "s3cret"))
        }
        other => panic!("expected key generation, got {other:?}"),
    }
    assert!(st.prefs.as_ref().unwrap().signing.passphrase.is_empty());
    assert!(st.prefs.as_ref().unwrap().signing.generating);

    // The new key becomes the signing key.
    let fx = update(&mut st, Msg::GpgKeyGenerated(Ok("NEWKEY".into())), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::SetGitConfig { key, value: Some(v), .. } if key == "user.signingkey" && v == "NEWKEY")));
    assert_eq!(st.prefs.as_ref().unwrap().git.as_ref().unwrap().signing_key, "NEWKEY");
    assert!(!st.prefs.as_ref().unwrap().signing.generating);
}

#[test]
fn avatars_are_fetched_in_batches_and_handed_to_the_shell() {
    let mut st = state();
    let emails: Vec<String> = (0..10).map(|i| format!("dev{i}@example.com")).collect();
    let fx = update(&mut st, Msg::WantAvatars(emails), Instant::now());
    let sizes: Vec<usize> = fx.iter().map(|e| if let Effect::FetchAvatars(b) = e { b.len() } else { 0 }).collect();
    assert_eq!(sizes, [8, 2]);

    update(&mut st, Msg::AvatarsLoaded(vec![("dev0@example.com".into(), None)]), Instant::now());
    assert_eq!(st.view.avatars_loaded.len(), 1);
}
