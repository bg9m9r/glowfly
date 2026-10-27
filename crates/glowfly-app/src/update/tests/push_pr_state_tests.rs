//! Where a branch stands on the code host, the target picked before a menu opened, and the
//! palette's pull request entries, against the `push_pr_tests` fixture.

use super::branch_tests::remote_commit;
use super::push_pr_tests::{cleanup, commit_on, fetch, with_host};
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::RepoMsg;
use crate::state::palette::{PaletteCommand, entries};
use crate::state::push_pr::{self, HostBranch};
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::forge::PullRequest;
use glowfly_core::repo::BranchInfo;
use std::time::Instant;

fn branch(st: &AppState, name: &str) -> BranchInfo {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).expect("branch").clone()
}

fn state_of(st: &AppState, name: &str) -> Option<HostBranch> {
    push_pr::host_branch(&branch(st, name), st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap())
}

fn target(st: &AppState, sel: Option<Oid>, name: &str) -> Option<String> {
    let sel = sel.map(Selection::Commit).unwrap_or(Selection::None);
    push_pr::chosen_target(&sel, &branch(st, name), st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap())
}

fn tip(st: &AppState, name: &str) -> Oid {
    branch(st, name).oid.unwrap()
}

#[test]
fn host_branch_states() {
    let (mut st, work, bare) = with_host("pr-host-states");
    assert_eq!(state_of(&st, "topic"), Some(HostBranch::Missing));
    assert_eq!(state_of(&st, "feature"), Some(HostBranch::Pushed));
    commit_on(&work, "feature", "feature", "More feature");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(state_of(&st, "feature"), Some(HostBranch::Ahead(1)));
    remote_commit(&bare, "feature", "remote.txt");
    fetch(&work, &bare);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(matches!(state_of(&st, "feature"), Some(HostBranch::Blocked(why)) if why.contains("is behind")));
    for (h, push) in [
        (Some(HostBranch::Missing), true),
        (Some(HostBranch::Ahead(2)), true),
        (Some(HostBranch::Pushed), false),
        (Some(HostBranch::Blocked(String::new())), false),
        (None, false),
    ] {
        assert_eq!(push_pr::needs_push(&h), push, "{h:?}");
    }
    // The host remote's branches aren't loaded.
    st.snap.as_mut().unwrap().remotes.clear();
    assert_eq!(state_of(&st, "topic"), None);
    cleanup(&work, &bare);
}

#[test]
fn chosen_target_rules() {
    let (mut st, work, bare) = with_host("pr-chosen-target");
    let main = tip(&st, "main");
    // main and feature share a tip: the default branch, else the first by name.
    assert_eq!(target(&st, Some(main), "topic"), Some("feature".into()));
    st.session.forge.as_mut().unwrap().default_branch = Some("main".into());
    assert_eq!(target(&st, Some(main), "topic"), Some("main".into()));
    st.session.forge.as_mut().unwrap().default_branch = None;
    // Move feature on (pushed): main's tip is main alone.
    commit_on(&work, "feature", "feature", "More feature");
    git(&work, &["push", "-q", "origin", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(target(&st, Some(main), "topic"), Some("main".into()));
    assert_eq!(target(&st, None, "topic"), None);
    // The branch's own tip, and its own upstream's (named differently), aren't targets.
    let topic = tip(&st, "topic");
    assert_eq!(target(&st, Some(topic), "topic"), None);
    git(&work, &["branch", "-q", "--track", "wip", "origin/feature"]);
    commit_on(&work, "wip", "wip", "Work in progress");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let feature = tip(&st, "feature");
    assert_eq!(target(&st, Some(feature), "wip"), None);
    assert_eq!(target(&st, Some(main), "wip"), Some("main".into()));
    // A remote branch as the source.
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    let remote = |sel: Oid| push_pr::chosen_target_for(&Selection::Commit(sel), "feature", Some(feature), None, snap, fs);
    assert_eq!(remote(main), Some("main".into()));
    assert_eq!(remote(feature), None);
    cleanup(&work, &bare);
}

fn pr_commands(st: &AppState) -> Vec<PaletteCommand> {
    entries(st)
        .into_iter()
        .map(|e| e.command)
        .filter(|c| matches!(c, PaletteCommand::CreatePr(_) | PaletteCommand::PushAndCreatePr(_)))
        .collect()
}

fn pr_for(source: &str) -> PullRequest {
    PullRequest {
        id: 7,
        title: "Topic".into(),
        description: String::new(),
        source: source.into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

#[test]
fn palette_pr_entries() {
    let (mut st, work, bare) = with_host("pr-palette");
    git(&work, &["checkout", "-q", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(pr_commands(&st), [PaletteCommand::CreatePr("topic".into()), PaletteCommand::PushAndCreatePr("topic".into())]);
    st.session.forge.as_mut().unwrap().prs = vec![pr_for("topic")];
    assert!(pr_commands(&st).is_empty());
    st.session.forge.as_mut().unwrap().prs.clear();
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::PushAndCreatePr("topic".into()))), Instant::now());
    assert_eq!(fx.iter().filter(|e| matches!(e, Effect::RunGit { .. })).count(), 1, "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Push"));
    st.job = None;
    git(&work, &["checkout", "-q", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(pr_commands(&st), [PaletteCommand::CreatePr("feature".into())]);
    st.session.forge = None;
    assert!(pr_commands(&st).is_empty());
    cleanup(&work, &bare);
}
#[test]
fn open_pr_is_found_by_the_upstream_name() {
    let (mut st, work, bare) = with_host("pr-open-by-upstream");
    git(&work, &["checkout", "-q", "-b", "wip", "--track", "origin/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.session.forge.as_mut().unwrap().prs = vec![pr_for("feature")];
    let (snap, fs) = (st.snap.as_ref().unwrap(), st.session.forge.as_ref().unwrap());
    assert_eq!(push_pr::open_pr(snap, fs, "wip").map(|p| p.id), Some(7), "wip is feature on the host");
    assert!(push_pr::open_pr(snap, fs, "topic").is_none());
    assert!(matches!(push_pr::push_item(snap, fs), Err(why) if why.contains("already has")));
    assert!(pr_commands(&st).is_empty(), "no create entries for a branch with an open pull request");
    cleanup(&work, &bare);
}
