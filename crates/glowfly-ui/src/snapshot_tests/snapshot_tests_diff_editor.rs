//! Open in external editor from the diff, through the real view: the row menu opens at the line
//! selected (the gutter's or the text's), else at the line clicked, and the header's ⋯ entry at
//! the selected line, else the hunk's first change, in both layouts.

use super::super::snapshot_tests::remove_fixture;
use super::snapshot_tests_diff_row_menu::{choose, right_click};
use super::snapshot_tests_text_select::{Grid, H, drag, open};
use eframe::egui::Pos2;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use glowfly_model::msg::Msg;
use glowfly_model::state::tools::OPEN_IN_EDITOR;
use glowfly_model::tools::ToolsMsg;

/// The line of the one `OpenFile` for app.txt among `msgs`, `None` when there isn't exactly one.
fn opened(msgs: &[Msg]) -> Option<Option<u32>> {
    match msgs {
        [Msg::Tools(ToolsMsg::OpenFile { path, line })] if path == "app.txt" => Some(*line),
        _ => None,
    }
}

/// Right-clicks `at` and chooses Open in external editor.
fn from_row_menu(h: &mut H, at: Pos2) -> Option<Option<u32>> {
    right_click(h, at);
    assert!(h.ctx.is_context_menu_open());
    opened(&choose(h, OPEN_IN_EDITOR))
}

/// Opens the header's ⋯ and chooses Open in external editor.
fn from_header(h: &mut H) -> Option<Option<u32>> {
    h.state_mut().1.clear();
    h.get_by_label("More file actions").click();
    h.run();
    opened(&choose(h, OPEN_IN_EDITOR))
}

#[test]
fn the_row_menu_opens_the_editor_at_the_line_clicked() {
    let (mut h, dir) = open("editor-row-menu", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    assert_eq!(from_row_menu(&mut h, g.at(3, 2)), Some(Some(3)), "+on Windows");
    assert_eq!(from_row_menu(&mut h, g.at(1, 2)), Some(Some(2)), "-fix: the next line the working file has");
    assert_eq!(from_row_menu(&mut h, g.at(0, 1)), Some(Some(1)), "an unchanged line");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn selected_lines_win_over_the_line_clicked() {
    let (mut h, dir) = open("editor-row-picked", ThemeChoice::Dark, DiffLayout::Inline, &[3], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    assert_eq!(from_row_menu(&mut h, g.at(0, 1)), Some(Some(3)), "the line picked in the gutter");
    drop(h);
    remove_fixture(&dir);

    let (mut h, dir) = open("editor-row-text", ThemeChoice::Dark, DiffLayout::Inline, &[], None);
    let g = Grid::of(&h, DiffLayout::Inline);
    drag(&mut h, g.at(2, 0), g.at(3, 3));
    assert_eq!(from_row_menu(&mut h, g.at(3, 1)), Some(Some(2)), "the first line of the text selected");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_headers_editor_entry_sends_the_diffs_line() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut h, dir) = open("editor-header", ThemeChoice::Dark, layout, &[], None);
        assert_eq!(from_header(&mut h), Some(Some(2)), "{layout:?}: the hunk's first change is the removed line");
        drop(h);
        remove_fixture(&dir);

        let (mut h, dir) = open("editor-header-picked", ThemeChoice::Dark, layout, &[3], None);
        assert_eq!(from_header(&mut h), Some(Some(3)), "{layout:?}: the line picked");
        drop(h);
        remove_fixture(&dir);
    }
}
