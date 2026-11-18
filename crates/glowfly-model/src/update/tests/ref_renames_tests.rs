//! Headless tests for hidden, shown-only and pinned-colour choices across the renames Glowfly
//! makes (update/ref_renames.rs): a branch renamed (with and without its remote copy), a remote
//! renamed or removed, their undo and redo, and a job that ends in another tab. The jobs'
//! results are sent by hand; the flows in glowfly-flows run them through the runner.

use super::tests::git;
use super::undo_rename_tests::{cleanup, opened, send, with_origin};
use crate::effect::{Effect, LoadEffect, SaveEffect, UndoResult};
use crate::msg::{BranchMsg, JobMsg, Msg, TabsMsg};
use crate::remote::RemoteMsg;
use crate::state::remotes::RemoteForm;
use crate::state::{AppState, Modal};
use glowfly_core::settings::GraphFilter;
use glowfly_core::undo::{Action, RemoteConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A repository with `feature` pushed to `origin`, opened. Returns (folder, bare folder, state).
fn fixture(name: &str) -> (PathBuf, PathBuf, AppState) {
    let (dir, bare) = with_origin(name);
    git(&dir, &["branch", "feature"]);
    git(&dir, &["push", "-q", "-u", "origin", "feature"]);
    let st = opened(&dir);
    (dir, bare, st)
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

fn pin(st: &mut AppState, key: &str, slot: u8) {
    let real = real(st);
    st.settings.set_branch_pin(&real, key, Some(slot));
}

fn pins(st: &AppState) -> BTreeMap<String, u8> {
    st.settings.branch_pins_for(&real(st))
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|r| r.to_string()).collect()
}

fn rename(remote: bool) -> Msg {
    Msg::Branch(BranchMsg::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote })
}

fn renamed(remote: Option<&str>) -> Action {
    Action::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote: remote.map(str::to_owned) }
}

fn finish(st: &mut AppState, ok: bool, output: &str, undo: UndoResult) -> Vec<Effect> {
    send(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: Some(undo) }))
}

fn recorded(a: Action) -> UndoResult {
    UndoResult::Recorded(Some(a))
}

fn done(redo: bool, a: Action) -> UndoResult {
    UndoResult::Done { redo, before: a.clone(), after: Box::new(a), stash: None }
}

fn settings_saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))).count()
}

