//! Repository trust: asking on open, holding auto-fetch, hosting-service calls and git's status
//! read until the user trusts the repository, and trusting repositories Trellis made.

use super::tests::{finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, KnownRepo};
use crate::msg::{JobMsg, Msg, PaletteMsg, RepoMsg, StageMsg, TabsMsg, TrustMsg};
use crate::state::palette::{Palette, PaletteCommand, entries};
use crate::state::trust::{Created, recorded};
use crate::state::{AppState, Modal, RunningJob};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::gitcli::args;
use trellis_core::settings::RepoTrust;

/// `scratch_repo` with an https remote, a core.sshCommand and an active pre-commit hook.
pub(super) fn untrusted_repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://github.com/o/r.git"]);
    git(&dir, &["config", "core.sshCommand", "ssh -v"]);
    let hook = dir.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn drive(st: &mut AppState, msg: Msg) {
    let fx = run(st, msg);
    finish(st, fx);
}

fn open(st: &mut AppState, dir: &Path) {
    drive(st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
}

fn asking(st: &AppState) -> bool {
    matches!(st.modal, Some(Modal::Trust(_)))
}

fn real(st: &AppState) -> PathBuf {
    st.snap.as_ref().unwrap().real_workdir.clone()
}

/// The effects of a tick past the auto-fetch interval.
fn tick_past_interval(st: &mut AppState) -> Vec<Effect> {
    st.settings.auto_fetch_minutes = 1;
    let at = st.last_fetch + Duration::from_secs(61);
    run(st, Msg::Tick(at))
}

fn is_forge(e: &Effect) -> bool {
    matches!(e, Effect::Forge { .. })
}

/// The `git_status` of the status read after a local operation.
fn status_read_git(st: &mut AppState) -> Option<bool> {
    let fx = run(st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))));
    let flag = fx.iter().find_map(|e| if let Effect::LoadStatus { git_status, .. } = e { Some(*git_status) } else { None });
    finish(st, fx);
    flag
}

pub(super) fn job(then_open: (PathBuf, Created)) -> RunningJob {
    RunningJob {
        label: "Clone".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_open: Some(then_open),
        keeps_undo: true,
    }
}

pub(super) fn done(st: &mut AppState, ok: bool, output: &str) {
    drive(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }));
}

