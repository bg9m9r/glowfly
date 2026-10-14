//! The REMOTE section (a remote group row with branches and one without, the group and header
//! menus) and the Add/Edit remote dialog (a taken name, a rename note, Save disabled while
//! nothing changed). Remotes are injected into the shared fixture's snapshot with fixed URLs so
//! the images are deterministic.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, center, pushed};
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::remotes::RemoteForm;
use crate::state::{AppState, Modal, RunningJob};
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::repo::RemoteInfo;
use trellis_core::settings::ThemeChoice;

/// The fixture with `origin` (two branches, a separate push URL with a password) and `backup`
/// (no branches yet).
fn with_remotes(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    let main = snap.locals.iter().find(|b| b.name == "main").and_then(|b| b.oid).unwrap();
    let search = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    snap.remotes = vec![("origin".into(), vec![("main".into(), main), ("search".into(), search)]), ("backup".into(), Vec::new())];
    snap.remote_info = vec![
        RemoteInfo {
            name: "origin".into(),
            urls: vec!["https://example.com/team/app.git".into()],
            push_urls: vec!["https://dev:secret@example.com/team/app-push.git".into()],
        },
        RemoteInfo { name: "backup".into(), urls: vec!["D:/backups/app.git".into()], push_urls: Vec::new() },
    ];
    (st, dir)
}

fn form(h: &mut H) -> &mut RemoteForm {
    match h.state_mut().0.modal.as_mut() {
        Some(Modal::Remote(f)) => f,
        _ => panic!("the dialog is closed"),
    }
}

pub(super) fn running(st: &mut AppState) {
    st.job = Some(RunningJob {
        label: "Fetch all".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_open: None,
        keeps_undo: true,
    });
}

#[test]
fn remote_add_dialog() {
    let (mut st, dir) = with_remotes(ThemeChoice::Dark, "remote-add-dialog");
    let mut f = RemoteForm::add(st.snap.as_ref().unwrap());
    f.name = "origin".into();
    f.url = "https://example.com/team/mirror.git".into();
    st.modal = Some(Modal::Remote(f));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("A remote named origin already exists.").is_some());
    assert!(h.get_by_role_and_label(Role::Button, "Add remote").is_disabled());
    h.snapshot("remote_add_dialog");

    form(&mut h).name = "backup2".into();
    h.run();
    assert!(!h.get_by_role_and_label(Role::Button, "Add remote").is_disabled());
    h.get_by_role_and_label(Role::Button, "Add remote").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::Submit))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_edit_dialog_light() {
    let (mut st, dir) = with_remotes(ThemeChoice::Light, "remote-edit-dialog");
    let origin = st.snap.as_ref().unwrap().remote_info[0].clone();
    let mut f = RemoteForm::edit(&origin);
    f.name = "upstream".into();
    st.modal = Some(Modal::Remote(f));
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("Renaming also renames its remote-tracking branches and updates the branches that track it.").is_some());
    assert!(!h.get_by_label("Save").is_disabled());
    h.snapshot("remote_edit_dialog_light");

    form(&mut h).name = "origin".into();
    h.run();
    assert!(h.get_by_label("Save").is_disabled(), "nothing changed");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_group_menu() {
    let (st, dir) = with_remotes(ThemeChoice::Dark, "remote-group-menu");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("No branches yet. Fetch this remote to load them.").is_some());
    let at = center(&h, "origin");
    click(&mut h, at, PointerButton::Secondary);
    for item in ["Fetch origin", "Edit remote…", "Copy fetch URL", "Copy push URL", "Remove remote…"] {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    h.snapshot("remote_group_menu");
    h.get_by_label("Fetch origin").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::Fetch(n)) if n == "origin")));

    let at = center(&h, "backup");
    click(&mut h, at, PointerButton::Secondary);
    assert!(h.query_by_label("Fetch backup").is_some());
    assert!(h.query_by_label("Copy push URL").is_none());
    h.get_by_label("Copy fetch URL").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Copy(u) if u == "D:/backups/app.git")));
    h.press_key(eframe::egui::Key::Escape);
    h.run();
    drop(h);
    remove_fixture(&dir);
}

/// A click at `pos` stepped frame by frame: a running job's spinner keeps repainting, so
/// `Harness::run` would never settle.
pub(super) fn click_steps(h: &mut H, pos: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(pos));
        events.push(Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run_steps(3);
}

#[test]
fn remote_header_menu() {
    let (st, dir) = with_remotes(ThemeChoice::Dark, "remote-header-menu");
    let mut h = harness(st, true);
    h.run();
    // The header is painted without a label: it sits right above the first remote's row.
    let row = h.get_by_label("origin").bounding_box().expect("the row");
    let at = pos2(((row.x0 + row.x1) / 2.0) as f32, row.y0 as f32 - 15.0);
    click(&mut h, at, PointerButton::Secondary);
    h.get_by_label("Add remote…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::AddDialog))));

    running(&mut h.state_mut().0);
    h.run_steps(2);
    let at = center(&h, "origin");
    click_steps(&mut h, at, PointerButton::Secondary);
    assert!(h.get_by_label("Fetch origin").is_disabled(), "a job is running");
    drop(h);
    remove_fixture(&dir);
}