/// The hidden and solo names of the single repository load `fx` starts.
fn load_filter(fx: &[Effect]) -> (Vec<String>, Vec<String>) {
    let loads: Vec<_> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Load(LoadEffect::Repo { opts, .. }) => Some((opts.hidden.clone(), opts.solo.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(loads.len(), 1, "one load: {fx:?}");
    loads.into_iter().next().unwrap()
}

/// Starts the undo or redo of the newest entry and sends its result.
fn undo_or_redo(st: &mut AppState, redo: bool, a: Action) -> Vec<Effect> {
    let fx = send(st, Msg::Job(if redo { JobMsg::Redo } else { JobMsg::Undo }));
    assert!(fx.iter().any(|e| matches!(e, Effect::Job(crate::effect::JobEffect::UndoRedo { .. }))), "{fx:?}");
    finish(st, true, "", done(redo, a))
}

#[test]
fn a_branch_rename_moves_the_hidden_entry_and_the_pin() {
    let (dir, bare, mut st) = fixture("refren-hidden");
    set_filter(&mut st, &["refs/heads/feature", "refs/remotes/origin/feature"], &[]);
    pin(&mut st, "feature", 9);
    send(&mut st, rename(false));
    let fx = finish(&mut st, true, "", recorded(renamed(None)));
    let want = names(&["refs/heads/feature-2", "refs/remotes/origin/feature"]);
    assert_eq!(filter(&st).hidden, want);
    assert_eq!(pins(&st), BTreeMap::from([("feature-2".to_owned(), 9)]));
    assert_eq!(settings_saves(&fx), 1, "{fx:?}");
    assert_eq!(load_filter(&fx), (want, vec![]));
    cleanup(&[&dir, &bare]);
}

#[test]
fn a_shown_only_branch_stays_shown_only_after_a_rename() {
    let (dir, bare, mut st) = fixture("refren-solo");
    set_filter(&mut st, &[], &["refs/heads/feature"]);
    send(&mut st, rename(false));
    let fx = finish(&mut st, true, "", recorded(renamed(None)));
    let want = names(&["refs/heads/feature-2"]);
    assert_eq!(load_filter(&fx), (vec![], want.clone()), "one load, with the moved filter");
    assert_eq!(filter(&st).solo, want);
    assert_eq!(settings_saves(&fx), 1);
    cleanup(&[&dir, &bare]);
}

#[test]
fn a_rename_with_the_remote_moves_the_remote_copy_only_when_every_step_worked() {
    let (dir, bare, mut st) = fixture("refren-remote-copy");
    set_filter(&mut st, &["refs/heads/feature", "refs/remotes/origin/feature"], &[]);
    send(&mut st, rename(true));
    finish(&mut st, true, "", recorded(renamed(Some("origin"))));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2", "refs/remotes/origin/feature-2"]));

    // The push failed: the old remote-tracking ref still exists, so its entry stays.
    set_filter(&mut st, &["refs/heads/feature", "refs/remotes/origin/feature"], &[]);
    pin(&mut st, "feature", 4);
    st.job = None;
    send(&mut st, rename(true));
    finish(&mut st, false, "error: failed to push", recorded(renamed(Some("origin"))));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2", "refs/remotes/origin/feature"]));
    assert_eq!(pins(&st), BTreeMap::from([("feature-2".to_owned(), 4)]), "the pin follows the local rename");
    cleanup(&[&dir, &bare]);
}

#[test]
fn undo_and_redo_of_a_branch_rename_move_the_entries_back_and_forward() {
    let (dir, bare, mut st) = fixture("refren-undo");
    set_filter(&mut st, &["refs/heads/feature"], &["refs/remotes/origin/main"]);
    pin(&mut st, "feature", 9);
    send(&mut st, rename(false));
    finish(&mut st, true, "", recorded(renamed(None)));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2"]));

    let fx = undo_or_redo(&mut st, false, renamed(None));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature"]));
    assert_eq!(filter(&st).solo, names(&["refs/remotes/origin/main"]));
    assert_eq!(pins(&st), BTreeMap::from([("feature".to_owned(), 9)]));
    assert_eq!(settings_saves(&fx), 1, "{fx:?}");

    undo_or_redo(&mut st, true, renamed(None));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2"]));
    assert_eq!(pins(&st), BTreeMap::from([("feature-2".to_owned(), 9)]));

    // A rename with the remote: its undo renames the local branch only.
    st.job = None;
    st.session.undo.record(renamed(Some("origin")));
    set_filter(&mut st, &["refs/heads/feature-2", "refs/remotes/origin/feature-2"], &[]);
    undo_or_redo(&mut st, false, renamed(Some("origin")));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature", "refs/remotes/origin/feature-2"]));
    undo_or_redo(&mut st, true, renamed(Some("origin")));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature-2", "refs/remotes/origin/feature-2"]));
    cleanup(&[&dir, &bare]);
}

fn remote_form_submit(st: &mut AppState, name: &str) -> Vec<Effect> {
    let info = st.snap.as_ref().unwrap().remote_info.iter().find(|r| r.name == "origin").cloned().unwrap();
    let mut form = RemoteForm::edit(&info);
    form.name = name.into();
    st.modal = Some(Modal::Remote(form));
    send(st, Msg::Remote(RemoteMsg::Submit))
}

fn remote_renamed() -> Action {
    Action::RenameRemote { old: "origin".into(), new: "upstream".into(), urls: None }
}

#[test]
fn a_remote_rename_rewrites_its_entries_and_keeps_pins() {
    let (dir, bare, mut st) = fixture("refren-remote");
    set_filter(&mut st, &["refs/remotes/origin/feature"], &["refs/remotes/origin/main"]);
    pin(&mut st, "feature", 9);
    remote_form_submit(&mut st, "upstream");
    let fx = finish(&mut st, true, "", recorded(remote_renamed()));
    assert_eq!(filter(&st).hidden, names(&["refs/remotes/upstream/feature"]));
    assert_eq!(filter(&st).solo, names(&["refs/remotes/upstream/main"]));
    assert_eq!(pins(&st), BTreeMap::from([("feature".to_owned(), 9)]));
    assert_eq!(settings_saves(&fx), 1);
    assert_eq!(load_filter(&fx), (names(&["refs/remotes/upstream/feature"]), names(&["refs/remotes/upstream/main"])));

    undo_or_redo(&mut st, false, remote_renamed());
    assert_eq!(filter(&st).hidden, names(&["refs/remotes/origin/feature"]));
    assert_eq!(filter(&st).solo, names(&["refs/remotes/origin/main"]));
    undo_or_redo(&mut st, true, remote_renamed());
    assert_eq!(filter(&st).hidden, names(&["refs/remotes/upstream/feature"]));
    assert_eq!(pins(&st), BTreeMap::from([("feature".to_owned(), 9)]));
    cleanup(&[&dir, &bare]);
}

fn remote_removed() -> Action {
    let config =
        RemoteConfig { entries: vec![("url".into(), "https://example.invalid/a.git".into())], branches: vec![], push_default: false };
    Action::RemoveRemote { name: "origin".into(), config }
}

#[test]
fn removing_a_remote_drops_its_entries_and_says_so() {
    let (dir, bare, mut st) = fixture("refren-remove");
    set_filter(&mut st, &["refs/heads/feature", "refs/remotes/origin/feature"], &["refs/remotes/origin/main"]);
    pin(&mut st, "feature", 9);
    send(&mut st, Msg::Remote(RemoteMsg::Remove("origin".into())));
    let fx = finish(&mut st, true, "", recorded(remote_removed()));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature"]));
    assert!(filter(&st).solo.is_empty());
    assert_eq!(pins(&st), BTreeMap::from([("feature".to_owned(), 9)]), "a pin has no remote in its key");
    assert_eq!(settings_saves(&fx), 1);
    let want = "Removed remote origin. The hidden or shown-only choices for 2 of its branches went with it.";
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some(want));
    assert_eq!(st.log.last().and_then(|l| l.run.as_ref()).map(|r| r.summary.as_str()), Some(want));

    // Undo puts the config back and leaves the filter alone; the toast offers the fetch.
    undo_or_redo(&mut st, false, remote_removed());
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature"]));
    let t = st.toasts.last().expect("a toast");
    assert_eq!(t.text, "Remote origin is back. Fetch it to bring back its remote-tracking branches.");
    assert!(t.action.as_ref().is_some_and(|a| a.label == "Fetch origin"));

    // Redo drops whatever is under the remote again.
    set_filter(&mut st, &["refs/remotes/origin/a"], &[]);
    undo_or_redo(&mut st, true, remote_removed());
    assert!(filter(&st).is_empty());
    assert_eq!(
        st.toasts.last().map(|t| t.text.as_str()),
        Some("Removed remote origin. The hidden or shown-only choice for 1 of its branches went with it.")
    );
    cleanup(&[&dir, &bare]);
}

#[test]
fn removing_a_remote_without_entries_keeps_the_plain_headline() {
    let (dir, bare, mut st) = fixture("refren-remove-none");
    set_filter(&mut st, &["refs/heads/feature"], &[]);
    send(&mut st, Msg::Remote(RemoteMsg::Remove("origin".into())));
    let fx = finish(&mut st, true, "", recorded(remote_removed()));
    assert_eq!(settings_saves(&fx), 0, "{fx:?}");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Remove remote origin succeeded"));
    assert_eq!(filter(&st).hidden, names(&["refs/heads/feature"]));

    // A rename with nothing listed and nothing pinned saves nothing either.
    set_filter(&mut st, &[], &[]);
    st.job = None;
    send(&mut st, rename(false));
    let fx = finish(&mut st, true, "", recorded(renamed(None)));
    assert_eq!(settings_saves(&fx), 0, "{fx:?}");
    cleanup(&[&dir, &bare]);
}

#[test]
fn a_rename_finished_in_a_background_tab_still_moves_the_entries() {
    let (dir, bare, mut st) = fixture("refren-background");
    let real = real(&st);
    st.tabs[0].real = Some(real);
    set_filter(&mut st, &["refs/heads/feature", "refs/remotes/origin/feature"], &[]);
    pin(&mut st, "feature", 9);
    send(&mut st, rename(true));
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(st.snap.is_none(), "another tab is active");
    let fx = finish(&mut st, true, "", recorded(renamed(Some("origin"))));
    assert_eq!(settings_saves(&fx), 1, "{fx:?}");
    let real = st.tabs[0].real.clone().unwrap();
    assert_eq!(st.settings.graph_filter_for(&real).hidden, names(&["refs/heads/feature-2", "refs/remotes/origin/feature-2"]));
    assert_eq!(st.settings.branch_pins_for(&real), BTreeMap::from([("feature-2".to_owned(), 9)]));
    cleanup(&[&dir, &bare]);
}
