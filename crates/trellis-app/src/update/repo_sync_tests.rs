//! Fetch or pull across listed repositories: at most four at a time, trust, stale results, the
//! empty case, the toast after the dialog closed, and a real pull through the runner.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, ReposEffect};
use crate::msg::{Msg, ReposMsg};
use crate::state::repo_sync::{MAX_RUNNING, RowState, SyncKind, SyncScope};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo_sync::{Outcome, SyncOp};
use trellis_core::settings::RepoTrust;

fn run(st: &mut AppState, m: ReposMsg) -> Vec<Effect> {
    update(st, Msg::Repos(m), Instant::now())
}

fn start(st: &mut AppState, kind: SyncKind, scope: SyncScope) -> Vec<Effect> {
    run(st, ReposMsg::StartSync { kind, scope })
}

/// A folder that is absolute on every system (nothing is read from it).
fn abs(rel: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join(format!("trellis-sync-none-{}", std::process::id())).join(rel)
}

/// The Sync effects in `fx`: (generation, row, path, op, trusted).
fn syncs(fx: &[Effect]) -> Vec<(u64, usize, PathBuf, SyncOp, bool)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Repos(ReposEffect::Sync { generation, index, path, op, trusted }) => {
                Some((*generation, *index, path.clone(), *op, *trusted))
            }
            _ => None,
        })
        .collect()
}

fn with_favorites(names: &[&str]) -> AppState {
    let mut st = state();
    st.settings.favorite_repos = names.iter().map(|n| trellis_core::paths::encode(&abs(n))).collect();
    st
}

fn synced(st: &mut AppState, generation: u64, index: usize, outcome: Outcome) -> Vec<Effect> {
    run(st, ReposMsg::Synced { generation, index, outcome })
}

#[test]
fn nothing_to_fetch_says_so() {
    let mut st = state();
    let fx = start(&mut st, SyncKind::Fetch, SyncScope::Favorites);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.modal.is_none() && st.repo_sync.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("No repositories to fetch: no favorite can be found."));
    // Missing folders don't count.
    st.recent = vec![abs("gone")];
    st.missing_repos = vec![abs("gone")];
    assert!(start(&mut st, SyncKind::Pull, SyncScope::Listed).is_empty());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("No repositories to pull: no listed repository can be found."));
}

#[test]
fn at_most_four_run_and_each_result_starts_the_next() {
    let mut st = with_favorites(&["a", "b", "c", "d", "e", "f"]);
    st.recent = vec![abs("a"), abs("g")];
    let fx = start(&mut st, SyncKind::Fetch, SyncScope::Listed);
    let first = syncs(&fx);
    assert_eq!(first.len(), MAX_RUNNING);
    assert_eq!(first.iter().map(|s| s.1).collect::<Vec<_>>(), [0, 1, 2, 3]);
    assert!(first.iter().all(|s| s.3 == SyncOp::Fetch { prune: true } && !s.4), "{first:?}");
    assert!(matches!(st.modal, Some(Modal::SyncRepos)));
    let sync = st.repo_sync.as_ref().unwrap();
    assert_eq!(sync.rows.len(), 7, "a is listed twice but runs once");
    let generation = sync.generation;
    // A stale or repeated result changes nothing.
    assert!(syncs(&synced(&mut st, generation + 1, 0, Outcome::UpToDate)).is_empty());
    let next = syncs(&synced(&mut st, generation, 0, Outcome::UpToDate));
    assert_eq!(next.iter().map(|s| s.1).collect::<Vec<_>>(), [4]);
    assert!(syncs(&synced(&mut st, generation, 0, Outcome::Failed("again".into()))).is_empty());
    assert_eq!(st.repo_sync.as_ref().unwrap().rows[0].state, RowState::Done(Outcome::UpToDate));
    // Starting again while it runs shows it, without starting anything.
    st.modal = None;
    assert!(start(&mut st, SyncKind::Pull, SyncScope::Favorites).is_empty());
    assert!(matches!(st.modal, Some(Modal::SyncRepos)));
    assert_eq!(st.repo_sync.as_ref().unwrap().generation, generation);
    for i in 1..7 {
        synced(&mut st, generation, i, Outcome::Updated { changed: 1 });
    }
    let sync = st.repo_sync.as_ref().unwrap();
    assert!(!sync.running());
    assert_eq!(sync.summary(), "Fetch of 7 repositories: 6 updated, 1 up to date.");
    // The dialog is open, so no toast.
    assert!(st.toasts.is_empty());
}

