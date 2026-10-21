//! Double-clicking a graph row's BRANCH / TAG chips: a local branch chip checks the branch out
//! (also when it folds copies on remotes), a remote branch chip checks out its tracking branch
//! (for copies on several remotes, the lead copy's), and a tag chip, the rest of the row or a
//! double-click with Shift held stays a row click. Nothing looks different, so there is no
//! snapshot. Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{double_click, row_y};
use crate::msg::{BranchMsg, Msg, RepoMsg, SelectionMsg};
use crate::state::AppState;
use crate::tag::TagMsg;
use eframe::egui::{Modifiers, pos2};
use git2::Oid;
use glowfly_core::repo::{RefKind, RefLabel};
use glowfly_core::settings::{ColumnWidths, ThemeChoice};

/// Where the first chip of a row sits in the BRANCH / TAG column.
const CHIP_X: f32 = 290.0;
/// A BRANCH / TAG width that fits a branch chip and a remote branch chip side by side.
pub(super) const WIDE_REFS: f32 = 220.0;
/// The fixture's rows (the WIP row is 0): the search branch's tip and the merge on main.
const SEARCH_ROW: usize = 1;
const MAIN_ROW: usize = 2;

fn tip(st: &AppState, branch: &str) -> Oid {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == branch).and_then(|b| b.oid).unwrap()
}

fn checkouts(msgs: &[Msg]) -> Vec<&BranchMsg> {
    msgs.iter()
        .filter_map(|m| match m {
            Msg::Branch(b @ (BranchMsg::Checkout(_) | BranchMsg::CheckoutRemote { .. })) => Some(b),
            _ => None,
        })
        .collect()
}

fn checks_out(b: &BranchMsg, want: &str) -> bool {
    matches!(b, BranchMsg::Checkout(n) if n == want)
}

#[test]
fn graph_label_double_click_checks_out_the_branch() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "dbl-click-branch");
    let mut h = harness(st, true);
    h.run();
    double_click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), Modifiers::NONE);
    let msgs = &h.state().1;
    let found = checkouts(msgs);
    assert!(matches!(found[..], [b] if checks_out(b, "search")), "one checkout of search");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Branch(BranchMsg::NewBranch { .. }))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_label_double_click_uses_the_chip_under_the_pointer() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dbl-click-remote");
    let (search, main) = (tip(&st, "search"), tip(&st, "main"));
    let snap = st.snap.as_mut().unwrap();
    snap.refs.entry(search).or_default().push(remote_label("origin/review"));
    snap.remotes = vec![("origin".into(), vec![("main".into(), main), ("review".into(), search)])];
    // A BRANCH / TAG column wide enough for both chips.
    st.settings.column_widths = ColumnWidths { refs: WIDE_REFS, ..ColumnWidths::default() };
    let mut h = harness(st, true);
    h.run();
    // The row's chips are [search, origin/review]; the second sits at about x 348-456.
    double_click(&mut h, pos2(390.0, row_y(SEARCH_ROW)), Modifiers::NONE);
    let found = checkouts(&h.state().1);
    let remote = |b: &BranchMsg| checks_out_remote(b, "origin", "review");
    assert!(matches!(found[..], [b] if remote(b)), "the remote chip's checkout only");

    h.state_mut().1.clear();
    double_click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), Modifiers::NONE);
    let found = checkouts(&h.state().1);
    assert!(matches!(found[..], [b] if checks_out(b, "search")), "the local chip's checkout only");
    drop(h);
    remove_fixture(&dir);
}

fn remote_label(name: &str) -> RefLabel {
    RefLabel { name: name.into(), full: format!("refs/remotes/{name}"), kind: RefKind::Remote, is_head: false }
}

fn checks_out_remote(b: &BranchMsg, want_remote: &str, want: &str) -> bool {
    matches!(b, BranchMsg::CheckoutRemote { remote, name } if remote == want_remote && name == want)
}

#[test]
fn graph_label_double_click_on_folded_copies() {
    // search with copies on origin and fork: one chip, which checks out the local branch.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dbl-click-copies");
    let search = tip(&st, "search");
    let snap = st.snap.as_mut().unwrap();
    snap.refs.entry(search).or_default().extend([remote_label("fork/search"), remote_label("origin/search")]);
    snap.remotes = vec![("fork".into(), vec![("search".into(), search)]), ("origin".into(), vec![("search".into(), search)])];
    let mut h = harness(st, true);
    h.run();
    double_click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), Modifiers::NONE);
    let found = checkouts(&h.state().1);
    assert!(matches!(found[..], [b] if checks_out(b, "search")), "one checkout of the local branch");
    drop(h);
    remove_fixture(&dir);

    // review on fork and origin only: one remote chip led by the default remote's copy.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dbl-click-remote-copies");
    let search = tip(&st, "search");
    let snap = st.snap.as_mut().unwrap();
    snap.refs.entry(search).or_default().extend([remote_label("fork/review"), remote_label("origin/review")]);
    snap.remotes = vec![("fork".into(), vec![("review".into(), search)]), ("origin".into(), vec![("review".into(), search)])];
    // A BRANCH / TAG column wide enough for both chips.
    st.settings.column_widths = ColumnWidths { refs: WIDE_REFS, ..ColumnWidths::default() };
    let mut h = harness(st, true);
    h.run();
    // The row's chips are [search, origin/review]; the second sits at about x 348-456.
    double_click(&mut h, pos2(390.0, row_y(SEARCH_ROW)), Modifiers::NONE);
    let found = checkouts(&h.state().1);
    assert!(matches!(found[..], [b] if checks_out_remote(b, "origin", "review")), "the lead copy's checkout only");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_label_double_click_elsewhere_selects() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dbl-click-elsewhere");
    let (search, main) = (tip(&st, "search"), tip(&st, "main"));
    // main's tip carries main and v0.1.0: put the tag's chip first.
    let labels = st.snap.as_mut().unwrap().refs.get_mut(&main).unwrap();
    labels.sort_by_key(|l| l.kind != RefKind::Tag);
    assert!(labels[0].kind == RefKind::Tag);
    let mut h = harness(st, true);
    h.run();
    let selects = |msgs: &[Msg], oid: Oid| msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(o)) if *o == oid));

    double_click(&mut h, pos2(CHIP_X, row_y(MAIN_ROW)), Modifiers::NONE);
    let msgs = &h.state().1;
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Branch(_) | Msg::Tag(TagMsg::Checkout { .. }))), "a tag isn't checked out");
    assert!(selects(msgs, main), "the row is selected instead");

    h.state_mut().1.clear();
    double_click(&mut h, pos2(700.0, row_y(SEARCH_ROW)), Modifiers::NONE);
    let msgs = &h.state().1;
    assert!(checkouts(msgs).is_empty(), "the message column checks nothing out");
    assert!(selects(msgs, search));

    h.state_mut().1.clear();
    double_click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), Modifiers::SHIFT);
    let msgs = &h.state().1;
    assert!(checkouts(msgs).is_empty(), "Shift keeps the selection behaviour");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Selection(SelectionMsg::SelectCommitRange(o)) if *o == search)));
    drop(h);
    remove_fixture(&dir);
}
