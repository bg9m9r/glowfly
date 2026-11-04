//! Layout and message tests of the diff header, drawn alone from a `HeaderModel`.

use super::*;
use crate::file_menu::{OPEN_DEFAULT, SHOW_IN_FOLDER};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::DiffTarget;
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_model::keymap::BindingId;
use glowfly_model::msg::{DiffMsg, FilesMsg, RepoMsg, StageMsg};
use glowfly_model::state::diff_header::{
    Actions, ChangeText, Context, FileKind, FoldCounts, OpenAction, Primary, RestoreAction, ViewGroup,
};
use glowfly_model::state::diff_nav::{Counter, StepKind};
use glowfly_model::state::tool_compare::OPEN_IN_DIFF_TOOL;
use glowfly_model::tools::ToolsMsg;

// The English texts, pinned: the header reads them from the catalog.
const CLOSE_NAME: &str = "Close diff";
const MORE: &str = "More file actions";
const OPEN: &str = "Open";
const RESTORE: &str = "Restore…";
const STAGE_FILE: &str = "Stage file";
const UNSTAGE_FILE: &str = "Unstage file";
const NEXT_NAME: &str = "Next change";
const PREV_NAME: &str = "Previous change";
const COLLAPSE_ALL: &str = "Collapse all";
const EXPAND_ALL: &str = "Expand all";
const IGNORE_WS: &str = "Ignore whitespace";
const WRAP: &str = "Wrap long lines";

fn oid() -> Oid {
    Oid::from_bytes(&[7; 20]).unwrap()
}

fn view() -> ViewGroup {
    ViewGroup { layout: DiffLayout::Inline, mode: DiffMode::Hunks, rows: true, ignore_ws: false, wrap: Some(false), folds: None }
}

/// An unstaged diff of src/ui/app.txt, with rows and a neighbour on both sides.
fn unstaged() -> HeaderModel {
    let path = "src/ui/app.txt";
    HeaderModel {
        target: DiffTarget::Unstaged(path.into()),
        path: path.into(),
        folder: "src/ui/".into(),
        name: "app.txt".into(),
        kind: FileKind::Text,
        status: Some('M'),
        change: ChangeText::Lines { added: 24, removed: 9 },
        context: Context::Unstaged,
        prev: StepKind::Change,
        next: StepKind::Change,
        show_nav: true,
        counter: Some(Counter { index: 1, total: 3, files: false }),
        svg: None,
        view: Some(view()),
        actions: Actions { restore: None, open: Some(OpenAction { path: path.into(), missing: None }), diff_tool: Some(None) },
        primary: Some(Primary::Stage(path.into())),
    }
}

/// The same file in a commit whose detail lists it.
fn commit() -> HeaderModel {
    let path = "src/ui/app.txt";
    HeaderModel {
        target: DiffTarget::Commit(oid(), path.into()),
        context: Context::Commit { short: "0707070".into(), summary: Some("Tidy the app".into()) },
        actions: Actions {
            restore: Some(RestoreAction { oid: oid(), path: path.into(), status: 'M' }),
            open: Some(OpenAction { path: path.into(), missing: None }),
            diff_tool: Some(None),
        },
        primary: None,
        ..unstaged()
    }
}

fn harness(m: HeaderModel, width: f32) -> Harness<'static, Vec<Msg>> {
    harness_in(m, width, Lang::En)
}

fn harness_in(m: HeaderModel, width: f32, t: Lang) -> Harness<'static, Vec<Msg>> {
    let keys = Keymap::default();
    let mut h = crate::snapshot_check::builder().with_size(egui::vec2(width, 360.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            let top = ui.cursor().top();
            show(ui, &m, &keys, t, msgs);
            assert_eq!(ui.min_rect().bottom() - top, HEIGHT, "one 48 px row");
        },
        Vec::new(),
    );
    h.run();
    h
}

fn middle(h: &Harness<'_, Vec<Msg>>, label: &str) -> f64 {
    let b = h.get_by_label(label).bounding_box().expect("laid out");
    (b.y0 + b.y1) / 2.0
}

fn click(m: HeaderModel, width: f32, label: &str) -> Vec<Msg> {
    let mut h = harness(m, width);
    h.get_by_label(label).click();
    h.run();
    std::mem::take(h.state_mut())
}

