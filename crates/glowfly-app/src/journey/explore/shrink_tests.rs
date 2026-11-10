//! Shrinking on plain predicates, without the app: it finds the culprits, keeps their order,
//! stops at its deadline and shortens text.

use super::super::action::{Action, Outside};
use super::{shrink, simpler};
use std::time::{Duration, Instant};

fn later() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

/// 40 actions with one Undo (at 7) and one Stash (at 30), the rest neither.
fn mixed() -> Vec<Action> {
    use Action::*;
    let filler = [Refresh, SelectWip, StageAll, Commit("msg 1".into()), Checkout("topic".into()), Redo];
    (0..40)
        .map(|i| match i {
            7 => Undo,
            30 => Stash,
            _ => filler[i % filler.len()].clone(),
        })
        .collect()
}

fn is_subsequence(small: &[Action], big: &[Action]) -> bool {
    let mut it = big.iter();
    small.iter().all(|a| it.any(|b| b == a))
}

#[test]
fn shrinks_to_the_culprit() {
    use Action::*;
    let out = shrink(mixed(), |l| l.contains(&Undo) && l.contains(&Stash), later());
    assert_eq!(out, [Undo, Stash]);
}

#[test]
fn keeps_order() {
    use Action::*;
    let input = mixed();
    let wanted = [Stash, StageAll, Undo];
    let out = shrink(input.clone(), |l| wanted.iter().all(|w| l.contains(w)), later());
    assert_eq!(out.len(), 3, "{out:?}");
    assert!(is_subsequence(&out, &input), "{out:?}");
}

#[test]
fn respects_deadline() {
    let input = mixed();
    let mut calls = 0;
    let out = shrink(
        input.clone(),
        |_| {
            calls += 1;
            true
        },
        Instant::now() - Duration::from_millis(1),
    );
    assert_eq!(out, input);
    assert_eq!(calls, 0);
}

#[test]
fn simplifies_text() {
    let breaks = |l: &[Action]| l.iter().any(|a| matches!(a, Action::Commit(_)));
    let out = shrink(vec![Action::Commit("a very long message".into())], breaks, later());
    assert!(breaks(&out), "{out:?}");
    let [Action::Commit(text)] = out.as_slice() else { panic!("{out:?}") };
    assert!(text.len() < "a very long message".len(), "{text:?}");
}

#[test]
fn simpler_shortens_text_and_lines_only() {
    let write = Action::Outside(Outside::Write { path: "a.txt".into(), lines: 3 });
    assert_eq!(simpler(&write), [Action::Outside(Outside::Write { path: "a.txt".into(), lines: 1 })]);
    assert!(simpler(&Action::Outside(Outside::Write { path: "a.txt".into(), lines: 1 })).is_empty());
    assert!(simpler(&Action::Commit("c".into())).is_empty(), "nothing shorter than one letter");
    assert!(simpler(&Action::Undo).is_empty());
    assert!(simpler(&Action::Checkout("topic".into())).is_empty(), "a branch name is not text to shorten");
}
