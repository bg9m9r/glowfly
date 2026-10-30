//! Headless tests for telling a fork's pull request from a same-named branch: the list's head
//! repository decides which remote the source is on (the code host's, the fork's when it is a
//! remote here, or none), for the PR menus, the panel's tips and checking the source out.
//! `origin` (github.com/o/r) and `ada` (github.com/ada/r) both have a `topic` branch.

use super::push_pr_fork_tests::{fetch_from, with_fork};
use super::push_pr_tests::{cleanup, commit_on, fetch};
use super::tests::{drive, git};
use crate::effect::{ForgeReply, ForgeResult};
use crate::msg::{AutoStashMsg, BranchMsg, ForgeMsg, Msg, RepoMsg};
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request_source::fork_known;
use crate::state::push_pr::open_pr;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::forge::{HeadRepo, PullRequest};
use std::path::PathBuf;

/// `with_fork` with `topic` on origin and a different `topic` on ada, both fetched; the oids of
/// origin/topic and ada/topic.
fn with_two_topics(name: &str) -> (AppState, [PathBuf; 3], Oid, Oid) {
    let (mut st, work, bare, ada) = with_fork(name);
    let ours = commit_on(&work, "topic", "ada-topic", "Ada's topic");
    git(&work, &["push", "-q", &bare.display().to_string(), "topic"]);
    git(&work, &["push", "-q", &ada.display().to_string(), "ada-topic:topic"]);
    git(&work, &["branch", "-q", "-D", "ada-topic"]);
    fetch(&work, &bare);
    fetch_from(&work, "ada", &ada);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let origin_topic =
        st.snap.as_ref().unwrap().remotes.iter().find(|(r, _)| r == "origin").unwrap().1.iter().find(|(b, _)| b == "topic").unwrap().1;
    (st, [work, bare, ada], origin_topic, ours)
}

fn listed(st: &mut AppState, prs: Vec<PullRequest>) {
    let target = st.session.forge.as_ref().unwrap().host.clone().unwrap();
    drive(st, Msg::Forge(ForgeMsg::ForgeReplied(ForgeReply { target, auth: None, results: vec![ForgeResult::Prs(Ok(prs))] })));
}

fn pr(id: u64, head_repo: HeadRepo) -> PullRequest {
    PullRequest {
        id,
        title: "Topic".into(),
        description: String::new(),
        source: "topic".into(),
        head_repo,
        target: "main".into(),
        author: "bob".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: format!("https://github.com/o/r/pull/{id}"),
        facets: Default::default(),
    }
}

fn source_tip(st: &AppState) -> Option<Oid> {
    st.session.pull_request.view.as_ref().unwrap().tips.source
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn done(dirs: [PathBuf; 3]) {
    cleanup(&dirs[0], &dirs[1]);
    let _ = std::fs::remove_dir_all(&dirs[2]);
}

#[test]
fn a_same_named_branch_is_not_a_forks_source() {
    let (mut st, dirs, _, _) = with_two_topics("fork-src-unknown");
    listed(&mut st, vec![pr(5, HeadRepo::Fork { name: "bob/r".into() })]);
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    assert!(fs.pr_for_branch("topic").is_none(), "origin/topic isn't bob's topic");
    assert!(fs.pr_from("ada", "topic").is_none());
    assert!(open_pr(snap, fs, "topic").is_none(), "the local topic has no pull request yet");
    assert_eq!(fs.source_remote(&fs.prs[0]), None);

    drive(&mut st, Msg::PullRequest(PullRequestMsg::Open(5)));
    assert_eq!(source_tip(&st), None, "origin/topic exists, but isn't the source");
    assert!(fork_known(st.session.pull_request.view.as_ref().unwrap()));

    drive(&mut st, Msg::Branch(BranchMsg::CheckoutPullRequest { id: 5 }));
    assert!(logged(&st, "#5 comes from a fork that isn't a remote here."));
    assert!(st.job.is_none(), "nothing checked out");

    // A deleted fork matches nothing either.
    listed(&mut st, vec![pr(5, HeadRepo::Deleted)]);
    assert!(st.session.forge.as_ref().unwrap().pr_for_branch("topic").is_none());
    done(dirs);
}

#[test]
fn a_forks_pull_request_comes_from_the_forks_remote() {
    let (mut st, dirs, origin_topic, ada_topic) = with_two_topics("fork-src-remote");
    listed(&mut st, vec![pr(6, HeadRepo::Fork { name: "Ada/R".into() })]);
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    assert_eq!(fs.pr_from("ada", "topic").map(|p| p.id), Some(6));
    assert!(fs.pr_for_branch("topic").is_none());
    assert_eq!(fs.source_remote(&fs.prs[0]).as_deref(), Some("ada"));
    assert!(open_pr(snap, fs, "topic").is_none(), "the local topic is pushed to origin");

    drive(&mut st, Msg::PullRequest(PullRequestMsg::Open(6)));
    assert_eq!(source_tip(&st), Some(ada_topic), "ada/topic, not origin/topic ({origin_topic})");

    // The local topic tracks nothing on ada: it is another branch with the same name.
    drive(&mut st, Msg::Branch(BranchMsg::CheckoutPullRequest { id: 6 }));
    assert!(logged(&st, "The local branch topic neither tracks nor pushes to ada/topic, where #6 comes from"));

    // Pushed to ada by config, the local topic's pull request is the fork's.
    git(&dirs[0], &["config", "branch.topic.pushRemote", "ada"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    assert_eq!(open_pr(snap, fs, "topic").map(|p| p.id), Some(6));
    // ...and it is the fork's branch, so checking the pull request out checks it out.
    let before = st.log.len();
    drive(&mut st, Msg::Branch(BranchMsg::CheckoutPullRequest { id: 6 }));
    assert_eq!(st.log.len(), before, "nothing refused: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    // The fixture leaves a.txt edited, so the checkout asks to stash it first.
    assert!(matches!(st.modal, Some(Modal::AutoStash(_))), "the checkout waits on the auto-stash prompt");
    drive(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("topic"));
    done(dirs);
}

#[test]
fn a_branch_tracking_the_forks_other_branch_is_refused() {
    let (mut st, dirs, _, _) = with_two_topics("fork-src-other");
    // The local topic tracks ada/other, with no push config: it isn't ada's topic.
    git(&dirs[0], &["push", "-q", &dirs[2].display().to_string(), "main:other"]);
    fetch_from(&dirs[0], "ada", &dirs[2]);
    git(&dirs[0], &["branch", "-q", "--set-upstream-to=ada/other", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    listed(&mut st, vec![pr(8, HeadRepo::Fork { name: "ada/r".into() })]);
    drive(&mut st, Msg::Branch(BranchMsg::CheckoutPullRequest { id: 8 }));
    assert!(logged(&st, "The local branch topic neither tracks nor pushes to ada/topic, where #8 comes from"));
    assert!(st.job.is_none() && st.modal.is_none(), "nothing checked out");
    done(dirs);
}

#[test]
fn a_pull_request_from_the_same_repository_keeps_the_code_hosts_branch() {
    let (mut st, dirs, origin_topic, _) = with_two_topics("fork-src-same");
    listed(&mut st, vec![pr(7, HeadRepo::Same)]);
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!(fs.pr_for_branch("topic").map(|p| p.id), Some(7));
    assert!(fs.pr_from("ada", "topic").is_none(), "ada/topic isn't its source");
    assert_eq!(fs.source_remote(&fs.prs[0]).as_deref(), Some("origin"));
    drive(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)));
    assert_eq!(source_tip(&st), Some(origin_topic));
    done(dirs);
}