#[test]
fn one_row_at_every_width() {
    for width in [1400.0, 900.0, 700.0, 520.0, 400.0, 360.0] {
        for m in [unstaged(), commit()] {
            let h = harness(m, width);
            let close = middle(&h, CLOSE_NAME);
            // ⋯ never gives way: whole, inside the header, before Stage file and ×.
            let more = h.get_by_label(MORE).bounding_box().expect("⋯ laid out");
            let before = h.query_by_label(STAGE_FILE).unwrap_or_else(|| h.get_by_label(CLOSE_NAME)).bounding_box().unwrap();
            assert!(more.x0 >= 0.0 && more.x1 <= before.x0, "{width}: ⋯ shows whole before the trailing group: {more:?} {before:?}");
            assert!((more.x1 - more.x0 - f64::from(controls::HEIGHT)).abs() < 0.5, "{width}: ⋯ is not cut: {more:?}");
            let labels = [PREV_NAME, NEXT_NAME, STAGE_FILE, MORE, "Inline \u{b7} Changes only", "View"];
            for label in labels {
                if let Some(node) = h.query_by_label(label) {
                    let b = node.bounding_box().expect("laid out");
                    let mid = (b.y0 + b.y1) / 2.0;
                    assert!((mid - close).abs() <= 2.0, "{width}: {label} at {mid}, Close at {close}");
                }
            }
        }
    }
}

#[test]
fn german_header_fits_in_one_row() {
    let de = Lang::De;
    let full = view().label(de);
    assert_eq!(full, "Inline · Nur Änderungen");
    // Everything the header shows on its row (the file actions are in ⋯).
    let labels = [
        de.tr("diff.header.prev.name"),
        de.tr("diff.header.next.name"),
        full.as_str(),
        de.tr("diff.header.view.button"),
        de.tr("diff.header.more"),
        de.tr("diff.header.stage_file"),
        de.tr("diff.header.close.name"),
    ];
    for width in [1400.0, 900.0, 700.0, 520.0] {
        for m in [unstaged(), commit()] {
            let h = harness_in(m, width, de);
            let close = middle(&h, de.tr("diff.header.close.name"));
            if width >= 900.0 {
                assert!(h.query_by_label(&full).is_some(), "{width}: the full View label");
            }
            assert!(h.query_by_label(de.tr("diff.header.more")).is_some(), "{width}: ⋯");
            let shown: Vec<_> = labels.iter().filter_map(|l| Some((*l, h.query_by_label(l)?.bounding_box()?))).collect();
            for (label, b) in &shown {
                let mid = (b.y0 + b.y1) / 2.0;
                assert!((mid - close).abs() <= 2.0, "{width}: {label} at {mid}, Close at {close}");
                assert!(b.x0 >= 0.0 && b.x1 <= f64::from(width), "{width}: {label} inside the header: {b:?}");
            }
            for (i, (a, ra)) in shown.iter().enumerate() {
                for (b, rb) in &shown[i + 1..] {
                    assert!(ra.x1 <= rb.x0 + 0.5 || rb.x1 <= ra.x0 + 0.5, "{width}: {a} overlaps {b}");
                }
            }
        }
    }
}

/// Opens ⋯ on `m` and clicks `label` in its menu.
fn in_more(m: HeaderModel, label: &str) -> Vec<Msg> {
    let mut h = harness(m, 1400.0);
    h.get_by_label(MORE).click();
    h.run();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(h.state_mut())
}

