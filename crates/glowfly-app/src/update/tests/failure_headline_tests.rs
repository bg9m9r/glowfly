//! Failures in plain words beyond a job's own toast: the auto-stash put-back and the worktree
//! remove's remote delete through the error classifier, git's line-ending warnings folded in the
//! job log, and the fallback headline in the UI language (skipping those warnings). Conflict
//! counts are in conflict_count_tests.rs.

use super::auto_stash_tests::up;
use super::tests::{clean_tree, drive, scratch_repo, state};
use crate::msg::{AutoStashMsg, JobMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::auto_stash::{AutoStashJob, PendingStash, Phase};
use crate::worktree::{RemoteOutcome, RemoveRequest, Removed, WorktreeMsg};
use glowfly_core::repo::Tracking;
use glowfly_core::stash_ops::{AutoStashFor, PutBack};
use glowfly_core::worktree_ops::{RemoveForce, RemoveOutcome};
use std::path::{Path, PathBuf};

const LOCKED: &str = "fatal: Unable to create '/r/.git/index.lock': File exists.";
const LOCKED_SAYS: &str = "Another git program is using this repository (index.lock exists).";

pub(super) fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

pub(super) fn last_toast(st: &AppState) -> &str {
    st.toasts.last().map(|t| t.text.as_str()).expect("a toast")
}

pub(super) fn running(st: &mut AppState, dir: &Path, label: &str) {
    st.job = Some(crate::state::RunningJob { label: label.into(), ..super::rebase_tests::running_job(dir) });
}

#[test]
fn put_back_failure_says_why() {
    let (mut st, dir) = opened("fh-put-back");
    let op = AutoStashFor::Checkout { target: "feature".into() };
    let message = op.message();
    let oid = git2::Oid::from_str("1111111111111111111111111111111111111111").unwrap();
    let job = AutoStashJob { op, message: message.clone(), before: Vec::new() };
    let workdir = st.workdir().unwrap();
    let pending =
        PendingStash { workdir: workdir.clone(), job, oid: Some(oid), phase: Phase::Ended { from: 0 }, told: false, saved: false };
    st.auto_stash.pending.push(pending);
    running(&mut st, &dir, "Put back stashed changes");
    let output = format!("$ git stash apply --index {oid}\n{LOCKED}");
    up(&mut st, Msg::AutoStash(AutoStashMsg::PutBack { workdir, oid, outcome: PutBack::Failed, output, undo: None }));
    let says = format!("Couldn't put back \"{message}\": {LOCKED_SAYS}");
    assert!(last_toast(&st).starts_with(&says), "{}", last_toast(&st));
    assert!(last_toast(&st).ends_with("The stash stays in the list, so nothing is lost."), "{}", last_toast(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn worktree_remote_delete_failure_says_why() {
    let (mut st, dir) = opened("fh-worktree");
    running(&mut st, &dir, "Remove worktree");
    let request = RemoveRequest {
        repo: dir.clone(),
        path: dir.with_file_name("fh-worktree-side"),
        name: "side".into(),
        force: RemoveForce::None,
        delete_branch: Some("side".into()),
        delete_remote: Some(Tracking { remote: "origin".into(), merge: "side".into() }),
        stop_monitor: false,
    };
    let failed = "remote: Repository not found.\nfatal: repository 'https://example.com/a.git/' not found";
    let msg = WorktreeMsg::Removed(Box::new(Removed {
        request,
        outcome: RemoveOutcome::Removed,
        output: String::new(),
        branch: Some(Ok(())),
        remote: Some(RemoteOutcome::Failed(failed.into())),
        undo: None,
    }));
    up(&mut st, Msg::Worktree(msg));
    let says = "Removed worktree side and deleted branch side, but origin/side wasn't deleted: The remote repository wasn't found.";
    assert!(last_toast(&st).starts_with(says), "{}", last_toast(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn line_ending_warnings_fold_in_the_job_log() {
    let (mut st, dir) = opened("fh-crlf");
    running(&mut st, &dir, "Stash");
    let warn = |f: &str| format!("warning: in the working copy of '{f}', LF will be replaced by CRLF the next time Git touches it");
    let output = format!("$ git stash push\n{}\n{}\n{}\nSaved working directory and index state", warn("a"), warn("b"), warn("c"));
    up(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output, undo: None }));
    let entry = st.log.iter().rev().find(|e| e.text.starts_with("Stash done")).expect("the job's entry");
    assert_eq!(
        entry.text,
        "Stash done\n$ git stash push\nGit warned about line endings in 3 files: they change between LF and CRLF the next time Git \
         touches them.\nSaved working directory and index state"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fallback_headlines_name_the_job_in_the_ui_language() {
    let (mut st, dir) = opened("fh-labels");
    st.settings.language = "de".into();
    running(&mut st, &dir, "Fetch");
    up(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output: "$ git fetch\nfatal: offline".into(), undo: None }));
    assert_eq!(last_toast(&st), "Abrufen fehlgeschlagen: fatal: offline");
    running(&mut st, &dir, "Fetch origin");
    up(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert_eq!(last_toast(&st), "origin abrufen erfolgreich");
    st.settings.language = "es".into();
    running(&mut st, &dir, "Remove submodule libs/x");
    up(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output: "fatal: nope".into(), undo: None }));
    assert_eq!(last_toast(&st), "Quitar el submódulo libs/x falló: fatal: nope");

    // The headline is git's last real line, not the line-ending warnings after it.
    st.settings.language = "en".into();
    running(&mut st, &dir, "Stash");
    let w = |f: &str| format!("warning: in the working copy of '{f}', LF will be replaced by CRLF");
    let output = format!("$ git stash push\nerror: could not write index\n{}\n{}", w("a"), w("b"));
    up(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }));
    assert_eq!(last_toast(&st), "Stash failed: error: could not write index");
    let _ = std::fs::remove_dir_all(&dir);
}