#[test]
fn a_clean_repository_is_not_asked_about() {
    let dir = scratch_repo("trust-clean-open");
    git(&dir, &["remote", "add", "origin", "https://example.invalid/repo.git"]);
    let mut st = state();
    open(&mut st, &dir);
    assert!(st.modal.is_none());
    assert!(st.settings.repo_trust.is_empty(), "nothing is saved for a clean repository");
    assert!(st.runs_git_unasked());
    let fx = tick_past_interval(&mut st);
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_undecided_repository_is_asked_about_and_held() {
    let dir = untrusted_repo("trust-ask");
    let mut st = state();
    let fx = run(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { opts, .. } if !opts.trusted)), "{fx:?}");
    finish(&mut st, fx);
    let Some(Modal::Trust(p)) = &st.modal else { panic!("expected the trust prompt") };
    assert!(p.scan.config.iter().any(|c| c.key == "core.sshCommand"), "{:?}", p.scan);
    assert_eq!(p.scan.hooks, ["pre-commit"]);
    assert!(!st.runs_git_unasked());

    assert!(!tick_past_interval(&mut st).iter().any(|e| matches!(e, Effect::RunGit { .. })), "no auto-fetch");
    assert!(st.forge.as_ref().is_some_and(|f| f.host.is_some()), "GitHub is detected");
    let fx = run(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!fx.iter().any(is_forge), "no hosting-service call: {fx:?}");
    finish(&mut st, fx);

    let fx = run(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    let known = fx.iter().find_map(|e| if let Effect::LoadRepo { known: Some(k), .. } = e { Some(k.clone()) } else { None });
    assert!(matches!(known, Some(KnownRepo { git_status: false, .. })), "{known:?}");
    finish(&mut st, fx);
    assert_eq!(status_read_git(&mut st), Some(false));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trusting_releases_what_was_held() {
    let dir = untrusted_repo("trust-yes");
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st));
    let real = real(&st);
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: true }));
    assert!(st.modal.is_none());
    assert_eq!(recorded(&st.settings, &real), Some(RepoTrust::Trusted));
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { opts, known: None, .. } if opts.trusted)), "{fx:?}");
    assert!(fx.iter().any(is_forge), "hosting-service data loads: {fx:?}");
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text.contains("trusted")));
    finish(&mut st, fx);

    assert_eq!(status_read_git(&mut st), Some(true));
    let fx = tick_past_interval(&mut st);
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => assert_eq!(cmds[0][..2], args(&["fetch", "--all"])[..]),
        other => panic!("expected a fetch, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn not_trusting_is_remembered_and_keeps_holding() {
    let dir = untrusted_repo("trust-no");
    let mut st = state();
    open(&mut st, &dir);
    let real = real(&st);
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: false }));
    assert!(st.modal.is_none());
    assert_eq!(recorded(&st.settings, &real), Some(RepoTrust::NotTrusted));
    assert!(st.log.last().is_some_and(|l| l.ok && l.text.contains("isn't trusted")));
    assert!(!st.view.show_log, "an ok line doesn't open the log");
    assert!(tick_past_interval(&mut st).is_empty());

    drive(&mut st, Msg::Tabs(TabsMsg::CloseTab(usize::MAX)));
    open(&mut st, &dir);
    assert!(st.snap.is_some());
    assert!(st.modal.is_none(), "the decision stands");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deciding_later_asks_again_only_on_the_next_open() {
    let dir = untrusted_repo("trust-later");
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st));
    drive(&mut st, Msg::CloseModal);
    assert!(st.settings.repo_trust.is_empty(), "nothing is saved");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.modal.is_none(), "a full reload doesn't ask again");
    drive(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    assert!(st.modal.is_none(), "an unchanged refresh doesn't ask again");
    assert!(!st.runs_git_unasked(), "git stays held");

    drive(&mut st, Msg::Tabs(TabsMsg::CloseTab(usize::MAX)));
    open(&mut st, &dir);
    assert!(asking(&st), "a fresh open asks again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_dialog_is_not_replaced() {
    let dir = untrusted_repo("trust-other-dialog");
    let mut st = state();
    let fx = run(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::ClearLog) });
    finish(&mut st, fx);
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })));
    drive(&mut st, Msg::CloseModal);
    drive(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    assert!(asking(&st), "the next refresh asks");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_cloned_repository_is_trusted() {
    let dir = untrusted_repo("trust-cloned");
    let mut st = state();
    st.job = Some(job((dir.clone(), Created::Clone)));
    done(&mut st, true, "$ git clone https://github.com/o/r.git");
    assert!(st.snap.is_some(), "the clone opened");
    assert!(st.modal.is_none());
    assert_eq!(recorded(&st.settings, &real(&st)), Some(RepoTrust::Trusted));
    assert!(st.runs_git_unasked());

    let mut st = state();
    st.job = Some(job((dir.clone(), Created::Clone)));
    done(&mut st, false, "fatal: could not read from remote");
    assert!(st.settings.repo_trust.is_empty(), "a failed clone records nothing");
    assert!(st.trust.created.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_a_new_repository_from_init_is_trusted() {
    let dir = untrusted_repo("trust-init");
    let mut st = state();
    st.job = Some(job((dir.clone(), Created::Init)));
    done(&mut st, true, "$ git init -b main\nReinitialized existing Git repository in /x/.git/");
    assert!(asking(&st), "an existing repository is scanned as usual");
    assert!(st.settings.repo_trust.is_empty());

    let mut st = state();
    st.job = Some(job((dir.clone(), Created::Init)));
    done(&mut st, true, "$ git init -b main\nInitialized empty Git repository in /x/.git/");
    assert!(st.modal.is_none());
    assert_eq!(recorded(&st.settings, &real(&st)), Some(RepoTrust::Trusted));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The open trust prompt.
fn prompt(st: &AppState) -> &crate::state::trust::TrustPrompt {
    let Some(Modal::Trust(p)) = &st.modal else { panic!("expected the trust prompt") };
    p
}

fn ask(st: &mut AppState, real: &Path) {
    drive(st, Msg::Trust(TrustMsg::Ask { real: real.to_path_buf() }));
}

#[test]
fn esc_then_ask_brings_the_prompt_back() {
    let dir = untrusted_repo("trust-ask-again");
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st));
    let real = real(&st);
    for _ in 0..2 {
        drive(&mut st, Msg::CloseModal);
        assert!(st.modal.is_none());
        assert!(st.settings.repo_trust.is_empty(), "nothing is saved");
        ask(&mut st, &real);
        let p = prompt(&st);
        assert!(!p.refused);
        assert!(p.scan.config.iter().any(|c| c.key == "core.sshCommand"), "{:?}", p.scan);
    }
    assert_eq!(st.trust.asked.len(), 1, "asked once per repository");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ask_after_dont_trust_shows_a_refused_prompt() {
    let dir = untrusted_repo("trust-ask-refused");
    let mut st = state();
    open(&mut st, &dir);
    let real = real(&st);
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: false }));
    ask(&mut st, &real);
    assert!(prompt(&st).refused);

    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: true }));
    assert_eq!(recorded(&st.settings, &real), Some(RepoTrust::Trusted));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { opts, .. } if opts.trusted)), "{fx:?}");
    assert!(fx.iter().any(is_forge), "{fx:?}");
    finish(&mut st, fx);
    ask(&mut st, &real);
    assert!(st.modal.is_none(), "a trusted repository isn't asked about");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ask_is_ignored_for_another_repository_a_clean_one_or_over_a_dialog() {
    let dir = untrusted_repo("trust-ask-ignored");
    let mut st = state();
    open(&mut st, &dir);
    drive(&mut st, Msg::CloseModal);
    ask(&mut st, Path::new("C:/elsewhere"));
    assert!(st.modal.is_none(), "not the active repository");

    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::ClearLog) });
    let here = real(&st);
    ask(&mut st, &here);
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })), "another dialog stays");
    let _ = std::fs::remove_dir_all(&dir);

    let clean = scratch_repo("trust-ask-clean");
    let mut st = state();
    open(&mut st, &clean);
    let real = real(&st);
    ask(&mut st, &real);
    assert!(st.modal.is_none(), "a clean repository isn't asked about");
    let _ = std::fs::remove_dir_all(&clean);
}

