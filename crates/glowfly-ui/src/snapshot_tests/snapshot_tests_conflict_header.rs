//! The conflict view's file bar: the primary Mark resolved with the counter, meter and how many
//! conflicts are resolved in the middle, the more-actions menu, and a binary file's bar and body
//! without duplicate buttons. Shares the fixture of `snapshot_tests`. The commit panel is
//! expanded beside the view, so its conflicted file row (with its own Mark resolved) is drawn
//! too.

use crate::snapshot_check::CheckSnapshot;
use crate::snapshot_tests::{harness, with_conflict};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::Choice;
use glowfly_model::state::AppState;
use std::path::PathBuf;

const MERGE_TOOL_BUTTON: &str = "Open in merge tool";

type H = Harness<'static, (AppState, Vec<glowfly_model::msg::Msg>)>;

/// settings.txt with two conflicts, the first picked as ours.
pub(crate) fn one_of_two(name: &str) -> (AppState, PathBuf) {
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
    let view = st.session.conflict.view.as_mut().unwrap();
    assert_eq!(view.chunk_count(), 2);
    view.picks[0] = Choice::Ours.into();
    st.cues.details_expanded_for = glowfly_model::state::layout::tool_view(&st);
    (st, dir)
}

#[test]
fn conflict_header_progress() {
    let (st, dir) = one_of_two("conflict-header-progress");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("conflict_header_progress");
    assert_eq!(h.query_all_by_label("1 of 2 resolved").count(), 1, "the bar's only: the output's strip says no more");
    assert!(save(&h).is_disabled(), "a conflict is left");
    for gone in ["Save with markers", "Reset picks…", MERGE_TOOL_BUTTON, "Save and mark resolved"] {
        assert!(h.query_by_label(gone).is_none(), "{gone} is in the menu now, or gone");
    }
    // One row, left to right: the path, the counter, the resolved count, Mark resolved and ×;
    // nothing is pushed out of the bar or widens the view over the sidebar.
    let (path, counter, done) = (header_path(&h), bounds(&h, "Conflict 1 of 2"), bounds(&h, "1 of 2 resolved"));
    let (mark, close) = (rect(save(&h).bounding_box().unwrap()), bounds(&h, "Close (Esc)"));
    assert!(
        path.right() < counter.left() && counter.right() < done.left() && done.right() < mark.left(),
        "{path:?} {counter:?} {done:?} {mark:?}"
    );
    assert!(mark.right() < close.left(), "{mark:?} {close:?}");
    for r in [counter, done, mark] {
        assert!((r.center().y - path.center().y).abs() < 2.0, "one row: {r:?} {path:?}");
    }
    let branch = bounds(&h, "login-page");
    assert!(path.left() > branch.right(), "the view stays right of the sidebar: {path:?} {branch:?}");
    if let Some(view) = h.state_mut().0.session.conflict.view.as_mut() {
        view.picks[1] = Choice::Theirs.into();
    }
    h.run();
    assert!(!save(&h).is_disabled());
    assert!(h.query_by_label("2 of 2 resolved").is_some());
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// With room, the middle group is centred on the bar.
#[test]
fn conflict_header_centres_its_middle() {
    let (mut st, dir) = one_of_two("conflict-header-centred");
    st.cues.details_expanded_for = None;
    let mut h = harness(st, true);
    h.run();
    let prev = rect(h.get_by_label_contains("Previous conflict").bounding_box().unwrap());
    let done = bounds(&h, "1 of 2 resolved");
    let (path, close) = (header_path(&h), bounds(&h, "Close (Esc)"));
    let (middle, bar) = ((prev.left() + done.right()) / 2.0, (path.left() - 12.0 + close.right() + 12.0) / 2.0);
    assert!((middle - bar).abs() < 2.0, "middle {middle} bar {bar}");
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// The bar's Mark resolved: the leftmost (the file row in the details panel has one too).
fn save(h: &H) -> egui_kittest::kittest::Node<'_> {
    h.query_all_by_label("Mark resolved")
        .min_by(|a, b| a.bounding_box().unwrap().x0.total_cmp(&b.bounding_box().unwrap().x0))
        .expect("Mark resolved")
}

/// The bar's path: the leftmost "settings.txt" (the file row in the details panel has one too).
fn header_path(h: &H) -> egui::Rect {
    let b = h.query_all_by_label("settings.txt").filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).expect("the path");
    rect(b)
}

fn rect(b: egui::accesskit::Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))
}

fn bounds(h: &H, label: &str) -> egui::Rect {
    rect(h.get_by_label(label).bounding_box().expect("bounds"))
}

#[test]
fn conflict_more_menu() {
    let (st, dir) = one_of_two("conflict-more-menu");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More conflict actions").click();
    h.run();
    h.check_snapshot("conflict_more_menu");
    for item in ["Save with markers", MERGE_TOOL_BUTTON, "Reset picks…", "Mark resolved as is"] {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    // No merge tool is set: the entry stays enabled and opens it, through Preferences if needed.
    assert!(!h.get_by_label(MERGE_TOOL_BUTTON).is_disabled());
    h.get_by_label(MERGE_TOOL_BUTTON).click();
    h.run();
    let sent =
        |m: &glowfly_model::msg::Msg| matches!(m, glowfly_model::msg::Msg::Tools(glowfly_model::tools::ToolsMsg::ResolveInMergeTool(_)));
    assert!(h.state().1.iter().any(sent), "it sends ResolveInMergeTool");
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_binary_fallback() {
    let (mut st, dir) = with_conflict("conflict-binary-fallback", "logo.bin", |side| format!("{side}\0image").into_bytes());
    st.cues.details_expanded_for = glowfly_model::state::layout::tool_view(&st);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("conflict_binary_fallback");
    assert!(h.query_by_label(MERGE_TOOL_BUTTON).is_none(), "only in the menu");
    assert_eq!(h.query_all_by_label("Mark resolved").count(), 1, "only the file row's");
    assert!(h.query_by_label_contains("Use ours").is_some() && h.query_by_label_contains("Use theirs").is_some());
    assert!(h.query_by_label("More conflict actions").is_some() && h.query_by_label("Close (Esc)").is_some());
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
