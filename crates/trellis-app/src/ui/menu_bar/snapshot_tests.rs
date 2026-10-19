//! The menu bar and its menus, the About dialog, and what clicking items sends. Shares the
//! fixture repository and renderer with `ui/snapshot_tests.rs`.

use crate::msg::{AppMsg, JobMsg, LayoutMsg, Msg, TabsMsg, UpdateMsg};
use crate::state::layout::Panel;
use crate::state::menu_bar::VERSION;
use crate::state::{AppState, Modal, git_env::GitState};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::ui::theme::{self, color};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::git_version::GitInfo;
use trellis_core::settings::ThemeChoice;
use trellis_core::undo::{Action, HeadPos};

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The fixture with the menu bar shown (the shared settings hide it).
fn with_bar(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.settings.show_menu_bar = true;
    (st, dir)
}

/// A harness drawing `st`, with the menu `title` clicked open.
fn opened(st: AppState, dark: bool, title: &str) -> H {
    let mut h = harness(st, dark);
    h.run();
    h.get_by_label(title).click();
    h.run();
    h
}

fn hover(h: &mut H, label: &str) {
    h.get_by_label(label).hover();
    for _ in 0..4 {
        h.run();
    }
}

fn sent(h: &H, f: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(f)
}

/// Opens menu `title`, clicks `item`, and returns the harness.
fn click(name: &str, title: &str, item: &str, prepare: impl FnOnce(&mut AppState)) -> (H, PathBuf) {
    let (mut st, dir) = with_bar(ThemeChoice::Dark, name);
    prepare(&mut st);
    let mut h = opened(st, true, title);
    h.get_by_label(item).click();
    h.run();
    (h, dir)
}

