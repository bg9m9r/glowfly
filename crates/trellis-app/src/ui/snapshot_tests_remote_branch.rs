//! The REMOTE row menu of the left panel: the local branch that tracks the remote branch
//! (checkout, fast-forward) or a new one, and the operations against the checked-out branch.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::repo::{BranchInfo, Tracking};
use trellis_core::settings::ThemeChoice;

fn tip(st: &AppState, name: &str) -> Oid {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap()
}

fn branch<'a>(st: &'a mut AppState, name: &str) -> &'a mut BranchInfo {
    st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == name).unwrap()
}

/// The fixture with `origin`'s branches `main` and `search` at the local tips, plus `extra`
/// (name, local branch whose tip it is at).
fn with_remote(theme: ThemeChoice, test: &str, extra: &[(&str, &str)]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, test);
    let mut branches: Vec<(String, Oid)> = ["main", "search"].iter().map(|n| (n.to_string(), tip(&st, n))).collect();
    branches.extend(extra.iter().map(|(n, at)| (n.to_string(), tip(&st, at))));
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), branches)];
    (st, dir)
}

/// Right-clicks the left panel's lowest row labelled `label` (the REMOTE one), near its left
/// end (an open menu covers the middle of the rows below the one it belongs to).
fn open_remote_row(h: &mut Harness<'_, (AppState, Vec<Msg>)>, label: &str) {
    let rows = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 250.0);
    let rect = rows.max_by(|a, b| a.y0.total_cmp(&b.y0)).expect("the sidebar row");
    let at = egui::pos2(rect.x0 as f32 + 40.0, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(h, at, PointerButton::Secondary);
}

fn sent(h: &Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&BranchMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Branch(b) if want(b)))
}

#[test]
fn remote_branch_menu_tracked() {
    let (mut st, dir) = with_remote(ThemeChoice::Dark, "remote-branch-menu-tracked", &[]);
    let b = branch(&mut st, "search");
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "search".into() });
    b.upstream = Some("origin/search".into());
    b.upstream_ref = Some("refs/remotes/origin/search".into());
    b.behind = 2;
    let mut h = harness(st, true);
    h.run();
    open_remote_row(&mut h, "search");
    h.check_snapshot("remote_branch_menu_tracked");
    for label in [
        "Checkout search",
        "Fast-forward search to this branch",
        "Interactive rebase main onto this branch…",
        "Check for conflicts with main",
        "Compare with main",
        "Set as upstream of main",
        "Create branch here…",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Checkout").is_none(), "the tracking branch is offered instead");
    h.get_by_label("Fast-forward search to this branch").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, BranchMsg::FastForwardBranch(n) if n == "search")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_branch_menu_untracked() {
    let (st, dir) = with_remote(ThemeChoice::Light, "remote-branch-menu-untracked", &[("topic", "main")]);
    let mut h = harness(st, false);
    h.run();
    open_remote_row(&mut h, "search");
    h.check_snapshot("remote_branch_menu_untracked");
    let checkout = h.get_by_label("Checkout");
    assert!(checkout.is_disabled(), "a local search exists and doesn't track it");
    checkout.click();
    h.run();
    assert!(!sent(&h, |m| matches!(m, BranchMsg::CheckoutRemote { .. })), "a disabled item sends nothing");
    assert!(h.query_by_label("Fast-forward search to this branch").is_none());

    h.press_key(egui::Key::Escape);
    h.run();
    open_remote_row(&mut h, "topic");
    h.get_by_label("Checkout").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, BranchMsg::CheckoutRemote { remote, name } if remote == "origin" && name == "topic")));
    drop(h);
    remove_fixture(&dir);
}
