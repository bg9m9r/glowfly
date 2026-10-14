//! The New tag dialog over the whole window: annotated (message required) and plain, its
//! initial focus, and the git command its button and Enter send. Shares the fixture repository
//! and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::{AppState, Modal};
use eframe::egui::{Event, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

fn head(st: &AppState) -> String {
    st.snap.as_ref().and_then(|s| s.head_oid).expect("born HEAD").to_string()
}

/// The commands of the first `Msg::Git` pushed, if any.
fn git_cmds(h: &Harness<'_, (AppState, Vec<Msg>)>) -> Option<Vec<Vec<String>>> {
    h.state().1.iter().find_map(|m| match m {
        Msg::Git { label, cmds } if label == "Create tag" => Some(cmds.clone()),
        _ => None,
    })
}

#[test]
fn new_tag_dialog_annotated() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-tag-annotated");
    let sha = head(&st);
    st.modal = Some(Modal::NewTag { at: sha.clone(), name: "v1.2.0".into(), message: String::new(), annotated: true });
    let mut h = harness(st, true);
    h.run();
    h.snapshot("new_tag_dialog_annotated");
    assert!(h.query_by_label_contains("New annotated tag at").is_some());
    assert!(h.get_by_label("Create tag").is_disabled());

    if let Some(Modal::NewTag { message, .. }) = h.state_mut().0.modal.as_mut() {
        *message = "Release notes".into();
    }
    h.run();
    assert!(!h.get_by_label("Create tag").is_disabled());
    h.get_by_label("Create tag").click();
    h.run();
    let cmds = git_cmds(&h).expect("Create tag sent");
    assert_eq!(cmds[0], ["tag", "-a", "v1.2.0", sha.as_str(), "-m", "Release notes"]);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_tag_dialog_plain_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "new-tag-plain");
    let sha = head(&st);
    st.modal = Some(Modal::NewTag { at: sha.clone(), name: String::new(), message: String::new(), annotated: false });
    let mut h = harness(st, false);
    h.run();
    h.snapshot("new_tag_dialog_plain_light");
    assert!(h.query_by_label_contains("New tag at").is_some());
    assert!(h.get_by_label("Create tag").is_disabled());

    // The name field has the keyboard from the start.
    h.input_mut().events.push(Event::Text("v2".into()));
    h.run();
    assert!(matches!(&h.state().0.modal, Some(Modal::NewTag { name, message, .. }) if name == "v2" && message.is_empty()));
    h.press_key(Key::Enter);
    h.run();
    let cmds = git_cmds(&h).expect("Enter creates the tag");
    assert_eq!(cmds[0], ["tag", "v2", sha.as_str()]);
    drop(h);
    remove_fixture(&dir);
}

/// In annotated mode, Enter in the name field with no message moves on to the message.
#[test]
fn new_tag_enter_moves_to_message() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-tag-enter");
    let sha = head(&st);
    st.modal = Some(Modal::NewTag { at: sha, name: "v3".into(), message: String::new(), annotated: true });
    let mut h = harness(st, true);
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert!(git_cmds(&h).is_none());
    h.input_mut().events.push(Event::Text("notes".into()));
    h.run();
    assert!(matches!(&h.state().0.modal, Some(Modal::NewTag { name, message, .. }) if name == "v3" && message == "notes"));
    drop(h);
    remove_fixture(&dir);
}
