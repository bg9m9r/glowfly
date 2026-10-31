//! Try again and Delete lock file: a failed job's spec runs once more from its toast, only in its
//! own open tab; a stale lock is offered for deleting once the check says no git runs, asks
//! first, and runs the job again after the delete; and the delete is refused while git runs. A
//! job that failed past its first command gets none, and a commit runs from the current draft.

use super::auto_stash_tests::{git_cmds, up};
use super::tests::{clean_tree, scratch_repo, state};
use crate::effect::{Effect, LockEffect};
use crate::msg::{CommitMsg, JobMsg, LockMsg, Msg, RepoMsg, StageMsg};
use crate::state::job_spec::{JobSpec, Retry};
use crate::state::tabs::TabId;
use crate::state::toasts::Toast;
use crate::state::{AppState, Modal};
use glowfly_core::gitcli::args;
use glowfly_core::lock_file::{Found, Refusal};
use std::path::PathBuf;

/// A clean repository with a Push running, and the path of a stale `index.lock` in it.
fn pushing(name: &str) -> (AppState, PathBuf, String) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    let mut st = state();
    super::tests::drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    up(&mut st, Msg::git("Push", &["push"]));
    assert!(st.job.is_some());
    let lock = dir.join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    let named = lock.to_string_lossy().replace('\\', "/");
    (st, dir, named)
}

fn locked_output(lock: &str) -> String {
    format!("$ git push\nfatal: Unable to create '{lock}': File exists.\n\nAnother git process seems to be running in this repository")
}

fn fail(st: &mut AppState, output: &str) -> Vec<Effect> {
    up(st, Msg::Job(JobMsg::JobDone { ok: false, output: output.into(), undo: None }))
}

fn last_toast(st: &AppState) -> &Toast {
    st.toasts.last().expect("a toast")
}

/// The labels of a toast's buttons, in order.
fn buttons(t: &Toast) -> Vec<&str> {
    [&t.action, &t.alt, &t.details].into_iter().flatten().map(|a| a.label.as_str()).collect()
}

/// The run the last toast's fix belongs to, and the lock check asked for it.
fn check_of(fx: &[Effect]) -> (crate::state::logs::EntryId, String) {
    let found = fx.iter().find_map(|e| match e {
        Effect::Lock(LockEffect::Check { lock, entry, .. }) => Some((*entry, lock.clone())),
        _ => None,
    });
    found.expect("the lock is checked")
}

