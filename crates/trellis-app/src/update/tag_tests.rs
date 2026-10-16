//! Headless tests for the New tag dialog's messages: the default push remote, the commands a
//! new tag starts, the refusals that keep the dialog open, the push setting, and an end-to-end
//! create-and-push against a bare `origin`.

use super::branch_tests::with_origin;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::{AppState, Modal, RunningJob};
use crate::tag::TagMsg;
use std::path::Path;
use std::time::Instant;

fn head(st: &AppState) -> String {
    st.snap.as_ref().and_then(|s| s.head_oid).expect("born HEAD").to_string()
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn create(at: &str, name: &str, message: &str, push: Option<&str>) -> Msg {
    Msg::Tag(TagMsg::CreateTag { at: at.into(), name: name.into(), message: message.into(), push: push.map(str::to_owned) })
}

fn open_dialog(st: &mut AppState, annotated: bool) {
    let at = head(st);
    update(st, Msg::Tag(TagMsg::NewTag { at, annotated }), Instant::now());
    assert!(matches!(st.modal, Some(Modal::NewTag(_))));
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn new_tag_picks_default_remote() {
    let (mut st, work, bare) = with_origin("tag-remote");
    open_dialog(&mut st, false);
    assert!(matches!(&st.modal, Some(Modal::NewTag(f)) if f.remote == "origin" && f.name.is_empty() && !f.annotated));
    cleanup(&[&work, &bare]);

    let dir = scratch_repo("tag-no-remote");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    open_dialog(&mut st, true);
    assert!(matches!(&st.modal, Some(Modal::NewTag(f)) if f.remote.is_empty() && f.annotated));
    cleanup(&[&dir]);

    // Without a snapshot the dialog still opens.
    let mut st = state();
    update(&mut st, Msg::Tag(TagMsg::NewTag { at: "0123456".into(), annotated: false }), Instant::now());
    assert!(matches!(&st.modal, Some(Modal::NewTag(f)) if f.at == "0123456" && f.remote.is_empty()));
}

#[test]
fn create_tag_commands() {
    let dir = scratch_repo("tag-cmds");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let sha = head(&st);
    open_dialog(&mut st, false);
    let fx = update(&mut st, create(&sha, " v1 ", "", None), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => assert_eq!(cmds, &[vec!["tag".to_owned(), "v1".into(), sha.clone()]]),
        other => panic!("unexpected effects {other:?}"),
    }
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Create tag"));
    assert!(st.modal.is_none());
    cleanup(&[&dir]);

    let (mut st, work, bare) = with_origin("tag-cmds-push");
    let sha = head(&st);
    open_dialog(&mut st, true);
    let fx = update(&mut st, create(&sha, "v1", " notes ", Some("origin")), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => {
            assert_eq!(cmds[0], ["tag", "-a", "v1", sha.as_str(), "-m", "notes"]);
            assert_eq!(cmds[1], ["push", "origin", "refs/tags/v1:refs/tags/v1"]);
            assert_eq!(cmds.len(), 2);
        }
        other => panic!("unexpected effects {other:?}"),
    }
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Create tag and push to origin"));
    assert!(st.modal.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn create_tag_rejects_bad_names() {
    let dir = scratch_repo("tag-bad-names");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let sha = head(&st);
    open_dialog(&mut st, false);
    let cases = [
        ("bad name", "Tag names can't contain spaces."),
        ("-x", "Tag names can't start with a dash."),
        ("a..b", "Tag names can't contain two dots in a row."),
        ("v1.lock", "No part of a tag name can end in .lock."),
    ];
    for (name, reason) in cases {
        let fx = update(&mut st, create(&sha, name, "", None), Instant::now());
        assert!(fx.is_empty(), "{name}: {fx:?}");
        assert_eq!(last_log(&st), reason);
        assert!(matches!(st.modal, Some(Modal::NewTag(_))), "{name}");
        assert!(st.job.is_none());
    }
    cleanup(&[&dir]);
}

#[test]
fn create_tag_rejects_existing_tag() {
    let dir = scratch_repo("tag-existing");
    git(&dir, &["tag", "v1"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let sha = head(&st);
    open_dialog(&mut st, false);
    for name in ["v1", "V1"] {
        let fx = update(&mut st, create(&sha, name, "", None), Instant::now());
        assert!(fx.is_empty(), "{name}: {fx:?}");
        assert_eq!(last_log(&st), "Tag v1 already exists.");
        assert!(matches!(st.modal, Some(Modal::NewTag(_))));
    }
    cleanup(&[&dir]);
}

#[test]
fn create_tag_unknown_remote_and_busy() {
    let (mut st, work, bare) = with_origin("tag-refusals");
    let sha = head(&st);
    open_dialog(&mut st, false);
    let fx = update(&mut st, create(&sha, "v1", "", Some("nope")), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "There is no remote named nope.");
    assert!(matches!(st.modal, Some(Modal::NewTag(_))));

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
    });
    let fx = update(&mut st, create(&sha, "v1", "", Some("origin")), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    assert!(matches!(st.modal, Some(Modal::NewTag(_))));
    cleanup(&[&work, &bare]);
}

#[test]
fn push_new_tags_setting_saved() {
    let mut st = state();
    let fx = update(&mut st, Msg::Tag(TagMsg::SetPushNewTags(true)), Instant::now());
    assert!(st.settings.push_new_tags);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.push_new_tags));
    assert!(update(&mut st, Msg::Tag(TagMsg::SetPushNewTags(true)), Instant::now()).is_empty());
}

#[test]
fn create_and_push_tag_end_to_end() {
    let (mut st, work, bare) = with_origin("tag-e2e");
    let sha = head(&st);
    open_dialog(&mut st, true);
    drive(&mut st, create(&sha, "v9", "notes", Some("origin")));
    assert!(st.job.is_none(), "the job finished");
    assert!(st.modal.is_none());
    let origin = git2::Repository::open_bare(&bare).unwrap();
    let pushed = origin.find_reference("refs/tags/v9").unwrap().peel_to_commit().unwrap().id();
    assert_eq!(pushed.to_string(), sha);
    assert!(st.snap.as_ref().unwrap().tags.iter().any(|(n, _)| n == "v9"));
    cleanup(&[&work, &bare]);
}
