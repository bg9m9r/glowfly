//! The trellis:// link dialogs (a link Trellis can't open, Repository not found with and
//! without a URL to clone, a located folder that doesn't match, choosing among several copies)
//! and Preferences > General's Trellis Links row. Each test clicks a button and checks the
//! message it sends.

use super::snapshot_tests::{harness, harness_sized, settings};
use crate::deep_link::{DeepLinkMsg, FindOrigin};
use crate::msg::Msg;
use crate::state::deep_link::{LinkDialog, SchemeView};
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::link::{Link, Target};
use trellis_core::settings::ThemeChoice;
use trellis_core::url_scheme::SchemeStatus;

const ROOT: &str = "0123456789abcdef0123456789abcdef01234567";

fn app(theme: ThemeChoice) -> AppState {
    AppState::new(settings(theme), Vec::new(), Instant::now())
}

fn send(st: &mut AppState, m: DeepLinkMsg) {
    crate::update::update(st, Msg::DeepLink(m), Instant::now());
}

/// Opens `link` and answers its search with no match: the Repository not found dialog.
fn not_found(link: Link) -> AppState {
    let mut st = app(ThemeChoice::Dark);
    send(&mut st, DeepLinkMsg::Open(link.to_uri()));
    let seq = st.deep_link.seq;
    send(&mut st, DeepLinkMsg::Found { seq, origin: FindOrigin::Search, result: Ok(Vec::new()) });
    assert!(matches!(st.modal, Some(Modal::DeepLink(LinkDialog::NotFound { .. }))));
    st
}

fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: &dyn Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

fn branch(url: Option<&str>) -> Link {
    Link { url: url.map(str::to_owned), root: Some(ROOT.into()), target: Target::Branch("feature/login".into()) }
}

#[test]
fn deep_link_invalid() {
    let mut st = app(ThemeChoice::Dark);
    send(&mut st, DeepLinkMsg::Open("trellis://draft/42?root=0123".into()));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("deep_link_invalid");
    h.get_by_label("Close").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::CloseModal)));
}

#[test]
fn deep_link_not_found() {
    let st = not_found(branch(Some("https://github.com/example/app.git")));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("deep_link_not_found");
    h.get_by_label("Locate on disk…").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Locate))));
    h.get_by_label("Clone…").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Clone))));
}

#[test]
fn deep_link_not_found_no_url() {
    let st = not_found(branch(None));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("deep_link_not_found_no_url");
    assert!(h.get_by_label("Clone…").is_disabled(), "nothing to clone from");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::CloseModal)));
}

#[test]
fn deep_link_not_found_problem() {
    let mut st = not_found(branch(Some("git@github.com:example/app.git")));
    let folder = if cfg!(windows) { r"D:\work\other-app" } else { "/home/me/work/other-app" };
    crate::update::update(
        &mut st,
        Msg::Prefs(crate::msg::PrefsMsg::FolderPicked { purpose: crate::msg::FolderPurpose::LocateLinkedRepo, path: Some(folder.into()) }),
        Instant::now(),
    );
    let seq = st.deep_link.seq;
    send(&mut st, DeepLinkMsg::Found { seq, origin: FindOrigin::Located, result: Ok(Vec::new()) });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("deep_link_not_found_problem");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::CloseModal)));
}

#[test]
fn deep_link_choose() {
    let mut st = app(ThemeChoice::Light);
    let paths: Vec<PathBuf> = if cfg!(windows) {
        vec![r"C:\src\app".into(), r"D:\backup\app".into()]
    } else {
        vec!["/home/me/src/app".into(), "/mnt/backup/app".into()]
    };
    st.modal = Some(Modal::DeepLink(LinkDialog::Choose { what: "tag v1.4.0".into(), paths: paths.clone(), picked: 1 }));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("deep_link_choose");
    h.get_by_label(&trellis_core::native_path::shown(&paths[0])).click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Choose(0)))));
    h.get_by_label("Open").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::OpenChosen))));
    assert!(!sent(&h, &|m| matches!(m, Msg::CloseModal)), "Open doesn't close first");
}

/// Preferences > General, tall enough to show the Trellis Links row at its end.
fn general(view: SchemeView) -> egui_kittest::Harness<'static, (AppState, Vec<Msg>)> {
    let mut st = app(ThemeChoice::Dark);
    st.prefs = Some(PrefsState::new(PrefPage::General));
    st.deep_link.scheme = view;
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 1620.0));
    h.run();
    h
}

#[test]
fn prefs_general_links_registered() {
    let mut h = general(SchemeView::Known(SchemeStatus::Registered));
    if cfg!(target_os = "macos") {
        return; // the row shows only the macOS note there
    }
    h.check_snapshot("prefs_general_links_registered");
    assert!(!h.get_by_label("Unregister").is_disabled());
    h.get_by_label("Register").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Register))));
    h.get_by_label("Unregister").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Unregister))));
}

#[test]
fn prefs_general_links_not_registered() {
    let mut h = general(SchemeView::Known(SchemeStatus::NotRegistered));
    if cfg!(target_os = "macos") {
        return;
    }
    h.check_snapshot("prefs_general_links_not_registered");
    assert!(h.get_by_label("Unregister").is_disabled(), "nothing to remove");
    h.get_by_label("Register").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Register))));
    // Another program holds the scheme: Register this copy.
    h.state_mut().0.deep_link.scheme = SchemeView::Known(SchemeStatus::OtherProgram(r#""C:\Tools\Other\other.exe" "%1""#.into()));
    h.run();
    assert!(h.query_by_label("Register this copy").is_some());
}