#[test]
fn try_again_runs_the_failed_job_again() {
    let (mut st, dir, lock) = pushing("lr-again");
    let fx = fail(&mut st, &locked_output(&lock));
    let t = last_toast(&st);
    assert!(t.text.starts_with("Another git program is using this repository"), "{}", t.text);
    assert_eq!(buttons(t), ["Try again", "Details"]);
    assert_eq!(check_of(&fx).1, lock, "the exact path git named is checked");
    let id = t.id;
    let fx = up(&mut st, Msg::ToastAction(id));
    assert_eq!(git_cmds(&fx), vec![args(&["push"])], "the same job again");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Push"));

    // Failing again offers it again; an HTTPS sign-in failure offers it too.
    fail(&mut st, "fatal: could not read Username for 'https://example.com': terminal prompts disabled");
    let t = last_toast(&st);
    assert!(t.text.starts_with("The server didn't accept the sign-in."), "{}", t.text);
    assert_eq!(buttons(t), ["Try again", "Details"]);
    // Other failures get no Try again.
    up(&mut st, Msg::git("Push", &["push"]));
    fail(&mut st, "fatal: You are not currently on a branch.");
    assert_eq!(buttons(last_toast(&st)), ["Create branch…", "Details"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_closed_or_other_tab_gets_no_retry() {
    let (mut st, dir, lock) = pushing("lr-tabs");
    fail(&mut st, &locked_output(&lock));
    let Some(Msg::Job(JobMsg::Retry(kept))) = st.logs.fixes.last().map(|(_, f)| &*f.msg) else { panic!("Try again is kept") };
    let retry: Retry = (**kept).clone();
    let closed = Retry { tab: TabId::next(), ..retry.clone() };
    let fx = up(&mut st, Msg::Job(JobMsg::Retry(Box::new(closed))));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })) && st.job.is_none());
    assert_eq!(last_toast(&st).text, "Push wasn't run again: its tab was closed.");
    let moved = Retry { workdir: Some(dir.join("elsewhere")), ..retry };
    let fx = up(&mut st, Msg::Job(JobMsg::Retry(Box::new(moved))));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })));
    assert_eq!(last_toast(&st).text, "Push wasn't run again: its tab shows another repository now.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_lock_file_asks_deletes_and_runs_the_job_again() {
    let (mut st, dir, lock) = pushing("lr-delete");
    let fx = fail(&mut st, &locked_output(&lock));
    let (entry, _) = check_of(&fx);
    let workdir = st.workdir().unwrap();
    // The runner found it stale and no git running.
    let stale = Ok(Found::Stale(PathBuf::from(&lock)));
    up(&mut st, Msg::Lock(LockMsg::Checked { entry, workdir, lock: lock.clone(), result: stale }));
    let t = last_toast(&st);
    assert_eq!(buttons(t), ["Delete lock file", "Try again", "Details"]);
    assert_eq!(st.logs.fix(entry).map(|f| f.label.as_str()), Some("Delete lock file"));
    assert_eq!(st.logs.alt_fix(entry).map(|f| f.label.as_str()), Some("Try again"), "the panel offers both");

    let id = t.id;
    up(&mut st, Msg::ToastAction(id));
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { title, .. }) if title == "Delete the lock file?"), "it asks first");
    let fx = up(&mut st, Msg::ConfirmAccepted);
    let delete = fx.into_iter().find_map(|e| match e {
        Effect::Lock(LockEffect::Delete { workdir, lock, retry }) => Some((workdir, lock, retry)),
        _ => None,
    });
    let (at, named, retry) = delete.expect("the delete");
    assert_eq!((at.as_path(), named.as_str()), (st.workdir().unwrap().as_path(), lock.as_str()), "the lock git named, in its repository");
    assert!(std::path::Path::new(&lock).exists(), "nothing is deleted before the runner checks again");
    // The runner's delete (its refusals are glowfly_core::lock_file's tests and the one below).
    std::fs::remove_file(&lock).unwrap();
    let fx = up(&mut st, Msg::Lock(LockMsg::Deleted { lock: named, retry, result: Ok(Found::Stale(PathBuf::from(&lock))) }));
    assert_eq!(git_cmds(&fx), vec![args(&["push"])], "then the job runs again");
    assert!(st.log.iter().any(|e| e.text == "Deleted index.lock."), "the delete is logged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_lock_file_is_refused_while_git_runs() {
    let (mut st, dir, lock) = pushing("lr-refused");
    let fx = fail(&mut st, &locked_output(&lock));
    let (entry, _) = check_of(&fx);
    let workdir = st.workdir().unwrap();
    // A check that found git running offers only Try again.
    let running = Err(Refusal::GitRunning(vec![(42, "git.exe".into())]));
    up(&mut st, Msg::Lock(LockMsg::Checked { entry, workdir: workdir.clone(), lock: lock.clone(), result: running }));
    assert_eq!(buttons(last_toast(&st)), ["Try again", "Details"]);

    // A git process starts after the button was offered: the runner's own check refuses.
    let Some(Msg::Job(JobMsg::Retry(kept))) = st.logs.fix(entry).map(|f| &*f.msg) else { panic!("Try again is kept") };
    let retry = kept.clone();
    let mut git = std::process::Command::new("git")
        .args(["hash-object", "--stdin"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let effect = Effect::Lock(LockEffect::Delete { workdir, lock: lock.clone(), retry });
    let result = crate::runner::testing::execute(effect);
    drop(git.stdin.take());
    let _ = git.wait();
    let Some(Msg::Lock(done @ LockMsg::Deleted { result: Err(Refusal::GitRunning(_) | Refusal::Unknown(_)), .. })) = result else {
        panic!("refused while git runs")
    };
    assert!(std::path::Path::new(&lock).exists(), "the lock stays");
    let fx = up(&mut st, Msg::Lock(done));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "the job doesn't run");
    let t = last_toast(&st);
    assert!(t.text.starts_with("index.lock wasn't deleted:"), "{}", t.text);
    assert_eq!(buttons(t), ["Try again"], "Try again stays on offer");

    // Asked while a job of Glowfly's own runs: refused before the runner is asked.
    let Some(Msg::Job(JobMsg::Retry(again))) = t.action.as_ref().map(|a| &*a.msg) else { panic!("Try again") };
    let again = again.clone();
    up(&mut st, Msg::git("Push", &["push"]));
    let workdir = st.workdir().unwrap();
    let fx = up(&mut st, Msg::Lock(LockMsg::Delete { workdir, lock: lock.clone(), retry: again }));
    assert!(fx.is_empty());
    assert_eq!(last_toast(&st).text, "index.lock wasn't deleted: Push is still running.");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A clean repository opened, and the "Unable to create" line for a stale `index.lock` in it.
fn opened(name: &str) -> (AppState, PathBuf, String) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    let mut st = state();
    super::tests::drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let lock = dir.join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    let named = lock.to_string_lossy().replace('\\', "/");
    (st, dir, format!("fatal: Unable to create '{named}': File exists."))
}

/// Starts a job of two commands, a fetch and then a push.
fn fetch_then_push(st: &mut AppState) {
    let spec = JobSpec::new("Fetch and push".into(), vec![args(&["fetch"]), args(&["push"])]);
    crate::update::jobs::start_job(st, spec, &mut Vec::new());
    assert!(st.job.is_some());
}

#[test]
fn a_job_that_failed_past_its_first_command_gets_no_retry() {
    let (mut st, dir, locked) = opened("lr-steps");
    fetch_then_push(&mut st);
    let fx = fail(
        &mut st,
        &format!(
            "$ git fetch
$ git push
{locked}"
        ),
    );
    assert_eq!(buttons(last_toast(&st)), ["Details"], "the fetch already ran, so running it all again repeats it");
    assert!(!fx.iter().any(|e| matches!(e, Effect::Lock(_))), "nor is the lock offered for deleting");
    assert!(st.logs.job_retry.is_none());

    fetch_then_push(&mut st);
    fail(
        &mut st,
        &format!(
            "$ git fetch
{locked}"
        ),
    );
    assert_eq!(buttons(last_toast(&st)), ["Try again", "Details"], "it stopped in its first command");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn try_again_on_a_commit_commits_the_draft_as_it_is_now() {
    let (mut st, dir, locked) = opened("lr-commit");
    std::fs::remove_file(dir.join(".git").join("index.lock")).unwrap();
    std::fs::write(
        dir.join("a.txt"),
        "changed
",
    )
    .unwrap();
    super::tests::drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    super::tests::drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "First words".into();
    up(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Commit"));
    fail(
        &mut st,
        &format!(
            "$ git commit -F msg
{locked}"
        ),
    );
    let t = last_toast(&st);
    assert_eq!(buttons(t), ["Try again", "Details"]);
    let id = t.id;

    // Edited after the failure: Try again commits the new text, never the old one.
    st.session.draft.summary = "Better words".into();
    let fx = up(&mut st, Msg::ToastAction(id));
    let message = fx.iter().find_map(|e| match e {
        Effect::RunGit { write_file: Some((_, m)), .. } => Some(String::from_utf8(m.clone()).unwrap()),
        _ => None,
    });
    assert_eq!(
        message.as_deref(),
        Some(
            "Better words
"
        )
    );
    assert_eq!(st.session.draft.summary, "Better words", "the draft stays until the commit succeeds");

    // A draft emptied since is refused with the commit box's own reason.
    fail(
        &mut st,
        &format!(
            "$ git commit -F msg
{locked}"
        ),
    );
    let id = last_toast(&st).id;
    st.session.draft.summary.clear();
    let fx = up(&mut st, Msg::ToastAction(id));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })) && st.job.is_none());
    assert_eq!(last_toast(&st).text, "Write a summary for the commit.");
    let _ = std::fs::remove_dir_all(&dir);
}
