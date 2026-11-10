//! Headless tests for picking files in Conflicted Files and Resolved Files
//! (`conflict_picks.rs`): Ctrl/Cmd picks with gaps without opening anything, Shift ranges from
//! the open file, removing a pick, what ends the picks (a plain click, the Unstaged and Staged
//! Files picks, an arrow key, the action), a NotUtf8 name refused, the actions on several picks
//! (one markers read before Mark resolved, one Mark conflicted), a status read pruning them, and
//! the open file left out of the picks not drawn selected.

use super::tests::state;
use super::update;
use crate::effect::{ConflictEffect, ConflictRead, Effect, FilesEffect, LocalOp};
use crate::msg::{ConflictMsg, LoadMsg, Msg, RepoMsg};
use crate::state::conflict::ConflictView;
use crate::state::conflict_picks::{drawn_selected, picked};
use crate::state::list_keys::{FileList, ListMove};
use crate::state::op_fixture::{rows, snapshot};
use crate::state::wip_lists::PickHow;
use crate::state::{AppState, Modal};
use git2::RepositoryState;
use glowfly_core::repo::{DiffTarget, FileEntry, Hazard, ResolvedFile};
use std::time::Instant;

fn send(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

/// Mid-merge with a.txt, b.txt and c.txt conflicted, and a.txt open in the conflict view.
fn conflicted(name: &str) -> AppState {
    let mut st = state();
    let mut snap = snapshot(name);
    snap.state = RepositoryState::Merge;
    snap.unstaged = rows(&[("a.txt", '!'), ("b.txt", '!'), ("c.txt", '!')]);
    st.snap = Some(snap);
    st.session.selection = crate::state::Selection::Wip;
    st.session.conflict.view = Some(ConflictView::loading("a.txt".into()));
    st
}

/// Mid-merge with nothing conflicted and a.txt, b.txt and c.txt resolved (both sides recorded,
/// staged), a.txt's staged diff open.
fn resolved(name: &str) -> AppState {
    let mut st = state();
    let mut snap = snapshot(name);
    snap.state = RepositoryState::Merge;
    snap.staged = rows(&[("a.txt", 'M'), ("b.txt", 'M'), ("c.txt", 'M')]);
    st.snap = Some(snap);
    st.session.selection = crate::state::Selection::Wip;
    st.session.conflict_files.resolved =
        ["a.txt", "b.txt", "c.txt"].map(|p| ResolvedFile { entry: FileEntry::new(p, ' '), both_sides: true }).to_vec();
    st.cues.diff_opening = Some(DiffTarget::Staged("a.txt".into()));
    st
}

fn pick(st: &mut AppState, resolved: bool, path: &str, how: PickHow) -> Vec<Effect> {
    send(st, Msg::Conflict(ConflictMsg::Pick { resolved, path: path.into(), how }))
}

fn picks(st: &AppState, resolved: bool) -> Vec<String> {
    picked(st, resolved).map(|p| p.paths).unwrap_or_default()
}

#[test]
fn ctrl_click_picks_with_gaps_without_opening() {
    let mut st = conflicted("cpicks-gaps");
    let fx = pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert!(fx.is_empty(), "nothing opens");
    assert_eq!(picks(&st, false), ["a.txt", "c.txt"], "the open file counts as picked");
    assert_eq!(st.session.conflict.view.as_ref().map(|c| c.path.as_str()), Some("a.txt"), "the conflict view stays");
    pick(&mut st, false, "a.txt", PickHow::Toggle);
    assert!(picked(&st, false).is_none(), "one pick left is no selection");
    assert_eq!(st.cues.conflict_picks.as_ref().map(|p| p.paths.len()), Some(1));
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert!(st.cues.conflict_picks.is_none(), "removing the last pick ends them");
}

#[test]
fn shift_click_picks_the_range_from_the_open_file() {
    let mut st = conflicted("cpicks-range");
    pick(&mut st, false, "c.txt", PickHow::Range);
    assert_eq!(picks(&st, false), ["a.txt", "b.txt", "c.txt"]);
    pick(&mut st, false, "b.txt", PickHow::Range);
    assert_eq!(picks(&st, false), ["a.txt", "b.txt"], "from the same anchor");
}

#[test]
fn a_plain_click_an_arrow_or_the_other_lists_end_the_picks() {
    let mut st = conflicted("cpicks-end");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    send(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("b.txt".into())));
    assert!(st.cues.conflict_picks.is_none(), "a plain click");

    pick(&mut st, false, "c.txt", PickHow::Toggle);
    send(&mut st, Msg::Shortcut(crate::keymap::KeyAction::FileList { list: FileList::Conflicted, mv: ListMove::Step { down: true } }));
    assert!(st.cues.conflict_picks.is_none(), "an arrow key");

    pick(&mut st, false, "c.txt", PickHow::Toggle);
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("u.txt", 'M'));
    send(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("u.txt".into()))));
    assert!(st.cues.conflict_picks.is_none(), "a plain click in another list");
}

