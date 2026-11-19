//! Headless tests for Take the trivial picks in all files (update/conflict_trivial.rs): the one
//! op sent for the usable conflicted files, the ones held back and named in the log, the toast
//! with its counts and its Undo, the open file moving on, an inactive tab keeping the undo, and
//! the Preferences switch for the picks made when a file opens.

use super::tests::state;
use super::update;
use crate::effect::{ConflictEffect, Effect, FilesEffect, JobEffect, LocalOp};
use crate::msg::{ConflictMsg, Inactive, JobMsg, Msg};
use crate::state::conflict::ConflictView;
use crate::state::op_fixture::{rows, snapshot};
use crate::state::{AppState, Selection};
use git2::RepositoryState;
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::{
    Chunk, ConflictContent, ConflictFile, ConflictIds, Hazard, Operation, Segment, SideLabels, TrivialSkip, TrivialTaken,
};
use glowfly_core::undo::{Action, ConflictChange};
use std::time::Instant;

fn send(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

/// Mid-merge with these paths conflicted.
fn merging(name: &str, paths: &[&str]) -> AppState {
    let mut st = state();
    let mut snap = snapshot(name);
    snap.state = RepositoryState::Merge;
    snap.unstaged = rows(&paths.iter().map(|p| (*p, '!')).collect::<Vec<_>>());
    st.snap = Some(snap);
    st.session.selection = Selection::Wip;
    st
}

fn take(st: &mut AppState) -> Vec<Effect> {
    send(st, Msg::Conflict(ConflictMsg::TakeTrivialPicks))
}

/// The paths of the one `TakeTrivialPicks` op among `fx`.
fn sent(fx: Vec<Effect>) -> Option<Vec<String>> {
    let mut ops = fx.into_iter().filter_map(|e| match e.unsealed() {
        Effect::Files(FilesEffect::LocalOp { op: LocalOp::TakeTrivialPicks(p), .. }) => Some(p),
        _ => None,
    });
    let first = ops.next();
    assert!(ops.next().is_none(), "one op");
    first
}

fn open(st: &mut AppState, path: &str) {
    st.session.conflict.view = Some(ConflictView::loading(path.into()));
}

fn last_toast(st: &AppState) -> &crate::state::toasts::Toast {
    st.toasts.last().expect("a toast")
}

fn undo_entry(paths: &[&str]) -> Action {
    let files = paths
        .iter()
        .map(|p| {
            let state = glowfly_core::undo::PathState {
                index: glowfly_core::undo::IndexAt::Merged(None, Default::default()),
                work: glowfly_core::undo::WorkAt::Absent,
            };
            glowfly_core::undo::ConflictFile { path: (*p).into(), before: state.clone(), after: state }
        })
        .collect();
    Action::Conflicts { change: ConflictChange::TrivialPicks, head: None, state: RepositoryState::Merge, heads: Vec::new(), files }
}

fn taken(resolved: &[&str], left: &[&str], skipped: Vec<(&str, TrivialSkip)>) -> TrivialTaken {
    let own = |v: &[&str]| v.iter().map(|p| (*p).to_owned()).collect();
    TrivialTaken { resolved: own(resolved), left: own(left), skipped: skipped.into_iter().map(|(p, s)| (p.to_owned(), s)).collect() }
}

#[test]
fn take_sends_one_op_for_the_usable_conflicted_files() {
    let mut st = merging("trivial-send", &["a.txt", "b.txt"]);
    let odd = glowfly_core::repo::FileEntry { hazard: Some(Hazard::NotUtf8(vec![0xff])), ..glowfly_core::repo::FileEntry::new("odd", '!') };
    st.snap.as_mut().unwrap().unstaged.push(odd);
    assert_eq!(sent(take(&mut st)), Some(vec!["a.txt".to_owned(), "b.txt".to_owned()]));
    assert!(st.toasts.is_empty());
}

#[test]
fn files_open_with_unsaved_picks_or_in_the_merge_tool_are_held_back() {
    let mut st = merging("trivial-held", &["a.txt", "b.txt", "c.txt"]);
    open(&mut st, "a.txt");
    st.session.conflict.view.as_mut().unwrap().picks = vec![ChunkPick::Sides(glowfly_core::repo::Choice::Ours)];
    st.session.conflict.view.as_mut().unwrap().auto = vec![None];
    assert_eq!(sent(take(&mut st)), Some(vec!["b.txt".to_owned(), "c.txt".to_owned()]));
    let log = st.log.last().map(|l| l.text.clone()).unwrap_or_default();
    assert_eq!(log, "Take the trivial picks skipped 1 file: a.txt (open with picks that aren't saved)");

    // A save running holds the open file back as well.
    let mut st = merging("trivial-saving", &["a.txt", "b.txt"]);
    open(&mut st, "a.txt");
    st.session.conflict.view.as_mut().unwrap().saving = true;
    assert_eq!(sent(take(&mut st)), Some(vec!["b.txt".to_owned()]));

    // Every file held back: nothing is sent and a failure toast says why.
    let mut st = merging("trivial-all-held", &["a.txt"]);
    open(&mut st, "a.txt");
    st.session.conflict.view.as_mut().unwrap().saving = true;
    assert_eq!(sent(take(&mut st)), None);
    let t = last_toast(&st);
    assert!(!t.ok);
    assert_eq!(t.text, "Every conflicted file is open with picks that aren't saved or in the merge tool");
}

#[test]
fn nothing_conflicted_toasts_why_and_sends_nothing() {
    let mut st = merging("trivial-none", &[]);
    assert_eq!(sent(take(&mut st)), None);
    let t = last_toast(&st);
    assert!(!t.ok && t.text == "Nothing is conflicted", "{}", t.text);

    let mut st = merging("trivial-odd", &[]);
    let odd = glowfly_core::repo::FileEntry { hazard: Some(Hazard::NotUtf8(vec![0xff])), ..glowfly_core::repo::FileEntry::new("odd", '!') };
    st.snap.as_mut().unwrap().unstaged.push(odd);
    assert_eq!(sent(take(&mut st)), None);
    assert!(!last_toast(&st).ok);
    assert_ne!(last_toast(&st).text, "Nothing is conflicted");
}

#[test]
fn a_mixed_result_toasts_the_counts_with_one_undo_and_names_the_skipped() {
    let mut st = merging("trivial-mixed", &["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"]);
    let entry = undo_entry(&["a.txt", "d.txt"]);
    let before = st.session.undo.undo.len();
    let result = taken(
        &["a.txt", "d.txt"],
        &["b.txt"],
        vec![("logo.bin", TrivialSkip::Binary), ("data.csv", TrivialSkip::Converted("filter=lfs".into()))],
    );
    let fx = send(&mut st, Msg::Conflict(ConflictMsg::TrivialPicksTaken { taken: result, undo: Some(entry.clone()) }));
    assert_eq!(st.session.undo.undo.len(), before + 1);
    assert_eq!(st.session.undo.undo.last(), Some(&entry));
    let t = last_toast(&st);
    assert!(t.ok);
    assert_eq!(
        t.text,
        "Resolved 2 files with their trivial picks. 3 files still need a look. The activity log names the files that were skipped."
    );
    let action = t.action.as_ref().expect("an Undo button");
    assert_eq!(action.label, "Undo");
    assert!(matches!(&*action.msg, Msg::Job(JobMsg::UndoWith { redo: false, action: a, .. }) if **a == entry));
    let log = st.log.iter().map(|l| l.text.as_str()).find(|l| l.starts_with("Take the trivial picks skipped")).expect("a log line");
    assert!(log.contains("logo.bin (binary)") && log.contains("data.csv (git converts it, filter=lfs)"), "{log}");
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(_))), "a status read follows: {fx:?}");
}

