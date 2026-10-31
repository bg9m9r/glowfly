//! The menu bar and its menus, the About dialog, and what clicking items sends. Shares the
//! fixture repository and renderer with `ui/snapshot_tests.rs`.

use crate::journal::JournalMsg;
use crate::msg::{AppMsg, JobMsg, LayoutMsg, Msg, TabsMsg, UpdateMsg};
use crate::state::journal::JournalView;
use crate::state::layout::Panel;
use crate::state::menu_bar::PLACEHOLDER_VERSION as VERSION;
use crate::state::{AppState, Modal, git_env::GitState};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::ui::theme::{self, color};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::git_version::GitInfo;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::undo::{Action, HeadPos};
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The fixture with the menu bar shown (the shared settings hide it).
pub(in crate::ui) fn with_bar(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
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

/// Hovers `label` and rests there, past a submenu's open delay.
fn hover(h: &mut H, label: &str) {
    h.get_by_label(label).hover();
    for _ in 0..4 {
        h.step();
    }
    h.run_ok();
}

fn sent(h: &H, f: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(f)
}

/// Opens menu `title`, clicks `item`, and returns the harness.
fn click(name: &str, title: &str, item: &str, prepare: impl FnOnce(&mut AppState)) -> (H, PathBuf) {
    let (mut st, dir) = with_bar(ThemeChoice::Dark, name);
    prepare(&mut st);
    let mut h = opened(st, true, title);
    // The tab bar's + is named New tab too; the menu's item is the wider one.
    let width = |n: &egui_kittest::kittest::Node<'_>| n.bounding_box().map_or(0.0, |b| b.x1 - b.x0);
    let node = h.query_all_by_label(item).max_by(|a, b| width(a).total_cmp(&width(b))).unwrap_or_else(|| panic!("{item}"));
    node.click();
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
    st.settings.favorite_repos = vec![glowfly_core::paths::encode(&base.join("web-app"))];
    let mut h = opened(st, true, "File");
    hover(&mut h, "Open recent");
    h.check_snapshot("menu_bar_file_recent");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_edit() {
    let (mut st, dir) = with_bar(ThemeChoice::Dark, "menu-bar-edit");
    st.session.undo.record(Action::Checkout { from: HeadPos::Branch("search".into()), to: HeadPos::Branch("main".into()), created: None });
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
    let why = st.session.undo.redo_entry(st.settings.lang(), true).1.expect("nothing to redo");
    let mut h = opened(st, true, "Edit");
    // Keyed and disabled: found by its label alone, greyed with the info mark beside it.
    assert!(h.get_by_label("Redo").is_disabled());
    h.get_by_label("Redo").click();
    h.run();
    assert!(!sent(&h, |m| matches!(m, Msg::Job(JobMsg::Redo))), "a disabled item sends nothing");
    assert!(h.query_by_label(&why).is_none(), "no reason text in the menu");
    h.get_by_label("Redo").focus();
    h.run();
    assert!(h.query_by_label(&why).is_some(), "the reason under the focused item");
    assert!(h.get_by_label("Redo").is_disabled());
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
    // The two diff choices each under their own label.
    let top = |label: &str| h.get_by_label(label).bounding_box().expect(label).y0;
    assert!(top("Diff layout") < top("Inline"), "Diff layout heads the layout choices");
    assert!(top("Inline") < top("Diff shows"), "Diff shows comes after them");
    assert!(top("Diff shows") < top("Changed lines only"), "and heads the mode choices");
    drop(h);
    // With the journal open: its item is ticked.
    let (mut st, _) = with_bar(ThemeChoice::Light, "menu-bar-view");
    st.session.journal.view = Some(JournalView { timeline: Some(Ok(Default::default())), ..Default::default() });
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
    assert_eq!(h.get_by_label("Operation journal").toggled(), Some(Toggled::False));
    let full = h.get_by_label("Full screen");
    assert_eq!((full.role(), full.toggled()), (Role::CheckBox, Some(Toggled::False)));
    drop(h);
    let (mut st, _) = with_bar(ThemeChoice::Dark, "menu-bar-view-checks");
    st.view.window.fullscreen = true;
    let h = opened(st, true, "View");
    assert_eq!(h.get_by_label("Full screen").toggled(), Some(Toggled::True));
    drop(h);
    let (mut st, _) = with_bar(ThemeChoice::Dark, "menu-bar-view-checks");
    st.session.journal.view = Some(JournalView { timeline: Some(Ok(Default::default())), ..Default::default() });
    let h = opened(st, true, "View");
    let journal = h.query_all_by_label("Operation journal").find(|n| n.role() == Role::CheckBox).expect("the check item");
    assert_eq!(journal.toggled(), Some(Toggled::True));
    drop(h);
    let (mut st, _) = with_bar(ThemeChoice::Dark, "menu-bar-view-checks");
    st.snap = None;
    let h = opened(st, true, "View");
    assert!(h.get_by_label("Operation journal").is_disabled(), "no repository");
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
    let cases: [Case; 10] = [
        ("menu-click-new-tab", "File", "New tab", |m| matches!(m, Msg::Tabs(TabsMsg::NewTab))),
        ("menu-click-quit", "File", "Quit", |m| matches!(m, Msg::App(AppMsg::Quit))),
        ("menu-click-zoom", "View", "Zoom in", |m| matches!(m, Msg::Layout(LayoutMsg::ZoomIn))),
        ("menu-click-hide-bar", "View", "Menu bar", |m| {
            matches!(m, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::MenuBar, shown: false }))
        }),
        ("menu-click-left-panel", "View", "Left panel", |m| {
            matches!(m, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::Left, shown: false }))
        }),
        ("menu-click-full-screen", "View", "Full screen", |m| matches!(m, Msg::Layout(LayoutMsg::ToggleFullscreen))),
        ("menu-click-journal", "View", "Operation journal", |m| matches!(m, Msg::Journal(JournalMsg::Toggle))),
        ("menu-click-about", "Help", "About Glowfly…", |m| matches!(m, Msg::App(AppMsg::ShowAbout))),
        ("menu-click-check-updates", "Help", "Check for updates", |m| matches!(m, Msg::Updates(UpdateMsg::CheckNow))),
        ("menu-click-export-logs", "Help", "Export logs…", |m| matches!(m, Msg::Logs(crate::logs::LogsMsg::Export))),
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
    assert!(h.query_by_label("Third-party notices…").is_some());
    h.get_by_label("Close").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

