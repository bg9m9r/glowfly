//! Headless tests for Compare with the current branch from a local branch's and a tag's menu
//! (SelectionMsg::CompareWithHead, update/selection.rs) and its translated reasons
//! (state/remote_branch.rs::compare_blocked).

use super::tests::{git, scratch_repo};
use super::undo_rename_tests::{cleanup, opened, send};
use crate::effect::{Effect, SelectionEffect};
use crate::msg::{Msg, SelectionMsg};
use crate::state::remote_branch::compare_blocked;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{LoadOptions, load};
use std::path::PathBuf;

/// `main` (checked out, with an uncommitted edit), `feature` one commit ahead of it, the annotated
/// tag `v1` on `feature` and the lightweight tag `at-head` on `main`.
fn fixture(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "feature"]);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["tag", "-a", "v1", "-m", "v1", "feature"]);
    git(&dir, &["tag", "at-head", "main"]);
    (opened(&dir), dir)
}

fn compare(oid: Oid, name: &str) -> Msg {
    Msg::Selection(SelectionMsg::CompareWithHead { oid, name: name.into() })
}

fn tag_oid(st: &AppState, name: &str) -> Oid {
    st.snap.as_ref().unwrap().tags.iter().find(|(n, _)| n == name).map(|(_, o)| *o).unwrap()
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn assert_pair(st: &AppState, fx: &[Effect], tip: Oid) {
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    match &st.session.selection {
        Selection::Commits(m) => assert_eq!(m.compare_ends(), Some((head, tip))),
        other => panic!("not a pair: {other:?}"),
    }
    assert!(fx.iter().any(|e| matches!(e, Effect::Selection(SelectionEffect::Compare { .. }))), "{fx:?}");
}

#[test]
fn local_branch_tip_selects_the_pair() {
    let (mut st, dir) = fixture("cmp-head-local");
    let tip = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "feature").and_then(|b| b.oid).unwrap();
    let fx = send(&mut st, compare(tip, "feature"));
    assert_pair(&st, &fx, tip);
    cleanup(&[&dir]);
}

#[test]
fn tag_commit_selects_the_pair() {
    let (mut st, dir) = fixture("cmp-head-tag");
    let tip = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "feature").and_then(|b| b.oid).unwrap();
    let oid = tag_oid(&st, "v1");
    assert_eq!(oid, tip, "an annotated tag holds its peeled commit");
    let fx = send(&mut st, compare(oid, "v1"));
    assert_pair(&st, &fx, tip);
    cleanup(&[&dir]);
}

#[test]
fn refused_with_the_reason() {
    let (mut st, dir) = fixture("cmp-head-refused");
    let before = st.session.selection.clone();
    let at_head = tag_oid(&st, "at-head");
    let fx = send(&mut st, compare(at_head, "at-head"));
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "at-head is at the same commit as main.");
    assert_eq!(st.session.selection, before);

    let fx = send(&mut st, compare(Oid::zero(), "v1"));
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "v1's commit isn't loaded in the graph.");

    git(&dir, &["checkout", "-q", "--detach"]);
    st.snap = Some(load(&dir, &LoadOptions::all(10)).unwrap());
    let v1 = tag_oid(&st, "v1");
    let fx = send(&mut st, compare(v1, "v1"));
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), "Check out a branch with commits to compare with.");
    assert_eq!(st.session.selection, before);
    cleanup(&[&dir]);
}

#[test]
fn reasons_are_translated() {
    let (mut st, dir) = fixture("cmp-head-lang");
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let snap = st.snap.as_ref().unwrap();
    let en = compare_blocked(Lang::En, snap, head, "at-head").unwrap();
    for t in [Lang::De, Lang::Es] {
        let got = compare_blocked(t, snap, head, "at-head").unwrap();
        assert_eq!(got, t.trf("remote_branch.compare.same_commit", &[("name", &"at-head"), ("branch", &"main")]));
        assert_ne!(got, en);
    }
    git(&dir, &["checkout", "-q", "--detach"]);
    st.snap = Some(load(&dir, &LoadOptions::all(10)).unwrap());
    let snap = st.snap.as_ref().unwrap();
    let en = compare_blocked(Lang::En, snap, head, "at-head").unwrap();
    for t in [Lang::De, Lang::Es] {
        let got = compare_blocked(t, snap, head, "at-head").unwrap();
        assert_eq!(got, t.tr("remote_branch.compare.no_branch"));
        assert_ne!(got, en);
    }
    cleanup(&[&dir]);
}