#[test]
fn file_actions_are_in_the_more_menu() {
    let h = harness(commit(), 1400.0);
    for label in [RESTORE, OPEN, SHOW_IN_FOLDER, OPEN_IN_DIFF_TOOL] {
        assert!(h.query_by_label(label).is_none(), "{label} is in ⋯, not on the row");
    }
    assert!(h.query_by_label(MORE).is_some());
    let name_w = |h: &Harness<'_, Vec<Msg>>| h.get_by_label("app.txt").bounding_box().map(|b| b.x1 - b.x0).unwrap();
    let wide = name_w(&h);
    let h = harness(commit(), 520.0);
    assert!(h.query_by_label(MORE).is_some(), "⋯ never gives way");
    assert!((name_w(&h) - wide).abs() < 0.5, "the name is never cut while the folder can give way");

    assert!(
        matches!(&in_more(commit(), "Restore…")[..], [Msg::Files(FilesMsg::RestoreFile { path, deleted: false, .. })] if path == "src/ui/app.txt")
    );
    assert!(matches!(&in_more(commit(), OPEN_DEFAULT)[..], [Msg::Files(FilesMsg::OpenWorkingFile(p))] if p == "src/ui/app.txt"));
    assert!(matches!(&in_more(commit(), SHOW_IN_FOLDER)[..], [Msg::Files(FilesMsg::ShowWorkingFile(p))] if p == "src/ui/app.txt"));
    assert!(matches!(&in_more(unstaged(), OPEN_IN_DIFF_TOOL)[..], [Msg::Tools(ToolsMsg::OpenDiff(_))]));
}

#[test]
fn the_row_runs_file_nav_view_more_then_stage_and_close() {
    let h = harness(unstaged(), 1400.0);
    let x = |label: &str| h.get_by_label(label).bounding_box().expect("laid out");
    let (name, prev, next, view, more, stage, close) =
        (x("app.txt"), x(PREV_NAME), x(NEXT_NAME), x("Inline \u{b7} Changes only"), x(MORE), x(STAGE_FILE), x(CLOSE_NAME));
    let chip = x("Unstaged");
    assert!(name.x1 < chip.x0 && chip.x1 + 15.5 <= prev.x0 && prev.x0 <= chip.x1 + 16.5, "16 px after the file: {chip:?} {prev:?}");
    assert!(prev.x1 <= next.x0 && next.x1 < view.x0 && view.x1 <= more.x0, "nav, then View and ⋯");
    assert!(more.x1 + 100.0 < stage.x0, "Stage file sits apart, at the trailing edge");
    assert!((close.x0 - stage.x1 - 8.0).abs() < 0.5, "8 px between Stage file and ×");
    // The harness's frame takes 8 px, the header's padding the rest.
    let edge = f64::from(1400.0 - 8.0 - crate::theme::spacing::PANEL_PAD_F);
    assert!((edge - close.x1).abs() < 0.5, "× at the edge: {close:?}, edge {edge}");
    assert_eq!(stage.y1 - stage.y0, f64::from(crate::theme::sizes::ControlSize::Small.metrics().h), "Stage file is Small");
}

#[test]
fn controls_send_their_messages() {
    assert!(matches!(&click(unstaged(), 1400.0, PREV_NAME)[..], [Msg::Diff(DiffMsg::JumpToChange(-1))]));
    assert!(matches!(&click(unstaged(), 1400.0, NEXT_NAME)[..], [Msg::Diff(DiffMsg::JumpToChange(1))]));
    assert!(matches!(&click(unstaged(), 1400.0, STAGE_FILE)[..], [Msg::Stage(StageMsg::StageFile(p))] if p == "src/ui/app.txt"));
    let staged = HeaderModel { primary: Some(Primary::Unstage("src/ui/app.txt".into())), context: Context::Staged, ..unstaged() };
    assert!(matches!(&click(staged, 1400.0, UNSTAGE_FILE)[..], [Msg::Stage(StageMsg::UnstageFile(_))]));
    assert!(matches!(&click(unstaged(), 1400.0, CLOSE_NAME)[..], [Msg::Repo(RepoMsg::CloseDiff)]));

    // With no tool set the item stays usable: update opens Preferences > External Tools.
    let unset =
        HeaderModel { actions: Actions { diff_tool: Some(Some(Lang::En.tr("tools.diff_setup.tip"))), ..unstaged().actions }, ..unstaged() };
    assert!(matches!(&in_more(unset, OPEN_IN_DIFF_TOOL)[..], [Msg::Tools(ToolsMsg::OpenDiff(_))]));
    let end = HeaderModel { next: StepKind::End, ..unstaged() };
    assert!(harness(end.clone(), 1400.0).get_by_label(NEXT_NAME).is_disabled());
    assert!(click(end, 1400.0, NEXT_NAME).is_empty());
}

