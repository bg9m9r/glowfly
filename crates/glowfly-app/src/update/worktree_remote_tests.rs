//! Headless tests for worktrees made from a remote branch or a pull request: the Create dialog
//! prefilled with the remote branch's name, `git worktree add --track -b` creating a branch that
//! tracks it, the refusals for its name and for a remote branch that is gone, and the pull
//! request entry choosing the local branch or the remote one (or refusing a fork's branch).

use super::branch_tests::with_origin;
use super::tests::{drive, git};
use super::update;
use crate::msg::{Msg, RepoMsg};
use crate::state::worktrees::{WorktreeForm, branch_problem};
use crate::state::{AppState, ForgeState, Modal};
use crate::worktree::WorktreeMsg;
use glowfly_core::forge::{Provider, Target};
use glowfly_core::worktree_ops::WorktreeBase;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn origin_feature() -> WorktreeBase {
    WorktreeBase::Remote { remote: "origin".into(), branch: "feature".into() }
}

fn wt_root(dir: &Path) -> PathBuf {
    dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()))
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(wt_root(d));
        let _ = std::fs::remove_dir_all(d);
    }
}

fn form(st: &mut AppState) -> &mut WorktreeForm {
    match st.modal.as_mut() {
        Some(Modal::NewWorktree(f)) => f,
        _ => panic!("the Create worktree dialog is not open: {:?}", st.log.last().map(|l| &l.text)),
    }
}

/// `with_origin` with local `feature` deleted (origin/feature stays) and local `mine` tracking
/// origin/feature.
fn only_on_origin(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    let _ = std::fs::remove_dir_all(wt_root(&work));
    git(&work, &["branch", "-q", "-D", "feature"]);
    git(&work, &["branch", "-q", "--track", "mine", "origin/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, bare)
}

#[test]
fn create_from_a_remote_branch_tracks_it() {
    let (mut st, work, bare) = only_on_origin("wtr-create");
    drive(&mut st, Msg::Worktree(WorktreeMsg::CreateDialog(origin_feature())));
    let f = form(&mut st);
    assert_eq!(f.branch, "feature", "prefilled with the remote branch's name");
    let folder = PathBuf::from(&f.folder);
    let root_name = wt_root(&work).file_name().unwrap().to_owned();
    assert!(folder.ends_with(Path::new(&root_name).join("feature")), "{}", f.folder);
    f.open = false;

    let snap = st.snap.as_ref().unwrap();
    let problem = |name: &str| branch_problem(snap, &origin_feature(), name);
    assert_eq!(problem(""), Some("Enter a name for the new local branch.".to_owned()));
    assert_eq!(
        problem("mine"),
        Some("mine already tracks origin/feature. Create the worktree from mine instead, or enter another name.".to_owned())
    );
    assert!(problem("main").is_some_and(|p| p.contains("main")), "an existing branch");
    assert_eq!(problem("feature"), None);
    let gone = WorktreeBase::Remote { remote: "origin".into(), branch: "gone".into() };
    assert_eq!(branch_problem(snap, &gone, "gone"), Some("origin/gone no longer exists. Fetch, then try again.".to_owned()));

    drive(&mut st, Msg::Worktree(WorktreeMsg::Submit));
    assert!(st.modal.is_none(), "{:?}", st.log.last().map(|l| &l.text));
    let log = st.log.iter().rev().find(|l| l.text.starts_with("Create worktree")).map(|l| l.text.clone()).unwrap();
    assert!(log.contains("worktree add --track -b feature"), "{log}");
    assert!(log.contains("refs/remotes/origin/feature"), "{log}");
    let repo = git2::Repository::open(&work).unwrap();
    let config = repo.config().unwrap().snapshot().unwrap();
    assert_eq!(config.get_str("branch.feature.remote").unwrap(), "origin");
    assert_eq!(config.get_str("branch.feature.merge").unwrap(), "refs/heads/feature");
    let listed = st.snap.as_ref().unwrap().worktrees.iter().any(|w| w.branch.as_deref() == Some("feature"));
    assert!(listed && folder.join("a.txt").exists());
    cleanup(&[&work, &bare]);
}

#[test]
fn a_refused_name_starts_nothing() {
    let (mut st, work, bare) = only_on_origin("wtr-refused");
    drive(&mut st, Msg::Worktree(WorktreeMsg::CreateDialog(origin_feature())));
    form(&mut st).branch = "mine".into();
    let fx = update(&mut st, Msg::Worktree(WorktreeMsg::Submit), Instant::now());
    assert!(fx.is_empty());
    assert!(st.log.last().unwrap().text.starts_with("mine already tracks origin/feature."));
    cleanup(&[&work, &bare]);
}

fn on_github(st: &mut AppState) {
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    st.forge = Some(ForgeState::new(Some(target), "origin".into(), None));
}

#[test]
fn create_for_a_pull_request() {
    let (mut st, work, bare) = with_origin("wtr-pr");
    git(&work, &["push", "-q", "origin", "main:only-remote"]);
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    on_github(&mut st);
    let pr = |source: &str| Msg::Worktree(WorktreeMsg::CreateForPr { id: 12, source: source.into() });

    drive(&mut st, pr("only-remote"));
    let f = form(&mut st).clone();
    assert_eq!(f.base, WorktreeBase::Remote { remote: "origin".into(), branch: "only-remote".into() });
    assert_eq!(f.title.as_deref(), Some("Create worktree for pull request #12"));
    assert_eq!(f.branch, "only-remote");
    st.modal = None;

    drive(&mut st, pr("feature"));
    assert_eq!(form(&mut st).base, WorktreeBase::Branch("feature".into()), "the local branch");
    st.modal = None;

    drive(&mut st, pr("nope"));
    assert!(st.modal.is_none());
    assert_eq!(
        st.log.last().unwrap().text,
        "Can't create a worktree for pull request #12: nope isn't a local branch or on origin (it may be on a fork). Fetch it first."
    );
    drive(&mut st, pr("-x"));
    assert!(st.modal.is_none());
    assert!(st.log.last().unwrap().text.ends_with("\"-x\" isn't a branch name Trellis can check out."));
    cleanup(&[&work, &bare]);
}
