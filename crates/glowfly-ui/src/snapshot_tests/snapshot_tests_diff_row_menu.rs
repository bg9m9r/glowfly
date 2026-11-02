//! The diff's right-click menu, Ctrl+A, the Esc order and the code's screen-reader node, through
//! the real view: each copy item sends what it names, the line action picks the line clicked
//! when none is, a right-click outside the selected text drops it, Shift+F10 opens the menu
//! below the topmost selected line, Ctrl+A selects the side last pressed in (the new side until
//! then), and Esc drops the text, then the lines, then closes the diff. The goldens show the
//! menu over selected text, in English and in German.

use super::super::snapshot_tests::remove_fixture;
use super::super::snapshot_tests_select::click;
use super::snapshot_tests_text_select::{Grid, H, ROWS_LEFT, drag, open};
use crate::msg::{DiffMsg, Msg, RepoMsg, SelectionMsg, StageMsg};
use crate::state::diff_text::TextSide;
use crate::state::lines::LineClick;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{DiffLayout, ThemeChoice};

/// Applies the messages the views pushed, as the app's update would, and clears them.
fn apply(h: &mut H) {
    let msgs = std::mem::take(&mut h.state_mut().1);
    for m in msgs {
        crate::update::update(&mut h.state_mut().0, m, std::time::Instant::now());
    }
    h.run();
}

/// A key press with `modifiers`, the messages before it dropped.
fn key(h: &mut H, key: Key, modifiers: Modifiers) {
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.run();
}

/// Right-clicks at `at`, the messages before it dropped.
fn right_click(h: &mut H, at: Pos2) {
    h.state_mut().1.clear();
    click(h, at, PointerButton::Secondary);
}

