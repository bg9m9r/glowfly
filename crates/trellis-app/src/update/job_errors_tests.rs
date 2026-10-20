//! Failure toasts that say what to do: git's cause in plain words, the fix beside Details, the
//! Stash and <op> retry for a guarded job that ran over changes the snapshot hadn't shown yet,
//! and no `<stdin>` whitespace warnings in a refused pull's log.

use super::auto_stash_tests::{git_cmds, up};
use super::branch_tests::{remote_commit, with_origin};
use super::jobs::{JobSpec, start_job};
use super::tests::{clean_tree, drive, finish, git, scratch_repo, state};
use crate::effect::Effect;
use crate::logs::LogKind;
use crate::msg::{BranchMsg, ConflictMsg, JobMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::prefs::PrefPage;
use crate::state::toasts::Toast;
use crate::state::{AppState, RunningJob};
use std::path::{Path, PathBuf};
use trellis_core::gitcli::args;

/// main and feature, where feature changes the first of f.txt's eight lines; opened clean.
fn two_branches(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n6\n7\n8\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(dir.join("f.txt"), "feature\n2\n3\n4\n5\n6\n7\n8\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "feature work"]);
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn last_toast(st: &AppState) -> &Toast {
    st.toasts.last().expect("a toast")
}

fn labels(t: &Toast) -> (Option<&str>, Option<&str>) {
    (t.action.as_ref().map(|a| a.label.as_str()), t.details.as_ref().map(|a| a.label.as_str()))
}

/// The message the toast's fix runs: the run's fix behind a `LogsMsg::Fix` proxy.
fn action_msg<'a>(st: &'a AppState, t: &'a Toast) -> &'a Msg {
    let msg = &t.action.as_ref().expect("a fix").msg;
    match **msg {
        Msg::Logs(crate::logs::LogsMsg::Fix(id)) => &st.logs.fix(id).expect("the fix is kept").msg,
        _ => msg,
    }
}

fn stash_count(dir: &Path) -> usize {
    trellis_core::stash_ops::list(dir).unwrap().len()
}

#[test]
fn stale_snapshot_checkout_offers_stash_and_check_out() {
    let (mut st, dir) = two_branches("je-checkout");
    // Edited without a refresh: the snapshot still shows a clean tree, so the gate lets it run.
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n6\n7\nmine\n").unwrap();
    let fx = up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    assert!(st.modal.is_none());
    finish(&mut st, fx);
    let t = last_toast(&st);
    assert!(!t.ok);
    assert_eq!(t.text, "Your changes to 1 file would be overwritten.");
    assert_eq!(labels(t), (Some("Stash and check out"), Some("Details")));
    let entry = st.log.iter().rev().find(|e| e.text.starts_with("Checkout FAILED")).expect("the job's log entry");
    assert!(entry.text.contains("Your local changes to the following files would be overwritten by checkout"), "{}", entry.text);
    let id = t.id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert_eq!(
        git_cmds(&fx),
        vec![args(&["stash", "push", "-m", "Auto-stash before checking out feature"]), args(&["checkout", "feature", "--"])]
    );
    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("feature"));
    let text = std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(text, "feature\n2\n3\n4\n5\n6\n7\nmine\n", "the edit is back");
    assert_eq!(stash_count(&dir), 0, "the stash is gone");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pull_over_staged_whitespace_logs_no_stdin_lines() {
    let (mut st, work, bare) = with_origin("je-pull");
    git(&work, &["config", "pull.rebase", "false"]);
    clean_tree(&work);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    remote_commit(&bare, "main", "remote.txt");
    std::fs::write(work.join("local.txt"), "local\n").unwrap();
    git(&work, &["add", "local.txt"]);
    git(&work, &["commit", "-q", "-m", "local"]);
    std::fs::write(work.join("g.txt"), "x   \ny \n").unwrap();
    git(&work, &["add", "g.txt"]);
    let fx = up(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    finish(&mut st, fx);
    let entry = st.log.iter().rev().find(|e| e.text.starts_with("Pull FAILED")).expect("the job's log entry");
    assert!(!entry.text.contains("<stdin>") && !entry.text.contains("whitespace errors"), "{}", entry.text);
    let t = last_toast(&st);
    assert_eq!(t.text, "Your changes to 1 file would be overwritten.");
    assert_eq!(labels(t).0, Some("Stash and pull"));
    for d in [&work, &bare] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A clean repository on main with a running (unguarded) job labelled `label`.
fn busy(name: &str, label: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let cmd = if label == "Fetch" { "fetch" } else { "push" };
    up(&mut st, Msg::git(label, &[cmd]));
    assert!(st.job.is_some());
    (st, dir)
}

fn done(st: &mut AppState, output: &str) -> Vec<Effect> {
    up(st, Msg::Job(JobMsg::JobDone { ok: false, output: output.into(), undo: None }))
}

const PUSH_FAILED: &str = "error: failed to push some refs to '/tmp/r-origin.git'";

#[test]
fn causes_get_their_fix() {
    let (mut st, dir) = busy("je-push", "Push");
    done(&mut st, &format!("$ git push\nTo /tmp/r-origin.git\n ! [rejected]        main -> main (fetch first)\n{PUSH_FAILED}"));
    let t = last_toast(&st);
    assert!(t.text.starts_with("The remote has commits your branch doesn't have."), "{}", t.text);
    assert_eq!(labels(t), (Some("Pull"), Some("Details")));
    assert!(
        matches!(action_msg(&st, t), Msg::Job(JobMsg::Git { label, cmds }) if label == "Pull" && *cmds == vec![args(&["pull", "--ff"])])
    );

    let cases: [(&str, &str, &str); 5] = [
        (
            "CONFLICT (content): Merge conflict in a.txt\nAutomatic merge failed; fix conflicts and then commit the result.",
            "Stopped on conflicts in 1 file.",
            "View conflicts",
        ),
        (
            "fatal: The current branch feat has no upstream branch.\nTo push the current branch and set the remote as upstream, use",
            "feat has no upstream yet.",
            "Push",
        ),
        ("fatal: You are not currently on a branch.", "No branch is checked out (detached HEAD).", "Create branch…"),
        (
            "git@example.com: Permission denied (publickey).\nfatal: Could not read from remote repository.",
            "The server didn't accept your SSH key.",
            "SSH keys",
        ),
        (
            "fatal: Unable to create '/tmp/r/.git/index.lock': File exists.",
            "Another git program is using this repository (index.lock exists).",
            "Details",
        ),
    ];
    for (output, text, button) in cases {
        up(&mut st, Msg::git("Push", &["push"]));
        done(&mut st, output);
        let t = last_toast(&st);
        assert!(t.text.starts_with(text), "{output}: {}", t.text);
        assert_eq!(labels(t).0, Some(button), "{output}");
        let ok = match button {
            "View conflicts" => matches!(action_msg(&st, t), Msg::Conflict(ConflictMsg::OpenConflict(p)) if p == "a.txt"),
            "Push" => matches!(action_msg(&st, t), Msg::Job(JobMsg::Push { force: false })),
            "Create branch…" => matches!(action_msg(&st, t), Msg::Branch(BranchMsg::NewBranch { start: None, name }) if name.is_empty()),
            "SSH keys" => matches!(action_msg(&st, t), Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh))),
            _ => t.details.is_none(),
        };
        assert!(ok, "{output}: the fix's message");
        st.modal = None;
    }

    // Another branch's rejection: nothing to pull here, so only Details.
    up(&mut st, Msg::git("Push", &["push"]));
    done(&mut st, &format!(" ! [rejected]        feature -> feature (non-fast-forward)\n{PUSH_FAILED}"));
    assert_eq!(labels(last_toast(&st)), (Some("Details"), None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unrecognised_and_cancelled_keep_their_text() {
    let (mut st, dir) = busy("je-fetch", "Fetch");
    done(&mut st, "$ git fetch\nfatal: offline");
    let t = last_toast(&st);
    assert_eq!(t.text, "Fetch failed: fatal: offline");
    assert_eq!(labels(t), (Some("Details"), None));

    up(&mut st, Msg::git("Push", &["push"]));
    st.askpass.ended = Some(crate::state::askpass::PromptEnd::Cancelled);
    done(&mut st, "fatal: Authentication failed for 'https://example.com/r.git/'");
    assert_eq!(last_toast(&st).text, "Push cancelled: no credentials were given");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn details_beside_fix() {
    let (mut st, dir) = busy("je-details", "Push");
    done(&mut st, "fatal: You are not currently on a branch.");
    let t = last_toast(&st);
    assert_eq!(labels(t), (Some("Create branch…"), Some("Details")));
    assert_eq!(t.lifetime(), crate::state::toasts::ACTION_TOAST_LIFETIME);
    let id = t.id;
    up(&mut st, Msg::ToastDetails(id));
    assert!(st.toasts.iter().all(|t| t.id != id), "the toast is gone");
    assert!(st.view.show_log && st.logs.tab == LogKind::Activity, "the activity log shows");

    // The fix runs only in its own repository's tab.
    up(&mut st, Msg::git("Push", &["push"]));
    done(&mut st, "fatal: You are not currently on a branch.");
    let id = last_toast(&st).id;
    let other = scratch_repo("je-details-other");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    up(&mut st, Msg::ToastAction(id));
    assert!(st.modal.is_none(), "no New branch dialog");
    assert_eq!(last_toast(&st).text, "Create branch… wasn't run: switch back to that repository's tab first.");

    // A quiet job's failure shows nothing.
    let before = st.toasts.len();
    let spec = JobSpec { quiet: true, ..JobSpec::new(super::notify::AUTO_FETCH.into(), vec![args(&["fetch"])]) };
    start_job(&mut st, spec, &mut Vec::new());
    assert!(st.job.as_ref().is_some_and(|j| j.quiet));
    done(&mut st, "fatal: Unable to create '/tmp/r/.git/index.lock': File exists.");
    assert_eq!(st.toasts.len(), before, "no toast");
    for d in [&dir, &other] {
        let _ = std::fs::remove_dir_all(d);
    }
}

const CHECKOUT_LOCAL: &str = "error: Your local changes to the following files would be overwritten by checkout:\n\tf.txt\nAborting";

#[test]
fn stale_retry_is_dropped() {
    let (mut st, dir) = super::trust_held_tests::held_repo("je-held");
    git(&dir, &["branch", "feature"]);
    up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    assert!(st.trust.held.is_some(), "held for the trust prompt");
    assert!(st.auto_stash.unstashed.as_ref().is_some_and(|u| u.label == "Checkout"), "its retry is recorded");
    drive(&mut st, Msg::CloseModal);
    assert!(st.trust.held.is_none());
    assert!(st.auto_stash.unstashed.is_none(), "dropped with the held job");

    // A later job with another label gets no Stash and <op>.
    let workdir = st.workdir();
    st.job = Some(RunningJob { label: "Update".into(), workdir, ..super::rebase_tests::running_job(Path::new(".")) });
    done(&mut st, CHECKOUT_LOCAL);
    assert_eq!(labels(last_toast(&st)).0, Some("Stash changes…"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_retry_only_answers_its_own_job() {
    let (mut st, dir) = two_branches("je-label");
    st.job = None;
    up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    assert!(st.auto_stash.unstashed.is_some());
    // Its record goes when any job ends; a job with another label gets no Stash and <op>.
    st.job.as_mut().unwrap().label = "Something else".into();
    done(&mut st, CHECKOUT_LOCAL);
    assert!(st.auto_stash.unstashed.is_none());
    assert_eq!(labels(last_toast(&st)).0, Some("Stash changes…"));
    let _ = std::fs::remove_dir_all(&dir);
}
