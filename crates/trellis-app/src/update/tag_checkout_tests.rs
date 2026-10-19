//! Headless tests for checking out a tag: the job and its label, the headline, the one-time
//! detached-HEAD warning with its "Create branch…" action and saved flag, a missing tag, a busy
//! app and a failed checkout.

use super::tests::{drive, finish_keeping, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::tags::DETACHED_WARNING;
use crate::tag::TagMsg;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::gitcli::args;
use trellis_core::undo::UndoKind;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn checkout(tag: &str) -> Msg {
    Msg::Tag(TagMsg::Checkout { tag: tag.into() })
}

/// `msg` and everything it leads to, returning the effects `finish_keeping` skipped.
fn drive_keeping(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    let fx = run(st, msg);
    finish_keeping(st, fx)
}

/// A clean repository on main with lightweight v1.0 at HEAD~1 and annotated v2.0 at HEAD; the
/// sha of HEAD~1.
fn tagged(name: &str) -> (AppState, PathBuf, String) {
    let dir = scratch_repo(name);
    git(&dir, &["commit", "-qam", "second"]);
    git(&dir, &["tag", "v1.0", "HEAD~1"]);
    git(&dir, &["tag", "-a", "v2.0", "-m", "rel"]);
    let first = git2::Repository::open(&dir).unwrap().revparse_single("HEAD~1").unwrap().id().to_string();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir, first)
}

#[test]
fn checkout_starts_one_job_named_after_the_tag() {
    let (mut st, dir, _) = tagged("tag-co-job");
    let fx = run(&mut st, checkout("v1.0"));
    match &fx[..] {
        [Effect::RunGit { cmds, record, .. }] => {
            assert_eq!(cmds, &vec![args(&["checkout", "refs/tags/v1.0"])]);
            assert_eq!(record, &Some(UndoKind::Checkout));
        }
        _ => panic!("expected one RunGit, got {} effects", fx.len()),
    }
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Checkout tag v1.0"));
    assert_eq!(st.tag_checkout.as_ref().map(|c| c.tag.as_str()), Some("v1.0"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn first_checkout_warns_once_then_names_the_tag() {
    let (mut st, dir, first) = tagged("tag-co-warn");
    assert!(!st.settings.detached_head_warned);
    let kept = drive_keeping(&mut st, checkout("v1.0"));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!((snap.head_branch.as_deref(), snap.head_oid.map(|o| o.to_string())), (None, Some(first.clone())));
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.ok && toast.text.starts_with("Checked out tag v1.0. HEAD is detached"), "{}", toast.text);
    let action = toast.action.as_ref().expect("Create branch… on the toast");
    assert_eq!(action.label, "Create branch…");
    assert!(matches!(&*action.msg, Msg::Branch(BranchMsg::NewBranch { start: Some(s), .. }) if *s == first));
    assert!(st.log.iter().any(|e| e.text == DETACHED_WARNING), "the warning is logged");
    assert!(st.log.iter().any(|e| e.text.starts_with("Checkout tag v1.0 done")));
    assert!(st.settings.detached_head_warned);
    assert!(kept.iter().any(|e| matches!(e, Effect::SaveSettings(s) if s.detached_head_warned)), "the flag is saved");
    assert!(st.tag_checkout.is_none());

    let warnings = st.log.iter().filter(|e| e.text == DETACHED_WARNING).count();
    let kept = drive_keeping(&mut st, checkout("v2.0"));
    let toast = st.toasts.last().unwrap();
    assert_eq!(toast.text, "Checked out tag v2.0 (detached HEAD)");
    assert!(toast.action.is_none());
    assert_eq!(st.log.iter().filter(|e| e.text == DETACHED_WARNING).count(), warnings, "warned only once");
    assert!(!kept.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_tag_and_busy_app_start_nothing() {
    let (mut st, dir, _) = tagged("tag-co-refused");
    assert!(run(&mut st, checkout("v9")).is_empty());
    assert_eq!(st.log.last().map(|e| e.text.as_str()), Some("Tag v9 no longer exists."));
    assert!(st.job.is_none() && st.tag_checkout.is_none());

    assert_eq!(run(&mut st, Msg::git("Fetch all", &["fetch", "--all"])).len(), 1, "another job runs");
    assert!(run(&mut st, checkout("v1.0")).is_empty());
    assert!(st.tag_checkout.is_none());
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch all"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_checkout_keeps_the_default_text_and_the_flag() {
    let (mut st, dir, _) = tagged("tag-co-failed");
    run(&mut st, checkout("v1.0"));
    let output = "$ git checkout refs/tags/v1.0\nerror: Your local changes would be overwritten by checkout";
    run(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output: output.into(), undo: None }));
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok && toast.text.starts_with("Checkout tag v1.0 failed: "), "{}", toast.text);
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Details"), "no Don't warn again");
    assert!(!st.settings.detached_head_warned);
    assert!(st.tag_checkout.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