#[test]
fn asking_closes_the_palette() {
    let dir = untrusted_repo("trust-ask-palette");
    let mut st = state();
    open(&mut st, &dir);
    drive(&mut st, Msg::CloseModal);
    st.palette = Some(Palette::default());
    let real = real(&st);
    ask(&mut st, &real);
    assert!(st.palette.is_none());
    assert!(asking(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The detail of the Review repository trust… entry, if it is offered.
fn review_entry(st: &AppState) -> Option<(usize, Option<String>)> {
    entries(st).into_iter().enumerate().find(|(_, e)| e.command == PaletteCommand::ReviewTrust).map(|(i, e)| (i, e.detail))
}

#[test]
fn review_trust_is_in_the_palette_only_while_held() {
    let dir = untrusted_repo("trust-palette");
    let mut st = state();
    open(&mut st, &dir);
    drive(&mut st, Msg::CloseModal);
    assert_eq!(review_entry(&st), Some((0, Some("undecided".into()))));

    st.palette = Some(Palette::default());
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::ReviewTrust)));
    assert!(st.palette.is_none());
    assert!(asking(&st));
    assert_eq!(st.palette_recent.position("review-trust"), Some(0));

    let real = real(&st);
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: false }));
    assert_eq!(review_entry(&st), Some((0, Some("not trusted".into()))));
    drive(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(review_entry(&st), None);
    let _ = std::fs::remove_dir_all(&dir);

    let clean = scratch_repo("trust-palette-clean");
    let mut st = state();
    open(&mut st, &clean);
    assert_eq!(review_entry(&st), None);
    let _ = std::fs::remove_dir_all(&clean);
}

#[test]
fn ask_is_not_kept_for_an_inactive_tab() {
    assert!(Msg::Trust(TrustMsg::Ask { real: PathBuf::from("C:/x") }).on_inactive().is_none());
}
