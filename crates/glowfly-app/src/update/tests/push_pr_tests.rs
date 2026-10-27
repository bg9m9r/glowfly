//! Headless tests for pushing a branch and starting a pull request: the push job, the dialog
//! that opens once it lands, and the cases that refuse or only log. `origin` reads as GitHub
//! while pushes go to a bare repository next to the working one.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, git};
use super::update;
use crate::effect::{Effect, ForgeCall};
use crate::msg::ForgeMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::push_pr::PrStart;
use crate::state::{AppState, Modal, RunningJob, branches};
use git2::Oid;
use glowfly_core::forge::NewPullRequest;
use glowfly_core::gitcli::{args, run};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// `with_origin` with `origin` on GitHub (pushes still go to the bare repository) and a local
/// `topic` branch one commit ("Add topic") past main, not pushed. HEAD stays on main.
pub(super) fn with_host(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    git(&work, &["remote", "set-url", "origin", "https://github.com/o/r.git"]);
    git(&work, &["config", "remote.origin.pushurl", &bare.display().to_string()]);
    commit_on(&work, "main", "topic", "Add topic");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.session.forge.as_ref().is_some_and(|f| f.host.is_some() && f.remote == "origin"), "GitHub is detected on origin");
    (st, work, bare)
}

/// Makes `branch` one commit (`summary`, same tree) past `parent`, without checking it out.
pub(super) fn commit_on(work: &Path, parent: &str, branch: &str, summary: &str) -> Oid {
    let (ok, out) = run(work, &args(&["commit-tree", &format!("{parent}^{{tree}}"), "-p", parent, "-m", summary]));
    assert!(ok, "{out}");
    git(work, &["update-ref", &format!("refs/heads/{branch}"), out.trim()]);
    Oid::from_str(out.trim()).unwrap()
}

/// Fetches the bare repository's branches into `origin`'s remote-tracking refs.
pub(super) fn fetch(work: &Path, bare: &Path) {
    git(work, &["fetch", "-q", &bare.display().to_string(), "+refs/heads/*:refs/remotes/origin/*"]);
}

pub(super) fn cleanup(work: &Path, bare: &Path) {
    let _ = std::fs::remove_dir_all(work);
    let _ = std::fs::remove_dir_all(bare);
}

fn push(branch: &str, target: Option<&str>) -> Msg {
    Msg::Forge(ForgeMsg::PushAndStartPr { branch: branch.into(), target: target.map(Into::into) })
}

fn dialog(source: &str, target: Option<&str>) -> Msg {
    Msg::Forge(ForgeMsg::CreatePrDialog { source: source.into(), target: target.map(Into::into) })
}

/// The open create dialog's (source, target, title).
fn create_pr(st: &AppState) -> Option<(String, String, String)> {
    match &st.modal {
        Some(Modal::CreatePr { source, target, title, .. }) => Some((source.clone(), target.clone(), title.clone())),
        _ => None,
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn run_git(fx: &[Effect]) -> Vec<&Vec<Vec<String>>> {
    fx.iter().filter_map(|e| if let Effect::RunGit { cmds, .. } = e { Some(cmds) } else { None }).collect()
}

fn open_after_load(st: &AppState) -> Option<PrStart> {
    st.session.forge.as_ref().and_then(|f| f.open_after_load.clone())
}

#[test]
fn push_and_start_pushes_missing_branch_with_upstream() {
    let (mut st, work, bare) = with_host("pr-push-missing");
    let fx = update(&mut st, push("topic", None), Instant::now());
    assert_eq!(run_git(&fx), [&vec![args(&["push", "-u", "origin", "refs/heads/topic:refs/heads/topic"])]]);
    assert_eq!(fx.len(), 1);
    assert_eq!(st.job.as_ref().and_then(|j| j.then_pr.clone()), Some(PrStart { source: "topic".into(), target: None, remote: None }));
    cleanup(&work, &bare);
}

#[test]
fn push_then_dialog_opens_after_reload() {
    let (mut st, work, bare) = with_host("pr-push-reload");
    drive(&mut st, push("topic", None));
    let bare_repo = git2::Repository::open_bare(&bare).unwrap();
    assert!(bare_repo.find_reference("refs/heads/topic").is_ok(), "pushed");
    let topic = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "topic").unwrap().clone();
    assert_eq!(branches::remote_tracking(&topic).map(|t| (t.remote.as_str(), t.merge.as_str())), Some(("origin", "topic")));
    assert_eq!(create_pr(&st), Some(("topic".into(), "main".into(), "Add topic".into())));
    assert_eq!(open_after_load(&st), None);
    cleanup(&work, &bare);
}

#[test]
fn chosen_target_is_preselected() {
    let (mut st, work, bare) = with_host("pr-push-target");
    drive(&mut st, push("topic", Some("feature")));
    assert_eq!(create_pr(&st).map(|p| p.1), Some("feature".into()));
    st.modal = None;
    drive(&mut st, dialog("feature", Some("nope")));
    assert_eq!(create_pr(&st).map(|p| (p.0, p.1)), Some(("feature".into(), "main".into())));
    cleanup(&work, &bare);
}

#[test]
fn ahead_branch_pushes_to_its_upstream_without_u() {
    let (mut st, work, bare) = with_host("pr-push-ahead");
    commit_on(&work, "feature", "feature", "More feature");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, push("feature", None), Instant::now());
    assert_eq!(run_git(&fx), [&vec![args(&["push", "origin", "refs/heads/feature:refs/heads/feature"])]]);
    cleanup(&work, &bare);
}

