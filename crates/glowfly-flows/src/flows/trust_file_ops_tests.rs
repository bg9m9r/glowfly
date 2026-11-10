//! Staging in a repository with a clean filter, end to end through update and the runner
//! (update/file_trust.rs): `git add` runs the filter, so while git is held there the stage waits
//! for the trust prompt, runs on Trust, and is dropped on Don't trust; in a repository that was
//! covered when it loaded, the runner checks again just before `git add` starts.

use super::tests::{git, scratch_repo, state};
use super::trust_recheck_tests::{cleanup, prompt, recheck, sealed, trust_and_run};
use super::trust_tests::{asking, open, real};
use glowfly_model::effect::{Effect, FilesEffect, LocalOp, Sealed};
use glowfly_model::msg::{Msg, StageMsg, TrustMsg};
use std::path::{Path, PathBuf};

/// Where the filter of `with_filter` leaves a copy of what it cleaned.
fn ran(dir: &Path) -> PathBuf {
    dir.join(".git/clean-ran")
}

/// Sets a clean filter for every .txt file of `dir` that writes `ran(dir)` when git runs it.
fn with_filter(dir: &Path) {
    std::fs::write(dir.join(".gitattributes"), "*.txt filter=mark\n").unwrap();
    git(dir, &["config", "filter.mark.clean", "tee .git/clean-ran"]);
}

fn stage() -> Msg {
    Msg::Stage(StageMsg::StageFile("a.txt".into()))
}

/// The checked stage among `fx`.
fn checked_stage(fx: Vec<Effect>) -> Effect {
    let is_stage = |e: &Effect| matches!(e, Effect::Files(FilesEffect::LocalOp { op: LocalOp::StageFile(_), .. }));
    fx.into_iter().find(|e| matches!(e, Effect::Checked(c) if is_stage(&c.effect))).expect("a checked stage")
}

fn staged(dir: &Path) -> String {
    glowfly_core::gitcli::run(dir, &glowfly_core::gitcli::args(&["diff", "--cached", "--name-only"])).1
}

#[test]
fn staging_runs_a_clean_filter_only_once_the_repository_is_trusted() {
    let dir = scratch_repo("trust-stage-filter");
    with_filter(&dir);
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st), "a filter runs a program");
    st.modal = None;
    assert!(!ran(&dir).exists(), "opening it ran nothing");
    let fx = sealed(&mut st, stage());
    assert!(!fx.iter().any(|e| matches!(e, Effect::Checked(_) | Effect::Files(_))), "nothing runs yet: {fx:?}");
    let p = prompt(&st);
    assert_eq!(p.held.as_deref(), Some("Stage"), "the prompt says what waits");
    assert!(p.scan.config.iter().any(|c| c.key.eq_ignore_ascii_case("filter.mark.clean")), "and what would run: {:?}", p.scan.config);
    assert!(!ran(&dir).exists());
    assert!(!staged(&dir).contains("a.txt"));
    // Trust and run stages it, checked again by the runner.
    let stage = checked_stage(trust_and_run(&mut st));
    assert!(recheck(&mut st, stage));
    assert!(ran(&dir).exists(), "git add ran the filter");
    assert!(staged(&dir).contains("a.txt"));
    cleanup(&[&dir]);
}

#[test]
fn dont_trust_leaves_the_file_unstaged_and_the_filter_unrun() {
    let dir = scratch_repo("trust-stage-refused");
    with_filter(&dir);
    let mut st = state();
    open(&mut st, &dir);
    st.modal = None;
    let _ = sealed(&mut st, stage());
    let real = real(&st);
    let fx = sealed(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: false }));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Checked(_) | Effect::Files(_))), "{fx:?}");
    // Asked again in the repository that is now not trusted: it waits again.
    let fx = sealed(&mut st, stage());
    assert!(!fx.iter().any(|e| matches!(e, Effect::Checked(_) | Effect::Files(_))), "{fx:?}");
    assert_eq!(prompt(&st).held.as_deref(), Some("Stage"));
    assert!(!ran(&dir).exists());
    assert!(!staged(&dir).contains("a.txt"));
    cleanup(&[&dir]);
}

#[test]
fn a_filter_set_after_the_repository_loaded_stops_the_stage_before_git_add() {
    let dir = scratch_repo("trust-stage-recheck");
    let mut st = state();
    open(&mut st, &dir);
    assert!(!asking(&st), "nothing in it runs programs");
    with_filter(&dir);
    let stage = checked_stage(sealed(&mut st, stage()));
    assert_eq!(stage.trust_check().map(|c| c.sealed.clone()), Some(Sealed::Read));
    let Effect::Checked(c) = stage else { panic!("checked") };
    let glowfly_model::effect::Checked { effect, check } = *c;
    let answer = match glowfly_runner::testing::execute_checked(check, effect) {
        Some(Msg::Scoped { msg, .. }) => *msg,
        _ => panic!("an answer for its repository"),
    };
    assert!(matches!(answer, Msg::Stage(StageMsg::LocalOpDone(Err(_)))), "it fails as its sender expects");
    assert!(!ran(&dir).exists(), "git add didn't run");
    assert!(!staged(&dir).contains("a.txt"));
    cleanup(&[&dir]);
}
