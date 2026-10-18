//! One repository under two names: a short 8.3 name or a link (a junction on Windows, a symlink
//! elsewhere, like macOS's `/var` for `/private/var`) and the resolved path libgit2 reports.
//! Whatever spelling a path came in by, the app treats it as the folder it names.

use super::tests::{drive, git, scratch_repo, state};
use super::trust_tests::{done, job, untrusted_repo};
use crate::msg::{GraphMsg, Msg, RepoMsg, TabsMsg};
use crate::state::palette::entries;
use crate::state::repo_lists::is_missing;
use crate::state::tabs::is_repo_at;
use crate::state::trust::{Created, recorded};
use crate::worktree::WorktreeMsg;
use std::path::{Path, PathBuf};
use trellis_core::paths::same_spelling;
use trellis_core::settings::{FilterOp, RepoTrust};

/// A second name for `dir`, made next to it: a symlink, or on Windows a junction where the
/// account may not make symlinks. `None` where neither can be made.
pub(super) fn alias_of(dir: &Path) -> Option<PathBuf> {
    let link = dir.with_file_name(format!("{}-alias", dir.file_name()?.to_string_lossy()));
    let _ = std::fs::remove_dir(&link);
    let _ = std::fs::remove_file(&link);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(dir, &link).is_ok()
        || std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(&link).arg(dir).output().is_ok_and(|o| o.status.success());
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(dir, &link).is_ok();
    made.then_some(link)
}

fn remove(dir: &Path, alias: &Path) {
    let _ = std::fs::remove_dir(alias).or_else(|_| std::fs::remove_file(alias));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn opening_by_another_name_replaces_that_name_in_the_lists() {
    let dir = scratch_repo("alias-open");
    let alias = alias_of(&dir).expect("made a link");
    let mut st = state();
    st.recent = vec![alias.clone(), "C:/elsewhere/other-repo".into()];
    st.missing_repos = vec![alias.clone()];
    drive(&mut st, Msg::Repo(RepoMsg::Open(alias.clone())));
    let snap = st.snap.as_ref().expect("opened");
    assert!(is_repo_at(snap, &alias) && is_repo_at(snap, &dir), "both names are this repository");
    assert!(!is_missing(&st, &alias), "{:?}", st.missing_repos);
    assert_eq!(st.recent.iter().filter(|p| is_repo_at(snap, p)).count(), 1, "listed once: {:?}", st.recent);

    // A recent entry still spelled the other way isn't offered as another repository.
    st.recent.push(alias.clone());
    let opens: Vec<String> =
        entries(&st).into_iter().map(|e| e.label).filter(|l| l.starts_with("Open ") && l.ends_with("-alias")).collect();
    assert!(opens.is_empty(), "{opens:?}");
    remove(&dir, &alias);
}

#[test]
fn a_clone_made_under_another_name_is_trusted() {
    let dir = untrusted_repo("alias-clone");
    let alias = alias_of(&dir).expect("made a link");
    let mut st = state();
    st.job = Some(job((alias.clone(), Created::Clone)));
    done(&mut st, true, "$ git clone https://github.com/o/r.git");
    let snap = st.snap.as_ref().expect("the clone opened");
    assert!(st.modal.is_none(), "not asked about a repository Trellis made");
    assert_eq!(recorded(&st.settings, &snap.real_workdir), Some(RepoTrust::Trusted));
    assert!(st.trust.created.is_empty());
    remove(&dir, &alias);
}

#[test]
fn the_loaded_repository_knows_the_name_it_was_opened_by() {
    let dir = scratch_repo("alias-snapshot");
    let alias = alias_of(&dir).expect("made a link");
    let snap = trellis_core::repo::load(&alias, &trellis_core::repo::LoadOptions::all(10)).unwrap();
    assert!(same_spelling(&snap.real_workdir, &trellis_core::paths::resolve(&dir)));
    assert!(is_repo_at(&snap, &alias));
    // A folder inside the repository opens it, but is not another name for it.
    let inner = dir.join("sub");
    std::fs::create_dir_all(&inner).unwrap();
    let snap = trellis_core::repo::load(&inner, &trellis_core::repo::LoadOptions::all(10)).unwrap();
    assert!(!is_repo_at(&snap, &inner));
    remove(&dir, &alias);
}

#[test]
fn a_tab_opened_by_another_name_is_the_same_tab() {
    let dir = scratch_repo("alias-tab");
    let alias = alias_of(&dir).expect("made a link");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    drive(&mut st, Msg::Repo(RepoMsg::Open(alias.clone())));
    assert_eq!(st.tabs.len(), 1, "one tab: {:?}", st.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>());
    assert_eq!(st.tabs[0].folder, st.snap.as_ref().unwrap().folder);
    assert!(st.tabs[0].folder.is_some());
    // Now known, the alias selects the tab straight away.
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(st.tab_showing(&alias), Some(0));
    remove(&dir, &alias);
}

#[test]
fn a_graph_filter_set_through_an_alias_applies_under_the_real_path() {
    let dir = scratch_repo("alias-filter");
    git(&dir, &["branch", "feature"]);
    let alias = alias_of(&dir).expect("made a link");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(alias.clone())));
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Hide("refs/heads/feature".into()))));
    // A fresh start that opens the folder by its own name.
    let mut again = state();
    again.settings = st.settings.clone();
    drive(&mut again, Msg::Repo(RepoMsg::Open(dir.clone())));
    let snap = again.snap.as_ref().unwrap();
    assert_eq!(again.settings.graph_filter_for(&snap.real_workdir).hidden, ["refs/heads/feature"]);
    assert_eq!(snap.loaded_with.hidden, ["refs/heads/feature"], "the load used it");
    assert_eq!(again.settings.graph_filters.len(), 1, "{:?}", again.settings.graph_filters.keys());
    remove(&dir, &alias);
}

#[test]
fn a_worktree_open_by_another_name_closes_that_tab_on_remove() {
    let dir = scratch_repo("alias-wt");
    let linked = dir.with_file_name(format!("{}-linked", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    let alias = alias_of(&linked).expect("made a link");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    drive(&mut st, Msg::Repo(RepoMsg::Open(alias.clone())));
    assert_eq!(st.tabs.len(), 2);
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    drive(&mut st, Msg::Worktree(WorktreeMsg::RemoveDialog(linked.clone())));
    match &st.modal {
        Some(crate::state::Modal::RemoveWorktree(f)) => assert!(f.close_tabs, "the tab is found by its identity"),
        _ => panic!("no Remove dialog: {:?}", st.log.last().map(|l| &l.text)),
    }
    drive(&mut st, Msg::Worktree(WorktreeMsg::Remove));
    assert_eq!(st.tabs.len(), 1, "the tab showing it closed first");
    assert!(!linked.exists(), "{:?}", st.log.last().map(|l| &l.text));
    let _ = std::fs::remove_dir(&alias).or_else(|_| std::fs::remove_file(&alias));
    let _ = std::fs::remove_dir_all(&linked);
    let _ = std::fs::remove_dir_all(&dir);
}