#[test]
fn picks_belong_to_one_list_at_a_time() {
    let mut st = conflicted("cpicks-one-list");
    st.snap.as_mut().unwrap().unstaged.extend([FileEntry::new("u.txt", 'M'), FileEntry::new("v.txt", 'M')]);
    send(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("u.txt".into()))));
    send(&mut st, Msg::Repo(RepoMsg::PickWipFile { staged: false, path: "v.txt".into(), how: PickHow::Toggle }));
    assert!(st.cues.wip_picks.is_some());
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert!(st.cues.wip_picks.is_none(), "picking in Conflicted Files ends the Unstaged picks");
    assert!(st.cues.conflict_picks.is_some());
    send(&mut st, Msg::Repo(RepoMsg::PickWipFile { staged: false, path: "u.txt".into(), how: PickHow::Toggle }));
    assert!(st.cues.conflict_picks.is_none(), "and the other way round");
}

#[test]
fn a_name_that_isnt_utf8_is_refused() {
    let mut st = conflicted("cpicks-utf8");
    let c = st.snap.as_mut().unwrap().unstaged.iter_mut().find(|f| f.path == "c.txt").unwrap();
    c.hazard = Some(Hazard::NotUtf8(b"c\xff.txt".to_vec()));
    let refused = crate::state::path_hazards::refusal(
        &st,
        &Msg::Conflict(ConflictMsg::Pick { resolved: false, path: "c.txt".into(), how: PickHow::Toggle }),
    );
    assert!(refused.is_some());
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert!(st.cues.conflict_picks.is_none());
}

#[test]
fn mark_resolved_of_picks_reads_them_once_then_ends_them() {
    let mut st = conflicted("cpicks-resolve");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    let paths = picks(&st, false);
    let fx = send(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(paths)));
    let read = fx.iter().find_map(|e| match e {
        Effect::Conflict(ConflictEffect::ReadFiles { paths, purpose: ConflictRead::BeforeMarkResolved, .. }) => Some(paths.clone()),
        _ => None,
    });
    assert_eq!(read, Some(vec!["a.txt".to_owned(), "c.txt".to_owned()]));
    let fx = send(&mut st, Msg::Conflict(ConflictMsg::MarkResolvedConfirmed(vec!["a.txt".into(), "c.txt".into()])));
    let staged = fx
        .iter()
        .any(|e| matches!(e, Effect::Files(FilesEffect::LocalOp { op: LocalOp::MarkResolved(p), .. }) if *p == ["a.txt", "c.txt"]));
    assert!(staged, "one operation for both");
    assert!(st.cues.conflict_picks.is_none());
}

