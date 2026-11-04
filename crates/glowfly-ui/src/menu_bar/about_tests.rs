//! Help > About Glowfly (`about.rs`): its golden, the table and Copy details, the links, and an
//! unknown git. Closing it through the window is in `snapshot_tests.rs`.

use super::snapshot_tests::{dialog_harness, dialog_harness_in};
use crate::msg::{AppMsg, Msg};
use crate::state::git_env::GitState;
use crate::state::menu_bar::{BuildInfo, PRIVACY, WEBSITE};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::git_version::GitInfo;
use glowfly_core::i18n::Lang;

/// The About dialog alone, as `modals.rs` draws it, with a fixed version and the sample build
/// so a release or the CI system doesn't change the image.
fn about_harness(git: GitState) -> Harness<'static, (GitState, Vec<Msg>)> {
    dialog_harness(git, draw_about)
}

fn draw_about(ui: &mut egui::Ui, cx: crate::ui::dialog_frame::Cx, git: &GitState, msgs: &mut Vec<Msg>) -> bool {
    super::about(ui, cx, git, super::ThisCopy { version: "1.2.3", build: &BuildInfo::SAMPLE }, msgs)
}

fn git() -> GitState {
    GitState { info: Some(GitInfo { path: None, version: Some((2, 47, 1)), chosen: false }), ..GitState::default() }
}

#[test]
fn about_dialog_light() {
    let mut h = dialog_harness_in(git(), draw_about, Lang::En, false);
    h.run();
    h.check_snapshot("about_dialog_light");
}

#[test]
fn about_dialog() {
    let git = GitState { info: Some(GitInfo { path: None, version: Some((2, 47, 1)), chosen: false }), ..GitState::default() };
    let mut h = about_harness(git);
    h.run();
    h.check_snapshot("about_dialog");
    // The dialog, its padding included, fits inside the window.
    let rect = h.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("about-rect"))).unwrap();
    assert!(h.ctx.screen_rect().contains_rect(rect), "{rect:?}");
    for cell in ["Version", "1.2.3", "Git", "2.47.1", "OS", "Windows", "Build", "x86_64-pc-windows-msvc"] {
        assert!(h.query_by_label(cell).is_some(), "the table's {cell}");
    }
    assert!(h.query_by_label_contains("Version 1.2.3").is_none(), "the version only in the table");
    h.get_by_label("Copy details").click();
    h.run();
    let copied = "Glowfly 1.2.3\nGit: 2.47.1\nOS: Windows\nBuild: x86_64-pc-windows-msvc";
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(t) if t == copied)), "copies the table");
    assert!(h.query_by_label("Rust").is_none(), "the notices have their own dialog");
    let links = [("Third-party notices…", None), ("Privacy", Some(PRIVACY)), ("Website", Some(WEBSITE))];
    for (link, url) in links {
        h.state_mut().1.clear();
        h.get_by_label(link).click();
        h.run();
        let pushed = &h.state().1;
        match url {
            None => assert!(pushed.iter().any(|m| matches!(m, Msg::App(AppMsg::ShowNotices))), "{link}"),
            Some(u) => assert!(pushed.iter().any(|m| matches!(m, Msg::OpenUrl(x) if x == u)), "{link}"),
        }
    }
}

#[test]
fn about_says_when_git_is_unknown() {
    let mut h = about_harness(GitState::default());
    h.run();
    assert!(h.query_by_label("Unknown").is_some());
    assert!(h.query_by_label("2.47.1").is_none());
}
