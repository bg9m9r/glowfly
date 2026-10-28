//! The auto-stash guard for operations that aren't read from a command line: a Git Flow start
//! and a pull request's checkout ask with their own operation; and changes that are only inside
//! a submodule never ask; a job started outside the guard still offers Stash and <op> when
//! changes are in its way.

use super::auto_stash_tests::{ask, git_cmds, up};
use super::gitflow_tests::{cleanup, initialized};
use super::jobs::{JobSpec, start_job};
use super::push_pr_tests::with_host;
use super::submodule_tests::{PATH, opened};
use super::tests::{clean_tree, drive, finish, git};
use crate::gitflow::GitFlowMsg;
use crate::msg::{AutoStashMsg, BranchMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::auto_stash::{blocks, body, button, changed_files};
use crate::state::branches::checkout_local;
use glowfly_core::forge::PullRequest;
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::stash_ops::AutoStashFor;
use std::path::Path;

fn start_login() -> Msg {
    Msg::GitFlow(GitFlowMsg::Start { kind: glowfly_core::gitflow::FlowKind::Feature, name: "login".into() })
}

fn head(st: &AppState) -> Option<&str> {
    st.snap.as_ref().unwrap().head_branch.as_deref()
}

/// A working file's text with git's line endings (core.autocrlf on Windows) as "\n".
fn text(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap().replace("\r\n", "\n")
}

fn stash_count(dir: &Path) -> usize {
    glowfly_core::stash_ops::list(dir).unwrap().len()
}

#[test]
fn git_flow_start_asks_and_stashes() {
    // scratch_repo leaves a.txt edited, so the tree is dirty.
    let (mut st, dir) = initialized("asg-start");
    assert!(up(&mut st, start_login()).is_empty());
    let a = ask(&st);
    assert_eq!(a.op, AutoStashFor::GitFlowStart { branch: "feature/login".into() });
    let text_of = body(Lang::En, &a.op, a.files);
    assert!(text_of.contains("starts feature/login and puts them back on it afterwards."), "{text_of}");
    assert_eq!(button(Lang::En, &a.op), "Stash and start");
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert_eq!(
        git_cmds(&fx),
        vec![
            args(&["stash", "push", "-m", "Auto-stash before starting feature/login"]),
            args(&["checkout", "-b", "feature/login", "refs/heads/develop"])
        ]
    );
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Start feature/login"));
    finish(&mut st, fx);
    assert_eq!(head(&st), Some("feature/login"));
    assert_eq!(text(&dir, "a.txt"), "one\ntwo\n", "the changes came along");
    assert_eq!(stash_count(&dir), 0);
    cleanup(&dir);
}

#[test]
fn git_flow_start_with_always_stash_skips_the_prompt() {
    let (mut st, dir) = initialized("asg-start-always");
    st.settings.auto_stash = true;
    let fx = up(&mut st, start_login());
    assert!(st.modal.is_none());
    assert_eq!(git_cmds(&fx)[0], args(&["stash", "push", "-m", "Auto-stash before starting feature/login"]));
    finish(&mut st, fx);
    assert_eq!(head(&st), Some("feature/login"));
    assert_eq!(text(&dir, "a.txt"), "one\ntwo\n");
    cleanup(&dir);
}

#[test]
fn git_flow_start_on_a_clean_tree_runs_plainly() {
    let (mut st, dir) = initialized("asg-start-clean");
    clean_tree(&dir);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = up(&mut st, start_login());
    assert!(st.modal.is_none());
    assert_eq!(git_cmds(&fx), vec![args(&["checkout", "-b", "feature/login", "refs/heads/develop"])]);
    cleanup(&dir);
}

fn pull_request(id: u64, source: &str) -> PullRequest {
    PullRequest {
        id,
        title: "Feature work".into(),
        description: String::new(),
        source: source.into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: "u".into(),
        facets: Default::default(),
    }
}

fn checkout_pr(id: u64) -> Msg {
    Msg::Branch(BranchMsg::CheckoutPullRequest { id })
}

#[test]
fn pull_request_checkout_asks_with_its_own_op() {
    // with_origin leaves a.txt edited; feature is local and on origin.
    let (mut st, work, bare) = with_host("asg-pr");
    st.session.forge.as_mut().unwrap().prs.push(pull_request(12, "feature"));
    let op = AutoStashFor::PullRequestCheckout { pr: "#12".into(), branch: "feature".into() };
    assert!(up(&mut st, checkout_pr(12)).is_empty());
    assert_eq!(ask(&st).op, op);
    let text_of = body(Lang::En, &op, 1);
    assert!(text_of.ends_with("Glowfly stashes them, checks out feature for #12 and puts them back afterwards."), "{text_of}");
    assert_eq!(button(Lang::En, &op), "Stash and check out");
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    let stash = args(&["stash", "push", "-m", "Auto-stash before checking out feature for #12"]);
    assert_eq!(git_cmds(&fx), vec![stash.clone(), checkout_local("feature")]);
    finish(&mut st, fx);
    assert_eq!(head(&st), Some("feature"));
    assert_eq!(text(&work, "a.txt"), "one\ntwo\n", "the changes are back");
    assert_eq!(stash_count(&work), 0);

    // Already checked out, or no longer listed: said in the log, nothing runs.
    assert!(nothing_ran(&up(&mut st, checkout_pr(12))));
    assert_eq!(last_log(&st), "feature is already checked out.");
    assert!(nothing_ran(&up(&mut st, checkout_pr(99))));
    assert_eq!(last_log(&st), Lang::En.tr("pr_checkout.gone"));

    // Only on the remote: a new branch tracking it.
    git(&work, &["checkout", "-q", "main"]);
    git(&work, &["branch", "-q", "-D", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(up(&mut st, checkout_pr(12)).is_empty());
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert_eq!(git_cmds(&fx), vec![stash, args(&["checkout", "--track", "origin/feature", "--"])]);
    st.job = None;

    // A clean tree runs the checkout alone.
    clean_tree(&work);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = up(&mut st, checkout_pr(12));
    assert!(st.modal.is_none());
    assert_eq!(git_cmds(&fx), vec![args(&["checkout", "--track", "origin/feature", "--"])]);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

/// Nothing ran (and no prompt opened).
fn nothing_ran(fx: &[crate::effect::Effect]) -> bool {
    !fx.iter().any(|e| matches!(e, crate::effect::Effect::RunGit { .. }))
}

fn last_log(st: &AppState) -> &str {
    st.log.last().map(|l| l.text.as_str()).unwrap_or_default()
}

#[test]
fn submodule_only_changes_dont_ask() {
    let (mut st, parent, lib) = opened("asg-sub");
    clean_tree(&parent);
    git(&parent, &["branch", "other"]);
    let sub = parent.join(PATH);
    std::fs::write(sub.join("a.txt"), "inside\n").unwrap();
    git(&sub, &["commit", "-q", "-am", "inside"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == PATH), "the moved submodule is listed: {:?}", snap.unstaged);
    assert!(!blocks(snap));
    assert_eq!(changed_files(snap), 0);
    let fx = up(&mut st, Msg::git("Checkout", &["checkout", "other", "--"]));
    assert!(st.modal.is_none());
    assert_eq!(git_cmds(&fx), vec![args(&["checkout", "other", "--"])], "no stash");
    finish(&mut st, fx);
    assert_eq!(head(&st), Some("other"));

    std::fs::write(parent.join("a.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert!(blocks(snap));
    assert_eq!(changed_files(snap), 1, "the submodule row isn't counted");
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&lib);
}

#[test]
fn a_job_started_outside_the_guard_offers_stash_and_carry_on() {
    let (mut st, dir) = super::job_errors_tests::two_branches("asg-plain");
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n6\n7\nmine\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // Started straight away, as a custom command or a branch row's action does: no prompt.
    let mut fx = Vec::new();
    let spec = JobSpec::new("Switch to feature".into(), vec![args(&["checkout", "feature", "--"])]);
    start_job(&mut st, spec, &mut fx);
    assert!(st.modal.is_none());
    finish(&mut st, fx);
    let t = st.toasts.last().expect("the failure toast");
    assert!(!t.ok);
    assert_eq!(t.text, "Your changes to 1 file would be overwritten.");
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("Stash and check out"));
    let id = t.id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert_eq!(
        git_cmds(&fx),
        vec![args(&["stash", "push", "-m", "Auto-stash before checking out feature"]), args(&["checkout", "feature", "--"])]
    );
    finish(&mut st, fx);
    assert_eq!(head(&st), Some("feature"));
    assert_eq!(text(&dir, "f.txt"), "feature\n2\n3\n4\n5\n6\n7\nmine\n", "the edit is back");
    assert_eq!(stash_count(&dir), 0);

    // A command the guard doesn't know gets the plain Stash changes… instead.
    let mut fx = Vec::new();
    start_job(&mut st, JobSpec::new("Tidy".into(), vec![args(&["status"])]), &mut fx);
    assert!(st.auto_stash.unstashed.is_none());
    st.job = None;
    let _ = std::fs::remove_dir_all(&dir);
}
