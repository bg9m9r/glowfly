//! The history table's edges: with BRANCH / TAG hidden the lanes and the "Graph" title start
//! 16 px in from the table's left edge; with DATE hidden the COMMIT column and the WIP row's
//! total end 16 px from its right edge. The lane bands and the selection still reach both edges.

use super::snapshot_tests::{remove_fixture, render, with_repo};
use super::snapshot_tests_line_counts::with_wip;
use crate::state::Selection;
use glowfly_core::settings::ThemeChoice;

#[test]
fn history_table_edges_no_refs() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "history-edges-no-refs");
    st.settings.columns.refs = false;
    let merge = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == "Merge branch 'login-page'").unwrap().oid;
    st.selection = Selection::Commit(merge);
    st.session.detail = Some(glowfly_core::repo::commit_detail(&dir, merge).unwrap());
    render("history_table_edges_no_refs", st, true);
    remove_fixture(&dir);
}

#[test]
fn history_table_edges_no_date() {
    // With the working tree's counts, so the WIP row shows its total at the right inset.
    let (mut st, dir) = with_wip(ThemeChoice::Dark, "history-edges-no-date");
    st.settings.columns.date = false;
    render("history_table_edges_no_date", st, true);
    remove_fixture(&dir);
}
