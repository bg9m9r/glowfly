//! Failures in plain words beyond a job's own toast: the auto-stash put-back and the worktree
//! remove's remote delete through the error classifier, conflicts counted from the status read
//! after the job (and from git's CONFLICT lines until then, or when that read fails), git's
//! line-ending warnings folded in the job log, and the fallback headline in the UI language.

use super::auto_stash_tests::up;
use super::tests::{clean_tree, drive, finish, git, scratch_repo, state};
use crate::msg::{AutoStashMsg, JobMsg, LoadMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::auto_stash::{AutoStashJob, PendingStash, Phase};
use crate::worktree::{RemoteOutcome, RemoveRequest, Removed, WorktreeMsg};
use glowfly_core::gitcli::{args, run_with_input};
use glowfly_core::repo::Tracking;
use glowfly_core::stash_ops::{AutoStashFor, PutBack};
use glowfly_core::worktree_ops::{RemoveForce, RemoveOutcome};
use std::path::{Path, PathBuf};

const LOCKED: &str = "fatal: Unable to create '/r/.git/index.lock': File exists.";
const LOCKED_SAYS: &str = "Another git program is using this repository (index.lock exists).";

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn last_toast(st: &AppState) -> &str {
    st.toasts.last().map(|t| t.text.as_str()).expect("a toast")
}

fn running(st: &mut AppState, dir: &Path, label: &str) {
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

/// `dir` stopped in a merge where side renamed a.txt (deleted on main) and both renamed r.txt:
/// b.txt, r.txt, r1.txt and r2.txt are conflicted. Returns git's output.
fn rename_conflicts(dir: &Path) -> String {
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.join("r.txt"), "x\ny\nz\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "base"]);
    git(dir, &["checkout", "-q", "-b", "side"]);
    git(dir, &["mv", "a.txt", "b.txt"]);
    git(dir, &["mv", "r.txt", "r1.txt"]);
    git(dir, &["commit", "-q", "-m", "side"]);
    git(dir, &["checkout", "-q", "main"]);
    git(dir, &["rm", "-q", "a.txt"]);
    git(dir, &["mv", "r.txt", "r2.txt"]);
    git(dir, &["commit", "-q", "-m", "main"]);
    let (ok, out) = run_with_input(dir, &args(&["merge", "side"]), "", &[]);
    assert!(!ok && out.contains("CONFLICT (rename/delete)"), "{out}");
    out
}

/// The merge's job ends with only the first of git's CONFLICT lines, as when git words the
/// others in a way Glowfly can't read: the toast says 1 file until the status counts 4.
#[test]
fn conflicts_are_counted_from_the_refreshed_status() {
    let (mut st, dir) = opened("fh-conflicts");
    let out = rename_conflicts(&dir);
    let first = out.lines().find(|l| l.starts_with("CONFLICT (")).unwrap();
    let output = format!("$ git merge side\n{first}\nAutomatic merge failed; fix conflicts and then commit the result.");
    running(&mut st, &dir, "Merge");
    let fx = up(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }));
    assert!(last_toast(&st).starts_with("Stopped on conflicts in 1 file."), "{}", last_toast(&st));

    // A status read that fails keeps git's count.
    let failed = LoadMsg::RepoUnchanged {
        generation: st.load_gen,
        result: Err("could not read the status".into()),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    };
    up(&mut st, Msg::Load(failed));
    assert!(st.toasts.iter().any(|t| t.text.starts_with("Stopped on conflicts in 1 file.")));
    assert!(st.logs.recount.is_some(), "still waiting");

    // The refresh the job's end asked for counts each conflicted path once, in the toast and the run.
    finish(&mut st, fx);
    assert!(st.toasts.iter().any(|t| t.text == "Stopped on conflicts in 4 files. Resolve them, then continue or abort."));
    let run = st.log.iter().rev().find_map(|e| e.run.as_ref()).expect("the merge's run");
    assert_eq!(run.summary, "Stopped on conflicts in 4 files. Resolve them, then continue or abort.");
    assert!(st.logs.recount.is_none(), "counted once");
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
    let _ = std::fs::remove_dir_all(&dir);
}
