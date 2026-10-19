//! Layout and message tests of the diff header, drawn alone from a `HeaderModel`.

use super::actions::{CLOSE_NAME, MORE, OPEN, RESTORE, STAGE_FILE, UNSTAGE_FILE};
use super::nav::{NEXT_NAME, PREV_NAME};
use super::view_menu::{COLLAPSE_ALL, EXPAND_ALL, IGNORE_WS, WRAP};
use super::*;
use crate::keymap::BindingId;
use crate::msg::{DiffMsg, FilesMsg, RepoMsg, StageMsg};
use crate::state::diff_header::{Actions, ChangeText, Context, FileKind, FoldCounts, OpenAction, Primary, RestoreAction, ViewGroup};
use crate::state::diff_nav::{Counter, StepKind};
use crate::state::tool_compare::{DIFF_TOOL_SETUP, OPEN_IN_DIFF_TOOL};
use crate::tools::ToolsMsg;
use crate::ui::file_menu::{OPEN_DEFAULT, SHOW_IN_FOLDER};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::repo::DiffTarget;
use trellis_core::settings::{DiffLayout, DiffMode};

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
    let keys = Keymap::default();
    let mut h = Harness::builder().with_size(egui::vec2(width, 360.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            let top = ui.cursor().top();
            show(ui, &m, &keys, Lang::En, msgs);
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
    for width in [1400.0, 900.0, 700.0, 520.0, 360.0] {
        for m in [unstaged(), commit()] {
            let h = harness(m, width);
            let close = middle(&h, CLOSE_NAME);
            let labels =
                [PREV_NAME, NEXT_NAME, OPEN_IN_DIFF_TOOL, STAGE_FILE, RESTORE, OPEN, SHOW_IN_FOLDER, MORE, "Inline \u{b7} Hunks", "View"];
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
fn file_actions_fold_into_a_menu_when_narrow() {
    let h = harness(commit(), 1400.0);
    for label in [RESTORE, OPEN, SHOW_IN_FOLDER, "src/ui/"] {
        assert!(h.query_by_label(label).is_some(), "{label} at 1400");
    }
    assert!(h.query_by_label(MORE).is_none());
    let name_w = |h: &Harness<'_, Vec<Msg>>| h.get_by_label("app.txt").bounding_box().map(|b| b.x1 - b.x0).unwrap();
    let wide = name_w(&h);

    let h = harness(commit(), 520.0);
    for label in [RESTORE, OPEN, SHOW_IN_FOLDER, "src/ui/"] {
        assert!(h.query_by_label(label).is_none(), "{label} gives way at 520");
    }
    assert!(h.query_by_label(MORE).is_some());
    assert!((name_w(&h) - wide).abs() < 0.5, "the name is never cut while the folder can give way");

    let open = |label: &str| {
        let mut h = harness(commit(), 520.0);
        h.get_by_label(MORE).click();
        h.run();
        h.get_by_label(label).click();
        h.run();
        std::mem::take(h.state_mut())
    };
    assert!(matches!(&open("Restore…")[..], [Msg::Files(FilesMsg::RestoreFile { path, deleted: false, .. })] if path == "src/ui/app.txt"));
    assert!(matches!(&open(OPEN_DEFAULT)[..], [Msg::Files(FilesMsg::OpenWorkingFile(p))] if p == "src/ui/app.txt"));
    assert!(matches!(&open(SHOW_IN_FOLDER)[..], [Msg::Files(FilesMsg::ShowWorkingFile(p))] if p == "src/ui/app.txt"));
}

#[test]
fn controls_send_their_messages() {
    assert!(matches!(&click(unstaged(), 1400.0, PREV_NAME)[..], [Msg::Diff(DiffMsg::JumpToChange(-1))]));
    assert!(matches!(&click(unstaged(), 1400.0, NEXT_NAME)[..], [Msg::Diff(DiffMsg::JumpToChange(1))]));
    assert!(matches!(&click(unstaged(), 1400.0, STAGE_FILE)[..], [Msg::Stage(StageMsg::StageFile(p))] if p == "src/ui/app.txt"));
    let staged = HeaderModel { primary: Some(Primary::Unstage("src/ui/app.txt".into())), context: Context::Staged, ..unstaged() };
    assert!(matches!(&click(staged, 1400.0, UNSTAGE_FILE)[..], [Msg::Stage(StageMsg::UnstageFile(_))]));
    assert!(matches!(&click(unstaged(), 1400.0, CLOSE_NAME)[..], [Msg::Repo(RepoMsg::CloseDiff)]));
    assert!(matches!(&click(unstaged(), 1400.0, OPEN_IN_DIFF_TOOL)[..], [Msg::Tools(ToolsMsg::OpenDiff(_))]));

    // With no tool set the icon stays usable: update opens Preferences > External Tools.
    let unset = HeaderModel { actions: Actions { diff_tool: Some(Some(DIFF_TOOL_SETUP)), ..unstaged().actions }, ..unstaged() };
    assert!(!harness(unset.clone(), 1400.0).get_by_label(OPEN_IN_DIFF_TOOL).is_disabled());
    assert!(matches!(&click(unset, 1400.0, OPEN_IN_DIFF_TOOL)[..], [Msg::Tools(ToolsMsg::OpenDiff(_))]));
    let end = HeaderModel { next: StepKind::End, ..unstaged() };
    assert!(harness(end.clone(), 1400.0).get_by_label(NEXT_NAME).is_disabled());
    assert!(click(end, 1400.0, NEXT_NAME).is_empty());
}

/// Opens the View popup of `m` and clicks `label` in it.
fn in_view_menu(m: HeaderModel, label: &str) -> Vec<Msg> {
    let mut h = harness(m.clone(), 1400.0);
    let button = m.view.unwrap().label();
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
    h.get_by_label("Inline \u{b7} Hunks").click();
    h.run();
    assert!(h.query_by_label(EXPAND_ALL).is_none() && h.query_by_label(COLLAPSE_ALL).is_none(), "no folds in Hunks");
    assert!(h.query_by_label("Inline").is_some());

    let binary = HeaderModel { view: None, kind: FileKind::Binary, change: ChangeText::Sizes { old: Some(1), new: Some(2) }, ..unstaged() };
    let h = harness(binary, 1400.0);
    assert!(h.query_by_label("Inline \u{b7} Hunks").is_none() && h.query_by_label("View").is_none(), "no View button");
}

#[test]
fn icon_only_controls_are_named() {
    let h = harness(commit(), 520.0);
    for name in [PREV_NAME, NEXT_NAME, OPEN_IN_DIFF_TOOL, MORE, CLOSE_NAME] {
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
    assert_eq!(step_tip(&keys, true, StepKind::Change), "Next change (F7)");
    keys.set(BindingId::NextChange, vec![crate::keymap::Chord::plain(crate::keymap::KeyName::F(8))]);
    assert_eq!(step_tip(&keys, true, StepKind::File), "Next change, in the next file (F8)");
    keys.set(BindingId::PrevChange, Vec::new());
    assert_eq!(step_tip(&keys, false, StepKind::End), "No changes before this file", "no key, no parentheses");
}

/// The header alone, `width` wide, in the dark palette over the app's background, for snapshots.
fn themed(m: HeaderModel, width: f32, height: f32) -> Harness<'static, Vec<Msg>> {
    use crate::ui::theme::{self, color};
    let keys = Keymap::default();
    let mut h = Harness::builder().with_size(egui::vec2(width, height)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_palette(&theme::DARK);
            theme::apply_style(ui.ctx(), &theme::DARK);
            ui.ctx().set_theme(theme::egui_theme(&theme::DARK));
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
    use crate::ui::snapshot_check::CheckSnapshot;
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
