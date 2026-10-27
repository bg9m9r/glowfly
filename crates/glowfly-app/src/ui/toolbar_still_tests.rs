//! The toolbar stays still when the names change: the pickers keep their slots whatever the
//! repository and branch are called, so switching tabs or branches never moves Undo or the
//! groups after it, and a chip appearing doesn't either.

use super::tests::{span, variant};
use crate::ui::snapshot_tests::{harness, harness_sized, remove_fixture};
use eframe::egui;
use egui_kittest::kittest::Queryable;

const UNDO: &str = "Undo the last action";

#[test]
fn undo_stays_put_whatever_the_names() {
    let mut undo = Vec::new();
    for case in 0..6 {
        let (st, dir) = variant(case, true, &format!("toolbar-fit-{case}"));
        let long_branch = st.snap.as_ref().unwrap().head_branch.clone().filter(|b| b.len() == 80);
        let long_repo = Some(st.snap.as_ref().unwrap().name.clone()).filter(|n| n.len() == 60);
        let has_pr = st.session.forge.is_some();
        // Wide enough for every group, the PR group and the chip included.
        let mut h = harness_sized(st, true, egui::vec2(1600.0, 820.0));
        h.run();
        undo.push(span(&h, UNDO).0);
        for name in long_branch.iter().chain(&long_repo) {
            // Cut on screen, but still found by its whole name.
            let (l, r) = span(&h, name);
            assert!(r - l < 300.0, "case {case}: {}", r - l);
        }
        if case == 4 {
            assert!(h.query_by_label("Conflicts with main").is_some(), "the chip");
        }
        assert_eq!(h.query_all_by_label_contains("create a pull request on").next().is_some(), has_pr, "case {case}");
        drop(h);
        remove_fixture(&dir);
    }
    // Long names, a detached HEAD and the conflict chip leave the middle groups where they are.
    assert!(undo.iter().all(|&x| x == undo[0]), "{undo:?}");
}

#[test]
fn switching_to_longer_names_leaves_undo_still() {
    let (st, dir) = variant(0, true, "toolbar-still-switch");
    let mut h = harness(st, true);
    h.run();
    let at = span(&h, UNDO).0;
    // Another tab's repository, with a long name and a long branch checked out.
    let snap = h.state_mut().0.snap.as_mut().unwrap();
    snap.name = "a-repository-with-a-much-longer-name".into();
    let long = "feature/a-branch-name-long-enough-to-be-cut".to_owned();
    snap.locals.iter_mut().find(|b| b.is_head).unwrap().name.clone_from(&long);
    snap.head_branch = Some(long);
    h.run();
    assert_eq!(span(&h, UNDO).0, at, "a longer repository and branch name");
    // A short branch, then a detached HEAD.
    h.state_mut().0.snap.as_mut().unwrap().head_branch = Some("x".into());
    h.run();
    assert_eq!(span(&h, UNDO).0, at, "a one-letter branch");
    h.state_mut().0.snap.as_mut().unwrap().head_branch = None;
    h.run();
    assert_eq!(span(&h, UNDO).0, at, "a detached HEAD");
    drop(h);
    remove_fixture(&dir);
}
