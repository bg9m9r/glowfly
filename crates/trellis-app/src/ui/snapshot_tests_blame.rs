//! The blame view: a Rust file whose lines come from three commits by two authors (dark and
//! light), a blame that failed, the Blame item under File history in file menus, clicking and
//! hovering a run's gutter, and the file history header's Blame button. Shares the fixture
//! repository and renderer with `snapshot_tests`.

use super::file_menu::{BLAME, history_items};
use super::snapshot_tests::{git, harness, remove_fixture, with_history, with_repo};
use super::snapshot_tests_file_menu::menu_style;
use super::theme::{apply_theme, color};
use crate::msg::BlameMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::blame::{BlameData, BlameView};
use crate::ui::common::short;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::highlight::{self, SyntaxHint};
use trellis_core::repo::{self, LoadOptions};
use trellis_core::settings::ThemeChoice;

const MAIN: &str = "use std::fmt;

/// A point on the plane.
struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    println!(\"{}\", p.x + p.y);
}
";

/// The fixture with src/main.rs (12 lines) added by Ada, lines 4-6 changed by Grace and line 10
/// by Ada again, blamed at HEAD into the view. Returns the state, the folder and Grace's commit.
fn with_blame(theme: ThemeChoice, name: &str) -> (AppState, PathBuf, Oid) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let write = |text: &str| std::fs::write(dir.join("src/main.rs"), text).unwrap();
    write(MAIN);
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "src/main.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add the point example"], d);
    let grace = MAIN.replace("struct Point {\n    x: i32,\n    y: i32,", "pub struct Point {\n    pub x: f64,\n    pub y: f64,");
    write(&grace);
    let by = "Grace Hopper <grace@example.com>";
    git(&dir, &["commit", "-q", "--author", by, "-m", "Make Point public with float fields", "src/main.rs"], "2026-01-12T10:00:00+00:00");
    let grace_oid = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    write(&grace.replace("x: 1, y: 2", "x: 1.0, y: 2.0"));
    git(&dir, &["commit", "-q", "-m", "Use float literals", "src/main.rs"], "2026-01-14T10:00:00+00:00");
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let blame = repo::blame_file(&dir, "src/main.rs", None).unwrap();
    let spans = highlight::highlight_file(SyntaxHint { path: "src/main.rs", language: None }, &blame.lines);
    assert!(spans.is_some(), "Rust is highlighted");
    let mut view = BlameView::loading("src/main.rs".into(), None);
    view.loading = false;
    view.data = Some(BlameData { blame, spans });
    st.blame = Some(view);
    (st, dir, grace_oid)
}

#[test]
fn blame_view() {
    let (st, dir, grace) = with_blame(ThemeChoice::Dark, "blame");
    let b = &st.blame.as_ref().unwrap().data.as_ref().unwrap().blame;
    let spans: Vec<(usize, usize)> = b.runs.iter().map(|r| (r.start, r.len)).collect();
    assert_eq!(spans, [(0, 3), (3, 3), (6, 3), (9, 1), (10, 2)]);
    assert_eq!(b.commits[b.runs[1].commit].oid, grace);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("blame_view");

    // A run's gutter selects its commit.
    h.get_by_label(&format!("Select {} on the graph", short(grace))).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Blame(BlameMsg::SelectCommit(o)) if *o == grace)));
    h.state_mut().1.clear();
    // Hovering it explains the click.
    h.get_by_label(&format!("Select {} on the graph", short(grace))).hover();
    h.run();
    h.run();
    assert!(h.query_by_label_contains("Click to select this commit on the graph").is_some());
    assert!(h.query_by_label_contains("grace@example.com").is_some());
    h.get_by_label("Close  (Esc)").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Blame(BlameMsg::Close))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_view_light() {
    let (st, dir, _) = with_blame(ThemeChoice::Light, "blame-light");
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("blame_view_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_error() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "blame-error");
    std::fs::write(dir.join("logo.png"), b"\x89PNG\r\n\x1a\n\x00\x00binary").unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "logo.png"], d);
    git(&dir, &["commit", "-q", "-m", "Add a logo"], d);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let why = repo::blame_file(&dir, "logo.png", None).unwrap_err().to_string();
    assert_eq!(why, "logo.png is a binary file; blame shows text files only");
    st.blame = Some(BlameView::failed("logo.png".into(), None, &why));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("blame_error");
    drop(h);
    remove_fixture(&dir);
}

const OID: &str = "3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29";

/// A small window drawing the File history and Blame items inside a menu frame.
fn menu(blocked: Option<&'static str>) -> Harness<'static, Vec<Msg>> {
    Harness::builder().with_size(egui::vec2(260.0, 110.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(true);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Dark);
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_width(220.0);
                menu_style(ui.style_mut());
                history_items(ui, msgs, "src/main.rs", Some(Oid::from_str(OID).unwrap()), blocked);
                let _ = ui.button("Copy path");
            });
        },
        Vec::new(),
    )
}

#[test]
fn blame_menu_commit_row() {
    let mut h = menu(None);
    h.run();
    h.check_snapshot("blame_menu_commit_row");
    h.get_by_label(BLAME).click();
    h.run();
    let at = Some(Oid::from_str(OID).unwrap());
    assert!(matches!(&h.state()[..], [Msg::Blame(BlameMsg::Open { path, at: a })] if path == "src/main.rs" && *a == at));
}

#[test]
fn blame_menu_deleted_row() {
    let mut h = menu(Some("This commit deletes the file"));
    h.run();
    assert!(h.get_by_label(BLAME).is_disabled());
    h.check_snapshot("blame_menu_deleted_row");
    h.get_by_label(BLAME).click();
    h.run();
    assert!(h.state().is_empty());
}

#[test]
fn history_header_blames_the_selected_entry() {
    // Entry 2 is from before the move, when the file was app.txt.
    let (st, dir) = with_history("history-blame", ThemeChoice::Dark, 500, 2);
    let e = st.history.as_ref().unwrap().entries[2].clone();
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(BLAME).click();
    h.run();
    assert!(
        matches!(&h.state().1[..], [Msg::Blame(BlameMsg::Open { path, at: Some(o) })] if *path == e.path && *o == e.oid),
        "blames the entry's path at its commit"
    );
    // Nothing selected: the button waits for a commit.
    h.state_mut().0.history.as_mut().unwrap().selected = None;
    h.state_mut().1.clear();
    h.run();
    assert!(h.get_by_label(BLAME).is_disabled());
    drop(h);
    remove_fixture(&dir);
}
