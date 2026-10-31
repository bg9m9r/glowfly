//! The warning before marking files with conflict markers left resolved: the marker check runs
//! through the runner. The other conflict list snapshots are glowfly-ui's.

use super::snapshot_tests::render;
use super::snapshot_tests::scenes::merging;
use crate::msg::{ConflictMsg, Msg};
use std::time::Instant;

#[test]
fn conflict_markers_warning() {
    let (mut st, dir) = merging("conflict-markers-warning", false);
    let paths = vec!["alpha.txt".to_owned(), "beta.txt".to_owned()];
    let fx = crate::update::update(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(paths)), Instant::now());
    for effect in fx {
        if let Some(m) = crate::runner::testing::execute(effect) {
            crate::update::update(&mut st, m, Instant::now());
        }
    }
    assert!(matches!(&st.modal, Some(crate::state::Modal::Confirm { title, .. }) if title == "Conflict markers left"));
    render("conflict_markers_warning", st, true);
    super::snapshot_tests::remove_fixture(&dir);
}