#[test]
fn the_toasts_undo_runs_that_one_entry() {
    let mut st = merging("trivial-undo", &["a.txt", "b.txt"]);
    let entry = undo_entry(&["a.txt"]);
    send(&mut st, Msg::Conflict(ConflictMsg::TrivialPicksTaken { taken: taken(&["a.txt"], &[], vec![]), undo: Some(entry.clone()) }));
    let id = last_toast(&st).id;
    let fx = send(&mut st, Msg::ToastAction(id));
    let ran =
        fx.into_iter().any(|e| matches!(e.unsealed(), Effect::Job(JobEffect::UndoRedo { action, redo: false, .. }) if action == entry));
    assert!(ran);
}

#[test]
fn nothing_resolved_says_so_without_undo() {
    let mut st = merging("trivial-nothing", &["a.txt", "b.txt"]);
    let before = st.session.undo.undo.len();
    send(&mut st, Msg::Conflict(ConflictMsg::TrivialPicksTaken { taken: taken(&[], &["a.txt", "b.txt"], vec![]), undo: None }));
    let t = last_toast(&st);
    assert!(!t.ok);
    assert_eq!(t.text, "No file has only trivial conflicts. 2 files still need a look.");
    assert!(t.action.is_none());
    assert_eq!(st.session.undo.undo.len(), before);

    // Nothing at all to say: no toast.
    let mut st = merging("trivial-empty", &[]);
    send(&mut st, Msg::Conflict(ConflictMsg::TrivialPicksTaken { taken: TrivialTaken::default(), undo: None }));
    assert!(st.toasts.is_empty());
}

