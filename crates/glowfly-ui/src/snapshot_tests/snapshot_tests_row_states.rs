//! The state ladder on rows and the targets inside them (control rules R7 and R8), each in the
//! dark and light view palettes: a hovered and a focused pull request row on the dashboard, a
//! hovered and a focused sidebar section header, a Git Flow group row hovered and its '+'
//! hovered (the row then unlit), a pressed remote group, a Git Flow panel row lit under its
//! Finish button, the pull request's Checks header and a run hovered, the Preferences page list
//! with the current page and another one focused, the palette's keyboard row (its current item,
//! a selection; also in high contrast) beside a hovered one, and the diff's comment button and
//! draft marker. Each is a pixel golden only: the text of a hover is the same as the view's own
//! snapshot.

use super::snapshot_tests::{harness, harness_palette, remove_fixture, with_repo};
use super::snapshot_tests_dashboard::state as dashboard_state;
use super::snapshot_tests_gitflow::with_flow_branches;
use super::snapshot_tests_pr_check_list::{every_state, with_checks};
use super::snapshot_tests_pull_request_line_comment::opened;
use super::snapshot_tests_worktrees::{H, center};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::RemoteInfo;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::state::PrefsState;
use glowfly_model::state::pull_request_checks::ChecksPane;
use std::path::PathBuf;

/// Runs `case` in the dark and the light view palettes, as golden `<name>_dark` and
/// `<name>_light`, each with its own fixture folder.
fn both(name: &str, case: impl Fn(ThemeChoice, bool, &str)) {
    case(ThemeChoice::Dark, true, &format!("{name}_dark"));
    case(ThemeChoice::Light, false, &format!("{name}_light"));
}

/// A harness on `st`, `dark` or light, after its first frames.
fn open(st: glowfly_model::state::AppState, dark: bool) -> H {
    let mut h = harness(st, dark);
    h.run();
    h
}

/// Moves the pointer to `at`.
fn point(h: &mut H, at: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.run();
}

/// Moves the pointer to `at` and holds the primary button down there.
fn press(h: &mut H, at: Pos2) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(at));
    events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run();
}

/// Checks golden `name`, then removes the fixture at `dir`, if any.
fn finish(mut h: H, name: &str, dir: Option<PathBuf>) {
    h.check_snapshot(name);
    drop(h);
    if let Some(dir) = dir {
        remove_fixture(&dir);
    }
}

const PR_ROW: &str = "Speed up search";

#[test]
fn dashboard_pr_row_hovered() {
    both("dashboard_pr_row_hovered", |theme, dark, name| {
        let mut h = open(dashboard_state(theme), dark);
        let at = center(&h, PR_ROW);
        point(&mut h, at);
        finish(h, name, None);
    });
}

#[test]
fn dashboard_pr_row_focused() {
    both("dashboard_pr_row_focused", |theme, dark, name| {
        let mut h = open(dashboard_state(theme), dark);
        h.get_by_label(PR_ROW).focus();
        h.run();
        assert!(h.get_by_label(PR_ROW).is_focused());
        finish(h, name, None);
    });
}

#[test]
fn sidebar_section_header_hovered() {
    both("sidebar_section_header_hovered", |theme, dark, name| {
        let (st, dir) = with_repo(theme, &format!("rows-{name}"));
        let mut h = open(st, dark);
        let add = center(&h, "New branch…");
        point(&mut h, pos2(add.x - 80.0, add.y));
        finish(h, name, Some(dir));
    });
}

#[test]
fn sidebar_section_header_focused() {
    both("sidebar_section_header_focused", |theme, dark, name| {
        let (st, dir) = with_repo(theme, &format!("rows-{name}"));
        let mut h = open(st, dark);
        // The header row comes just before its + in the Tab order.
        h.get_by_label("New branch…").focus();
        h.run();
        h.input_mut().events.push(Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        });
        h.run();
        assert!(!h.get_by_label("New branch…").is_focused(), "the focus moved back to the header");
        finish(h, name, Some(dir));
    });
}

#[test]
fn sidebar_gitflow_group_plus_hovered() {
    both("sidebar_gitflow_group_plus_hovered", |theme, dark, name| {
        let (st, dir) = with_flow_branches(theme, &format!("rows-{name}"));
        let mut h = open(st, dark);
        let plus = h.get_by_label("Start feature").bounding_box().expect("the +");
        assert!(plus.width() >= 24.0 && plus.height() >= 24.0, "the + takes the least hit area: {plus:?}");
        let at = center(&h, "Start feature");
        point(&mut h, at);
        finish(h, name, Some(dir));
    });
}

#[test]
fn sidebar_gitflow_group_hovered() {
    both("sidebar_gitflow_group_hovered", |theme, dark, name| {
        let (st, dir) = with_flow_branches(theme, &format!("rows-{name}"));
        let mut h = open(st, dark);
        let at = center(&h, "Features");
        point(&mut h, at);
        finish(h, name, Some(dir));
    });
}

#[test]
fn sidebar_remote_group_pressed() {
    both("sidebar_remote_group_pressed", |theme, dark, name| {
        let (mut st, dir) = with_repo(theme, &format!("rows-{name}"));
        let snap = st.snap.as_mut().unwrap();
        let branches: Vec<_> = snap.locals.iter().filter_map(|b| Some((b.name.clone(), b.oid?))).take(2).collect();
        snap.remotes = vec![("origin".into(), branches)];
        snap.remote_info = vec![RemoteInfo {
            name: "origin".into(),
            urls: vec!["https://example.com/team/app.git".into()],
            push_urls: Vec::new(),
            skip_fetch_all: false,
        }];
        let mut h = open(st, dark);
        let at = center(&h, "origin");
        press(&mut h, at);
        finish(h, name, Some(dir));
    });
}