#[test]
fn pushed_branch_opens_dialog_at_once() {
    let (mut st, work, bare) = with_host("pr-push-pushed");
    let fx = update(&mut st, push("feature", None), Instant::now());
    assert!(run_git(&fx).is_empty());
    assert!(st.job.is_none());
    assert_eq!(create_pr(&st).map(|p| p.0), Some("feature".into()));
    // Asked again with the dialog open: what was typed stays.
    if let Some(Modal::CreatePr { title, target, .. }) = st.modal.as_mut() {
        *title = "Typed title".into();
        *target = "topic".into();
    }
    update(&mut st, push("feature", Some("main")), Instant::now());
    assert_eq!(create_pr(&st), Some(("feature".into(), "topic".into(), "Typed title".into())));
    cleanup(&work, &bare);
}

#[test]
fn diverged_branch_is_refused() {
    let (mut st, work, bare) = with_host("pr-push-diverged");
    remote_commit(&bare, "feature", "remote.txt");
    fetch(&work, &bare);
    commit_on(&work, "feature", "feature", "Local feature");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, push("feature", None), Instant::now());
    assert!(run_git(&fx).is_empty());
    assert!(st.job.is_none());
    assert!(st.modal.is_none());
    let b = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "feature").unwrap();
    let why = branches::push_blocked(b, true).unwrap();
    assert!(why.contains("is behind"), "{why}");
    assert_eq!(last_log(&st), why);
    cleanup(&work, &bare);
}

#[test]
fn failed_push_opens_nothing() {
    let (mut st, work, bare) = with_host("pr-push-fails");
    let nowhere = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-pr-nowhere-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&nowhere);
    git(&work, &["config", "remote.origin.pushurl", &nowhere.display().to_string()]);
    drive(&mut st, push("topic", None));
    assert!(st.job.is_none());
    assert!(st.log.iter().any(|l| !l.ok), "the failure is reported");
    assert!(st.modal.is_none());
    assert_eq!(open_after_load(&st), None);
    cleanup(&work, &bare);
}

#[test]
fn push_in_another_tab_only_logs() {
    let (mut st, work, bare) = with_host("pr-push-other-tab");
    st.job = Some(RunningJob {
        label: "Push".into(),
        workdir: Some(bare.clone()),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: Some(PrStart { source: "topic".into(), target: None, remote: None }),
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }), Instant::now());
    assert!(logged(&st, "Its tab is no longer active"));
    assert!(st.modal.is_none());
    assert_eq!(open_after_load(&st), None);
    cleanup(&work, &bare);
}

#[test]
fn other_modal_is_not_replaced() {
    let (mut st, work, bare) = with_host("pr-push-other-modal");
    st.session.forge.as_mut().unwrap().open_after_load = Some(PrStart { source: "topic".into(), target: None, remote: None });
    st.modal = Some(Modal::RenameBranch { old: "feature".into(), name: "feature2".into(), touched: Default::default() });
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(matches!(st.modal, Some(Modal::RenameBranch { .. })));
    assert!(logged(&st, "Use Create pull request… to continue"));
    assert_eq!(open_after_load(&st), None);
    cleanup(&work, &bare);
}

#[test]
fn create_refuses_source_missing_on_host() {
    let (mut st, work, bare) = with_host("pr-create-missing");
    let pr = |source: &str| NewPullRequest {
        source: source.into(),
        target: "main".into(),
        title: "T".into(),
        description: String::new(),
        draft: false,
        issues: Vec::new(),
        source_repo: None,
    };
    let create = |pr| Msg::Forge(ForgeMsg::CreatePr { pr, from: String::new(), into: String::new() });
    let fx = update(&mut st, create(pr("topic")), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::Forge { .. })));
    assert!(logged(&st, "isn't on origin yet"));
    let fx = update(&mut st, create(pr("feature")), Instant::now());
    let creates = fx.iter().filter(|e| matches!(e, Effect::Forge { call: ForgeCall::Create(_), .. })).count();
    assert_eq!((creates, fx.len()), (1, 1));
    cleanup(&work, &bare);
}

#[test]
fn dialog_from_remote_branch() {
    let (mut st, work, bare) = with_host("pr-dialog-remote");
    remote_commit(&bare, "feature", "remote.txt");
    fetch(&work, &bare);
    git(&work, &["branch", "-D", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, dialog("feature", None));
    assert_eq!(create_pr(&st).map(|p| (p.0, p.2)), Some(("feature".into(), "remote.txt".into())));
    // A local branch tracking a differently named upstream starts from the upstream's name.
    st.modal = None;
    git(&work, &["branch", "-q", "--track", "wip", "origin/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, dialog("wip", None));
    assert_eq!(create_pr(&st).map(|p| (p.0, p.2)), Some(("feature".into(), "remote.txt".into())));
    cleanup(&work, &bare);
}

#[test]
fn no_host_does_nothing() {
    let (mut st, work, bare) = with_host("pr-no-host");
    st.session.forge = None;
    assert!(update(&mut st, push("topic", None), Instant::now()).is_empty());
    assert!(update(&mut st, dialog("feature", None), Instant::now()).is_empty());
    assert!(st.modal.is_none());
    assert!(st.job.is_none());
    cleanup(&work, &bare);
}
