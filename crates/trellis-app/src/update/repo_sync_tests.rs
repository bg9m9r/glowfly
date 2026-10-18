//! Fetch or pull across listed repositories: at most four at a time, trust, stale results, the
//! empty case, the toast after the dialog closed, Stop and Retry, and a real pull through the
//! runner.

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
            Effect::Repos(ReposEffect::Sync { generation, index, path, op, trust }) => {
                Some((*generation, *index, path.clone(), *op, trust.as_ref().is_some_and(|t| t.is_trusted())))
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

fn failed(reason: &str) -> Outcome {
    Outcome::Failed { reason: reason.into(), detail: String::new() }
}

fn states(st: &AppState) -> Vec<RowState> {
    st.repo_sync.as_ref().unwrap().rows.iter().map(|r| r.state.clone()).collect()
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
    assert!(syncs(&synced(&mut st, generation, 0, failed("again"))).is_empty());
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
    synced(&mut st, generation, 1, failed("no route"));
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Fetch of 2 repositories: 1 up to date, 1 failed.");
    assert!(!toast.ok);
    let action = toast.action.as_ref().expect("Show");
    assert!(matches!(*action.msg, Msg::Repos(ReposMsg::ShowSync)));
    run(&mut st, ReposMsg::ShowSync);
    assert!(matches!(st.modal, Some(Modal::SyncRepos)));
}

#[test]
fn stop_keeps_waiting_repositories_from_starting() {
    let mut st = with_favorites(&["a", "b", "c", "d", "e", "f"]);
    let generation = syncs(&start(&mut st, SyncKind::Fetch, SyncScope::Favorites))[0].0;
    update(&mut st, Msg::CloseModal, Instant::now());
    let fx = run(&mut st, ReposMsg::StopSync);
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(states(&st)[4..], [RowState::Stopped, RowState::Stopped]);
    assert!(st.repo_sync.as_ref().unwrap().running(), "the four running finish");
    assert_eq!(st.repo_sync.as_ref().unwrap().stopping(), Some(4));
    for i in 0..4 {
        assert!(syncs(&synced(&mut st, generation, i, Outcome::UpToDate)).is_empty(), "nothing new starts");
    }
    let sync = st.repo_sync.as_ref().unwrap();
    assert!(!sync.running());
    assert!(sync.summary().contains("2 stopped"), "{}", sync.summary());
    let toast = st.toasts.last().expect("the end toast, the dialog being closed");
    assert!(toast.text.contains("4 up to date, 2 stopped"), "{}", toast.text);
    // Stop on a finished run does nothing.
    assert!(run(&mut st, ReposMsg::StopSync).is_empty());
    assert_eq!(st.toasts.len(), 1);
}

#[test]
fn retry_runs_a_failed_or_stopped_row_again() {
    let mut st = with_favorites(&["a", "b", "c", "d", "e", "f"]);
    let generation = syncs(&start(&mut st, SyncKind::Fetch, SyncScope::Favorites))[0].0;
    let retry = |st: &mut AppState, generation, index| syncs(&run(st, ReposMsg::RetrySync { generation, index }));
    // Waiting and running rows can't be retried.
    assert!(retry(&mut st, generation, 0).is_empty() && retry(&mut st, generation, 5).is_empty());
    assert_eq!(states(&st)[5], RowState::Waiting);
    run(&mut st, ReposMsg::StopSync);
    // Row 0 fails; row 4 would start but was stopped.
    assert!(syncs(&synced(&mut st, generation, 0, failed("no route"))).is_empty());
    synced(&mut st, generation, 1, Outcome::UpToDate);
    // A stale generation, or an up-to-date row, does nothing.
    assert!(retry(&mut st, generation + 1, 0).is_empty());
    assert!(retry(&mut st, generation, 1).is_empty());
    let again = retry(&mut st, generation, 0);
    assert_eq!(again.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(), [(generation, 0)]);
    assert_eq!(states(&st)[0], RowState::Running);
    // Four run again (0, 2, 3 and the stopped 4); the fifth waits for one to finish.
    assert_eq!(retry(&mut st, generation, 4).iter().map(|s| s.1).collect::<Vec<_>>(), [4]);
    assert!(retry(&mut st, generation, 5).is_empty());
    assert_eq!(states(&st)[5], RowState::Waiting);
    assert_eq!(syncs(&synced(&mut st, generation, 2, Outcome::UpToDate)).iter().map(|s| s.1).collect::<Vec<_>>(), [5]);
}

#[test]
fn retry_after_the_run_ended_and_for_a_refused_repository() {
    let mut st = with_favorites(&["a", "b"]);
    let generation = syncs(&start(&mut st, SyncKind::Pull, SyncScope::Favorites))[0].0;
    synced(&mut st, generation, 0, failed("no route"));
    synced(&mut st, generation, 1, failed("no route"));
    assert!(!st.repo_sync.as_ref().unwrap().running());
    let again = syncs(&run(&mut st, ReposMsg::RetrySync { generation, index: 0 }));
    assert_eq!(again.iter().map(|s| (s.1, s.3)).collect::<Vec<_>>(), [(0, SyncOp::Pull)]);
    // A repository refused since is skipped without running.
    crate::state::trust::record(&mut st.settings, &abs("b"), RepoTrust::NotTrusted);
    assert!(run(&mut st, ReposMsg::RetrySync { generation, index: 1 }).is_empty());
    assert_eq!(states(&st)[1], RowState::Done(Outcome::Untrusted));
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

#[test]
fn a_trusted_favorite_that_gained_a_command_since_is_skipped() {
    let (work, _bare) = behind_origin("sync-stale-print");
    let before = trellis_core::trust::scan_at(&work).unwrap().print();
    git(&work, &["config", "core.sshCommand", "ssh-wrapper"]);
    let mut st = state();
    st.settings.favorite_repos = vec![trellis_core::paths::encode(&work)];
    crate::state::trust::record_trusted(&mut st.settings, &work, before);
    drive(&mut st, Msg::Repos(ReposMsg::StartSync { kind: SyncKind::Pull, scope: SyncScope::Favorites }));
    let sync = st.repo_sync.as_ref().unwrap();
    assert_eq!(sync.rows[0].state, RowState::Done(Outcome::Untrusted), "{:?}", sync.rows);
    assert!(!work.join("b.txt").exists(), "nothing ran");
}

#[test]
fn a_favorite_open_in_a_tab_uses_the_decision_where_the_tab_found_it() {
    let mut st = with_favorites(&["linked"]);
    // The tab opened the favorite and found it elsewhere on disk (a link).
    st.tabs[0].path = Some(abs("linked"));
    st.tabs[0].real = Some(abs("target"));
    crate::state::trust::record(&mut st.settings, &abs("target"), RepoTrust::NotTrusted);
    let fx = syncs(&start(&mut st, SyncKind::Fetch, SyncScope::Favorites));
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.repo_sync.as_ref().unwrap().rows[0].state, RowState::Done(Outcome::Untrusted));

    st.repo_sync = None;
    let print = trellis_core::trust::TrustPrint(["0123456789abcdef".to_string()].into());
    crate::state::trust::record_trusted(&mut st.settings, &abs("target"), print.clone());
    let fx: Vec<_> = start(&mut st, SyncKind::Fetch, SyncScope::Favorites)
        .into_iter()
        .filter_map(|e| if let Effect::Repos(ReposEffect::Sync { trust, .. }) = e { Some(trust) } else { None })
        .collect();
    assert_eq!(fx, [Some(trellis_core::settings::SavedTrust::trusted(Some(print)))]);
}
