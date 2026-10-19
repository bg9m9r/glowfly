//! Headless tests for leaving an edited Preferences page: what asks "Save your changes?" (closing,
//! another page, another profile, another tab on a repository page) and what doesn't, the three
//! answers, a refused save keeping the page, and the approval that lets the held leave through
//! once.

use super::gitflow_tests::{cleanup, initialized};
use super::prefs_save_tests::{edited, git_mut, identity, on_page, page, prefs, run, writes};
use super::profiles_tests::{PERSONAL, WORK, two_profiles};
use super::tests::{drive, scratch_repo, state};
use crate::msg::{Msg, PrefsMsg, RepoMsg, SparseMsg, TabsMsg};
use crate::profile::ProfileMsg;
use crate::state::prefs::GitConfigValues;
use crate::state::tabs::Tab;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use crate::tools::ToolsMsg;
use trellis_core::sparse_ops::{SparseMode, SparseStatus};

fn asking(st: &AppState) -> bool {
    matches!(st.modal, Some(Modal::ConfirmChoice { .. }))
}

/// The Identity page with the name edited to " Grace " and ClosePrefs held by the question.
fn closing_edited_identity() -> AppState {
    let mut st = identity();
    git_mut(&mut st).repo_name = " Grace ".into();
    let fx = prefs(&mut st, PrefsMsg::ClosePrefs);
    assert!(writes(&fx).is_empty(), "{fx:?}");
    st
}

#[test]
fn closing_an_edited_page_asks_first() {
    let st = closing_edited_identity();
    assert_eq!(page(&st), PrefPage::RepoIdentity, "held");
    assert!(st.prefs.as_ref().unwrap().leaving.is_some());
    match &st.modal {
        Some(Modal::ConfirmChoice { title, body, button, alt, .. }) => {
            assert_eq!(title, "Save your changes?");
            assert_eq!(body, "Identity has changes that aren't saved yet.");
            assert_eq!((button.as_str(), alt.as_str()), ("Save", "Discard"));
        }
        _ => panic!("expected the question"),
    }
}

#[test]
fn save_writes_then_leaves() {
    let mut st = closing_edited_identity();
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(writes(&fx), [(false, "user.name", Some("Grace"))]);
    assert!(st.prefs.is_none(), "closed after saving");
}

#[test]
fn discard_reverts_then_leaves() {
    let mut st = closing_edited_identity();
    let fx = run(&mut st, Msg::ConfirmAlt);
    assert!(writes(&fx).is_empty());
    assert!(st.modal.is_none() && st.prefs.is_none());
}

#[test]
fn cancel_keeps_the_page_and_its_edits() {
    let mut st = closing_edited_identity();
    run(&mut st, Msg::CloseModal);
    assert_eq!(page(&st), PrefPage::RepoIdentity);
    assert!(edited(&st));
    assert!(st.prefs.as_ref().unwrap().leaving.is_none(), "the held leave is forgotten");
}

#[test]
fn unedited_pages_leave_at_once() {
    let mut st = identity();
    prefs(&mut st, PrefsMsg::ClosePrefs);
    assert!(st.prefs.is_none() && st.modal.is_none());
    // Edits on a page without the bar never ask.
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    st.settings.show_toolbar = !st.settings.show_toolbar;
    prefs(&mut st, PrefsMsg::ClosePrefs);
    assert!(st.prefs.is_none() && st.modal.is_none());
}

#[test]
fn another_page_asks_and_save_goes_there() {
    let mut st = on_page(PrefPage::RepoHooks, GitConfigValues::default());
    git_mut(&mut st).hooks_path = ".githooks".into();
    prefs(&mut st, PrefsMsg::Navigate(PrefPage::General));
    assert!(asking(&st));
    assert_eq!(page(&st), PrefPage::RepoHooks);
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(writes(&fx), [(false, "core.hooksPath", Some(".githooks"))]);
    assert_eq!(page(&st), PrefPage::General);
}

#[test]
fn every_way_to_another_page_asks() {
    for msg in [Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Tools)), Msg::Tools(ToolsMsg::ShowPage)] {
        let mut st = identity();
        git_mut(&mut st).repo_name = "Grace".into();
        run(&mut st, msg);
        assert!(asking(&st));
        assert_eq!(page(&st), PrefPage::RepoIdentity);
    }
}

#[test]
fn the_open_pages_row_keeps_its_edits() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    prefs(&mut st, PrefsMsg::Navigate(PrefPage::RepoIdentity));
    assert!(st.modal.is_none());
    assert_eq!(git_mut(&mut st).repo_name, "Grace");
}

