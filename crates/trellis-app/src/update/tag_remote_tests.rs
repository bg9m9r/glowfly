//! Headless tests for the TAGS row menu's remote push and delete: the one job each request
//! starts, the refusals, the confirmation, the headlines that name the failing remote, and an
//! end-to-end push and delete against two bare remotes.

use super::branch_tests::with_origin;
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::tags::delete_remote_confirm;
use crate::state::{AppState, Modal, RunningJob};
use crate::tag::TagMsg;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// `with_origin` plus a second bare remote `backup` and a local tag `v1`. Returns (state,
/// working repository, origin, backup).
fn with_two(name: &str) -> (AppState, PathBuf, PathBuf, PathBuf) {
    let (mut st, work, origin) = with_origin(name);
    let backup = work.with_file_name(format!("{}-backup.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&backup);
    std::fs::create_dir_all(&backup).unwrap();
    git(&backup, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "backup", &backup.display().to_string()]);
    git(&work, &["tag", "v1"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, origin, backup)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn names(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| (*s).to_owned()).collect()
}

fn push(tag: &str, remotes: &[&str]) -> Msg {
    Msg::Tag(TagMsg::Push { tag: tag.into(), remotes: names(remotes) })
}

fn delete(tag: &str, remotes: &[&str]) -> Msg {
    Msg::Tag(TagMsg::DeleteRemote { tag: tag.into(), remotes: names(remotes) })
}

/// The commands of the single job `fx` starts.
fn job_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn label(st: &AppState) -> String {
    st.job.as_ref().map(|j| j.label.clone()).unwrap_or_default()
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn toast(st: &AppState) -> String {
    st.toasts.last().map(|t| t.text.clone()).unwrap_or_default()
}

fn running() -> RunningJob {
    RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    }
}

fn cmd(a: &[&str]) -> Vec<String> {
    names(a)
}

#[test]
fn push_all_commands() {
    let (mut st, work, origin, backup) = with_two("tag-push-all");
    let fx = update(&mut st, push("v1", &["origin", "backup"]), Instant::now());
    assert_eq!(
        job_cmds(&fx),
        [cmd(&["push", "origin", "refs/tags/v1:refs/tags/v1"]), cmd(&["push", "backup", "refs/tags/v1:refs/tags/v1"])]
    );
    assert_eq!(label(&st), "Push tag v1 to origin and backup");
    assert!(st.tag_job.is_some());

    // "Push to origin" goes through the same message with one remote.
    st.job = None;
    let fx = update(&mut st, push("v1", &["origin"]), Instant::now());
    assert_eq!(job_cmds(&fx), [cmd(&["push", "origin", "refs/tags/v1:refs/tags/v1"])]);
    assert_eq!(label(&st), "Push tag v1 to origin");
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn delete_remote_commands() {
    let (mut st, work, origin, backup) = with_two("tag-delete-cmds");
    let fx = update(&mut st, delete("v1", &["origin"]), Instant::now());
    assert_eq!(job_cmds(&fx), [cmd(&["push", "origin", "--delete", "refs/tags/v1"])]);
    assert_eq!(label(&st), "Delete tag v1 from origin");

    st.job = None;
    let fx = update(&mut st, delete("v1", &["origin", "backup"]), Instant::now());
    assert_eq!(job_cmds(&fx), [cmd(&["push", "origin", "--delete", "refs/tags/v1"]), cmd(&["push", "backup", "--delete", "refs/tags/v1"])]);
    assert_eq!(label(&st), "Delete tag v1 from origin and backup");

    st.job = None;
    let fx = update(&mut st, delete("v1", &["origin", "origin"]), Instant::now());
    assert_eq!(job_cmds(&fx), [cmd(&["push", "origin", "--delete", "refs/tags/v1"])], "duplicates are dropped");
    assert_eq!(label(&st), "Delete tag v1 from origin");
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn remote_tag_refusals() {
    let (mut st, work, origin, backup) = with_two("tag-remote-refusals");
    let cases: [(Msg, &str); 4] = [
        (push("v1", &["origin", "nope"]), "There is no remote named nope."),
        (push("v9", &["origin"]), "Tag v9 no longer exists."),
        (push("-x", &["origin"]), "Not a valid tag name."),
        (delete("-x", &["origin"]), "Not a valid tag name."),
    ];
    for (msg, want) in cases {
        let what = want;
        let fx = update(&mut st, msg, Instant::now());
        assert!(fx.is_empty(), "{what}: {fx:?}");
        assert!(st.tag_job.is_none() && st.job.is_none(), "{what}");
        assert_eq!(last_log(&st), want, "{what}");
    }

    let logged = st.log.len();
    for msg in [push("v1", &[]), delete("v1", &[])] {
        assert!(update(&mut st, msg, Instant::now()).is_empty());
    }
    assert_eq!(st.log.len(), logged, "an empty list is refused silently");
    assert!(st.tag_job.is_none());

    st.job = Some(running());
    let fx = update(&mut st, delete("v1", &["origin"]), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    assert!(st.tag_job.is_none());

    // A delete doesn't need the local tag.
    st.job = None;
    let fx = update(&mut st, delete("v9", &["origin"]), Instant::now());
    assert_eq!(job_cmds(&fx), [cmd(&["push", "origin", "--delete", "refs/tags/v9"])]);
    assert!(st.tag_job.is_some());
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn refused_without_snapshot() {
    let mut st = super::tests::state();
    assert!(update(&mut st, push("v1", &["origin"]), Instant::now()).is_empty());
    assert!(st.log.is_empty() && st.tag_job.is_none());
}

#[test]
fn confirm_then_delete() {
    let (mut st, work, origin, backup) = with_two("tag-confirm-delete");
    update(&mut st, delete_remote_confirm("v1", names(&["origin", "backup"])), Instant::now());
    let Some(Modal::ConfirmDanger { title, body, button, .. }) = &st.modal else { panic!("expected the confirmation") };
    assert_eq!(title, "Delete tag from all remotes");
    assert_eq!(body, "Delete tag v1 from origin and backup? Your local tag stays, and anyone who already fetched it keeps their copy.");
    assert_eq!(button, "Delete from all remotes");
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert_eq!(job_cmds(&fx).len(), 2);
    assert_eq!(label(&st), "Delete tag v1 from origin and backup");

    st.job = None;
    update(&mut st, delete_remote_confirm("v1", names(&["origin"])), Instant::now());
    let Some(Modal::ConfirmDanger { title, body, button, .. }) = &st.modal else { panic!("expected the confirmation") };
    assert_eq!(title, "Delete tag from remote");
    assert_eq!(body, "Delete tag v1 from origin? Your local tag stays, and anyone who already fetched it keeps their copy.");
    assert_eq!(button, "Delete from origin");
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert_eq!(job_cmds(&fx), [cmd(&["push", "origin", "--delete", "refs/tags/v1"])]);
    cleanup(&[&work, &origin, &backup]);
}

/// Starts `msg`, then ends its job with `ok` and `output`.
fn end(st: &mut AppState, msg: Msg, ok: bool, output: &str) {
    st.job = None;
    update(st, msg, Instant::now());
    assert!(st.tag_job.is_some());
    update(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }), Instant::now());
    assert!(st.tag_job.is_none());
}

#[test]
fn failure_headline_names_remote() {
    let (mut st, work, origin, backup) = with_two("tag-failure-headline");
    let output = "$ git push origin --delete refs/tags/v1\nTo x\n - [deleted] v1\n$ git push backup --delete refs/tags/v1\n\
                  error: unable to delete 'refs/tags/v1': remote ref does not exist\nerror: failed to push some refs to 'x'";
    end(&mut st, delete("v1", &["origin", "backup"]), false, output);
    let want = "Could not delete tag v1 from backup: the remote has no such tag. Deleted from origin.";
    assert_eq!(toast(&st), want);
    assert_eq!(last_log(&st), want);

    let output = "$ git push origin --delete refs/tags/v1\nfatal: 'origin' does not appear to be a git repository\n\
                  fatal: Could not read from remote repository.";
    end(&mut st, delete("v1", &["origin", "backup"]), false, output);
    let want = "Could not delete tag v1 from origin: 'origin' does not appear to be a git repository. Not tried: backup.";
    assert_eq!(toast(&st), want);
    assert_eq!(last_log(&st), want);

    let output = "$ git push origin refs/tags/v1:refs/tags/v1\nTo x\n ! [rejected]        v1 -> v1 (already exists)\n\
                  error: failed to push some refs to 'x'\nhint: Updates were rejected because the tag already exists in the remote.";
    end(&mut st, push("v1", &["origin", "backup"]), false, output);
    let want = "Could not push tag v1 to origin: the remote already has a different tag with that name. Not tried: backup.";
    assert_eq!(toast(&st), want);
    assert_eq!(last_log(&st), want);
    cleanup(&[&work, &origin, &backup]);
}

#[test]
fn success_and_stale_headline() {
    let (mut st, work, origin, backup) = with_two("tag-success-headline");
    end(&mut st, delete("v1", &["origin", "backup"]), true, "");
    assert_eq!(toast(&st), "Deleted tag v1 from origin and backup.");
    end(&mut st, push("v1", &["origin", "backup"]), true, "");
    assert_eq!(toast(&st), "Pushed tag v1 to origin and backup.");

    st.job = None;
    update(&mut st, delete("v1", &["origin", "backup"]), Instant::now());
    st.tag_job.as_mut().unwrap().label = "Something else".into();
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }), Instant::now());
    assert_eq!(toast(&st), "Delete tag v1 from origin and backup succeeded");
    assert!(st.tag_job.is_none());
    cleanup(&[&work, &origin, &backup]);
}

fn has_tag(bare: &Path) -> bool {
    git2::Repository::open_bare(bare).unwrap().find_reference("refs/tags/v1").is_ok()
}

#[test]
fn remote_tag_end_to_end() {
    let (mut st, work, origin, backup) = with_two("tag-remote-e2e");
    drive(&mut st, push("v1", &["origin", "backup"]));
    assert!(st.job.is_none(), "the job finished");
    assert!(has_tag(&origin) && has_tag(&backup));
    assert_eq!(toast(&st), "Pushed tag v1 to origin and backup.");

    drive(&mut st, delete("v1", &["origin", "backup"]));
    assert!(!has_tag(&origin) && !has_tag(&backup));
    assert!(st.snap.as_ref().unwrap().tags.iter().any(|(n, _)| n == "v1"), "the local tag stays");
    assert_eq!(toast(&st), "Deleted tag v1 from origin and backup.");

    // A remote that has gone away stops the job there; the headline names it and what was done.
    drive(&mut st, push("v1", &["origin"]));
    assert!(has_tag(&origin));
    cleanup(&[&backup]);
    drive(&mut st, delete("v1", &["origin", "backup"]));
    assert!(!has_tag(&origin));
    let log = last_log(&st);
    assert!(log.starts_with("Could not delete tag v1 from backup: ") && log.ends_with(" Deleted from origin."), "{log}");
    assert_eq!(toast(&st), log);
    cleanup(&[&work, &origin, &backup]);
}
