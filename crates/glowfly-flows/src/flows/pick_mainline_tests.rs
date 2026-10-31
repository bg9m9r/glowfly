//! Headless tests for one mainline over several commits: the dialog's form for merges among
//! other commits, the parent numbers it offers, the command, and a revert of a merge and its
//! side commit end to end.

use super::pick_tests::with_merge;
use super::selection_tests::{commit_on, fixture, git, head, refresh, up};
use super::tests::drive;
use crate::effect::Effect;
use crate::msg::{Msg, SelectionMsg};
use crate::state::journal::short;
use crate::state::pick::PickOp;
use crate::state::{AppState, Modal};
use git2::{Oid, RepositoryState};
use glowfly_core::gitcli::args;
use glowfly_core::pick_ops::Picking;
use glowfly_core::undo::{MoveOp, UndoKind};

fn last_log(st: &AppState) -> Option<&str> {
    st.log.last().map(|l| l.text.as_str())
}

fn with_mainline(op: PickOp, oids: Vec<Oid>, mainline: u32) -> Msg {
    Msg::Selection(SelectionMsg::PickWithMainline { op, oids, mainline })
}

#[test]
fn merges_with_other_commits_take_one_mainline() {
    let (mut st, f) = fixture("mainline-several");
    let merge = with_merge(&mut st, &f);
    git(&f.dir, &["checkout", "-q", "-b", "other", &f.c(1).to_string()]);
    refresh(&mut st);
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![merge, f.c(5)]))).is_empty());
    let Some(Modal::Mainline(form)) = &st.modal else { panic!("the mainline dialog opens") };
    assert_eq!(form.oids, [f.c(5), merge], "oldest first");
    assert_eq!((form.merges.len(), form.plain, form.choices(), form.most_parents()), (1, Some(f.c(5)), 1, 2));
    assert!(!form.single() && form.fits(1) && !form.fits(2));
    st.modal = None;

    let fx = up(&mut st, with_mainline(PickOp::CherryPick, vec![merge, f.c(5)], 1));
    let expected = args(&["cherry-pick", "-m", "1", &f.c(5).to_string(), &merge.to_string()]);
    let [Effect::Pick(j)] = &fx[..] else { panic!("{fx:?}") };
    assert_eq!((&j.cmds, j.picking, &j.oids), (&vec![expected], Picking::CherryPick, &vec![f.c(5), merge]));
    assert_eq!(j.record, Some(UndoKind::Move { op: MoveOp::CherryPick, detail: Some("2 commits".into()) }));
    st.job = None;

    // Parent 2 doesn't fit c5, which has one parent.
    assert!(up(&mut st, with_mainline(PickOp::CherryPick, vec![merge, f.c(5)], 2)).is_empty());
    assert_eq!(last_log(&st), Some(super::pick::MAINLINE_MISFIT));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn two_merges_offer_both_parents() {
    let (mut st, f) = fixture("mainline-two-merges");
    let first = with_merge(&mut st, &f);
    git(&f.dir, &["checkout", "-q", "-b", "more", &f.c(5).to_string()]);
    std::fs::write(f.dir.join("m.txt"), "more\n").unwrap();
    commit_on(&f.dir, "more", 8);
    git(&f.dir, &["checkout", "-q", "main"]);
    git(&f.dir, &["merge", "-q", "--no-ff", "more", "-m", "merge more"]);
    let second = head(&f.dir);
    git(&f.dir, &["checkout", "-q", "-b", "other", &f.c(1).to_string()]);
    refresh(&mut st);
    up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![second, first])));
    let Some(Modal::Mainline(form)) = &st.modal else { panic!("the mainline dialog opens") };
    assert_eq!((form.merges.len(), form.plain, form.choices()), (2, None, 2));
    st.modal = None;
    let fx = up(&mut st, with_mainline(PickOp::CherryPick, vec![second, first], 2));
    assert!(matches!(&fx[..], [Effect::Pick(j)] if j.cmds[0][1..3] == ["-m", "2"]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn reverting_a_merge_and_its_side_commit_skips_the_side() {
    let (mut st, f) = fixture("mainline-revert-side");
    let merge = with_merge(&mut st, &f);
    up(&mut st, Msg::Selection(SelectionMsg::Revert(vec![f.side, merge])));
    let Some(Modal::Mainline(form)) = &st.modal else { panic!("the mainline dialog opens") };
    assert_eq!((form.oids.clone(), form.choices()), (vec![merge, f.side], 1), "newest first");
    st.modal = None;
    drive(&mut st, with_mainline(PickOp::Revert, vec![merge, f.side], 1));
    assert!(st.job.is_none());
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    assert!(!f.dir.join("s.txt").exists(), "the merge's revert took the side's file away");
    let text = format!("Reverted 1 of 2 commits; skipped {}, whose changes are already undone on this branch.", short(f.side));
    assert!(st.toasts.iter().any(|t| t.ok && t.text == text), "{:?}", st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    assert!(st.log.iter().any(|l| l.text.contains(&short(f.side)) && l.text.starts_with("Skipped")));
    let _ = std::fs::remove_dir_all(&f.dir);
}