#[test]
fn trust_decisions_decide_what_runs() {
    let mut st = with_favorites(&["yes", "no", "undecided"]);
    crate::state::trust::record(&mut st.settings, &abs("yes"), RepoTrust::Trusted);
    crate::state::trust::record(&mut st.settings, &abs("no"), RepoTrust::NotTrusted);
    st.settings.auto_prune = false;
    let fx = syncs(&start(&mut st, SyncKind::Pull, SyncScope::Favorites));
    let ran: Vec<(PathBuf, SyncOp, bool)> = fx.into_iter().map(|s| (s.2, s.3, s.4)).collect();
    assert_eq!(ran, [(abs("yes"), SyncOp::Pull, true), (abs("undecided"), SyncOp::Pull, false)], "the refused one never runs");
    assert_eq!(st.repo_sync.as_ref().unwrap().rows[1].state, RowState::Done(Outcome::Untrusted));
    let fetch = syncs(&{
        st.repo_sync = None;
        start(&mut st, SyncKind::Fetch, SyncScope::Favorites)
    });
    assert_eq!(fetch[0].3, SyncOp::Fetch { prune: false }, "follows Preferences' prune");
}

#[test]
fn a_run_that_ends_behind_a_closed_dialog_says_so_in_a_toast() {
    let mut st = with_favorites(&["a", "b"]);
    let fx = syncs(&start(&mut st, SyncKind::Fetch, SyncScope::Favorites));
    let generation = fx[0].0;
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(st.modal.is_none());
    synced(&mut st, generation, 0, Outcome::UpToDate);
    assert!(st.toasts.is_empty(), "still running");
    synced(&mut st, generation, 1, Outcome::Failed("no route".into()));
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Fetch of 2 repositories: 1 up to date, 1 failed.");
    assert!(!toast.ok);
    let action = toast.action.as_ref().expect("Show");
    assert!(matches!(*action.msg, Msg::Repos(ReposMsg::ShowSync)));
    run(&mut st, ReposMsg::ShowSync);
    assert!(matches!(st.modal, Some(Modal::SyncRepos)));
}

/// A repository with an `origin` that has one commit it lacks; returns (work, bare).
fn behind_origin(name: &str) -> (PathBuf, PathBuf) {
    let work = scratch_repo(name);
    git(&work, &["checkout", "-q", "--", "."]);
    let bare = work.with_file_name(format!("{}-origin.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "origin", &bare.display().to_string()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    let other = work.with_file_name(format!("{}-other", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&other);
    git(work.parent().unwrap(), &["clone", "-q", &bare.display().to_string(), &other.display().to_string()]);
    super::tests::set_identity(&other);
    std::fs::write(other.join("b.txt"), "two\n").unwrap();
    git(&other, &["add", "."]);
    git(&other, &["commit", "-q", "-m", "second"]);
    git(&other, &["push", "-q"]);
    let _ = std::fs::remove_dir_all(&other);
    (work, bare)
}

#[test]
fn pulling_listed_repositories_runs_git_in_each() {
    let (work, _bare) = behind_origin("sync-pull");
    let lone = scratch_repo("sync-pull-lone");
    let mut st = state();
    st.recent = vec![work.clone(), lone.clone()];
    drive(&mut st, Msg::Repos(ReposMsg::StartSync { kind: SyncKind::Pull, scope: SyncScope::Listed }));
    let sync = st.repo_sync.as_ref().unwrap();
    let state_of = |p: &Path| sync.rows.iter().find(|r| r.path == p).map(|r| r.state.clone());
    assert!(matches!(state_of(&work), Some(RowState::Done(Outcome::Updated { .. }))), "{:?}", sync.rows);
    assert_eq!(state_of(&lone), Some(RowState::Done(Outcome::NoRemotes)));
    assert!(work.join("b.txt").exists(), "fast-forwarded");
    assert_eq!(sync.summary(), "Pull of 2 repositories: 1 updated, 1 with nothing to pull.");
}