#[test]
fn tabs_ask_only_on_repository_pages() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    run(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(asking(&st), "the edits belong to this tab's repository");

    let mut st = on_page(PrefPage::General, GitConfigValues::default());
    git_mut(&mut st).longpaths = true;
    run(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(!asking(&st), "global settings stay with any tab");
    st.modal = None;
    prefs(&mut st, PrefsMsg::ClosePrefs);
    assert!(asking(&st), "but closing still asks");
}

#[test]
fn another_profile_asks_about_the_edits() {
    let mut st = two_profiles();
    st.prefs.as_mut().unwrap().profiles.author_name = "Ada Lovelace".into();
    assert!(edited(&st));
    run(&mut st, Msg::Profile(ProfileMsg::Select(PERSONAL)));
    assert!(asking(&st));
    assert_eq!(st.prefs.as_ref().unwrap().profiles.selected, Some(WORK));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(writes(&fx), [(true, "user.name", Some("Ada Lovelace"))]);
    assert_eq!(st.settings.profile(WORK).unwrap().author_name, "Ada Lovelace");
    assert_eq!(st.prefs.as_ref().unwrap().profiles.selected, Some(PERSONAL));
}

/// An initialized Git Flow repository with its page open and the feature prefix edited.
fn gitflow_edited(name: &str, feature: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = initialized(name);
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    assert!(!edited(&st));
    st.prefs.as_mut().unwrap().gitflow.form.feature = feature.into();
    assert!(edited(&st));
    (st, dir)
}

#[test]
fn git_flow_discard_resets_the_form_and_switches_the_tab() {
    let (mut st, dir) = gitflow_edited("leave-tab", "feat/");
    st.tabs.push(Tab::unloaded(trellis_core::paths::temp_dir().join("trellis-other-tab")));
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    assert!(asking(&st));
    assert_eq!(st.active_tab, 0);
    let filled = st.prefs.as_ref().unwrap().gitflow.filled.clone();
    run(&mut st, Msg::ConfirmAlt);
    assert_eq!(st.active_tab, 1);
    if let Some(p) = st.prefs.as_ref() {
        assert_eq!(p.gitflow.form, filled);
    }
    cleanup(&dir);
}

#[test]
fn git_flow_saves_through_its_job_and_then_leaves() {
    let (mut st, dir) = gitflow_edited("leave-save", "feat/");
    prefs(&mut st, PrefsMsg::Navigate(PrefPage::General));
    assert!(asking(&st));
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(page(&st), PrefPage::General, "left once the save started");
    assert_eq!(st.snap.as_ref().unwrap().gitflow.feature.as_deref(), Some("feat/"));
    cleanup(&dir);
}

#[test]
fn a_refused_git_flow_save_stays() {
    let (mut st, dir) = gitflow_edited("leave-refused", "");
    st.tabs.push(Tab::unloaded(trellis_core::paths::temp_dir().join("trellis-other-tab")));
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    run(&mut st, Msg::ConfirmAccepted);
    let p = st.prefs.as_ref().unwrap();
    assert_eq!(p.gitflow.error.as_deref(), Some("Enter a feature prefix."));
    assert_eq!(p.page, PrefPage::RepoGitFlow);
    assert!(p.leaving.is_none() && st.job.is_none());
    assert_eq!(st.active_tab, 0, "no tab switch");
    cleanup(&dir);
}

/// A repository with the Sparse checkout page showing `status`.
fn sparse_page(name: &str, status: SparseStatus) -> (AppState, std::path::PathBuf) {
    let dir = scratch_repo(&format!("prefs-leave-{name}"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    let generation = st.prefs.as_ref().unwrap().sparse.generation;
    run(&mut st, Msg::Sparse(SparseMsg::Loaded { generation, result: Ok(status) }));
    (st, dir)
}

#[test]
fn a_refused_sparse_save_stays() {
    let on = SparseStatus { enabled: true, mode: SparseMode::Patterns, paths: vec!["/src/".into()], tracked: 3, ..Default::default() };
    let (mut st, dir) = sparse_page("refused", on);
    st.prefs.as_mut().unwrap().sparse.paths.clear();
    assert!(edited(&st));
    prefs(&mut st, PrefsMsg::ClosePrefs);
    assert!(asking(&st));
    run(&mut st, Msg::ConfirmAccepted);
    let p = st.prefs.as_ref().expect("still open");
    assert!(p.sparse.paths_error.is_some());
    assert_eq!(p.page, PrefPage::RepoSparse);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sparse_edits_while_off_are_not_unsaved() {
    let (mut st, dir) = sparse_page("off", SparseStatus { tracked: 3, ..Default::default() });
    st.prefs.as_mut().unwrap().sparse.paths = "src".into();
    assert!(!edited(&st), "Turn on uses them");
    prefs(&mut st, PrefsMsg::ClosePrefs);
    assert!(st.prefs.is_none() && st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_approval_lets_one_message_through() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    st.prefs.as_mut().unwrap().leave_approved = true;
    prefs(&mut st, PrefsMsg::Navigate(PrefPage::General));
    assert!(!asking(&st), "approved");
    assert_eq!(page(&st), PrefPage::General);
    assert!(!st.prefs.as_ref().unwrap().leave_approved, "used up");

    // An approval that wasn't used doesn't outlive the message it came with.
    let mut st = identity();
    st.prefs.as_mut().unwrap().leave_approved = true;
    prefs(&mut st, PrefsMsg::SettingsEdited);
    git_mut(&mut st).repo_name = "Grace".into();
    prefs(&mut st, PrefsMsg::ClosePrefs);
    assert!(asking(&st));
}
