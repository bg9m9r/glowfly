//! The toolbar stays still across checkouts: the branch picker is fitted to the widest local
//! branch name, so checking out any of them (or detaching HEAD at a narrower label) never moves
//! Undo or the groups after it, and a chip appearing doesn't either. A short list gets the
//! slot's floor, a long name its cap with the middle cut, and a re-fit (a longer repository
//! name, as on a tab switch) is one jump that then holds still.

use super::fit::BRANCH_FIT;
use super::pickers::part_w;
use super::tests::{span, variant};
use crate::ui::snapshot_tests::snapshot_tests_worktrees::H;
use crate::ui::snapshot_tests::{harness, harness_sized, remove_fixture};
use eframe::egui;
use egui_kittest::kittest::Queryable;

const UNDO: &str = "Undo the last action";

/// Checks out `name` in the harness's snapshot (without touching the repository).
fn check_out(h: &mut H, name: Option<&str>) {
    let snap = h.state_mut().0.snap.as_mut().unwrap();
    for b in &mut snap.locals {
        b.is_head = Some(b.name.as_str()) == name;
    }
    snap.head_branch = name.map(str::to_owned);
    h.run();
}

/// The branch picker's width, found by the checked-out branch's name.
fn branch_part(h: &H) -> f64 {
    let name = h.state().0.snap.as_ref().and_then(|s| s.head_branch.clone()).expect("a branch");
    let (l, r) = span(h, &name);
    r - l
}

#[test]
fn checking_out_any_local_branch_leaves_undo_still() {
    let (mut st, dir) = variant(0, true, "toolbar-still-checkout");
    let snap = st.snap.as_mut().unwrap();
    let head = snap.locals.iter().find(|b| b.is_head).unwrap().clone();
    // The longest of the list, and a one-letter one.
    for name in ["feature/search-results", "x"] {
        snap.locals.push(glowfly_core::repo::BranchInfo { name: name.into(), is_head: false, ..head.clone() });
    }
    let names: Vec<String> = snap.locals.iter().map(|b| b.name.clone()).collect();
    let mut h = harness(st, true);
    h.run();
    let at = span(&h, UNDO).0;
    for name in &names {
        check_out(&mut h, Some(name));
        assert_eq!(span(&h, UNDO).0, at, "checking out {name}");
    }
    // A detached HEAD's label is narrower than the longest branch.
    check_out(&mut h, None);
    assert_eq!(span(&h, UNDO).0, at, "a detached HEAD");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_chip_moves_nothing() {
    // The 80-character branch without the chip (case 1) and with it (case 4), and with a
    // hosting service (case 5, the pull request group after Undo).
    let undo = |case: usize| {
        let (st, dir) = variant(case, true, &format!("toolbar-still-chip-{case}"));
        let mut h = harness_sized(st, true, egui::vec2(1600.0, 820.0));
        h.run();
        if case == 4 {
            assert!(h.query_by_label("Conflicts with main").is_some(), "the chip");
        }
        let x = span(&h, UNDO).0;
        drop(h);
        remove_fixture(&dir);
        x
    };
    assert_eq!(undo(1), undo(4), "the conflict chip");
    assert_eq!(undo(0), undo(5), "a hosting service");
}

#[test]
fn a_repository_with_only_main_gets_the_floor() {
    let (mut st, dir) = variant(0, true, "toolbar-still-floor");
    let snap = st.snap.as_mut().unwrap();
    snap.locals.retain(|b| b.is_head);
    assert_eq!(snap.head_branch.as_deref(), Some("main"));
    let mut h = harness(st, true);
    h.run();
    assert_eq!(branch_part(&h), f64::from(part_w(BRANCH_FIT.0, true)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn an_80_character_name_gets_the_cap_cut_in_the_middle() {
    let (st, dir) = variant(1, true, "toolbar-still-cap");
    let mut h = harness_sized(st, true, egui::vec2(1600.0, 820.0));
    h.run();
    assert_eq!(branch_part(&h), f64::from(part_w(BRANCH_FIT.1, true)));
    // Cut on screen, so the whole name shows on hover; still found by it, after its caption.
    let name = h.state().0.snap.as_ref().and_then(|s| s.head_branch.clone()).unwrap();
    assert!(name.len() >= 80, "{name}");
    h.get_by_label(&format!("Branch: {name}")).hover();
    for _ in 0..4 {
        h.step();
    }
    h.run();
    assert!(h.query_by_label(&name).is_some(), "the whole name in a tooltip (the picker reads \"Branch: …\")");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_longer_repository_name_refits_once_then_holds_still() {
    let (mut st, dir) = variant(0, true, "toolbar-still-refit");
    st.snap.as_mut().unwrap().name = "glowfly".into();
    let mut h = harness_sized(st, true, egui::vec2(1600.0, 820.0));
    h.run();
    let before = span(&h, UNDO).0;
    // Another tab's repository, with a longer name.
    h.state_mut().0.snap.as_mut().unwrap().name = "a-repository-with-a-longer-name".into();
    h.step();
    let after = span(&h, UNDO).0;
    assert!(after > before, "the repository part widened: {before} to {after}");
    // Settles without a repaint loop, and stays where the one jump put it.
    assert!(h.run_ok().is_some(), "the toolbar settles");
    for _ in 0..3 {
        h.step();
        assert_eq!(span(&h, UNDO).0, after);
    }
    assert_eq!(h.run_ok(), Some(1), "no repaint loop");
    drop(h);
    remove_fixture(&dir);
}
