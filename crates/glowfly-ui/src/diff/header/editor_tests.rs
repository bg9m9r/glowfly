//! The ⋯ menu's Open in external editor: between Open and the diff tool, sending the line the
//! header's model names, disabled for a missing file and absent without a working file.

use super::tests::{MORE, harness, in_more, unstaged};
use super::*;
use crate::file_menu::OPEN_DEFAULT;
use egui_kittest::kittest::Queryable;
use glowfly_model::state::diff_header::EditorAction;
use glowfly_model::state::tool_compare::OPEN_IN_DIFF_TOOL;
use glowfly_model::state::tools::OPEN_IN_EDITOR;
use glowfly_model::tools::ToolsMsg;

fn with_editor(line: Option<u32>, setup: Option<&'static str>) -> HeaderModel {
    HeaderModel { editor: Some(EditorAction { setup, line }), ..unstaged() }
}

/// The line of the one `OpenFile` that clicking the entry sends, for the file src/ui/app.txt.
fn sent(m: HeaderModel) -> Option<Option<u32>> {
    match &in_more(m, OPEN_IN_EDITOR)[..] {
        [Msg::Tools(ToolsMsg::OpenFile { path, line })] if path == "src/ui/app.txt" => Some(*line),
        _ => None,
    }
}

#[test]
fn the_more_menu_opens_the_editor_at_the_models_line() {
    assert_eq!(sent(with_editor(Some(42), None)), Some(Some(42)));
    assert_eq!(sent(with_editor(None, None)), Some(None));
    let note = Some(Lang::En.tr("tools.editor_setup.tip"));
    assert_eq!(sent(with_editor(Some(7), note)), Some(Some(7)), "no editor set: still sent, update opens Preferences");
}

#[test]
fn a_missing_file_disables_the_editor_entry() {
    let mut m = with_editor(Some(3), None);
    m.actions.open.as_mut().unwrap().missing = Some("not in the working tree");
    let mut h = harness(m, 1400.0);
    h.get_by_label(MORE).click();
    h.run();
    assert!(h.get_by_label(OPEN_IN_EDITOR).is_disabled());
    h.get_by_label(OPEN_IN_EDITOR).click();
    h.run();
    assert!(h.state().is_empty());
}

#[test]
fn no_working_file_no_editor_entry() {
    let mut m = unstaged();
    m.actions.open = None;
    let mut h = harness(m, 1400.0);
    h.get_by_label(MORE).click();
    h.run();
    assert!(h.query_by_label(OPEN_IN_EDITOR).is_none());
    let mut h = harness(unstaged(), 1400.0);
    h.get_by_label(MORE).click();
    h.run();
    assert!(h.query_by_label(OPEN_IN_EDITOR).is_none(), "without a model's editor action the entry is not shown");
}

#[test]
fn the_editor_entry_sits_between_open_and_the_diff_tool() {
    let mut h = harness(with_editor(Some(1), None), 1400.0);
    h.get_by_label(MORE).click();
    h.run();
    let y = |label: &str| h.get_by_label(label).bounding_box().expect("laid out").y0;
    assert!(y(OPEN_DEFAULT) < y(OPEN_IN_EDITOR) && y(OPEN_IN_EDITOR) < y(OPEN_IN_DIFF_TOOL));
}
