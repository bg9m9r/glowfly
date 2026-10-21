//! The blame line menu (with a commit to step back to, and for a commit that added the file and
//! is not on the graph), and the blame header while finding, with steps to go back to, a
//! skipped commit and both toggles on.

use super::blame::menu::{LineTarget, line_menu};
use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_blame::with_blame;
use super::snapshot_tests_file_menu::menu_style;
use super::theme::{apply_theme, color};
use crate::msg::{BlameMsg, Msg};
use crate::state::blame::BlameStep;
use crate::state::blame_find::search;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use git2::Oid;
use glowfly_core::repo::BlameCommit;
use glowfly_core::settings::{BlameMoves, BlamePrefs, ThemeChoice};

// The English texts, pinned.
const BLAME_BEFORE: &str = "Blame before this commit";
const SKIP: &str = "Skip this commit";
const SHOW_IN_HISTORY: &str = "Show in file history";
const SELECT_ON_GRAPH: &str = "Select on the graph";
const COPY_LINE: &str = "Copy line";
const COPY_SHA: &str = "Copy commit SHA";
const NOTHING_BEFORE: &str = "This commit added these lines with the file, or older history is not in this clone";

const OID: &str = "3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29";
const LINE: &str = "    pub x: f64,";

fn commit() -> BlameCommit {
    BlameCommit {
        oid: Oid::from_str(OID).unwrap(),
        author: "Grace Hopper".into(),
        email: "grace@example.com".into(),
        time: 0,
        summary: "Make Point public with float fields".into(),
        body: String::new(),
    }
}

/// A small window drawing the line menu for line 4 inside a menu frame.
fn menu(before: bool, on_graph: bool) -> Harness<'static, Vec<Msg>> {
    let c = commit();
    // Wide enough for the disabled items' reasons after their labels.
    Harness::builder().with_size(egui::vec2(440.0, 190.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(true);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Dark);
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(200.0);
                menu_style(ui.style_mut());
                let t = LineTarget { line: 4, text: LINE, commit: &c, before, skipped: false, on_graph };
                line_menu(ui, glowfly_core::i18n::Lang::En, msgs, &t);
            });
        },
        Vec::new(),
    )
}

fn click(h: &mut Harness<'static, Vec<Msg>>, label: &str) -> Vec<Msg> {
    h.state_mut().clear();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(h.state_mut())
}

#[test]
fn blame_row_menu() {
    let oid = Oid::from_str(OID).unwrap();
    let mut h = menu(true, true);
    h.run();
    h.check_snapshot("blame_row_menu");
    assert!(matches!(&click(&mut h, BLAME_BEFORE)[..], [Msg::Blame(BlameMsg::BlameParent { line: 4 })]));
    assert!(matches!(&click(&mut h, SKIP)[..], [Msg::Blame(BlameMsg::SkipCommit(o))] if *o == oid));
    assert!(matches!(&click(&mut h, SHOW_IN_HISTORY)[..], [Msg::Blame(BlameMsg::OpenHistory { select: Some(o) })] if *o == oid));
    assert!(matches!(&click(&mut h, SELECT_ON_GRAPH)[..], [Msg::Blame(BlameMsg::SelectCommit(o))] if *o == oid));
    assert!(matches!(&click(&mut h, COPY_LINE)[..], [Msg::Copy(t)] if t == LINE));
    assert!(matches!(&click(&mut h, COPY_SHA)[..], [Msg::Copy(t)] if t == OID));
}

#[test]
fn blame_row_menu_first_commit() {
    let mut h = menu(false, false);
    h.run();
    assert!(h.get_by_label(BLAME_BEFORE).is_disabled());
    assert!(h.get_by_label(SELECT_ON_GRAPH).is_disabled());
    assert!(!h.get_by_label(SKIP).is_disabled());
    h.check_snapshot("blame_row_menu_first_commit");
    assert!(click(&mut h, BLAME_BEFORE).is_empty());
    assert!(click(&mut h, SELECT_ON_GRAPH).is_empty());
    h.get_by_label(BLAME_BEFORE).hover();
    h.run();
    h.run();
    assert!(h.query_by_label_contains(NOTHING_BEFORE).is_some(), "why it is off");
}

#[test]
fn blame_find() {
    let (mut st, dir, grace) = with_blame(ThemeChoice::Dark, "blame-find");
    st.settings.blame = BlamePrefs { ignore_whitespace: true, moves: BlameMoves::AcrossFiles, wrap_lines: false };
    let view = st.blame.as_mut().unwrap();
    let lines = view.data.as_ref().unwrap().blame.lines.clone();
    view.find.query = "Point".into();
    view.find.hits = search(&lines, "Point");
    assert_eq!(view.find.hits, [2, 3, 9]);
    view.find.current = Some(0);
    view.cursor = Some(2);
    view.skipped = vec![grace];
    view.back.push(BlameStep { path: "src/main.rs".into(), at: None, skipped: Vec::new(), top_line: 0, cursor: None });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("blame_find");
    assert!(h.query_by_label("1 of 3").is_some());
    assert!(!h.get_by_label("Back").is_disabled());
    assert!(h.query_by_label("Skipping 1 commit").is_some());
    h.get(By::new().role(Role::TextInput).value("Point")).type_text("s");
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Blame(BlameMsg::FindChanged))));
    drop(h);
    remove_fixture(&dir);
}
