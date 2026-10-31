//! Repository trust passed on: a worktree Glowfly creates and submodules it checks out for the
//! first time, from a trusted repository, are trusted too; never over a decision already saved,
//! and never from a repository that isn't trusted.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::trust_print_tests::trusted_repo;
use super::trust_tests::{asking, done, real, run, untrusted_repo};
use crate::msg::{Msg, RepoMsg, TrustMsg};
use crate::state::trust::record;
use crate::state::{AppState, Modal};
use crate::submodule::{Batch, SubmoduleMsg};
use crate::worktree::WorktreeMsg;
use glowfly_core::paths::{resolve, same_spelling};
use glowfly_core::settings::{RepoTrust, SavedTrust};
use glowfly_core::worktree_ops::WorktreeBase;
use std::path::{Path, PathBuf};

/// A folder next to `dir` for its new worktree, emptied first.
fn wt_folder(dir: &Path) -> PathBuf {
    let root = dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    root.join("side")
}

/// Creates a worktree for a new branch `side` at `folder` through the dialog, running the job
/// and what follows to the end.
fn create_worktree(st: &mut AppState, folder: &Path, open: bool) {
    drive(st, Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Branch("main".into()))));
    let Some(Modal::NewWorktree(f)) = st.modal.as_mut() else { panic!("the Create worktree dialog") };
    f.branch = "side".into();
    f.folder = folder.display().to_string();
    f.open = open;
    let fx = run(st, Msg::Worktree(WorktreeMsg::Submit));
    finish(st, fx);
}

fn saved(st: &AppState, folder: &Path) -> Option<SavedTrust> {
    st.settings.saved_trust(&resolve(folder))
}

fn cleanup(dir: &Path, folder: &Path) {
    let _ = std::fs::remove_dir_all(folder.parent().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_worktree_made_from_a_trusted_repository_is_trusted() {
    let (mut st, dir) = trusted_repo("inherit-wt");
    let folder = wt_folder(&dir);
    create_worktree(&mut st, &folder, false);
    assert!(folder.join(".git").exists(), "{:?}", st.log.last().map(|l| &l.text));
    let t = saved(&st, &folder).expect("the worktree is trusted");
    assert!(t.is_trusted() && t.print.as_ref().is_some_and(|p| !p.is_empty()), "{t:?}");
    assert!(st.log.iter().any(|l| l.text.starts_with("Trusted side: made from")), "{:?}", st.log.last().map(|l| &l.text));
    assert_eq!(st.tabs.len(), 1);
    cleanup(&dir, &folder);
}

#[test]
fn a_worktree_opened_after_creating_isn_t_asked_about() {
    let (mut st, dir) = trusted_repo("inherit-wt-open");
    let folder = wt_folder(&dir);
    create_worktree(&mut st, &folder, true);
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1), "{:?}", st.log.last().map(|l| &l.text));
    assert!(st.snap.as_ref().is_some_and(|s| same_spelling(&s.real_workdir, &resolve(&folder))));
    assert!(!asking(&st), "no prompt for the new worktree");
    assert!(!st.held());
    cleanup(&dir, &folder);
}

#[test]
fn nothing_is_passed_on_from_a_repository_that_isn_t_trusted() {
    // Held: the job waits for the prompt, and Trust and run passes nothing on.
    let dir = untrusted_repo("inherit-wt-held");
    let mut st = state();
    super::trust_tests::open(&mut st, &dir);
    drive(&mut st, Msg::CloseModal);
    let folder = wt_folder(&dir);
    create_worktree(&mut st, &folder, false);
    assert!(asking(&st), "the job waits");
    let real = real(&st);
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    finish(&mut st, fx);
    assert!(folder.join(".git").exists());
    assert!(saved(&st, &folder).is_none(), "made by a held job: nothing passed on");
    cleanup(&dir, &folder);

    // Clean: there is nothing to pass on.
    let dir = scratch_repo("inherit-wt-clean");
    let mut st = state();
    super::trust_tests::open(&mut st, &dir);
    let folder = wt_folder(&dir);
    create_worktree(&mut st, &folder, false);
    assert!(folder.join(".git").exists());
    assert!(saved(&st, &folder).is_none());
    cleanup(&dir, &folder);
}