/// Clicks the open menu's item `label` and returns the messages it sent. The hunk header's
/// button can have the same label: the menu's is the lower one, below the line.
fn choose(h: &mut H, label: &str) -> Vec<Msg> {
    h.state_mut().1.clear();
    let low = |n: &egui_kittest::kittest::Node| n.bounding_box().map_or(0.0, |b| b.y0);
    h.get_all_by_label(label).max_by(|a, b| low(a).total_cmp(&low(b))).expect("the item").click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

fn copied(msgs: &[Msg]) -> Vec<&str> {
    msgs.iter().filter_map(|m| if let Msg::Copy(t) = m { Some(t.as_str()) } else { None }).collect()
}

#[test]
fn each_copy_item_sends_what_it_names() {
    let (mut h, dir) = open("row-menu-copy", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    // `pp`, `fix` and `fix` of `fix the build`.
    drag(&mut h, g.at(0, 1), g.at(2, 3));
    let inside = g.at(1, 1);
    right_click(&mut h, inside);
    assert!(h.ctx.is_context_menu_open());
    assert!(h.state().0.view.diff_text.is_some(), "a right-click inside the selection keeps it");
    assert_eq!(copied(&choose(&mut h, "Copy")), ["pp\nfix\nfix"]);
    right_click(&mut h, inside);
    assert_eq!(copied(&choose(&mut h, "Copy with diff signs")), [" pp\n-fix\n+fix"]);
    right_click(&mut h, inside);
    // Whole lines, each ending in a newline.
    assert_eq!(copied(&choose(&mut h, "Copy lines")), ["app\nfix\nfix the build\n"]);
    right_click(&mut h, inside);
    // Read again from the repository, so the patch has the file's own bytes.
    let msgs = choose(&mut h, "Copy hunk as patch");
    let generation = h.state().0.hunk_ops_gen().expect("the diff on screen");
    assert!(matches!(msgs[..], [Msg::Diff(DiffMsg::CopyHunkPatch { index: 0, generation: g })] if g == generation));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn without_text_copy_is_disabled_and_copy_lines_takes_the_line_clicked() {
    let (mut h, dir) = open("row-menu-no-text", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    right_click(&mut h, g.at(3, 2));
    let copy = h.get_by_label("Copy");
    assert!(copy.is_disabled(), "nothing to copy");
    assert!(h.get_by_label("Copy with diff signs").is_disabled());
    assert_eq!(copied(&choose(&mut h, "Copy lines")), ["on Windows\n"]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_right_click_outside_the_selection_drops_it() {
    let (mut h, dir) = open("row-menu-outside", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(2, 0), g.at(2, 3));
    right_click(&mut h, g.at(3, 5));
    assert!(h.ctx.is_context_menu_open());
    assert!(h.state().0.view.diff_text.is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_line_action_picks_the_changed_line_clicked_when_none_is() {
    let (mut h, dir) = open("row-menu-revert", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    right_click(&mut h, g.at(2, 4));
    let msgs = choose(&mut h, "Revert 1 line");
    assert!(matches!(
        msgs[..],
        [Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 2, how: LineClick::Only }), Msg::Stage(StageMsg::RevertLines)]
    ));
    // On an unchanged line, no line actions.
    right_click(&mut h, g.at(0, 1));
    assert!(h.ctx.is_context_menu_open());
    assert!(h.query_by_label("Revert 1 line").is_none());
    drop(h);
    remove_fixture(&dir);

    // With a line picked in the gutter, it acts on that one, not the line clicked.
    let (mut h, dir) = open("row-menu-revert-picked", ThemeChoice::Dark, DiffLayout::Inline, &[2], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    right_click(&mut h, g.at(3, 1));
    let msgs = choose(&mut h, "Revert 1 line");
    assert!(matches!(msgs[..], [Msg::Stage(StageMsg::RevertLines)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shift_f10_opens_the_menu_below_the_topmost_selected_line() {
    // `-fix` picked in the gutter, `on Windows` selected as text: `-fix` is higher.
    let (mut h, dir) = open("row-menu-keyed", ThemeChoice::Dark, DiffLayout::Inline, &[1], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(3, 0), g.at(3, 4));
    key(&mut h, Key::F10, Modifiers::SHIFT);
    assert!(h.ctx.is_context_menu_open(), "Shift+F10 opened it");
    let top = h.get_by_label("Copy").bounding_box().expect("the first item has bounds").y0 as f32;
    let below = g.at(1, 0).y + g.row_h / 2.0;
    assert!(top > below && top < below + g.row_h, "below `-fix`'s row: {top} after {below}");
    // It acts on `-fix`'s line selection.
    assert!(matches!(choose(&mut h, "Revert 1 line")[..], [Msg::Stage(StageMsg::RevertLines)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn ctrl_a_selects_the_side_last_pressed_in_and_the_new_side_first() {
    let (mut h, dir) = open("row-menu-select-all", ThemeChoice::Dark, DiffLayout::Split, &[], None);
    let g = Grid::of(&h, DiffLayout::Split);
    // A press on the hunk header gives the diff the keyboard but no side.
    click(&mut h, pos2(ROWS_LEFT + 400.0, g.header_y), PointerButton::Primary);
    key(&mut h, Key::A, Modifiers::COMMAND);
    assert!(matches!(h.state().1[..], [Msg::Diff(DiffMsg::SelectAllText { side: TextSide::New })]));
    apply(&mut h);
    let sel = h.state().0.view.diff_text.clone().expect("everything on the new side");
    assert_eq!(sel.side, TextSide::New);
    // A press on the old side's code: Ctrl+A selects that side.
    click(&mut h, pos2(g.text_x[0] + g.char_w, g.first_y + g.row_h), PointerButton::Primary);
    key(&mut h, Key::A, Modifiers::COMMAND);
    assert!(matches!(h.state().1[..], [Msg::Diff(DiffMsg::SelectAllText { side: TextSide::Old })]));
    apply(&mut h);
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::Copy);
    h.run();
    assert_eq!(copied(&h.state().1), ["app\nfix"], "the old side's lines");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn ctrl_a_in_a_diff_never_selects_commits() {
    let (st, dir) = super::super::snapshot_tests_revert::with_line_diff("row-menu-keys", ThemeChoice::Dark, DiffLayout::Inline, &[]);
    let mut h = super::super::snapshot_tests::keys_harness(st, true);
    h.run();
    let g = Grid::of(&h, DiffLayout::Inline);
    click(&mut h, g.at(0, 1), PointerButton::Primary);
    key(&mut h, Key::A, Modifiers::COMMAND);
    let msgs = std::mem::take(&mut h.state_mut().1);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::SelectAllText { side: TextSide::Inline }))));
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Selection(SelectionMsg::SelectAll))), "not the graph's Select all");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_drops_the_text_then_the_lines_then_closes_the_diff() {
    let (st, dir) = super::super::snapshot_tests_revert::with_line_diff("row-menu-esc", ThemeChoice::Dark, DiffLayout::Inline, &[1]);
    let mut h = super::super::snapshot_tests::keys_harness(st, true);
    h.run();
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(2, 0), g.at(3, 2));
    h.state_mut().1.clear();
    let esc = |h: &mut H| {
        key(h, Key::Escape, Modifiers::NONE);
        let first = h.state().1.iter().find_map(|m| match m {
            Msg::Diff(DiffMsg::ClearTextSelection) => Some("text"),
            Msg::Diff(DiffMsg::ClearLineSelection) => Some("lines"),
            Msg::Repo(RepoMsg::CloseDiff) => Some("close"),
            _ => None,
        });
        apply(h);
        first
    };
    assert_eq!(esc(&mut h), Some("text"));
    assert!(h.state().0.view.diff_text.is_none() && !h.state().0.view.diff_lines.is_empty());
    assert_eq!(esc(&mut h), Some("lines"));
    assert_eq!(esc(&mut h), Some("close"));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn screen_readers_get_the_code_as_a_document_with_the_selected_text() {
    let (mut h, dir) = open("row-menu-access", ThemeChoice::Dark, DiffLayout::Inline, &[1], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(0, 1), g.at(2, 3));
    let doc = h.get_by_label("Diff of app.txt");
    assert_eq!(doc.role(), Role::Document);
    assert_eq!(doc.value().as_deref(), Some("pp\nfix\nfix"));
    assert_eq!(doc.description().as_deref(), Some("1 line selected"));
    drop(h);
    remove_fixture(&dir);
}

/// The menu over `fix the build`, selected and right-clicked, in the language `lang`.
fn golden(name: &str, lang: &str) {
    let (mut h, dir) = open(name, ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    h.state_mut().0.settings.language = lang.into();
    h.run();
    drag(&mut h, g.at(2, 0), g.at(2, 13));
    right_click(&mut h, g.at(2, 5));
    assert!(h.ctx.is_context_menu_open());
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_row_menu_dark() {
    golden("diff_row_menu_dark", "en");
}

#[test]
fn diff_row_menu_de() {
    golden("diff_row_menu_de", "de");
}