/// Opens the View popup of `m` and clicks `label` in it.
fn in_view_menu(m: HeaderModel, label: &str) -> Vec<Msg> {
    let mut h = harness(m.clone(), 1400.0);
    let button = m.view.unwrap().label(Lang::En);
    h.get_by_label(&button).click();
    h.run();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(h.state_mut())
}

#[test]
fn the_view_menu_changes_the_settings() {
    assert!(matches!(&in_view_menu(unstaged(), "Split")[..], [Msg::Diff(DiffMsg::SetDiffLayout(DiffLayout::Split))]));
    assert!(matches!(&in_view_menu(unstaged(), "Whole file")[..], [Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile))]));
    assert!(matches!(&in_view_menu(unstaged(), IGNORE_WS)[..], [Msg::Diff(DiffMsg::SetIgnoreWhitespace(true))]));
    assert!(matches!(&in_view_menu(unstaged(), WRAP)[..], [Msg::Diff(DiffMsg::SetWordWrap(true))]));
    let whole = HeaderModel {
        view: Some(ViewGroup { mode: DiffMode::WholeFile, folds: Some(FoldCounts { open: 1, closed: 1 }), ..view() }),
        ..unstaged()
    };
    assert!(matches!(&in_view_menu(whole.clone(), EXPAND_ALL)[..], [Msg::Diff(DiffMsg::ExpandAllUnchanged)]));
    assert!(matches!(&in_view_menu(whole, COLLAPSE_ALL)[..], [Msg::Diff(DiffMsg::CollapseAllUnchanged)]));

    let mut h = harness(unstaged(), 1400.0);
    h.get_by_label("Inline \u{b7} Changes only").click();
    h.run();
    assert!(h.query_by_label(EXPAND_ALL).is_none() && h.query_by_label(COLLAPSE_ALL).is_none(), "no folds in Changes only");
    assert!(h.query_by_label("Inline").is_some());

    let binary = HeaderModel { view: None, kind: FileKind::Binary, change: ChangeText::Sizes { old: Some(1), new: Some(2) }, ..unstaged() };
    let h = harness(binary, 1400.0);
    assert!(h.query_by_label("Inline \u{b7} Changes only").is_none() && h.query_by_label("View").is_none(), "no View button");
}

#[test]
fn icon_only_controls_are_named() {
    let h = harness(commit(), 520.0);
    for name in [PREV_NAME, NEXT_NAME, MORE, CLOSE_NAME] {
        let node = h.get_by_label(name);
        assert_eq!(node.role(), egui::accesskit::Role::Button, "{name}");
    }
}

#[test]
fn chips_show_the_change_and_context() {
    let h = harness(commit(), 1400.0);
    assert!(h.query_by_label("M +24 \u{2212}9").is_some(), "the status chip");
    assert!(h.query_by_label("0707070").is_some(), "the context chip");
    let sizes = HeaderModel { view: None, change: ChangeText::Sizes { old: Some(12_700), new: Some(14_438) }, ..unstaged() };
    let h = harness(sizes, 1400.0);
    assert!(h.query_by_label("M 12.4 KB to 14.1 KB").is_some());
    let loading = HeaderModel { view: None, change: ChangeText::Sizes { old: None, new: None }, ..unstaged() };
    assert!(harness(loading, 1400.0).query_by_label("M").is_some(), "the letter only while the sides load");
}

#[test]
fn step_tips_follow_the_keymap() {
    let mut keys = Keymap::default();
    assert_eq!(step_tip(&keys, Lang::En, true, StepKind::Change), "Next change (F7)");
    keys.set(BindingId::NextChange, vec![glowfly_model::keymap::Chord::plain(glowfly_model::keymap::KeyName::F(8))]);
    assert_eq!(step_tip(&keys, Lang::En, true, StepKind::File), "Next change, in the next file (F8)");
    keys.set(BindingId::PrevChange, Vec::new());
    assert_eq!(step_tip(&keys, Lang::En, false, StepKind::End), "No changes before this file", "no key, no parentheses");
}

