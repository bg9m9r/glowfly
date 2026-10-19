//! The left-panel section headers' + actions: each one's message opens its dialog, and each is
//! disabled with its reason (a running job, a clean tree, no commit yet, a detached HEAD, a
//! branch that already has a pull request).

use super::push_pr_tests::{cleanup as cleanup_host, with_host};
use super::rebase_tests::running_job;
use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::msg::{BranchMsg, ForgeMsg, Msg, RepoMsg, StashMsg};
use crate::state::section_actions::{
    self, NO_COMMIT_TAG, NO_COMMIT_WORKTREE, SectionAction, add_remote, add_submodule, add_worktree, create_pr, new_branch, new_tag, stash,
};
use crate::state::{AppState, Modal};
use crate::tag::TagMsg;
use crate::worktree::WorktreeMsg;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::PullRequest;
use trellis_core::i18n::Lang;
use trellis_core::stash_ops::StashScope;
use trellis_core::worktree_ops::WorktreeBase;

/// The one-commit fixture (a.txt modified), opened.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

/// Runs `a`'s message.
fn run(st: &mut AppState, a: &SectionAction) {
    update(st, a.msg(), Instant::now());
}

fn job_running() -> Option<String> {
    Some(Lang::En.tr("commit.blocked.job_running").to_owned())
}

#[test]
fn new_branch_opens_from_head() {
    let (mut st, dir) = opened("sa-branch");
    let a = new_branch(&st, Lang::En);
    assert_eq!((a.name.as_str(), a.label(), a.blocked.clone()), ("New branch", "New branch…".to_owned(), None));
    assert!(matches!(a.msg(), Msg::Branch(BranchMsg::NewBranch { start: None, ref name }) if name.is_empty()));
    run(&mut st, &a);
    assert!(matches!(st.modal, Some(Modal::NewBranch { start: None, .. })));
    st.modal = None;
    st.job = Some(running_job(&dir));
    assert_eq!(new_branch(&st, Lang::En).blocked, job_running());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn add_remote_and_submodule_open_their_dialogs() {
    let (mut st, dir) = opened("sa-remote");
    assert!(add_remote(Lang::En).blocked.is_none() && add_submodule(Lang::En).blocked.is_none());
    run(&mut st, &add_remote(Lang::En));
    assert!(matches!(&st.modal, Some(Modal::Remote(f)) if f.original.is_none()), "the add form");
    st.modal = None;
    run(&mut st, &add_submodule(Lang::En));
    assert!(matches!(st.modal, Some(Modal::AddSubmodule(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_tag_is_on_head() {
    let (mut st, dir) = opened("sa-tag");
    let head = st.snap.as_ref().unwrap().head_oid.unwrap().to_string();
    let a = new_tag(&st, Lang::En, false);
    assert!(a.blocked.is_none());
    assert!(matches!(a.msg(), Msg::Tag(TagMsg::NewTag { ref at, annotated: false }) if *at == head));
    run(&mut st, &a);
    assert!(matches!(&st.modal, Some(Modal::NewTag(f)) if !f.annotated && f.at == head));
    st.modal = None;
    let annotated = new_tag(&st, Lang::En, true);
    run(&mut st, &annotated);
    assert!(matches!(&st.modal, Some(Modal::NewTag(f)) if f.annotated));
    st.snap.as_mut().unwrap().head_oid = None;
    for annotated in [false, true] {
        assert_eq!(new_tag(&st, Lang::En, annotated).blocked.as_deref(), Some(NO_COMMIT_TAG));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stash_needs_changes_and_no_job() {
    let (mut st, dir) = opened("sa-stash");
    let a = stash(&st, Lang::En, StashScope::All);
    assert!(a.blocked.is_none(), "{:?}", a.blocked);
    assert!(matches!(a.msg(), Msg::Stash(StashMsg::PushDialog { scope: StashScope::All, path: None })));
    run(&mut st, &a);
    assert!(matches!(&st.modal, Some(Modal::StashPush(f)) if f.scope == StashScope::All));
    st.modal = None;
    assert!(matches!(stash(&st, Lang::En, StashScope::Staged).msg(), Msg::Stash(StashMsg::PushDialog { scope: StashScope::Staged, .. })));

    st.job = Some(running_job(&dir));
    assert_eq!(stash(&st, Lang::En, StashScope::All).blocked, job_running());
    st.job = None;
    let snap = st.snap.as_mut().unwrap();
    snap.staged.clear();
    snap.unstaged.clear();
    let clean = Some(Lang::En.tr("toolbar.tip.blocked.no_changes").to_owned());
    assert_eq!(stash(&st, Lang::En, StashScope::All).blocked, clean);
    assert_eq!(stash(&st, Lang::En, StashScope::Staged).blocked, clean);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn add_worktree_starts_from_head() {
    let (mut st, dir) = opened("sa-worktree");
    let a = add_worktree(&st, Lang::En);
    assert!(a.blocked.is_none());
    let base = WorktreeBase::Branch("main".into());
    assert!(matches!(a.msg(), Msg::Worktree(WorktreeMsg::CreateDialog(ref b)) if *b == base));
    run(&mut st, &a);
    assert!(matches!(&st.modal, Some(Modal::NewWorktree(f)) if f.base == base));
    st.modal = None;

    let snap = st.snap.as_mut().unwrap();
    let head = snap.head_oid.unwrap();
    snap.head_branch = None;
    assert_eq!(section_actions::worktree_base(snap), Some(WorktreeBase::Commit(head)));
    assert!(matches!(add_worktree(&st, Lang::En).msg(), Msg::Worktree(WorktreeMsg::CreateDialog(WorktreeBase::Commit(o))) if o == head));
    st.snap.as_mut().unwrap().head_oid = None;
    assert_eq!(add_worktree(&st, Lang::En).blocked.as_deref(), Some(NO_COMMIT_WORKTREE));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_pr_needs_a_host_a_branch_and_no_open_pr() {
    let (mut st, work, bare) = with_host("sa-pr");
    let pr = |st: &AppState| create_pr(st, Lang::En, st.forge.as_ref().unwrap());
    let a = pr(&st).expect("a code host");
    assert_eq!((a.name.as_str(), a.blocked.clone()), ("Create pull request", None));
    assert!(matches!(a.msg(), Msg::Forge(ForgeMsg::CreatePrDialog { ref source, target: None }) if source == "main"));
    run(&mut st, &a);
    assert!(matches!(&st.modal, Some(Modal::CreatePr { source, .. }) if source == "main"));
    st.modal = None;

    st.forge.as_mut().unwrap().prs.push(PullRequest {
        id: 7,
        title: "Main work".into(),
        description: String::new(),
        source: "main".into(),
        target: "develop".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: "u".into(),
        facets: Default::default(),
    });
    assert_eq!(pr(&st).unwrap().blocked.as_deref(), Some("main already has pull request #7."));

    st.snap.as_mut().unwrap().head_branch = None;
    let detached = Lang::En.tr("toolbar.tip.blocked.detached_pull_request");
    assert_eq!(pr(&st).unwrap().blocked.as_deref(), Some(detached));

    st.forge.as_mut().unwrap().host = None;
    assert!(pr(&st).is_none(), "no code host");
    cleanup_host(&work, &bare);
}
