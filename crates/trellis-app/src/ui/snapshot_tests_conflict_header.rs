//! The conflict view's header: the primary Save and mark resolved with how many conflicts are
//! resolved, the more-actions menu, and a binary file's header and body without duplicate
//! buttons. Shares the fixture of `snapshot_tests`. The commit panel is expanded beside the
//! view, so its conflicted file row (with its own Mark resolved) is drawn too.

use crate::state::AppState;
use crate::state::tool_compare::MERGE_TOOL_BUTTON;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, with_conflict};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::repo::Choice;

/// settings.txt with two conflicts, the first picked as ours.
fn one_of_two(name: &str) -> (AppState, PathBuf) {
    let settings = |side: &str| {
        let mut lines: Vec<String> = (1..=12).map(|i| format!("setting_{i} = {i}")).collect();
        match side {
            "ours" => {
                lines[2] = "theme = \"dark\"".into();
                lines[8] = "font_size = 13".into();
            }
            "theirs" => {
                lines[2] = "theme = \"solarized\"".into();
                lines[8] = "font_size = 14".into();
            }
            _ => {}
        }
        (lines.join("\n") + "\n").into_bytes()
    };
    let (mut st, dir) = with_conflict(name, "settings.txt", settings);
    let view = st.conflict.as_mut().unwrap();
    assert_eq!(view.chunk_count(), 2);
    view.picks[0] = Choice::Ours.into();
    st.view.details_expanded_for = crate::state::layout::tool_view(&st);
    (st, dir)
}

#[test]
fn conflict_header_progress() {
    let (st, dir) = one_of_two("conflict-header-progress");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("conflict_header_progress");
    assert_eq!(h.query_all_by_label("1 of 2 conflicts resolved").count(), 2, "the header's and the output's");
    assert!(h.get_by_label("Save and mark resolved").is_disabled(), "a conflict is left");
    for gone in ["Save with markers", "Reset picks…", MERGE_TOOL_BUTTON] {
        assert!(h.query_by_label(gone).is_none(), "{gone} is in the menu now");
    }
    assert_eq!(h.query_all_by_label("Mark resolved").count(), 1, "only the file row's");
    // The details panel leaves the button row too little room after the counter and the steps:
    // the reason moves up into the path's row, ending above Save, and nothing is pushed out of
    // the header or widens the view over the sidebar.
    let (why, save) = (bounds(&h, SAVE_WHY), bounds(&h, "Save and mark resolved"));
    assert!(why.bottom() <= save.top() && why.right() <= save.right() + 1.0, "the reason is above Save: {why:?} {save:?}");
    let (counter, path) = (bounds(&h, "conflict 1 of 2"), header_path(&h));
    assert!(counter.left() >= path.left(), "the counter stays in the header: {counter:?} {path:?}");
    assert!(path.right() <= why.left(), "the path is left of the reason: {path:?} {why:?}");
    let branch = bounds(&h, "login-page");
    assert!(path.left() > branch.right(), "the view stays right of the sidebar: {path:?} {branch:?}");
    if let Some(view) = h.state_mut().0.conflict.as_mut() {
        view.picks[1] = Choice::Theirs.into();
    }
    h.run();
    assert!(!h.get_by_label("Save and mark resolved").is_disabled());
    assert!(h.query_by_label(SAVE_WHY).is_none(), "no reason once every conflict is picked");
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn save_reason_sits_beside_save_with_room() {
    let (mut st, dir) = one_of_two("conflict-header-reason-beside");
    st.view.details_expanded_for = None;
    let mut h = harness(st, true);
    h.run();
    let (why, save) = (bounds(&h, SAVE_WHY), bounds(&h, "Save and mark resolved"));
    assert!(why.right() <= save.left() && why.center().y > save.top() && why.center().y < save.bottom(), "{why:?} {save:?}");
    let counter = bounds(&h, "conflict 1 of 2");
    assert!(counter.right() <= why.left(), "the counter is left of the reason: {counter:?} {why:?}");
    assert!(counter.left() >= header_path(&h).left(), "the counter stays in the header");
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// The header's path: the leftmost "settings.txt" (the file row in the details panel has one too).
fn header_path(h: &Harness<'static, (AppState, Vec<crate::msg::Msg>)>) -> egui::Rect {
    let b = h.query_all_by_label("settings.txt").filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).expect("the path");
    egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))
}

const SAVE_WHY: &str = "Pick every conflict first (1 left), or use Save with markers";

fn bounds(h: &Harness<'static, (AppState, Vec<crate::msg::Msg>)>, label: &str) -> egui::Rect {
    let b = h.get_by_label(label).bounding_box().expect("bounds");
    egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))
}

#[test]
fn conflict_more_menu() {
    let (st, dir) = one_of_two("conflict-more-menu");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More conflict actions").click();
    h.run();
    h.check_snapshot("conflict_more_menu");
    for item in ["Save with markers", MERGE_TOOL_BUTTON, "Reset picks…"] {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    assert_eq!(h.query_all_by_label("Mark resolved").count(), 2, "the menu's and the file row's");
    // No merge tool is set: the entry stays enabled and opens it, through Preferences if needed.
    assert!(!h.get_by_label(MERGE_TOOL_BUTTON).is_disabled());
    h.get_by_label(MERGE_TOOL_BUTTON).click();
    h.run();
    let sent = |m: &crate::msg::Msg| matches!(m, crate::msg::Msg::Tools(crate::tools::ToolsMsg::ResolveInMergeTool(_)));
    assert!(h.state().1.iter().any(sent), "it sends ResolveInMergeTool");
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_binary_fallback() {
    let (mut st, dir) = with_conflict("conflict-binary-fallback", "logo.bin", |side| format!("{side}\0image").into_bytes());
    st.view.details_expanded_for = crate::state::layout::tool_view(&st);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("conflict_binary_fallback");
    for gone in [MERGE_TOOL_BUTTON, "Save and mark resolved"] {
        assert!(h.query_by_label(gone).is_none(), "{gone}: only in the menu");
    }
    assert_eq!(h.query_all_by_label("Mark resolved").count(), 1, "only the file row's");
    assert!(h.query_by_label_contains("Use ours").is_some() && h.query_by_label_contains("Use theirs").is_some());
    assert!(h.query_by_label("More conflict actions").is_some());
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