#[test]
fn menu_bar_closed() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-closed");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("menu_bar_closed");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_hidden_draws_nothing() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "menu-bar-hidden");
    assert!(!st.settings.show_menu_bar);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Edit").is_none());
    assert!(h.query_by_label("Window").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_file() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-file");
    let mut h = opened(st, true, "File");
    h.check_snapshot("menu_bar_file");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_file_recent() {
    let (mut st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-recent");
    // Labels only (never opened), absolute on every OS and the same on every machine.
    let base = PathBuf::from(if cfg!(windows) { "C:/work" } else { "/work" });
    st.recent = vec![base.join("api-server"), base.join("docs")];
    st.settings.favorite_repos = vec![trellis_core::paths::encode(&base.join("web-app"))];
    let mut h = opened(st, true, "File");
    hover(&mut h, "Open recent");
    h.check_snapshot("menu_bar_file_recent");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_edit() {
    let (mut st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-edit");
    st.undo.record(Action::Checkout { from: HeadPos::Branch("search".into()), to: HeadPos::Branch("main".into()), created: None });
    let mut h = opened(st, true, "Edit");
    h.check_snapshot("menu_bar_edit");
    h.get_by_label("Undo checkout of main").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Job(JobMsg::Undo))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn disabled_items_keep_their_labels() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-disabled-labels");
    let mut h = opened(st, true, "Edit");
    // Keyed and disabled: found by its label alone, its reason's short form painted beside it.
    assert!(h.get_by_label("Redo").is_disabled());
    h.get_by_label("Redo").click();
    h.run();
    assert!(!sent(&h, |m| matches!(m, Msg::Job(JobMsg::Redo))), "a disabled item sends nothing");
    drop(h);

    remove_fixture(&dir);

    let (mut st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-disabled-recent");
    st.recent.clear();
    st.settings.favorite_repos.clear();
    let h = opened(st, true, "File");
    assert!(h.get_by_label("Open recent").is_disabled(), "a blocked submenu is a disabled item");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_view_dark_and_light() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-view");
    let mut h = opened(st, true, "View");
    h.check_snapshot("menu_bar_view");
    drop(h);
    let (st, _) = with_bar(ThemeChoice::Light, "menu-bar-view");
    let mut h = opened(st, false, "View");
    h.check_snapshot("menu_bar_view_light");
    drop(h);
    remove_fixture(&dir);
}

/// The panel items are check items: a checkbox each, ticked while the panel is shown.
#[test]
fn view_menu_checks_the_shown_panels() {
    use eframe::egui::accesskit::{Role, Toggled};
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-view-checks");
    let h = opened(st, true, "View");
    for label in ["Left panel", "Commit details panel", "Toolbar", "Menu bar"] {
        let node = h.get_by_label(label);
        assert_eq!((node.role(), node.toggled()), (Role::CheckBox, Some(Toggled::True)), "{label}");
    }
    assert_eq!(h.get_by_label("Terminal").toggled(), Some(Toggled::False));
    assert_eq!(h.get_by_label("Activity log").toggled(), Some(Toggled::False));
    drop(h);
    let (mut st, _) = with_bar(ThemeChoice::Dark, "menu-bar-view-checks");
    st.settings.show_left_panel = false;
    let h = opened(st, true, "View");
    assert_eq!(h.get_by_label("Left panel").toggled(), Some(Toggled::False));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_view_theme() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-theme");
    let mut h = opened(st, true, "View");
    hover(&mut h, "Theme");
    h.check_snapshot("menu_bar_view_theme");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_window() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-window");
    let mut h = opened(st, true, "Window");
    h.check_snapshot("menu_bar_window");
    h.get_by_label("api-server").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Tabs(TabsMsg::SelectTab(1)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_help() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-help");
    let mut h = opened(st, true, "Help");
    h.check_snapshot("menu_bar_help");
    drop(h);
    remove_fixture(&dir);
}

/// A test folder, a menu, an item in it, and the message it should send.
type Case = (&'static str, &'static str, &'static str, fn(&Msg) -> bool);

#[test]
fn menu_items_send_their_messages() {
    let cases: [Case; 7] = [
        ("menu-click-new-tab", "File", "New tab", |m| matches!(m, Msg::Tabs(TabsMsg::NewTab))),
        ("menu-click-quit", "File", "Quit", |m| matches!(m, Msg::App(AppMsg::Quit))),
        ("menu-click-zoom", "View", "Zoom in", |m| matches!(m, Msg::Layout(LayoutMsg::ZoomIn))),
        ("menu-click-hide-bar", "View", "Menu bar", |m| {
            matches!(m, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::MenuBar, shown: false }))
        }),
        ("menu-click-left-panel", "View", "Left panel", |m| {
            matches!(m, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::Left, shown: false }))
        }),
        ("menu-click-about", "Help", "About Trellis…", |m| matches!(m, Msg::App(AppMsg::ShowAbout))),
        ("menu-click-check-updates", "Help", "Check for updates", |m| matches!(m, Msg::Updates(UpdateMsg::CheckNow))),
    ];
    for (name, title, item, want) in cases {
        let (h, dir) = click(name, title, item, |_| {});
        assert!(sent(&h, want), "{title} > {item}");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn about_through_the_window() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "about-window");
    st.modal = Some(Modal::About);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&format!("Version {VERSION}")).is_some());
    assert!(h.query_by_label("Third-party notices").is_some());
    h.get_by_label("Close").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

/// The About dialog alone, as `modals.rs` draws it, with a fixed version so a release doesn't
/// change the image.
fn about_harness(git: GitState) -> Harness<'static, (GitState, Vec<Msg>)> {
    Harness::builder().with_size(egui::vec2(1024.0, 700.0)).build_state(
        |ctx, (git, msgs): &mut (GitState, Vec<Msg>)| {
            color::set_palette(&theme::DARK);
            theme::apply_style(ctx, &theme::DARK);
            ctx.set_theme(theme::egui_theme(&theme::DARK));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg())).show(ctx, |_| {});
            let id = egui::Id::new("trellis-modal");
            let cx = crate::ui::dialog_frame::Cx { t: trellis_core::i18n::Lang::En, keyboard: true, key: id.with("about") };
            let resp = theme::dialog(ctx, id).show(ctx, |ui| super::about(ui, cx, git, "1.2.3", msgs));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("about-rect"), resp.response.rect));
        },
        (git, Vec::new()),
    )
}

#[test]
fn about_dialog() {
    let git = GitState { info: Some(GitInfo { path: None, version: Some((2, 47, 1)), chosen: false }), ..GitState::default() };
    let mut h = about_harness(git);
    h.run();
    h.check_snapshot("about_dialog");
    // The resizable dialog, its padding included, fits inside the window.
    let rect = h.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("about-rect"))).unwrap();
    assert!(h.ctx.screen_rect().contains_rect(rect), "{rect:?}");
    assert!(h.query_by_label("Git 2.47.1").is_some());
    h.get_by_label("Copy version info").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(t) if t.starts_with("Trellis ") && t.ends_with("\nGit 2.47.1"))));
    h.get_by_label("Rust").click();
    h.run();
    h.check_snapshot("about_dialog_notice");
    h.get_by_label("Copy all notices").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(t) if t == trellis_core::notices::TEXT)));
}

#[test]
fn about_leaves_out_an_unknown_git() {
    let mut h = about_harness(GitState::default());
    h.run();
    assert!(h.query_by_label_contains("Git 2").is_none());
}
