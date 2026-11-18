//! Hidden and shown-only choices across renames, through the runner: a renamed branch stays
//! hidden through the reload, the undo and the redo; a rename with the remote keeps both of its
//! refs hidden; a remote rename moves its entries and a removal drops them; and a rename made
//! outside Glowfly still drops the entry on the next load.

use super::branch_tests::with_origin;
use super::tests::{drive, git};
use glowfly_core::settings::GraphFilter;
use glowfly_model::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use glowfly_model::remote::RemoteMsg;
use glowfly_model::state::AppState;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn real(st: &AppState) -> PathBuf {
    st.snap.as_ref().unwrap().real_workdir.clone()
}

fn filter(st: &AppState) -> GraphFilter {
    st.settings.graph_filter_for(&real(st))
}

fn set_filter(st: &mut AppState, hidden: &[&str], solo: &[&str]) {
    let f = GraphFilter { hidden: hidden.iter().map(|r| r.to_string()).collect(), solo: solo.iter().map(|r| r.to_string()).collect() };
    let real = real(st);
    st.settings.set_graph_filter(&real, f);
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|r| r.to_string()).collect()
}

fn has_label(st: &AppState, name: &str) -> bool {
    st.snap.as_ref().unwrap().refs.values().flatten().any(|l| l.name == name)
}

fn rename(remote: bool) -> Msg {
    Msg::Branch(BranchMsg::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote })
}

/// `with_origin`, with `feature` one commit ahead of `main` so its label stands on its own row.
fn fixture(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    git(&work, &["checkout", "-q", "feature"]);
    std::fs::write(work.join("feature.txt"), "feature").unwrap();
    git(&work, &["add", "feature.txt"]);
    git(&work, &["commit", "-q", "-m", "feature work"]);
    git(&work, &["push", "-q", "origin", "feature"]);
    git(&work, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, bare)
}

#[test]
fn a_hidden_renamed_branch_stays_hidden_through_the_reload_and_undo() {
    let (mut st, work, bare) = fixture("refren-flow-hidden");
    set_filter(&mut st, &["refs/heads/feature"], &[]);
    let real = real(&st);
    st.settings.set_branch_pin(&real, "feature", Some(9));
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!has_label(&st, "feature"), "hidden to start with");

    drive(&mut st, rename(false));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2"]));
    assert!(!has_label(&st, "feature-2") && !has_label(&st, "feature"), "never flashes back in");
    assert!(st.snap.as_ref().unwrap().locals.iter().any(|b| b.name == "feature-2"));
    assert_eq!(st.settings.branch_pins_for(&real), BTreeMap::from([("feature-2".to_owned(), 9)]));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature"]));
    assert!(!has_label(&st, "feature"), "still hidden after the undo");
    assert_eq!(st.settings.branch_pins_for(&real), BTreeMap::from([("feature".to_owned(), 9)]));

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2"]));
    assert!(!has_label(&st, "feature-2"));
    assert_eq!(st.settings.branch_pins_for(&real), BTreeMap::from([("feature-2".to_owned(), 9)]));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_rename_with_the_remote_keeps_both_refs_hidden() {
    let (mut st, work, bare) = fixture("refren-flow-both");
    set_filter(&mut st, &["refs/heads/feature", "refs/remotes/origin/feature"], &[]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, rename(true));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2", "refs/remotes/origin/feature-2"]));
    assert!(!has_label(&st, "feature-2") && !has_label(&st, "origin/feature-2"));
    assert!(st.snap.as_ref().unwrap().remotes.iter().any(|(_, bs)| bs.iter().any(|(b, _)| b == "feature-2")), "the remote copy exists");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_shown_only_remote_branch_follows_a_remote_rename_and_goes_with_a_removal() {
    let (mut st, work, bare) = fixture("refren-flow-remote");
    set_filter(&mut st, &[], &["refs/remotes/origin/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Remote(RemoteMsg::EditDialog("origin".into())));
    let Some(glowfly_model::state::Modal::Remote(form)) = st.modal.as_mut() else { panic!("no remote dialog") };
    form.name = "upstream".into();
    drive(&mut st, Msg::Remote(RemoteMsg::Submit));
    assert_eq!(filter(&st).solo, names(&["refs/remotes/upstream/feature"]), "moved, not pruned");

    drive(&mut st, Msg::Remote(RemoteMsg::Remove("upstream".into())));
    assert!(st.settings.graph_filters.is_empty(), "no filter is kept for the repository");
    let t = st.toasts.last().expect("a toast");
    assert_eq!(t.text, "Removed remote upstream. The hidden or shown-only choice for 1 of its branches went with it.");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_rename_outside_glowfly_still_drops_the_entry() {
    let (mut st, work, bare) = fixture("refren-flow-outside");
    set_filter(&mut st, &["refs/heads/feature"], &[]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!has_label(&st, "feature"));
    git(&work, &["branch", "-m", "feature", "feature-2"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(filter(&st).is_empty(), "the next load drops what it can't find");
    assert!(has_label(&st, "feature-2"), "and the renamed branch shows");
    cleanup(&[&work, &bare]);
}