#[test]
fn primary_tips_follow_the_language_and_keymap() {
    use super::actions::primary_tip;
    let mut keys = Keymap::default();
    let stage = Primary::Stage("a.txt".into());
    let unstage = Primary::Unstage("a.txt".into());
    let s = keys.hint(BindingId::Stage).expect("Stage has a default key").to_owned();
    assert_eq!(primary_tip(&stage, &keys, Lang::En), format!("Stage this file ({s}). With lines selected, {s} stages only those lines."));
    let de = primary_tip(&stage, &keys, Lang::De);
    assert!(de.starts_with("Diese Datei bereitstellen") && de.contains(&s), "{de}");
    assert!(!primary_tip(&unstage, &keys, Lang::De).contains("Unstage"));
    assert!(!primary_tip(&unstage, &keys, Lang::Es).contains("Unstage"));
    keys.set(BindingId::Unstage, Vec::new());
    assert_eq!(primary_tip(&unstage, &keys, Lang::En), "Unstage this file", "no key, no key sentence");
    assert_eq!(primary_tip(&unstage, &keys, Lang::De), "Diese Datei zurücknehmen");
}

/// The header alone, `width` wide, in the dark palette over the app's background, for snapshots.
fn themed(m: HeaderModel, width: f32, height: f32) -> Harness<'static, Vec<Msg>> {
    use crate::theme::{self, color};
    let keys = Keymap::default();
    let mut h = crate::snapshot_check::builder().with_size(egui::vec2(width, height)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_palette(&theme::view_palette(true));
            theme::apply_style(ui.ctx(), &theme::view_palette(true));
            ui.ctx().set_theme(theme::egui_theme(&theme::view_palette(true)));
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            show(ui, &m, &keys, Lang::En, msgs);
        },
        Vec::new(),
    );
    h.run();
    h
}

#[test]
fn file_header_snapshots() {
    use crate::snapshot_check::CheckSnapshot;
    let mut h = themed(commit(), 520.0, 48.0);
    assert!(h.query_by_label(MORE).is_some());
    h.check_snapshot("file_header_narrow");

    let whole = HeaderModel {
        view: Some(ViewGroup { mode: DiffMode::WholeFile, folds: Some(FoldCounts { open: 1, closed: 2 }), ..view() }),
        ..unstaged()
    };
    let mut h = themed(whole, 900.0, 300.0);
    h.get_by_label("Inline \u{b7} Whole file").click();
    h.run();
    assert!(h.query_by_label(EXPAND_ALL).is_some());
    h.check_snapshot("file_header_view_menu");

    let binary = HeaderModel {
        name: "logo.png".into(),
        path: "src/ui/logo.png".into(),
        kind: FileKind::Image,
        view: None,
        change: ChangeText::Sizes { old: Some(12_700), new: Some(14_438) },
        counter: Some(Counter { index: 2, total: 5, files: true }),
        ..unstaged()
    };
    let mut h = themed(binary, 900.0, 48.0);
    h.check_snapshot("file_header_binary");
}

#[test]
fn the_svg_switch_sends_its_choice_by_click_and_by_key() {
    let svg =
        |on| HeaderModel { svg: Some(on), name: "icon.svg".into(), path: "src/ui/icon.svg".into(), kind: FileKind::Image, ..unstaged() };
    assert!(matches!(&click(svg(false), 1400.0, "Preview")[..], [Msg::Diff(DiffMsg::SetSvgPreview(true))]));
    assert!(matches!(&click(svg(true), 1400.0, "Text")[..], [Msg::Diff(DiffMsg::SetSvgPreview(false))]));
    assert!(click(svg(false), 1400.0, "Text").is_empty(), "the raised segment sends nothing");
    // From the keyboard: the segment focused, Space and Enter pick it.
    for key in [egui_kittest::kittest::Key::Space, egui_kittest::kittest::Key::Enter] {
        let mut h = harness(svg(false), 1400.0);
        h.get_by_label("Preview").focus();
        h.run();
        h.get_by_label("Preview").key_press(key);
        h.run();
        assert!(matches!(&std::mem::take(h.state_mut())[..], [Msg::Diff(DiffMsg::SetSvgPreview(true))]), "{key:?}");
    }
    // Without the switch there is no Text or Preview in the header.
    let h = harness(unstaged(), 1400.0);
    assert!(h.query_by_label("Preview").is_none());
}
