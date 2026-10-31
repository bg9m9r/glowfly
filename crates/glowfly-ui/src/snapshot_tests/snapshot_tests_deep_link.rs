//! The glowfly:// link dialogs (a link Glowfly can't open, one of a kind only a newer version
//! knows, Repository not found with and without a URL to clone, a located folder that doesn't
//! match, choosing among several copies) and Preferences > General's Glowfly Links row. Each
//! test clicks a button and checks the message it sends.

use super::snapshot_tests::{harness, harness_sized, settings};
use crate::deep_link::{DeepLinkMsg, FindOrigin};
use crate::msg::{Msg, UpdateMsg};
use crate::state::deep_link::{LinkDialog, PickReason, SchemeView};
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::link::{Link, Target};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::url_scheme::SchemeStatus;
use std::path::PathBuf;
use std::time::Instant;

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
    send(&mut st, DeepLinkMsg::Open("glowfly://branch/feature?root=0123".into()));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("deep_link_invalid");
    h.get_by_label("Close").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::CloseModal)));
}

#[test]
fn deep_link_newer() {
    let mut st = app(ThemeChoice::Dark);
    send(&mut st, DeepLinkMsg::Open("glowfly://draft/42?root=0123".into()));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("deep_link_newer");
    assert!(h.query_by_label("This link needs a newer Glowfly").is_some());
    h.get_by_label("Check for updates").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::Updates(UpdateMsg::CheckNow))));
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
    let reason = Some(PickReason::OpenTab);
    st.modal = Some(Modal::DeepLink(LinkDialog::Choose { what: "tag v1.4.0".into(), paths: paths.clone(), picked: 0, reason }));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("deep_link_choose");
    assert!(h.query_by_label("open in a tab").is_some(), "the preselected copy says why");
    h.get_by_label(&glowfly_core::native_path::shown(&paths[1])).click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Choose(1)))));
    h.get_by_label("Open").click();
    h.run();
    assert!(sent(&h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::OpenChosen))));
    assert!(!sent(&h, &|m| matches!(m, Msg::CloseModal)), "Open doesn't close first");
}

/// The height that shows all of Preferences > General, the Glowfly Links row at its end.
const GENERAL_HEIGHT: f32 = 1800.0;

/// Preferences > General, tall enough to show the Glowfly Links row at its end.
fn general(view: SchemeView) -> egui_kittest::Harness<'static, (AppState, Vec<Msg>)> {
    let mut st = app(ThemeChoice::Dark);
    st.prefs = Some(PrefsState::new(PrefPage::General));
    st.deep_link.scheme = view;
    let mut h = harness_sized(st, true, egui::vec2(1400.0, GENERAL_HEIGHT));
    h.run();
    if !cfg!(target_os = "macos") {
        in_view(&h, "Unregister");
    }
    h
}

/// Asserts that the widget labelled `label` is drawn clear of the window's bottom, above the
/// status bar and the page's scroll cue, so a snapshot of the page shows it whole.
fn in_view(h: &egui_kittest::Harness<'static, (AppState, Vec<Msg>)>, label: &str) {
    const BOTTOM_MARGIN: f64 = 80.0;
    let bounds = h.get_by_label(label).bounding_box().expect("a drawn widget has bounds");
    let bottom = bounds.y1 / f64::from(h.ctx.pixels_per_point());
    let limit = f64::from(GENERAL_HEIGHT) - BOTTOM_MARGIN;
    assert!(bottom <= limit, "{label} ends at {bottom}, below {limit} in the {GENERAL_HEIGHT} px window");
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
