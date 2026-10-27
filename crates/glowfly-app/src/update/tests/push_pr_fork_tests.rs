//! Headless tests for pull requests across forks: the PR repositories detect finds, the create
//! dialog's From / Into defaults and picks, pushes to a fork, the request sent to the Into
//! repository, and replies from a repository that isn't the code host's. `origin` and `ada`
//! read as GitHub while pushes go to bare repositories next to the working one.

use super::push_pr_tests::{cleanup, commit_on, with_host};
use super::tests::{drive, git};
use super::update;
use crate::effect::{Effect, ForgeCall, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg, RepoMsg};
use crate::state::pr_repos::{PrRepoPick, RepoSide};
use crate::state::push_pr::PrStart;
use crate::state::{AppState, Modal};
use glowfly_core::forge::{NewPullRequest, PullRequest, Target, parse_remote};
use glowfly_core::gitcli::args;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A bare repository named after `work` and `suffix`, with `main` pushed to it.
fn bare_next_to(work: &Path, suffix: &str) -> PathBuf {
    let bare = work.with_file_name(format!("{}-{suffix}.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(work, &["push", "-q", &bare.display().to_string(), "main"]);
    bare
}

/// Adds remote `name` reading as `url`, pushing to `bare`, and fetches it.
fn add_remote(work: &Path, name: &str, url: &str, bare: &Path) {
    git(work, &["remote", "add", name, url]);
    git(work, &["config", &format!("remote.{name}.pushurl"), &bare.display().to_string()]);
    fetch_from(work, name, bare);
}

fn fetch_from(work: &Path, name: &str, bare: &Path) {
    git(work, &["fetch", "-q", &bare.display().to_string(), &format!("+refs/heads/*:refs/remotes/{name}/*")]);
}

/// `with_host` plus `ada` (github.com/ada/r, the user's fork) with main fetched.
pub(super) fn with_fork(name: &str) -> (AppState, PathBuf, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_host(name);
    let ada = bare_next_to(&work, "ada");
    add_remote(&work, "ada", "https://github.com/ada/r.git", &ada);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, bare, ada)
}

fn t(url: &str) -> Target {
    parse_remote(url, &[]).unwrap()
}

fn msg(m: ForgeMsg) -> Msg {
    Msg::Forge(m)
}

/// The open create dialog's (source, target, targets, repos).
fn dialog(st: &AppState) -> Option<(String, String, Vec<String>, PrRepoPick)> {
    match &st.modal {
        Some(Modal::CreatePr { source, target, targets, repos, .. }) => {
            Some((source.clone(), target.clone(), targets.clone(), repos.clone()))
        }
        _ => None,
    }
}

fn repos(st: &AppState) -> (String, String) {
    let r = dialog(st).expect("the create dialog").3;
    (r.from, r.into)
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn run_git(fx: &[Effect]) -> Vec<&Vec<Vec<String>>> {
    fx.iter().filter_map(|e| if let Effect::RunGit { cmds, .. } = e { Some(cmds) } else { None }).collect()
}

fn new_pr(source: &str) -> NewPullRequest {
    NewPullRequest {
        source: source.into(),
        target: "main".into(),
        title: "T".into(),
        description: String::new(),
        draft: false,
        issues: Vec::new(),
        source_repo: None,
    }
}

fn create(pr: NewPullRequest, from: &str, into: &str) -> Msg {
    msg(ForgeMsg::CreatePr { pr, from: from.into(), into: into.into() })
}

/// The (target, source_repo) of each create request among `fx`.
fn creates(fx: &[Effect]) -> Vec<(Target, Option<Target>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Forge { target, call: ForgeCall::Create(pr), .. } => Some((target.clone(), pr.source_repo.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn detect_lists_the_fork_after_the_code_host() {
    let (st, work, bare, ada) = with_fork("pr-fork-detect");
    let fs = st.session.forge.as_ref().unwrap();
    let found: Vec<(&str, Target)> = fs.pr_repos.iter().map(|r| (r.remote.as_str(), r.target.clone())).collect();
    assert_eq!(found, [("origin", t("https://github.com/o/r")), ("ada", t("https://github.com/ada/r"))]);
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

#[test]
fn a_fork_remote_row_opens_the_dialog_into_the_code_host() {
    let (mut st, work, bare, ada) = with_fork("pr-fork-from-row");
    commit_on(&work, "main", "fork-topic", "Fork topic");
    git(&work, &["push", "-q", &ada.display().to_string(), "fork-topic"]);
    fetch_from(&work, "ada", &ada);
    git(&work, &["branch", "-D", "fork-topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, msg(ForgeMsg::CreatePrFrom { remote: "ada".into(), source: "fork-topic".into() }));
    let (source, target, targets, picked) = dialog(&st).expect("the create dialog");
    assert_eq!((source.as_str(), target.as_str()), ("fork-topic", "main"));
    assert_eq!(targets, ["feature", "main"], "the code host's branches");
    assert_eq!((picked.from.as_str(), picked.into.as_str()), ("ada", "origin"));
    assert_eq!(st.modal.as_ref().map(|m| matches!(m, Modal::CreatePr { title, .. } if title == "Fork topic")), Some(true));
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

#[test]
fn a_fork_on_origin_opens_into_upstream() {
    let (mut st, work, bare) = with_host("pr-fork-case-b");
    git(&work, &["remote", "set-url", "origin", "https://github.com/ada/r.git"]);
    let up = bare_next_to(&work, "upstream");
    commit_on(&work, "main", "next", "Next");
    git(&work, &["push", "-q", &up.display().to_string(), "next"]);
    add_remote(&work, "upstream", "https://github.com/o/r.git", &up);
    git(&work, &["push", "-q", "-u", "origin", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!((fs.remote.as_str(), fs.pr_repos.len()), ("origin", 2));
    drive(&mut st, msg(ForgeMsg::CreatePrDialog { source: "topic".into(), target: None }));
    assert_eq!(repos(&st), ("origin".into(), "upstream".into()));
    let (_, target, targets, _) = dialog(&st).unwrap();
    assert_eq!((target.as_str(), targets), ("main", vec!["main".to_owned(), "next".to_owned()]));

    // The pull request is created on upstream from the fork; its reply is logged, not listed.
    let fx = update(&mut st, create(new_pr("topic"), "origin", "upstream"), Instant::now());
    assert_eq!(creates(&fx), [(t("https://github.com/o/r"), Some(t("https://github.com/ada/r")))]);
    let reply = |target: Target, results| msg(ForgeMsg::ForgeReplied(ForgeReply { target, auth: None, results }));
    let pr = PullRequest {
        id: 12,
        title: "Topic".into(),
        description: String::new(),
        source: "topic".into(),
        target: "main".into(),
        author: String::new(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: "https://github.com/o/r/pull/12".into(),
        facets: Default::default(),
    };
    let upstream = t("https://github.com/o/r");
    update(&mut st, reply(upstream.clone(), vec![ForgeResult::Created(Ok(Box::new(pr.clone())))]), Instant::now());
    assert!(logged(&st, "Created pull request #12 Topic"));
    assert!(st.session.forge.as_ref().unwrap().prs.is_empty(), "the list is origin's");
    update(&mut st, reply(upstream, vec![ForgeResult::Prs(Ok(vec![pr.clone()]))]), Instant::now());
    assert!(st.session.forge.as_ref().unwrap().prs.is_empty(), "other results from upstream are ignored");
    let before = st.log.len();
    update(&mut st, reply(t("https://github.com/x/y"), vec![ForgeResult::Created(Ok(Box::new(pr)))]), Instant::now());
    assert_eq!(st.log.len(), before, "a repository that isn't a remote any more is dropped");
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&up);
}

#[test]
fn create_goes_to_the_into_repository() {
    let (mut st, work, bare, ada) = with_fork("pr-fork-create");
    git(&work, &["push", "-q", &ada.display().to_string(), "topic"]);
    fetch_from(&work, "ada", &ada);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, create(new_pr("topic"), "ada", "origin"), Instant::now());
    assert_eq!(creates(&fx), [(t("https://github.com/o/r"), Some(t("https://github.com/ada/r")))]);
    let fx = update(&mut st, create(new_pr("main"), "ada", "ada"), Instant::now());
    assert_eq!(creates(&fx), [(t("https://github.com/ada/r"), None)]);
    // A source that isn't on the From remote is refused.
    let fx = update(&mut st, create(new_pr("feature"), "ada", "origin"), Instant::now());
    assert!(creates(&fx).is_empty());
    assert!(logged(&st, "feature isn't on ada yet"));
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

#[test]
fn picking_a_repository_reloads_the_targets() {
    let (mut st, work, bare, ada) = with_fork("pr-fork-pick");
    commit_on(&work, "main", "wip", "Wip");
    git(&work, &["push", "-q", &ada.display().to_string(), "wip"]);
    fetch_from(&work, "ada", &ada);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, msg(ForgeMsg::CreatePrDialog { source: "topic".into(), target: Some("feature".into()) }));
    assert_eq!(dialog(&st).map(|d| d.1), Some("feature".into()));
    drive(&mut st, msg(ForgeMsg::PickPrRepo { side: RepoSide::Into, remote: "ada".into() }));
    let (_, target, targets, _) = dialog(&st).unwrap();
    assert_eq!((target.as_str(), targets), ("main", vec!["main".to_owned(), "wip".to_owned()]), "feature isn't on ada");
    drive(&mut st, msg(ForgeMsg::PickPrRepo { side: RepoSide::From, remote: "ada".into() }));
    assert_eq!(repos(&st), ("ada".into(), "ada".into()));
    let before = dialog(&st);
    drive(&mut st, msg(ForgeMsg::PickPrRepo { side: RepoSide::Into, remote: "elsewhere".into() }));
    assert_eq!(dialog(&st), before, "an unknown remote changes nothing");
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

#[test]
fn push_to_the_from_remote_then_the_dialog_opens_from_it() {
    let (mut st, work, bare, ada) = with_fork("pr-fork-push");
    let fx = update(&mut st, msg(ForgeMsg::PushToForPr { branch: "topic".into(), remote: "ada".into(), target: None }), Instant::now());
    assert_eq!(run_git(&fx), [&vec![args(&["push", "-u", "ada", "refs/heads/topic:refs/heads/topic"])]]);
    let then = PrStart { source: "topic".into(), target: None, remote: Some("ada".into()) };
    assert_eq!(st.job.as_ref().and_then(|j| j.then_pr.clone()), Some(then));
    st.job = None;
    drive(&mut st, msg(ForgeMsg::PushToForPr { branch: "topic".into(), remote: "ada".into(), target: None }));
    assert_eq!(repos(&st), ("ada".into(), "origin".into()));
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

#[test]
fn a_branch_tracking_the_fork_is_pushed_there() {
    let (mut st, work, bare, ada) = with_fork("pr-fork-tracking");
    git(&work, &["push", "-q", "-u", "ada", "topic"]);
    commit_on(&work, "topic", "topic", "More topic");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, msg(ForgeMsg::PushAndStartPr { branch: "topic".into(), target: None }), Instant::now());
    assert_eq!(run_git(&fx), [&vec![args(&["push", "ada", "refs/heads/topic:refs/heads/topic"])]]);
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

#[test]
fn a_stack_layer_on_the_fork_targets_the_fork() {
    let (mut st, work, bare, ada) = with_fork("pr-fork-stack");
    commit_on(&work, "main", "layer1", "Layer one");
    commit_on(&work, "layer1", "layer2", "Layer two");
    git(&work, &["push", "-q", "-u", "ada", "layer1", "layer2"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, msg(ForgeMsg::CreatePrDialog { source: "layer2".into(), target: Some("layer1".into()) }));
    assert_eq!(repos(&st), ("ada".into(), "ada".into()));
    assert_eq!(dialog(&st).map(|d| d.1), Some("layer1".into()));
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&ada);
}

/// origin is the user's fork (the code host's remote), upstream the base repository.
fn with_upstream(name: &str) -> (AppState, PathBuf, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_host(name);
    git(&work, &["remote", "set-url", "origin", "https://github.com/ada/r.git"]);
    let up = bare_next_to(&work, "upstream");
    add_remote(&work, "upstream", "https://github.com/o/r.git", &up);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.session.forge.as_ref().map(|f| f.pr_repos.len()), Some(2));
    (st, work, bare, up)
}

#[test]
fn a_branch_started_from_upstream_is_pushed_to_the_code_hosts_remote() {
    let (mut st, work, bare, up) = with_upstream("pr-fork-from-base");
    commit_on(&work, "main", "started", "Started");
    git(&work, &["branch", "-q", "--set-upstream-to=upstream/main", "started"]);
    commit_on(&work, "main", "local", "Local");
    git(&work, &["branch", "-q", "--set-upstream-to=main", "local"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, msg(ForgeMsg::PushAndStartPr { branch: "started".into(), target: None }), Instant::now());
    assert_eq!(run_git(&fx), [&vec![args(&["push", "origin", "refs/heads/started:refs/heads/started"])]], "not onto upstream's main");
    st.job = None;
    // Tracking a local branch: as before, no -u even with upstream present.
    let fx = update(&mut st, msg(ForgeMsg::PushAndStartPr { branch: "local".into(), target: None }), Instant::now());
    assert_eq!(run_git(&fx), [&vec![args(&["push", "origin", "refs/heads/local:refs/heads/local"])]]);
    st.job = None;
    // The dialog opens from origin with the branch's own name, into upstream.
    drive(&mut st, msg(ForgeMsg::CreatePrDialog { source: "started".into(), target: None }));
    let (source, _, _, picked) = dialog(&st).expect("the create dialog");
    assert_eq!((source.as_str(), picked.from.as_str(), picked.into.as_str()), ("started", "origin", "upstream"));
    // Picking upstream as From keeps the name: upstream's main isn't this branch.
    drive(&mut st, msg(ForgeMsg::PickPrRepo { side: RepoSide::From, remote: "upstream".into() }));
    assert_eq!(dialog(&st).map(|d| d.0), Some("started".into()));
    let fx =
        update(&mut st, msg(ForgeMsg::PushToForPr { branch: "started".into(), remote: "upstream".into(), target: None }), Instant::now());
    assert_eq!(run_git(&fx), [&vec![args(&["push", "upstream", "refs/heads/started:refs/heads/started"])]]);
    cleanup(&work, &bare);
    let _ = std::fs::remove_dir_all(&up);
}
