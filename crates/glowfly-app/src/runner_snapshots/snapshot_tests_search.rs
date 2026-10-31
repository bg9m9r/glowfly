//! The graph's search bar filtering by a path: the changed-file search runs through the runner.
//! The other search bar snapshots are glowfly-ui's.

use super::snapshot_tests::scenes::{send, with_two_authors};
use super::snapshot_tests::{harness, remove_fixture};
use crate::msg::{Msg, SearchMsg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

/// Sends `msg`, runs the path searches it asks for and feeds their reports back.
fn send_and_search(st: &mut AppState, msg: SearchMsg) {
    let fx = crate::update::update(st, Msg::Search(msg), Instant::now());
    for effect in fx {
        if matches!(effect, crate::effect::Effect::SearchPaths { .. })
            && let Some(reply) = crate::runner::testing::execute(effect)
        {
            crate::update::update(st, reply, Instant::now());
        }
    }
}

#[test]
fn graph_search_paths() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-paths");
    st.view.show_search = true;
    send(&mut st, SearchMsg::ToggleFilter);
    st.session.tab_view.search = "path:notes.txt".into();
    send_and_search(&mut st, SearchMsg::QueryChanged);
    assert!(!st.session.search.searching_files());
    let snap = st.snap.as_ref().unwrap();
    let shown: Vec<&str> = st.session.search.matches.iter().map(|&i| snap.commits[i].summary.as_str()).collect();
    assert_eq!(shown, ["Write release notes"]);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("1 match").is_some());
    h.check_snapshot("graph_search_paths");
    drop(h);
    remove_fixture(&dir);
}
