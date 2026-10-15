//! Revert Hunk on a commit diff: the button on the hunk header, disabled while whitespace is
//! hidden or a reload is pending, and the message a click sends. Shares the fixture repository
//! and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::Msg;
use crate::msg::StageMsg;
use crate::state::{AppState, Selection};
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::intraline;
use trellis_core::repo::{self, DiffTarget};
use trellis_core::settings::{DiffLayout, ThemeChoice};

/// The fixture with the "Fix build on Windows" commit selected and its diff of app.txt open.
pub(super) fn with_commit_diff(name: &str, theme: ThemeChoice, layout: DiffLayout, ignore_ws: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_ref().unwrap();
    let oid = snap.commits.iter().find(|c| c.summary == "Fix build on Windows").expect("the fix commit").oid;
    st.selection = Selection::Commit(oid);
    st.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    let target = DiffTarget::Commit(oid, "app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    st.settings.diff_ignore_whitespace = ignore_ws;
    (st, dir)
}

#[test]
fn diff_commit_revert() {
    let (st, dir) = with_commit_diff("revert", ThemeChoice::Dark, DiffLayout::Inline, false);
    assert_eq!(st.hunk_ops_blocked(), None);
    render("diff_commit_revert", st, true);
    remove_fixture(&dir);
}

#[test]
fn diff_commit_revert_blocked_light() {
    let (st, dir) = with_commit_diff("revert-blocked", ThemeChoice::Light, DiffLayout::Split, true);
    assert!(st.hunk_ops_blocked().is_some());
    render("diff_commit_revert_blocked_light", st, false);
    remove_fixture(&dir);
}

fn reverts(msgs: &[Msg]) -> Vec<(usize, u64)> {
    msgs.iter()
        .filter_map(|m| if let Msg::Stage(StageMsg::RevertHunk { index, generation }) = m { Some((*index, *generation)) } else { None })
        .collect()
}

#[test]
fn revert_button_sends_msg() {
    let (st, dir) = with_commit_diff("revert-click", ThemeChoice::Dark, DiffLayout::Inline, false);
    let generation = st.hunk_ops_gen().expect("buttons enabled");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Revert Hunk").click();
    h.run();
    assert_eq!(reverts(&h.state().1), [(0, generation)]);
    drop(h);

    // Disabled: whitespace hidden, or a reload on its way.
    for pending in [false, true] {
        let (mut st, _) = with_commit_diff("revert-click", ThemeChoice::Dark, DiffLayout::Inline, !pending);
        if pending {
            st.diff_gen += 1;
            assert_eq!(st.hunk_ops_gen(), None);
        }
        let mut h = harness(st, true);
        h.run();
        h.get_by_label("Revert Hunk").click();
        h.run();
        assert!(reverts(&h.state().1).is_empty(), "pending reload: {pending}");
    }
    remove_fixture(&dir);
}
