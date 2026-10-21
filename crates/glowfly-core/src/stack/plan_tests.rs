//! Planning restacks on hand-made stacks (pure).

use super::*;
use git2::Oid;

fn o(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn on(branch: &str, lower: &str, base: u8, behind: bool) -> StackLink {
    StackLink { branch: branch.into(), on: lower.into(), base: o(base), behind }
}

fn stacks(mut links: Vec<StackLink>) -> Stacks {
    links.sort_by(|a, b| a.branch.cmp(&b.branch));
    Stacks { links }
}

fn step(onto: &str, upstream: u8, top: &str) -> RestackStep {
    RestackStep { onto: onto.into(), upstream: o(upstream), top: top.into() }
}

fn above(b: &str) -> RestackScope {
    RestackScope::Above(b.into())
}

#[test]
fn a_behind_middle_layer_moves_with_its_top_in_one_step() {
    // L <- M (behind, built on L's old tip 1) <- T.
    let s = stacks(vec![on("M", "L", 1, true), on("T", "M", 2, false)]);
    let p = plan(&s, &above("L"), Some("L"), false);
    assert_eq!(p.steps, vec![step("L", 1, "T")]);
    assert_eq!(p.return_to.as_deref(), Some("L"));
    assert_eq!(p.moved(&s), ["T", "M"]);
}

#[test]
fn a_fork_takes_one_step_per_leaf() {
    // L <- M (behind) <- T1, T2; M's tip is 2.
    let s = stacks(vec![on("M", "L", 1, true), on("T1", "M", 2, false), on("T2", "M", 2, false)]);
    let p = plan(&s, &above("L"), None, false);
    assert_eq!(p.steps, vec![step("L", 1, "T1"), step("M", 2, "T2")]);
    assert_eq!(p.return_to, None, "detached: nothing to go back to");
    let mut moved = p.moved(&s);
    moved.sort();
    assert_eq!(moved, ["M", "T1", "T2"]);
}

#[test]
fn only_the_behind_layer_and_what_is_above_it_move() {
    let s = stacks(vec![on("M", "L", 1, false), on("T", "M", 5, true)]);
    let p = plan(&s, &above("L"), Some("M"), true);
    assert_eq!(p.steps, vec![step("M", 5, "T")]);
    assert!(p.autostash);
    assert_eq!(p.moved(&s), ["T"]);
}

#[test]
fn nothing_behind_plans_nothing() {
    let s = stacks(vec![on("M", "L", 1, false), on("T", "M", 2, false)]);
    let p = plan(&s, &above("L"), Some("L"), false);
    assert!(p.is_empty());
    assert_eq!(p.return_to, None);
    assert!(plan(&s, &above("T"), Some("L"), false).is_empty(), "nothing above T");
}

#[test]
fn onto_leaves_siblings_out() {
    // L <- M (behind), L <- N (behind): Onto(M) moves only M.
    let s = stacks(vec![on("M", "L", 1, true), on("N", "L", 1, true)]);
    let p = plan(&s, &RestackScope::Onto("M".into()), Some("L"), false);
    assert_eq!(p.steps, vec![step("L", 1, "M")]);
    assert!(plan(&s, &RestackScope::Onto("L".into()), Some("L"), false).is_empty(), "L isn't stacked");
}

#[test]
fn return_to_is_left_out_when_head_is_the_last_top() {
    let s = stacks(vec![on("M", "L", 1, true)]);
    assert_eq!(plan(&s, &above("L"), Some("M"), false).return_to, None);
    assert_eq!(plan(&s, &above("L"), Some("other"), false).return_to.as_deref(), Some("other"));
}

#[test]
fn a_behind_layer_above_a_behind_layer_gets_its_own_step() {
    // L <- M (behind, base 1) <- T (behind on M, base 2, a former tip of M): replaying 1..T
    // wouldn't contain M's tip, so M moves first and T goes onto it from its own old base.
    let s = stacks(vec![on("M", "L", 1, true), on("T", "M", 2, true)]);
    let p = plan(&s, &above("L"), Some("L"), false);
    assert_eq!(p.steps, vec![step("L", 1, "M"), step("M", 2, "T")]);
    assert_eq!(p.moved(&s), ["M", "T"]);

    // ... and with a current layer between them, the step below the behind one keeps it.
    let s = stacks(vec![on("M", "L", 1, true), on("N", "M", 2, false), on("T", "N", 3, true)]);
    assert_eq!(plan(&s, &above("L"), None, false).steps, vec![step("L", 1, "N"), step("N", 3, "T")]);
}
