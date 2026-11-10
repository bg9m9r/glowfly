//! Headless tests for working inside a submodule from its row, through the runner on real
//! repositories: Record this checkout stages the moved gitlink, the diff of the gitlink carries
//! the move, Fetch and Pull run in the submodule's folder, and Discard changes inside leaves the
//! changes in a stash there and the parent's row clean.

use super::submodule_tests::{PATH, cleanup, commit_detached, opened, sub};
use super::tests::{drive, git};
use glowfly_core::repo::{DiffTarget, MoveKind, SubmoduleState};
use glowfly_core::settings::SavedTrust;
use glowfly_core::undo::DISCARD_STASH_MESSAGE;
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::{AppState, gitlink_card};
use glowfly_model::submodule::SubmoduleMsg;
use std::path::Path;

fn inside(st: &mut AppState, m: SubmoduleMsg) {
    drive(st, Msg::Submodule(m));
}

/// Trusts the submodule's folder, as opening it once would.
fn trust(st: &mut AppState, parent: &Path) {
    st.settings.set_trust(&parent.join(PATH), Some(SavedTrust::trusted(None)));
}

fn head_of(dir: &Path) -> git2::Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

#[test]
fn record_stages_the_moved_gitlink() {
    let (mut st, parent, lib) = opened("smi-record");
    let moved = commit_detached(&parent, "moved\n");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(sub(&st).state, SubmoduleState::OutOfSync);

    // The diff of the gitlink carries the move.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged(PATH.into()))));
    let m = gitlink_card::shown(&st).expect("the gitlink's move");
    assert_eq!((m.kind.clone(), m.ahead.total, m.new), (MoveKind::Forward, 1, Some(moved)));
    assert_eq!(gitlink_card::record_offered(&st).as_deref(), Some(PATH));

    inside(&mut st, SubmoduleMsg::Record(PATH.into()));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.iter().any(|f| f.path == PATH), "{:?}", snap.staged);
    assert_eq!(sub(&st).state, SubmoduleState::InSync, "the index records the checkout now");
    cleanup(&[&parent, &lib]);
}

#[test]
fn fetch_and_pull_run_inside() {
    let (mut st, parent, lib) = opened("smi-pull");
    trust(&mut st, &parent);
    let dir = parent.join(PATH);
    let before = head_of(&dir);
    // A new commit upstream, in the library the submodule was cloned from.
    git(&lib, &["commit", "-q", "--allow-empty", "-m", "upstream"]);
    let upstream = head_of(&lib);

    inside(&mut st, SubmoduleMsg::Fetch(PATH.into()));
    let fetched = git2::Repository::open(&dir).unwrap().refname_to_id("refs/remotes/origin/main").unwrap();
    assert_eq!(fetched, upstream);
    assert_eq!(head_of(&dir), before, "fetch leaves the checkout");

    inside(&mut st, SubmoduleMsg::Pull(PATH.into()));
    assert_eq!(head_of(&dir), upstream);
    assert_eq!(sub(&st).state, SubmoduleState::OutOfSync, "the parent refreshed");
    assert!(st.submodules.inside.is_none());
    cleanup(&[&parent, &lib]);
}

#[test]
fn pull_refuses_a_detached_head() {
    let (mut st, parent, lib) = opened("smi-detached");
    trust(&mut st, &parent);
    git(&parent.join(PATH), &["checkout", "-q", "--detach"]);
    inside(&mut st, SubmoduleMsg::Pull(PATH.into()));
    assert!(st.job.is_none());
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.text.contains("detached HEAD"), "{}", toast.text);
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Open submodule"));
    cleanup(&[&parent, &lib]);
}

#[test]
fn discard_inside_keeps_the_changes_in_a_stash() {
    let (mut st, parent, lib) = opened("smi-discard");
    trust(&mut st, &parent);
    let dir = parent.join(PATH);
    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(sub(&st).state, SubmoduleState::Dirty);

    inside(&mut st, SubmoduleMsg::DiscardInside { path: PATH.into(), confirmed: true });
    assert!(!dir.join("new.txt").exists());
    let stashes: Vec<String> = glowfly_core::stash_ops::list(&dir).unwrap().into_iter().map(|s| s.message).collect();
    assert!(stashes.iter().any(|m| m.ends_with(DISCARD_STASH_MESSAGE)), "{stashes:?}");
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.text.starts_with("Discarded the changes inside libs/lib"), "{}", toast.text);
    cleanup(&[&parent, &lib]);
}
