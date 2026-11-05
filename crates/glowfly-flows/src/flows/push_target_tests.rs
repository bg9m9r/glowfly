//! Headless tests for where Push goes under push config (#599), on real repositories: main and
//! feature pushed to a bare `origin` (their upstreams) and main to a second bare `fork`. Each
//! test sets push.default and blanks remote.pushDefault in the repository's own config, so the
//! developer's global config can't decide them. The toolbar's Push and the branch menu's Push
//! follow `BranchInfo::push`: refused where git would refuse, an explicit push to the upstream
//! when the configured push remote is missing, and counted against a fork they push to.

use super::branch_tests::with_origin;
use super::tests::{drive, git};
use super::update;
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_model::effect::{Effect, JobEffect};
use glowfly_model::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::branches;
use glowfly_model::state::toolbar_tips;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A bare `fork` next to `work`, added as a remote, with main pushed there (and fetched).
fn add_fork(work: &Path) -> PathBuf {
    let fork = work.with_file_name(format!("{}-fork.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&fork);
    std::fs::create_dir_all(&fork).unwrap();
    git(&fork, &["init", "-q", "--bare", "-b", "main"]);
    git(work, &["remote", "add", "fork", &fork.display().to_string()]);
    git(work, &["push", "-q", "fork", "main"]);
    fork
}

/// `with_origin(name)` plus `add_fork`, with push.default `mode` and remote.pushDefault blank.
fn setup(name: &str, mode: &str) -> (AppState, PathBuf, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    let fork = add_fork(&work);
    config(&mut st, &work, &[("push.default", mode), ("remote.pushDefault", "")]);
    (st, work, bare, fork)
}

fn config(st: &mut AppState, work: &Path, entries: &[(&str, &str)]) {
    for (key, value) in entries {
        git(work, &["config", key, value]);
    }
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn commit(st: &mut AppState, work: &Path, msg: &str) {
    git(work, &["commit", "-q", "--allow-empty", "-m", msg]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

/// The git commands `msg` starts (none when it is refused), leaving no job running.
fn pushes(st: &mut AppState, msg: Msg) -> Vec<Vec<String>> {
    let fx = update(st, msg, Instant::now());
    st.job = None;
    fx.into_iter().filter_map(|e| if let Effect::Job(JobEffect::RunGit { cmds, .. }) = e { Some(cmds) } else { None }).flatten().collect()
}

fn toolbar_push() -> Msg {
    Msg::Job(JobMsg::Push { force: false })
}

fn row_push(remote: Option<&str>) -> Msg {
    Msg::Branch(BranchMsg::PushBranch { branch: "main".into(), remote: remote.map(str::to_owned) })
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn main_branch(st: &AppState) -> &glowfly_core::repo::BranchInfo {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "main").unwrap()
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn upstream_mode_with_a_fork_push_remote_is_refused() {
    let (mut st, work, bare, fork) = setup("pt-flow-refused", "upstream");
    commit(&mut st, &work, "ours");
    config(&mut st, &work, &[("remote.pushDefault", "fork")]);
    let why = "push.default is upstream, and remote.pushDefault sends main to fork rather than origin, where its upstream origin/main \
               is. Git refuses that push. Use Push to, or set push.default to simple or current.";
    assert_eq!(toolbar_tips::push_blocked(&st).as_deref(), Some(why));
    assert_eq!(toolbar_tips::push_target(Lang::En, st.snap.as_ref().unwrap()).as_deref(), Some("Push main to fork"));
    assert!(pushes(&mut st, toolbar_push()).is_empty(), "no job");
    assert_eq!(last_log(&st), why);
    assert!(pushes(&mut st, row_push(None)).is_empty(), "the row's Push is refused the same");
    assert_eq!(last_log(&st), why);
    let explicit = pushes(&mut st, row_push(Some("origin")));
    assert_eq!(explicit, [args(&["push", "--", "origin", "refs/heads/main:refs/heads/main"])], "Push to origin still goes");
    cleanup(&[&work, &bare, &fork]);
}

#[test]
fn a_missing_push_remote_pushes_to_the_upstream() {
    let (mut st, work, bare, fork) = setup("pt-flow-missing", "simple");
    commit(&mut st, &work, "ours");
    config(&mut st, &work, &[("branch.main.pushRemote", "gone")]);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(toolbar_tips::push_target(Lang::En, snap).as_deref(), Some("Push main to origin/main"));
    let note = toolbar_tips::push_note(Lang::En, snap).unwrap();
    assert!(note.contains("\"gone\"") && note.contains("branch.main.pushRemote"), "{note}");
    assert_eq!(toolbar_tips::push_blocked(&st), None);
    assert_eq!(pushes(&mut st, toolbar_push()), [args(&["push", "--", "origin", "refs/heads/main:refs/heads/main"])]);
    cleanup(&[&work, &bare, &fork]);
}

#[test]
fn push_to_a_fork_is_judged_against_the_fork() {
    let (mut st, work, bare, fork) = setup("pt-flow-fork", "simple");
    // fork/main stays at the first commit; origin/main gets two more.
    commit(&mut st, &work, "two");
    commit(&mut st, &work, "three");
    git(&work, &["push", "-q", "origin", "main"]);
    config(&mut st, &work, &[("remote.pushDefault", "fork")]);
    let b = main_branch(&st);
    assert_eq!((b.ahead, b.behind), (0, 0), "level with origin/main");
    assert_eq!(branches::push_blocked(Lang::En, b, true), None, "fork/main lacks two commits");
    assert_eq!(toolbar_tips::push_ahead(st.snap.as_ref().unwrap()), Some(("fork/main".into(), Some(2))));
    assert_eq!(pushes(&mut st, row_push(None)), [args(&["push", "--", "fork", "refs/heads/main:refs/heads/main"])]);
    assert_eq!(pushes(&mut st, toolbar_push()), [args(&["push"])], "git's own choice");
    // Level with the fork, one ahead of origin: nothing to push there.
    commit(&mut st, &work, "four");
    git(&work, &["push", "-q", "fork", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let b = main_branch(&st);
    assert_eq!((b.ahead, b.behind), (1, 0), "ahead of origin/main");
    assert_eq!(branches::push_blocked(Lang::En, b, true).as_deref(), Some("main is up to date with fork/main."));
    cleanup(&[&work, &bare, &fork]);
}
