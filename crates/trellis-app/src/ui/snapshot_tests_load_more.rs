//! The graph header of a history longer than the commit limit: how many commits are shown, and
//! the Load more link.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::Msg;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

#[test]
fn graph_header_offers_more_commits() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "load-more");
    st.snap.as_mut().unwrap().truncated = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_load_more");
    h.get_by_label("Load more").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::LoadMoreCommits)));

    // With room in the message column, the count shows too.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "load-more");
    st.snap.as_mut().unwrap().truncated = true;
    st.settings.columns.author = false;
    st.settings.columns.date = false;
    render("graph_load_more_count", st, true);
    remove_fixture(&dir);
}
