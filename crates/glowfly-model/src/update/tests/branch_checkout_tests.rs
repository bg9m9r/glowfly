//! Checking out what is already checked out (update/branch_checkout.rs): a double-click on the
//! current branch, or on the remote branch it tracks, does nothing and says nothing; and a
//! failure said twice leaves one toast, its timer restarted (state/toasts.rs).

use super::tests::{git, scratch_repo, state};
use super::update;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use glowfly_core::repo::{self, LoadOptions};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A state on a repository whose checked-out `main` tracks `origin/main`, at the same commit.
fn on_main(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&dir, &["config", "branch.main.remote", "origin"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

fn texts(st: &AppState) -> Vec<&str> {
    st.toasts.iter().map(|t| t.text.as_str()).collect()
}

fn remote(name: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutRemote { remote: "origin".into(), name: name.into() })
}

#[test]
fn double_clicking_the_current_branch_does_nothing() {
    let (mut st, dir) = on_main("checkout-head-local");
    let now = Instant::now();
    for _ in 0..3 {
        let fx = update(&mut st, Msg::Branch(BranchMsg::Checkout("main".into())), now);
        assert!(fx.is_empty() && st.job.is_none(), "no job starts");
    }
    assert!(st.toasts.is_empty(), "no toast: {:?}", texts(&st));
    assert!(st.log.iter().all(|e| e.ok), "no failure logged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn double_clicking_the_remote_branch_head_tracks_does_nothing() {
    let (mut st, dir) = on_main("checkout-head-remote");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.locals.iter().any(|b| b.is_head && crate::state::remote_branch::tracks(b, "origin", "main")));
    let now = Instant::now();
    for _ in 0..3 {
        let fx = update(&mut st, remote("main"), now);
        assert!(fx.is_empty() && st.job.is_none(), "no job starts");
    }
    assert!(st.toasts.is_empty(), "no toast: {:?}", texts(&st));
    assert!(st.log.iter().all(|e| e.ok), "no failure logged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_same_failure_twice_leaves_one_toast_with_its_timer_restarted() {
    let (mut st, dir) = on_main("checkout-gone-twice");
    let t0 = Instant::now();
    update(&mut st, remote("gone"), t0);
    assert_eq!(texts(&st), ["origin/gone no longer exists."]);
    let (id, first) = (st.toasts[0].id, st.toasts[0].first);
    let later = t0 + Duration::from_secs(3);
    update(&mut st, remote("gone"), later);
    assert_eq!(texts(&st), ["origin/gone no longer exists."], "one toast, not two");
    let t = &st.toasts[0];
    assert_eq!((t.id, t.first, t.at, t.counted), (id, first, later, Duration::ZERO), "the same toast, its timer restarted");
    assert!(t.action.is_some() && t.details.is_none(), "one Details button, not two");
    assert_eq!(st.log.iter().filter(|e| !e.ok).count(), 2, "each failure is still in the activity log");
    let _ = std::fs::remove_dir_all(&dir);
}