#[test]
fn gitflow_panel_row_hovered_over_finish() {
    both("gitflow_panel_row_hovered_over_finish", |theme, dark, name| {
        let (mut st, dir) = with_flow_branches(theme, &format!("rows-{name}"));
        st.session.gitflow_panel = true;
        let mut h = open(st, dark);
        let finish_at = h.get_all_by_label("Finish…").next().expect("a Finish button").bounding_box().expect("its rect");
        point(&mut h, pos2(((finish_at.x0 + finish_at.x1) / 2.0) as f32, ((finish_at.y0 + finish_at.y1) / 2.0) as f32));
        finish(h, name, Some(dir));
    });
}

/// The pull request with every kind of check listed.
fn checks(theme: ThemeChoice, dark: bool, name: &str) -> (H, PathBuf) {
    let (st, dir) = with_checks(theme, &format!("rows-{name}"), |v| v.checks.pane = ChecksPane::Loaded(every_state()));
    (open(st, dark), dir)
}

#[test]
fn pr_checks_header_hovered() {
    both("pr_checks_header_hovered", |theme, dark, name| {
        let (mut h, dir) = checks(theme, dark, name);
        let header = h.get_by_role_and_label(Role::Button, "Checks").bounding_box().expect("the header");
        point(&mut h, pos2(((header.x0 + header.x1) / 2.0) as f32, ((header.y0 + header.y1) / 2.0) as f32));
        finish(h, name, Some(dir));
    });
}

#[test]
fn pr_checks_run_hovered() {
    both("pr_checks_run_hovered", |theme, dark, name| {
        let (mut h, dir) = checks(theme, dark, name);
        let at = center(&h, "build: Failed");
        point(&mut h, at);
        finish(h, name, Some(dir));
    });
}

#[test]
fn prefs_nav_selected_focused_other() {
    both("prefs_nav_selected_focused_other", |theme, dark, name| {
        let mut st = glowfly_model::state::AppState::new(super::snapshot_tests::settings(theme), Vec::new(), std::time::Instant::now());
        st.prefs = Some(PrefsState::new(glowfly_model::state::PrefPage::Ui));
        let mut h = open(st, dark);
        let other = h.get_all_by_role(Role::Button).find(|n| n.label().as_deref() == Some("Accessibility")).expect("the Accessibility row");
        other.focus();
        h.run();
        finish(h, name, None);
    });
}

/// The palette over a repository in `theme` drawn in `p`, its keyboard row on the first command
/// and the pointer on Push, as golden `name`.
fn palette_scene(theme: ThemeChoice, p: crate::theme::color::Palette, name: &str) {
    let (mut st, dir) = with_repo(theme, &format!("rows-{name}"));
    st.palette = Some(glowfly_model::state::palette::Palette::default());
    let mut h = harness_palette(st, p);
    h.run();
    let at = center(&h, "Push");
    point(&mut h, at);
    let rows: Vec<_> = h.get_all_by_role(Role::Button).collect();
    let current: Vec<_> = rows.iter().filter(|n| n.is_selected() == Some(true)).filter_map(|n| n.label()).collect();
    assert_eq!(current, ["Stage all files"], "only the keyboard row is announced as selected");
    for row in ["Stage all files", "Push"] {
        assert!(h.get_by_role_and_label(Role::Button, row).toggled().is_none(), "{row} doesn't read as a toggle");
    }
    finish(h, name, Some(dir));
}

#[test]
fn palette_cursor_and_hover() {
    both("palette_cursor_and_hover", |theme, dark, name| palette_scene(theme, crate::theme::view_palette(dark), name));
    palette_scene(ThemeChoice::HighContrastDark, crate::theme::HIGH_CONTRAST_DARK, "palette_cursor_and_hover_high_contrast_dark");
}

#[test]
fn diff_comment_add_hovered() {
    both("diff_comment_add_hovered", |theme, dark, name| {
        let (st, dir, _) = opened(theme, &format!("rows-{name}"), |_| {}, true, None);
        let mut h = open(st, dark);
        let mark = h.get_by_role_and_label(Role::Button, "1 comment on this line").bounding_box().expect("the thread marker");
        point(&mut h, pos2(mark.x0 as f32 - 200.0, ((mark.y0 + mark.y1) / 2.0) as f32));
        let add = h.get_by_role_and_label(Role::Button, "Comment on this line").bounding_box().expect("the button");
        assert!(add.width() >= 23.5, "the button takes the least hit width: {add:?}");
        point(&mut h, pos2(((add.x0 + add.x1) / 2.0) as f32, ((add.y0 + add.y1) / 2.0) as f32));
        finish(h, name, Some(dir));
    });
}

#[test]
fn diff_draft_marker_rest() {
    both("diff_draft_marker_rest", |theme, dark, name| {
        let (st, dir, _) = opened(theme, &format!("rows-{name}"), |_| {}, false, Some("Could this say more?"));
        let h = open(st, dark);
        assert!(h.query_by_label("Your unposted comment").is_some());
        finish(h, name, Some(dir));
    });
}

#[test]
fn diff_draft_marker_hovered() {
    both("diff_draft_marker_hovered", |theme, dark, name| {
        let (st, dir, _) = opened(theme, &format!("rows-{name}"), |_| {}, false, Some("Could this say more?"));
        let mut h = open(st, dark);
        let at = center(&h, "Your unposted comment");
        point(&mut h, at);
        finish(h, name, Some(dir));
    });
}