/// The About dialog alone, as `modals.rs` draws it, with a fixed version so a release doesn't
/// change the image.
fn about_harness(git: GitState) -> Harness<'static, (GitState, Vec<Msg>)> {
    dialog_harness(git, |ui, cx, git, msgs| super::about(ui, cx, git, "1.2.3", msgs))
}

/// The third-party notices dialog alone, as `modals.rs` draws it.
fn notices_harness() -> Harness<'static, (GitState, Vec<Msg>)> {
    dialog_harness(GitState::default(), |ui, cx, _, msgs| super::notices(ui, cx, msgs))
}

type DrawDialog = fn(&mut egui::Ui, crate::ui::dialog_frame::Cx, &GitState, &mut Vec<Msg>) -> bool;

/// A dialog drawn by `draw` in the dark theme, its rect kept under "about-rect".
fn dialog_harness(git: GitState, draw: DrawDialog) -> Harness<'static, (GitState, Vec<Msg>)> {
    crate::ui::snapshot_check::builder().with_size(egui::vec2(1024.0, 700.0)).build_state(
        move |ctx, (git, msgs): &mut (GitState, Vec<Msg>)| {
            color::set_palette(&theme::GARDEN_DARK);
            theme::apply_style(ctx, &theme::GARDEN_DARK);
            ctx.set_theme(theme::egui_theme(&theme::GARDEN_DARK));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg())).show(ctx, |_| {});
            let id = egui::Id::new("glowfly-modal");
            let cx = crate::ui::dialog_frame::Cx { t: glowfly_core::i18n::Lang::En, keyboard: true, key: id.with("about") };
            let resp = theme::show_modal(ctx, theme::dialog(ctx, id), |ui| draw(ui, cx, git, msgs));
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
    // The dialog, its padding included, fits inside the window.
    let rect = h.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("about-rect"))).unwrap();
    assert!(h.ctx.screen_rect().contains_rect(rect), "{rect:?}");
    assert!(h.query_by_label("Git 2.47.1").is_some());
    h.get_by_label("Copy version info").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(t) if t.starts_with("Glowfly ") && t.ends_with("\nGit 2.47.1"))));
    assert!(h.query_by_label("Rust").is_none(), "the notices have their own dialog");
    h.get_by_label("Third-party notices…").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::App(AppMsg::ShowNotices))));
}

#[test]
fn notices_dialog() {
    let mut h = notices_harness();
    h.run();
    h.check_snapshot("notices_dialog");
    let rect = h.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("about-rect"))).unwrap();
    assert!(h.ctx.screen_rect().contains_rect(rect), "{rect:?}");
    assert!(h.query_by_label("Third-party notices").is_some());
    h.get_by_label("Copy all notices").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(t) if t == glowfly_core::notices::TEXT)));
    h.get_by_label("Back").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::App(AppMsg::ShowAbout))));
}

#[test]
fn notices_dialog_open() {
    let mut h = notices_harness();
    h.run();
    h.get_by_label("Rust").click();
    h.run();
    h.check_snapshot("notices_dialog_open");
}

#[test]
fn about_leaves_out_an_unknown_git() {
    let mut h = about_harness(GitState::default());
    h.run();
    assert!(h.query_by_label_contains("Git 2").is_none());
}
