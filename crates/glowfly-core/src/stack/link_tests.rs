//! Linking branches into stacks from synthetic first-parent maps: no repository needed.

use super::*;
use git2::Oid;
use std::collections::HashMap;

fn o(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// The walked commits, each with its first parent (None: a parent in the base, here 0).
fn walked(edges: &[(u8, u8)]) -> HashMap<Oid, Option<Oid>> {
    edges.iter().map(|&(c, p)| (o(c), Some(o(p)))).collect()
}

fn tip(name: &str, t: u8, former: &[u8]) -> BranchTip {
    BranchTip { name: name.into(), tip: o(t), former: former.iter().map(|&f| o(f)).collect() }
}

fn on(branch: &str, lower: &str, base: u8, behind: bool) -> StackLink {
    StackLink { branch: branch.into(), on: lower.into(), base: o(base), behind }
}

#[test]
fn a_chain_links_each_branch_to_the_one_below() {
    // main (0) <- 1 (a) <- 2 (b) <- 3 (c)
    let input = StackInput {
        branches: vec![tip("a", 1, &[]), tip("b", 2, &[]), tip("c", 3, &[])],
        first_parent: walked(&[(1, 0), (2, 1), (3, 2)]),
    };
    let s = link(&input);
    assert_eq!(s.links, vec![on("b", "a", 1, false), on("c", "b", 2, false)]);
    assert_eq!(s.below("a"), None);
    assert_eq!(s.above("a").len(), 1);
    assert_eq!(s.descendants("a"), ["b", "c"]);
    assert_eq!((s.depth("a"), s.depth("b"), s.depth("c")), (0, 1, 2));
}

#[test]
fn a_branch_straight_off_the_base_has_no_link() {
    let input = StackInput { branches: vec![tip("a", 1, &[]), tip("x", 5, &[])], first_parent: walked(&[(1, 0), (5, 0)]) };
    assert!(link(&input).is_empty());
}

#[test]
fn two_branches_at_one_tip_are_not_stacked_on_each_other() {
    let input = StackInput { branches: vec![tip("a", 1, &[]), tip("b", 1, &[])], first_parent: walked(&[(1, 0)]) };
    assert!(link(&input).is_empty());
}

#[test]
fn a_former_tip_gives_a_behind_link() {
    // a was 1, amended to 9 (off the base); b is built on 1.
    let input = StackInput { branches: vec![tip("a", 9, &[1]), tip("b", 2, &[])], first_parent: walked(&[(9, 0), (1, 0), (2, 1)]) };
    assert_eq!(link(&input).links, vec![on("b", "a", 1, true)]);
}

#[test]
fn the_nearest_lower_branch_wins() {
    // c's line passes b's tip (2) before a's (1).
    let input = StackInput {
        branches: vec![tip("a", 1, &[]), tip("b", 2, &[]), tip("c", 3, &[])],
        first_parent: walked(&[(1, 0), (2, 1), (3, 2)]),
    };
    assert_eq!(link(&input).below("c"), Some(&on("c", "b", 2, false)));
}

#[test]
fn a_current_tip_beats_a_former_one_at_the_same_commit() {
    // 1 is y's tip and z's former tip; x is built on it.
    let input = StackInput {
        branches: vec![tip("x", 2, &[]), tip("y", 1, &[]), tip("z", 7, &[1])],
        first_parent: walked(&[(1, 0), (2, 1), (7, 0)]),
    };
    assert_eq!(link(&input).below("x"), Some(&on("x", "y", 1, false)));
}

#[test]
fn two_branches_at_the_lower_commit_link_to_the_first_by_name() {
    let input = StackInput {
        branches: vec![tip("top", 2, &[]), tip("zeta", 1, &[]), tip("alpha", 1, &[])],
        first_parent: walked(&[(1, 0), (2, 1)]),
    };
    assert_eq!(link(&input).links, vec![on("top", "alpha", 1, false)]);
}

#[test]
fn mutual_behind_links_keep_only_the_one_that_does_not_cycle() {
    // x: 3 <- 1, with 1 a former tip of y; y: 4 <- 5 <- 2, with 2 a former tip of x. x's hit is
    // nearer (1 step), so it goes in and y's would close a cycle.
    let input = StackInput {
        branches: vec![tip("x", 3, &[2]), tip("y", 4, &[1])],
        first_parent: walked(&[(1, 0), (3, 1), (2, 0), (5, 2), (4, 5)]),
    };
    assert_eq!(link(&input).links, vec![on("x", "y", 1, true)]);
}

#[test]
fn a_commit_outside_the_walk_ends_the_line() {
    // b's parent 2 isn't walked (it is in the base), so a's tip beyond it isn't reached.
    let input = StackInput { branches: vec![tip("a", 1, &[]), tip("b", 3, &[])], first_parent: walked(&[(1, 0), (3, 2)]) };
    assert!(link(&input).is_empty());
}

#[test]
fn a_merged_branch_is_never_a_layer() {
    // a's tip (1) isn't walked: it is in the base.
    let input = StackInput { branches: vec![tip("a", 1, &[]), tip("b", 2, &[])], first_parent: walked(&[(2, 1)]) };
    assert!(link(&input).is_empty());
}
