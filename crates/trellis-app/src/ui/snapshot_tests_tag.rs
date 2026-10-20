//! The New tag dialog over the whole window: annotated (message required) and plain, its
//! initial focus, the live name checks, the push option, and the message its button and Enter
//! send. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::tags::TagForm;
use crate::state::{AppState, Modal};
use crate::tag::TagMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

fn head(st: &AppState) -> String {
    st.snap.as_ref().and_then(|s| s.head_oid).expect("born HEAD").to_string()
}

fn form(at: &str, name: &str, annotated: bool) -> Modal {
    Modal::NewTag(TagForm { at: at.into(), name: name.into(), annotated, ..TagForm::default() })
}

/// The first `TagMsg::CreateTag` pushed, as (name, message, push), if any.
fn created(h: &Harness<'_, (AppState, Vec<Msg>)>) -> Option<(String, String, Option<String>)> {
    h.state().1.iter().find_map(|m| match m {
        Msg::Tag(TagMsg::CreateTag { name, message, push, .. }) => Some((name.clone(), message.clone(), push.clone())),
        _ => None,
    })
}

fn closed(h: &Harness<'_, (AppState, Vec<Msg>)>) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::CloseModal))
}

fn set_name(h: &mut Harness<'_, (AppState, Vec<Msg>)>, name: &str) {
    if let Some(Modal::NewTag(f)) = h.state_mut().0.modal.as_mut() {
        f.name = name.into();
    }
    h.run();
}

#[test]
fn new_tag_dialog_annotated() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-tag-annotated");
    let sha = head(&st);
    st.modal = Some(form(&sha, "v1.2.0", true));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("new_tag_dialog_annotated");
    assert!(h.query_by_label_contains("New annotated tag at").is_some());
    assert!(h.get_by_label("Create tag").is_disabled());
    assert!(h.query_by_label("Optional").is_none(), "an annotated tag needs its message");

    if let Some(Modal::NewTag(f)) = h.state_mut().0.modal.as_mut() {
        f.message = "Release notes".into();
    }
    h.run();
    assert!(!h.get_by_label("Create tag").is_disabled());
    h.get_by_label("Create tag").click();
    h.run();
    assert_eq!(created(&h), Some(("v1.2.0".into(), "Release notes".into(), None)));
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tag(TagMsg::CreateTag { at, .. }) if *at == sha)));
    // Update closes the dialog once the job starts.
    assert!(!closed(&h));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_tag_dialog_plain_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "new-tag-plain");
    let sha = head(&st);
    st.modal = Some(form(&sha, "", false));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("new_tag_dialog_plain_light");
    assert!(h.query_by_label_contains("New tag at").is_some());
    assert!(h.get_by_label("Create tag").is_disabled());
    assert!(h.query_by_label("Optional").is_some(), "a plain tag's message is optional");
    assert!(h.query_by_label("A message makes an annotated tag.").is_some());

    // The name field has the keyboard from the start.
    h.input_mut().events.push(Event::Text("v2".into()));
    h.run();
    assert!(matches!(&h.state().0.modal, Some(Modal::NewTag(f)) if f.name == "v2" && f.message.is_empty()));
    h.press_key(Key::Enter);
    h.run();
    assert_eq!(created(&h), Some(("v2".into(), String::new(), None)), "Enter creates the tag");
    drop(h);
    remove_fixture(&dir);
}

/// In annotated mode, Enter in the name field with no message moves on to the message.
#[test]
fn new_tag_enter_moves_to_message() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-tag-enter");
    let sha = head(&st);
    st.modal = Some(form(&sha, "v3", true));
    let mut h = harness(st, true);
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert!(created(&h).is_none());
    h.input_mut().events.push(Event::Text("notes".into()));
    h.run();
    assert!(matches!(&h.state().0.modal, Some(Modal::NewTag(f)) if f.name == "v3" && f.message == "notes"));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_tag_dialog_invalid_name() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-tag-invalid");
    let sha = head(&st);
    st.modal = Some(form(&sha, "release 1.0", false));
    let mut h = harness(st, true);
    h.run();
    // Nobody typed the name yet: its problem waits for an edit or a submit.
    assert!(h.query_by_label("Tag names can't contain spaces.").is_none());
    assert!(h.get_by_label("Create tag").is_disabled());
    h.press_key(Key::Enter);
    h.run();
    assert!(created(&h).is_none());
    h.check_snapshot("new_tag_dialog_invalid_name");
    assert!(h.query_by_label("Tag names can't contain spaces.").is_some(), "Enter shows it");
    assert!(matches!(&h.state().0.modal, Some(Modal::NewTag(f)) if f.name == "release 1.0"));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_tag_dialog_existing_name() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "new-tag-existing");
    let sha = head(&st);
    let mut f = form(&sha, "v0.1.0", false);
    if let Modal::NewTag(t) = &mut f {
        t.touched.mark("name");
    }
    st.modal = Some(f);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("new_tag_dialog_existing_light");
    let exists = "Tag v0.1.0 already exists.";
    assert!(h.query_by_label(exists).is_some());
    assert!(h.get_by_label("Create tag").is_disabled());
    // Letter case doesn't matter; the message names the existing tag.
    set_name(&mut h, "V0.1.0");
    assert!(h.query_by_label(exists).is_some());
    assert!(h.get_by_label("Create tag").is_disabled());
    set_name(&mut h, "v0.2.0");
    assert!(h.query_by_label_contains("already exists").is_none());
    assert!(!h.get_by_label("Create tag").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_tag_dialog_push_remotes() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-tag-push");
    let sha = head(&st);
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new()), ("upstream".into(), Vec::new())];
    st.settings.push_new_tags = true;
    st.modal = Some(Modal::NewTag(TagForm {
        at: sha.clone(),
        name: "v1.3.0".into(),
        annotated: false,
        remote: "origin".into(),
        ..TagForm::default()
    }));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("new_tag_dialog_push");
    assert!(h.query_by_label("Push to").is_some());
    assert!(h.query_by_label("origin").is_some());
    h.get_by_label("Create tag").click();
    h.run();
    assert_eq!(created(&h), Some(("v1.3.0".into(), String::new(), Some("origin".into()))));
    assert!(!closed(&h));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_tag_dialog_push_single_remote() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-tag-push-one");
    let sha = head(&st);
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new())];
    st.modal = Some(form(&sha, "v1.4.0", false));
    let mut h = harness(st, true);
    h.run();
    // The view fills in the remote it would push to.
    assert!(matches!(&h.state().0.modal, Some(Modal::NewTag(f)) if f.remote == "origin"));
    h.get_by_label("Push to origin").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tag(TagMsg::SetPushNewTags(true)))));
    // The checkbox only asks; until update saves the setting, Create tag doesn't push.
    h.get_by_label("Create tag").click();
    h.run();
    assert_eq!(created(&h), Some(("v1.4.0".into(), String::new(), None)));
    drop(h);
    remove_fixture(&dir);
}