#[test]
fn mark_conflicted_of_resolved_picks_asks_once_for_both() {
    let mut st = resolved("cpicks-conflicted");
    pick(&mut st, true, "c.txt", PickHow::Toggle);
    let paths = picks(&st, true);
    assert_eq!(paths, ["a.txt", "c.txt"]);
    let t = st.settings.lang();
    let body = crate::state::conflict_files::mark_conflicted_body(t, &paths);
    let ask = Msg::confirm(
        t.tr("conflict.files.mark_conflicted.title"),
        body.clone(),
        t.tr("conflict.files.mark_conflicted.button"),
        Msg::Conflict(ConflictMsg::MarkConflicted(paths)),
    );
    send(&mut st, ask);
    let Some(Modal::Confirm { body: shown, .. }) = &st.modal else { panic!("one confirmation") };
    assert_eq!(*shown, body);
    assert!(body.contains('2'), "{body}");
    let fx = send(&mut st, Msg::ConfirmAccepted);
    let op = fx
        .iter()
        .any(|e| matches!(e, Effect::Files(FilesEffect::LocalOp { op: LocalOp::MarkConflicted(p), .. }) if *p == ["a.txt", "c.txt"]));
    assert!(op, "one operation for both");
    assert!(st.cues.conflict_picks.is_none());
}

#[test]
fn a_status_read_prunes_picks_that_left_the_list() {
    let mut st = conflicted("cpicks-prune");
    pick(&mut st, false, "b.txt", PickHow::Toggle);
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert_eq!(picks(&st, false), ["a.txt", "b.txt", "c.txt"]);
    // a.txt was marked resolved elsewhere.
    let status = crate::effect::StatusRead::Whole(rows(&[("b.txt", '!'), ("c.txt", '!')]), Vec::new(), RepositoryState::Merge);
    let loaded = LoadMsg::StatusLoaded {
        generation: st.load_gen,
        result: Ok(status),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    };
    send(&mut st, Msg::Load(loaded));
    let held = st.cues.conflict_picks.as_ref().map(|p| p.paths.iter().cloned().collect::<Vec<_>>());
    assert_eq!(held, Some(vec!["b.txt".to_owned(), "c.txt".to_owned()]));
}

/// The rows of that list drawn selected, in order, `open` being the open file.
fn drawn(st: &AppState, resolved: bool, open: &str) -> Vec<&'static str> {
    let held = crate::state::conflict_picks::held(st, resolved);
    ["a.txt", "b.txt", "c.txt"].into_iter().filter(|p| drawn_selected(held, p, *p == open)).collect()
}

#[test]
fn the_open_file_taken_out_of_the_picks_isnt_drawn_selected() {
    let mut st = conflicted("cpicks-drawn");
    assert_eq!(drawn(&st, false, "a.txt"), ["a.txt"], "no picks: the open file");
    pick(&mut st, false, "b.txt", PickHow::Toggle);
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert_eq!(drawn(&st, false, "a.txt"), ["a.txt", "b.txt", "c.txt"]);
    pick(&mut st, false, "a.txt", PickHow::Toggle);
    assert_eq!(picks(&st, false), ["b.txt", "c.txt"]);
    assert_eq!(drawn(&st, false, "a.txt"), ["b.txt", "c.txt"], "drawn as the action takes them");
    assert_eq!(st.session.conflict.view.as_ref().map(|c| c.path.as_str()), Some("a.txt"), "still open");

    // A single pick left still shows, without the open file.
    pick(&mut st, false, "b.txt", PickHow::Toggle);
    assert!(picked(&st, false).is_none());
    assert_eq!(drawn(&st, false, "a.txt"), ["c.txt"]);
    pick(&mut st, false, "b.txt", PickHow::Toggle);
    assert_eq!(drawn(&st, false, "a.txt"), ["b.txt", "c.txt"], "no hidden pick reappears");
    assert_eq!(picks(&st, false), ["b.txt", "c.txt"]);

    // Resolved Files too.
    let mut st = resolved("cpicks-drawn-resolved");
    pick(&mut st, true, "c.txt", PickHow::Toggle);
    pick(&mut st, true, "a.txt", PickHow::Toggle);
    assert_eq!(drawn(&st, true, "a.txt"), ["c.txt"]);
}