#[test]
fn an_existing_decision_for_the_folder_is_kept() {
    let (mut st, dir) = trusted_repo("inherit-wt-kept");
    let folder = wt_folder(&dir);
    record(&mut st.settings, &folder, RepoTrust::NotTrusted);
    create_worktree(&mut st, &folder, false);
    assert!(folder.join(".git").exists());
    assert_eq!(st.saved_trust_for(&folder).map(|t| t.trust), Some(RepoTrust::NotTrusted));
    assert!(!st.log.iter().any(|l| l.text.starts_with("Trusted ")));
    cleanup(&dir, &folder);
}

/// A trusted repository with a submodule at libs/lib that isn't checked out.
fn with_uninitialized_submodule(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, parent, lib) = super::submodule_tests::opened(name);
    let real = real(&st);
    record(&mut st.settings, &real, RepoTrust::Trusted);
    super::submodule_tests::deinit(&mut st, &parent);
    (st, parent, lib)
}

#[test]
fn initializing_submodules_trusts_the_ones_checked_out() {
    let (mut st, parent, lib) = with_uninitialized_submodule("inherit-sm-init");
    let folder = parent.join("libs").join("lib");
    let fx = run(&mut st, Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::InitAll, confirmed: true }));
    finish(&mut st, fx);
    assert!(folder.join(".git").exists(), "{:?}", st.log.last().map(|l| &l.text));
    let t = saved(&st, &folder).expect("the submodule is trusted");
    assert!(t.is_trusted() && t.print.is_some());
    super::submodule_tests::cleanup(&[&parent, &lib]);
}

#[test]
fn updating_a_checked_out_submodule_passes_nothing_on() {
    let (mut st, parent, lib) = super::submodule_tests::opened("inherit-sm-update");
    let real = real(&st);
    record(&mut st.settings, &real, RepoTrust::Trusted);
    let folder = parent.join("libs").join("lib");
    std::fs::write(folder.join("a.txt"), "inside\n").unwrap();
    git(&folder, &["commit", "-q", "-am", "inside"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = run(&mut st, Msg::Submodule(SubmoduleMsg::Update { path: "libs/lib".into(), confirmed: true }));
    assert!(st.job.is_some());
    finish(&mut st, fx);
    assert!(saved(&st, &folder).is_none(), "it was checked out already");
    super::submodule_tests::cleanup(&[&parent, &lib]);
}

#[test]
fn a_failed_initialize_trusts_nothing() {
    let (mut st, parent, lib) = with_uninitialized_submodule("inherit-sm-failed");
    let folder = parent.join("libs").join("lib");
    run(&mut st, Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::InitAll, confirmed: true }));
    assert!(st.trust.inherit.is_some(), "it expects the submodule");
    done(&mut st, false, "fatal: clone failed");
    assert!(st.trust.inherit.is_none());
    assert!(saved(&st, &folder).is_none());
    super::submodule_tests::cleanup(&[&parent, &lib]);
}

#[test]
fn an_added_submodule_is_trusted() {
    let lib = scratch_repo("inherit-sm-add-lib");
    let (mut st, dir) = trusted_repo("inherit-sm-add");
    run(&mut st, Msg::Submodule(SubmoduleMsg::AddDialog));
    let Some(Modal::AddSubmodule(f)) = st.modal.as_mut() else { panic!("the Add submodule dialog") };
    f.url = lib.display().to_string();
    f.path = "libs/lib".into();
    f.path_edited = true;
    let fx = run(&mut st, Msg::Submodule(SubmoduleMsg::AddEdited));
    finish(&mut st, fx);
    let fx = run(&mut st, Msg::Submodule(SubmoduleMsg::Add));
    assert!(st.job.is_some(), "{:?}", st.log.last().map(|l| &l.text));
    finish(&mut st, fx);
    let folder = dir.join("libs").join("lib");
    assert!(folder.join(".git").exists(), "{:?}", st.log.last().map(|l| &l.text));
    let t = saved(&st, &folder).expect("the new submodule is trusted");
    assert!(t.is_trusted() && t.print.is_some());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&lib);
}
