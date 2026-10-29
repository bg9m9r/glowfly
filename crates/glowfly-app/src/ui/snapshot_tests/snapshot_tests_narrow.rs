//! The window at about 900, 1100 and 1300 px wide, in English and German: the toolbar folds
//! what doesn't fit into More, the graph's columns give way (`ui/graph/fit.rs`) and the pull
//! request and issue headers fold their links into a ⋯ when the title would be squeezed. The
//! overlap checks themselves are in `toolbar_narrow_tests.rs` and `graph/fit.rs`; here are the
//! snapshots, and the headers' links.

use super::snapshot_tests::{harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_issue::{crash, crash_details, github, set_details, with_issue};
use super::snapshot_tests_pull_request::with_pr;
use crate::msg::Msg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

const HEIGHT: f32 = 820.0;

#[test]
fn narrow_windows() {
    for width in [900.0, 1100.0, 1300.0] {
        for lang in ["en", "de"] {
            let name = format!("narrow_{width}_{lang}");
            let (mut st, dir) = with_repo(ThemeChoice::Dark, &name);
            st.settings.language = lang.into();
            let mut h = harness_sized(st, true, egui::vec2(width, HEIGHT));
            h.run();
            h.check_snapshot(&name);
            drop(h);
            remove_fixture(&dir);
        }
    }
}

#[test]
fn a_narrow_window_hides_graph_columns_and_says_which() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "narrow-columns");
    let mut h = harness_sized(st, true, egui::vec2(900.0, HEIGHT));
    h.run();
    assert!(h.query_by_label_contains("Hidden to fit the window:").is_some(), "the hidden columns are named");
    drop(h);
    let (st, dir2) = with_repo(ThemeChoice::Dark, "wide-columns");
    let mut h = harness_sized(st, true, egui::vec2(1600.0, HEIGHT));
    h.run();
    assert!(h.query_by_label_contains("Hidden to fit the window:").is_none(), "nothing hidden in a wide window");
    drop(h);
    remove_fixture(&dir);
    remove_fixture(&dir2);
}

#[test]
fn narrow_pull_request_header_folds_its_links() {
    for lang in ["en", "de"] {
        let (mut st, dir) = with_pr(ThemeChoice::Dark, &format!("narrow-pr-{lang}"), true);
        st.settings.language = lang.into();
        let t = st.settings.lang();
        let mut h = harness_sized(st, true, egui::vec2(900.0, HEIGHT));
        h.run();
        assert!(h.query_by_label(t.tr("panel.header.copy_link")).is_none(), "{lang}: Copy link folded");
        h.get_by_label(t.tr("panel.header.more.name")).click();
        h.run();
        h.check_snapshot(&format!("narrow_pull_request_header_{lang}"));
        h.get_by_label(t.tr("panel.header.copy_link")).click();
        h.run();
        assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(u) if u.ends_with("/pull/12"))), "{lang}");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn issue_header_links_show_when_they_fit() {
    // At 900 px the issue panel has about 550 px, room for its links beside a 240 px title.
    for (width, folded) in [(720.0, true), (900.0, false), (1600.0, false)] {
        let (mut st, dir) = with_issue(ThemeChoice::Dark, &format!("narrow-issue-{width}"), github(), crash(), None);
        set_details(&mut st, crash_details());
        let mut h = harness_sized(st, true, egui::vec2(width, HEIGHT));
        h.run();
        assert_eq!(h.query_by_label("More link actions").is_some(), folded, "{width}");
        drop(h);
        remove_fixture(&dir);
    }
}
