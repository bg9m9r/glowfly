//! Running custom commands: git commands as toolbar jobs, shell commands as RunShell jobs, what
//! a row can't fill, confirmations, and the repository's own commands waiting for trust.

use super::custom_commands_tests::{command, file, load, opened};
use super::tests::git;
use super::update;
use crate::custom_commands::{CommandRef, CustomCommandEffect, CustomCommandMsg, ShellRun, Target};
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg, TrustMsg};
use crate::state::{AppState, Modal, RunningJob, Selection};
use std::time::Instant;
use trellis_core::custom_commands::{FileRead, Source};
use trellis_core::settings::RepoTrust;

fn run(st: &mut AppState, source: Source, name: &str, at: Target) -> Vec<Effect> {
    let msg = CustomCommandMsg::Run { command: CommandRef { source, name: name.into() }, at, confirmed: false };
    update(st, Msg::CustomCommand(msg), Instant::now())
}

fn head(st: &AppState) -> git2::Oid {
    st.snap.as_ref().unwrap().head_oid.unwrap()
}

/// The commands of the only RunGit in `fx`.
fn git_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, prompts: true, .. }] => cmds.clone(),
        other => panic!("one prompting RunGit expected: {other:?}"),
    }
}

fn last_log(st: &AppState) -> &str {
    st.log.last().map(|l| l.text.as_str()).unwrap_or_default()
}

#[test]
fn a_git_command_runs_as_a_toolbar_job() {
    let (mut st, dir) = opened("cc-run-git");
    let text = [command("Log", "git", "log --oneline {sha}", ""), command("Reset", "git", "git reset --hard {sha}", "")].concat();
    load(&mut st, file(&text), FileRead::default());
    let oid = head(&st);
    st.selection = Selection::Commit(oid);
    let fx = run(&mut st, Source::User, "Log", Target::Palette);
    assert_eq!(git_cmds(&fx), [["log", "--oneline", &oid.to_string()]]);
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Log"));
    st.job = None;
    let fx = run(&mut st, Source::User, "Reset", Target::Commit(oid));
    assert!(matches!(fx.as_slice(), [Effect::RunGit { record: Some(_), .. }]), "reset is recorded for undo: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shell_command_runs_as_a_job_and_refreshes() {
    let (mut st, dir) = opened("cc-run-shell");
    load(&mut st, file(&command("Hi", "shell", "echo {branch} {files}", "")), FileRead::default());
    let fx = run(&mut st, Source::User, "Hi", Target::File { path: "it's.txt".into(), at: None });
    let workdir = st.workdir().unwrap();
    match fx.as_slice() {
        [Effect::CustomCommand(CustomCommandEffect::RunShell(ShellRun { cwd, script }))] => {
            assert_eq!(cwd, &workdir);
            assert_eq!(script, "echo 'main' 'it'\\''s.txt'");
        }
        other => panic!("one RunShell expected: {other:?}"),
    }
    assert!(st.job.as_ref().is_some_and(|j| j.label == "Hi" && !j.keeps_undo));
    let fx = update(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: "$ echo\nmain".into(), undo: None }), Instant::now());
    assert!(st.job.is_none());
    assert!(st.log.iter().any(|l| l.text.contains("Hi")), "an activity row");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn what_a_command_needs_and_lost_or_busy_commands() {
    let (mut st, dir) = opened("cc-run-missing");
    git(&dir, &["checkout", "-q", "--detach"]);
    super::tests::drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Refresh));
    load(&mut st, file(&command("Branch log", "git", "log {branch}", "")), FileRead::default());
    let fx = run(&mut st, Source::User, "Branch log", Target::Palette);
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(last_log(&st), "Branch log wasn't run. Needs a checked-out branch.");
    load(&mut st, file(&command("Status", "git", "status", "")), FileRead::default());
    let fx = run(&mut st, Source::User, "Branch log", Target::Palette);
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "Branch log is no longer in your commands files.");
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: true,
    });
    assert!(run(&mut st, Source::User, "Status", Target::Palette).is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_command_that_confirms_asks_first() {
    let (mut st, dir) = opened("cc-run-confirm");
    load(&mut st, file(&command("Wipe", "git", "reset --hard {sha}", "confirm = true")), FileRead::default());
    let oid = head(&st);
    let fx = run(&mut st, Source::User, "Wipe", Target::Commit(oid));
    assert!(fx.is_empty());
    match &st.modal {
        Some(Modal::Confirm { title, body, button, .. }) => {
            assert_eq!((title.as_str(), button.as_str()), ("Wipe", "Run"));
            assert_eq!(body, &format!("Run Wipe?\n\nRuns: git reset --hard {oid}"));
        }
        _ => panic!("a confirmation"),
    }
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert_eq!(git_cmds(&fx), [["reset", "--hard", &oid.to_string()]]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The held job's name in the open trust prompt.
fn held(st: &AppState) -> Option<&str> {
    match &st.modal {
        Some(Modal::Trust(p)) => p.held.as_deref(),
        _ => None,
    }
}

#[test]
fn repository_commands_wait_for_trust_in_a_clean_repository() {
    let (mut st, dir) = opened("cc-run-trust");
    assert!(st.modal.is_none() && !st.held(), "a clean repository");
    let repo = file(&[command("Mine", "git", "status", ""), command("Sh", "shell", "echo hi", "")].concat());
    load(&mut st, file(&command("Yours", "git", "status", "")), repo);
    let fx = run(&mut st, Source::User, "Yours", Target::Palette);
    assert_eq!(git_cmds(&fx), [["status"]], "the user's own commands run");
    st.job = None;
    let fx = run(&mut st, Source::Repo, "Mine", Target::Palette);
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(held(&st), Some("Mine"));
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    let fx = update(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: true }), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::RunGit { cmds, .. } if cmds == &[vec!["status".to_owned()]])), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Mine"));
    st.job = None;
    // Trusted now: it runs at once, and so does a shell command.
    let fx = run(&mut st, Source::Repo, "Sh", Target::Palette);
    assert!(matches!(fx.as_slice(), [Effect::CustomCommand(CustomCommandEffect::RunShell(_))]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancelling_the_trust_prompt_drops_the_command() {
    let (mut st, dir) = opened("cc-run-cancel");
    load(&mut st, FileRead::default(), file(&command("Sh", "shell", "echo hi", "")));
    assert!(run(&mut st, Source::Repo, "Sh", Target::Palette).is_empty());
    assert_eq!(held(&st), Some("Sh"));
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(st.modal.is_none() && st.job.is_none());
    assert!(last_log(&st).starts_with("Sh wasn't run."), "{}", last_log(&st));
    // A repository trusted before runs them at once.
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);
    assert!(matches!(run(&mut st, Source::Repo, "Sh", Target::Palette).as_slice(), [Effect::CustomCommand(_)]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn in_a_held_repository_every_command_waits() {
    let (mut st, dir) = super::trust_held_tests::held_repo("cc-run-held");
    let user = file(&command("Yours", "git", "status", ""));
    load(&mut st, user, file(&command("Mine", "shell", "echo hi", "")));
    assert!(run(&mut st, Source::User, "Yours", Target::Palette).is_empty());
    assert_eq!(held(&st), Some("Yours"));
    st.modal = None;
    super::trust::dismissed(&mut st);
    assert!(run(&mut st, Source::Repo, "Mine", Target::Palette).is_empty());
    assert_eq!(held(&st), Some("Mine"));
    let _ = std::fs::remove_dir_all(&dir);
}
