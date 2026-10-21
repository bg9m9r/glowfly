//! Headless tests for annotating a lightweight tag: which tags open the dialog, the one job it
//! starts, the refusals that keep the dialog open, and an end-to-end run that leaves an
//! annotated tag at the same commit.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::{AppState, Modal, RunningJob};
use crate::tag::TagMsg;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn head(st: &AppState) -> String {
    st.snap.as_ref().and_then(|s| s.head_oid).expect("born HEAD").to_string()
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

/// A scratch repository with lightweight tag v1 and annotated tag v2 at HEAD, opened.
fn tagged(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "tag.gpgSign", "false"]);
    git(&dir, &["tag", "v1"]);
    git(&dir, &["tag", "-a", "v2", "-m", "x"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn open(st: &mut AppState, tag: &str) {
    update(st, Msg::Tag(TagMsg::OpenAnnotate { tag: tag.into() }), Instant::now());
}

fn annotate(at: &str, message: &str) -> Msg {
    Msg::Tag(TagMsg::Annotate { tag: "v1".into(), at: at.into(), message: message.into() })
}

fn dialog_open(st: &AppState) -> bool {
    matches!(st.modal, Some(Modal::AnnotateTag(_)))
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_annotate_only_for_lightweight() {
    let (mut st, dir) = tagged("annotate-open");
    let sha = head(&st);
    open(&mut st, "v1");
    assert!(matches!(&st.modal, Some(Modal::AnnotateTag(f)) if f.tag == "v1" && f.at == sha && f.message.is_empty()));

    st.modal = None;
    open(&mut st, "v2");
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "Tag v2 is already annotated.");

    let logs = st.log.len();
    open(&mut st, "nope");
    assert!(st.modal.is_none());
    assert_eq!(st.log.len(), logs, "a missing tag is ignored");
    cleanup(&dir);
}

#[test]
fn annotate_starts_one_job() {
    let (mut st, dir) = tagged("annotate-job");
    let sha = head(&st);
    open(&mut st, "v1");
    let fx = update(&mut st, annotate(&sha, " notes "), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, expect, record, .. }] => {
            assert_eq!(cmds, &[vec!["tag".to_owned(), "-a".into(), "-f".into(), "v1".into(), sha.clone(), "-m".into(), "notes".into()]]);
            assert_eq!(expect, &Some(("refs/tags/v1".to_owned(), git2::Oid::from_str(&sha).unwrap())));
            assert!(record.is_none(), "nothing is recorded for undo");
        }
        other => panic!("unexpected effects {other:?}"),
    }
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Annotate tag v1"));
    assert!(st.modal.is_none());
    cleanup(&dir);
}

#[test]
fn annotate_refusals_keep_dialog() {
    let (mut st, dir) = tagged("annotate-refusals");
    let sha = head(&st);
    open(&mut st, "v1");
    let refuse = |st: &mut AppState, msg: Msg, want: &str| {
        let fx = update(st, msg, Instant::now());
        assert!(fx.is_empty(), "{want}: {fx:?}");
        assert!(last_log(st).contains(want), "{want}: {}", last_log(st));
        assert!(dialog_open(st), "{want}");
    };
    refuse(&mut st, annotate(&sha, ""), "Enter a tag message.");
    refuse(&mut st, annotate(&sha, "  \n "), "Enter a tag message.");
    refuse(&mut st, annotate("0123456789abcdef0123456789abcdef01234567", "notes"), "has moved to");
    let bad = Msg::Tag(TagMsg::Annotate { tag: "a..b".into(), at: sha.clone(), message: "notes".into() });
    refuse(&mut st, bad, "Not a valid tag name.");
    let annotated = Msg::Tag(TagMsg::Annotate { tag: "v2".into(), at: sha.clone(), message: "notes".into() });
    refuse(&mut st, annotated, "Tag v2 is already annotated.");

    st.job = Some(RunningJob {
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
    });
    refuse(&mut st, annotate(&sha, "notes"), "Another operation is still running.");
    st.job = None;

    // Deleted behind Glowfly's back, then refreshed: the dialog still shows, but nothing runs.
    git(&dir, &["tag", "-d", "v1"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(dialog_open(&st));
    refuse(&mut st, annotate(&sha, "notes"), "Tag v1 no longer exists.");
    assert!(matches!(&st.modal, Some(Modal::AnnotateTag(f)) if f.tag == "v1"));
    cleanup(&dir);
}

#[test]
fn annotate_end_to_end() {
    let (mut st, dir) = tagged("annotate-e2e");
    let sha = head(&st);
    open(&mut st, "v1");
    drive(&mut st, annotate(&sha, "Release notes"));
    assert!(st.job.is_none(), "the job finished");
    assert!(st.modal.is_none());

    let repo = git2::Repository::open(&dir).unwrap();
    let target = repo.find_reference("refs/tags/v1").unwrap().target().unwrap();
    let tag = repo.find_tag(target).expect("a tag object");
    assert_eq!(tag.target_id().to_string(), sha);
    assert_eq!(tag.message().unwrap().trim(), "Release notes");

    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.lightweight_tags.contains("v1"));
    assert!(snap.tags.iter().any(|(n, oid)| n == "v1" && oid.to_string() == sha));
    let toasts: Vec<&str> = st.toasts.iter().map(|t| t.text.as_str()).collect();
    assert!(toasts.contains(&"Annotate tag v1 succeeded"), "{toasts:?}");
    cleanup(&dir);
}
