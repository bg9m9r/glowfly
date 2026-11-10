//! Undo and redo of a worktree created with a new branch, through the runner: undo removes the
//! folder and deletes the branch, redo makes both again, and while the worktree is open in a tab
//! the undo is refused.

use super::tests::{drive, git, scratch_repo, state};
use glowfly_core::i18n::Lang;
use glowfly_core::worktree_ops::WorktreeBase;
use glowfly_model::msg::{JobMsg, Msg, RepoMsg, TabsMsg};
use glowfly_model::state::{AppState, Modal};
use glowfly_model::worktree::WorktreeMsg;
use std::path::{Path, PathBuf};

fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    glowfly_model::testing::clean_tree(&dir);
    let folder = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&folder);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, folder)
}

/// Creates a worktree at `folder` on new branch `wt` from main, through the dialog.
fn create(st: &mut AppState, folder: &Path, open: bool) {
    drive(st, Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Branch("main".into()))));
    let Some(Modal::NewWorktree(form)) = st.modal.as_mut() else { panic!("no Create worktree dialog") };
    form.folder = folder.display().to_string();
    form.folder_edited = true;
    form.branch = "wt".into();
    form.open = open;
    drive(st, Msg::Worktree(WorktreeMsg::Submit));
    assert!(st.modal.is_none(), "the dialog closed: {:?}", st.log.last().map(|l| &l.text));
}

fn has_branch(dir: &Path, name: &str) -> bool {
    git2::Repository::open(dir).unwrap().find_reference(&format!("refs/heads/{name}")).is_ok()
}

fn linked(dir: &Path) -> usize {
    glowfly_core::repo::worktrees_at(dir).unwrap().iter().filter(|w| !w.is_main()).count()
}

fn undo_label(st: &AppState) -> String {
    st.session.undo.undo_entry(Lang::En, true).0
}

fn cleanup(dir: &Path, folder: &Path) {
    if folder.exists() {
        git(dir, &["worktree", "remove", "--force", &folder.display().to_string()]);
    }
    let _ = std::fs::remove_dir_all(folder);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn undo_removes_the_worktree_and_its_branch_and_redo_makes_them_again() {
    let (mut st, dir, folder) = opened("undo-flow-wt");
    create(&mut st, &folder, false);
    assert!(folder.is_dir() && has_branch(&dir, "wt"));
    let name = folder.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(undo_label(&st), format!("Undo creation of worktree {name} with branch wt"));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(!folder.exists(), "the folder is gone");
    assert!(!has_branch(&dir, "wt"), "the unchanged branch is deleted");
    assert_eq!(linked(&dir), 0);

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(folder.is_dir() && has_branch(&dir, "wt"), "made again");
    assert_eq!(linked(&dir), 1);
    cleanup(&dir, &folder);
}

#[test]
fn undo_waits_while_the_worktree_is_open_in_a_tab() {
    let (mut st, dir, folder) = opened("undo-flow-wt-open");
    create(&mut st, &folder, true);
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1), "opened in a new tab");
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(undo_label(&st).starts_with("Undo creation of worktree"), "recorded in the original tab: {}", undo_label(&st));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(folder.is_dir(), "nothing removed");
    let line = st.log.last().map(|l| l.text.clone()).unwrap_or_default();
    assert!(line.ends_with("is open in a tab. Close that tab, then undo."), "{line}");

    drive(&mut st, Msg::Tabs(TabsMsg::CloseTab(1)));
    assert_eq!(st.tabs.len(), 1);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(!folder.exists() && !has_branch(&dir, "wt"), "removed once no tab shows it");
    cleanup(&dir, &folder);
}
