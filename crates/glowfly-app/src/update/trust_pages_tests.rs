//! Repository trust and the Git LFS and Sparse Checkout pages: while git is held in the
//! repository their reads run no git in it (the LFS page reads nothing, the sparse status
//! leaves its paths unread), the install check runs outside it, and trusting it reads again.

use super::tests::{drive, scratch_repo, state};
use super::trust_held_tests::held_repo;
use super::trust_tests::{real, run};
use crate::effect::Effect;
use crate::lfs::LfsMsg;
use crate::msg::{Msg, PrefsMsg, RepoMsg, SparseMsg, TrustMsg};
use crate::state::{AppState, Modal, PrefPage};
use glowfly_core::lfs::LfsPattern;
use glowfly_core::sparse_ops::{SparseMode, SparseStatus};
use std::path::PathBuf;

fn loads_lfs(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LoadLfs { .. }))
}

/// `run_git` of the one LoadSparse in `fx`.
fn sparse_read(fx: &[Effect]) -> Option<bool> {
    fx.iter().find_map(|e| match e {
        Effect::LoadSparse { run_git, .. } => Some(*run_git),
        _ => None,
    })
}

fn trust(st: &mut AppState) -> Vec<Effect> {
    let real = real(st);
    run(st, Msg::Trust(TrustMsg::Decide { real, trusted: true }))
}

fn open_clean(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.runs_git_unasked());
    (st, dir)
}

fn detect_dir(fx: &[Effect]) -> Option<PathBuf> {
    fx.iter().find_map(|e| match e {
        Effect::DetectLfs { cwd } => Some(cwd.clone()),
        _ => None,
    })
}

#[test]
fn the_lfs_page_reads_nothing_while_git_is_held_and_reads_once_trusted() {
    let (mut st, dir) = held_repo("pages-lfs-held");
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)));
    assert!(!loads_lfs(&fx), "{fx:?}");
    let page = &st.prefs.as_ref().unwrap().lfs;
    assert!(page.held && page.status.is_none());

    assert!(!loads_lfs(&run(&mut st, Msg::Lfs(LfsMsg::Reload))));
    let pattern = LfsPattern { raw: "*.psd".into(), display: "*.psd".into(), lockable: false };
    let mut fx = Vec::new();
    super::lfs::patterns_read(&mut st, Some(vec![pattern]), &mut fx);
    assert!(!loads_lfs(&fx), "new patterns don't read with git either");

    let fx = trust(&mut st);
    assert!(loads_lfs(&fx), "{fx:?}");
    assert!(!st.prefs.as_ref().unwrap().lfs.held);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_install_check_runs_outside_a_held_repository() {
    let (mut st, dir) = held_repo("pages-detect-held");
    let fx = run(&mut st, Msg::Lfs(LfsMsg::Detect));
    assert_eq!(detect_dir(&fx), Some(AppState::outside_dir()));
    trust(&mut st);
    let fx = run(&mut st, Msg::Lfs(LfsMsg::Detect));
    assert_eq!(detect_dir(&fx), st.workdir(), "a trusted repository");
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = open_clean("pages-detect-clean");
    let fx = run(&mut st, Msg::Lfs(LfsMsg::Detect));
    assert_eq!(detect_dir(&fx), st.workdir(), "a clean repository");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_sparse_page_reads_without_git_while_held() {
    let (mut st, dir) = held_repo("pages-sparse-held");
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    assert_eq!(sparse_read(&fx), Some(false), "{fx:?}");
    assert!(st.prefs.as_ref().unwrap().sparse.held);
    let fx = trust(&mut st);
    assert_eq!(sparse_read(&fx), Some(true), "trusting reads again with git: {fx:?}");
    let fx = run(&mut st, Msg::Sparse(SparseMsg::Reload));
    assert_eq!(sparse_read(&fx), Some(true));
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = open_clean("pages-sparse-clean");
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    assert_eq!(sparse_read(&fx), Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_held_sparse_status_never_refills_the_form() {
    let (mut st, dir) = held_repo("pages-sparse-keep");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    let page = &mut st.prefs.as_mut().unwrap().sparse;
    page.paths = "src\ndocs".into();
    page.mode = SparseMode::Cone;
    page.filled = ("src\ndocs".into(), SparseMode::Cone);
    let generation = page.generation;
    let held = SparseStatus { enabled: true, mode: SparseMode::Patterns, held: true, tracked: 3, skipped: 1, ..Default::default() };
    run(&mut st, Msg::Sparse(SparseMsg::Loaded { generation, result: Ok(held.clone()) }));
    let page = &st.prefs.as_ref().unwrap().sparse;
    assert_eq!(page.status.as_ref(), Some(&held));
    assert_eq!((page.paths.as_str(), page.mode), ("src\ndocs", SparseMode::Cone));
    assert_eq!(page.filled, ("src\ndocs".to_owned(), SparseMode::Cone));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turning_sparse_checkout_on_in_a_held_repository_waits_for_the_prompt() {
    let (mut st, dir) = held_repo("pages-sparse-turn-on");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    let generation = st.prefs.as_ref().unwrap().sparse.generation;
    let off = SparseStatus { tracked: 3, ..Default::default() };
    run(&mut st, Msg::Sparse(SparseMsg::Loaded { generation, result: Ok(off) }));
    st.prefs.as_mut().unwrap().sparse.paths = "src".into();
    let fx = run(&mut st, Msg::Sparse(SparseMsg::Apply));
    assert!(!fx.iter().any(|e| matches!(e, Effect::SparseCheckout { .. })), "{fx:?}");
    let Some(Modal::Trust(p)) = &st.modal else { panic!("expected the trust prompt") };
    assert_eq!(p.held.as_deref(), Some("Turn on sparse checkout"));
    let _ = std::fs::remove_dir_all(&dir);
}