#[test]
fn the_open_file_moves_on_when_it_was_resolved() {
    let mut st = merging("trivial-moves", &["a.txt", "b.txt", "c.txt"]);
    open(&mut st, "a.txt");
    let fx = send(
        &mut st,
        Msg::Conflict(ConflictMsg::TrivialPicksTaken {
            taken: taken(&["a.txt", "b.txt"], &[], vec![]),
            undo: Some(undo_entry(&["a.txt", "b.txt"])),
        }),
    );
    assert_eq!(st.session.conflict.view.as_ref().map(|c| c.path.as_str()), Some("c.txt"));
    assert!(fx.iter().any(|e| matches!(e, Effect::Conflict(ConflictEffect::Load { path, .. }) if path == "c.txt")), "{fx:?}");

    let mut st = merging("trivial-closes", &["a.txt"]);
    open(&mut st, "a.txt");
    send(
        &mut st,
        Msg::Conflict(ConflictMsg::TrivialPicksTaken { taken: taken(&["a.txt"], &[], vec![]), undo: Some(undo_entry(&["a.txt"])) }),
    );
    assert!(st.session.conflict.view.is_none());

    let mut st = merging("trivial-stays", &["a.txt", "b.txt"]);
    open(&mut st, "a.txt");
    send(
        &mut st,
        Msg::Conflict(ConflictMsg::TrivialPicksTaken { taken: taken(&["b.txt"], &["a.txt"], vec![]), undo: Some(undo_entry(&["b.txt"])) }),
    );
    assert_eq!(st.session.conflict.view.as_ref().map(|c| c.path.as_str()), Some("a.txt"));
}

#[test]
fn an_inactive_tab_keeps_only_the_undo() {
    let a = undo_entry(&["a.txt"]);
    let some = ConflictMsg::TrivialPicksTaken { taken: taken(&["a.txt"], &[], vec![]), undo: Some(a.clone()) };
    assert!(matches!(some.on_inactive(), Some(Inactive::Undo(x)) if x == a));
    let none = ConflictMsg::TrivialPicksTaken { taken: TrivialTaken::default(), undo: None };
    assert!(none.on_inactive().is_none());
    assert!(ConflictMsg::TakeTrivialPicks.on_inactive().is_none());
}

/// top, a chunk whose sides differ only in whitespace (a trivial pick), a real conflict.
fn file() -> ConflictFile {
    let line = |s: &str| vec![format!("{s}\n")];
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() },
        content: ConflictContent::Text(vec![
            Segment::Clean(line("top")),
            Segment::Conflict(Chunk { base: line("a b"), ours: line("a  b"), theirs: line("a b") }),
            Segment::Clean(line("mid")),
            Segment::Conflict(Chunk { base: line("b"), ours: line("x"), theirs: line("y") }),
            Segment::Clean(line("end")),
        ]),
        no_ancestor: false,
        base_markers: false,
    }
}

fn opened(st: &mut AppState) -> &ConflictView {
    open(st, "f.txt");
    let generation = st.session.conflict.generation;
    let msg = ConflictMsg::ConflictLoaded { path: "f.txt".into(), generation, result: Ok(file()), saved: None };
    send(st, Msg::Conflict(msg));
    st.session.conflict.view.as_ref().unwrap()
}

#[test]
fn the_switch_off_opens_a_file_without_automatic_picks() {
    let mut st = merging("trivial-switch", &["f.txt"]);
    st.settings.conflict_auto_picks = false;
    let v = opened(&mut st);
    assert_eq!(v.picks, [ChunkPick::default(), ChunkPick::default()]);
    assert_eq!((v.auto.clone(), v.current), (vec![None, None], 0));

    let mut st = merging("trivial-switch-on", &["f.txt"]);
    assert!(st.settings.conflict_auto_picks, "on by default");
    let v = opened(&mut st);
    assert_eq!(v.picks[0], ChunkPick::Sides(glowfly_core::repo::Choice::Ours));
    assert_eq!((v.picks[1].clone(), v.current), (ChunkPick::default(), 1));
}
