//! The conflict view's header: the primary Save and mark resolved with how many conflicts are
//! resolved, the more-actions menu, and a binary file's header and body without duplicate
//! buttons. Shares the fixture of `snapshot_tests`.

use crate::state::AppState;
use crate::state::tool_compare::MERGE_TOOL_BUTTON;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, with_conflict};
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
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
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
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_binary_fallback() {
    let (st, dir) = with_conflict("conflict-binary-fallback", "logo.bin", |side| format!("{side}\0image").into_bytes());
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
