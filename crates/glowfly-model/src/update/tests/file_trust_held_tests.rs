//! Headless tests for file work that waits for the trust prompt (update/file_trust.rs,
//! update/trust_waiting.rs): a conflict save that waited settles the conflict view like one sent
//! at once, one that is dropped leaves the view ready to save again, and waiting work runs only
//! on the prompt that names it.

use super::file_trust_tests::{decide, files, held, real, refused, send, stage_file, waits};
use crate::effect::{FilesEffect, LocalOp};
use crate::msg::{ConflictMsg, Msg, StageMsg, TrustMsg, WorkFileMsg};
use crate::state::conflict::ConflictView;
use crate::state::op_fixture::rows;
use crate::state::{AppState, Modal};
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::{Choice, Chunk, ConflictContent, ConflictFile, ConflictIds, Operation, Segment, Side, SideLabels};

/// `st` with a.txt conflicted and open in the conflict view: two chunks, the first picked, the
/// second too when `all`.
fn editing(mut st: AppState, all: bool) -> AppState {
    let line = |s: &str| vec![format!("{s}\n")];
    let chunk =
        |n: u8| Segment::Conflict(Chunk { base: line("base"), ours: line(&format!("ours {n}")), theirs: line(&format!("theirs {n}")) });
    let file = ConflictFile {
        path: "a.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() },
        content: ConflictContent::Text(vec![Segment::Clean(line("top")), chunk(1), Segment::Clean(line("mid")), chunk(2)]),
        no_ancestor: false,
        base_markers: false,
    };
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", '!'), ("b.txt", '!')]);
    let mut view = ConflictView::loading("a.txt".into());
    view.set_file(file);
    view.picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(if all { Choice::Theirs } else { Choice::Unresolved })];
    st.session.conflict.view = Some(view);
    st
}

fn view(st: &AppState) -> &ConflictView {
    st.session.conflict.view.as_ref().expect("the conflict view")
}

/// (saving, steps on when resolved, a Save with markers noted).
fn marks(st: &AppState) -> (bool, bool, bool) {
    let c = view(st);
    (c.saving, c.step_on_resolve, c.pending_save.is_some())
}

fn done(st: &mut AppState) {
    send(st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))));
}

#[test]
fn a_conflict_save_that_waited_settles_the_view_like_one_sent_at_once() {
    for (name, make) in [("fth-save-held", held as fn(&str) -> AppState), ("fth-save-refused", refused)] {
        let mut st = editing(make(name), true);
        assert!(files(send(&mut st, Msg::Conflict(ConflictMsg::SaveConflict))).is_empty(), "it waits");
        assert_eq!(waits(&st).as_deref(), Some("Resolve conflict"));
        assert_eq!(marks(&st), (true, true, false), "marked as a save on its way");
        let ran = files(decide(&mut st, true));
        assert!(matches!(&ran[..], [(FilesEffect::LocalOp { op: LocalOp::ResolveConflict { .. }, .. }, Some(_))]), "{ran:?}");
        assert_eq!(marks(&st), (true, true, false), "still, while it runs");
        done(&mut st);
        let c = view(&st);
        assert!(!c.saving && c.step_on_resolve, "it steps on once the status shows the file resolved");
    }
}

#[test]
fn a_save_with_markers_that_waited_says_it_saved_and_keeps_what_it_wrote() {
    let mut st = editing(held("fth-partial"), false);
    assert!(files(send(&mut st, Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers))).is_empty(), "it waits");
    assert_eq!(marks(&st), (true, false, true));
    let ran = files(decide(&mut st, true));
    assert!(matches!(&ran[..], [(FilesEffect::LocalOp { op: LocalOp::SavePartialResolution { .. }, .. }, Some(_))]), "{ran:?}");
    done(&mut st);
    let c = view(&st);
    assert_eq!(marks(&st), (false, false, false));
    assert_eq!(c.baseline.0, c.picks, "what it wrote is what the file holds now");
    assert!(st.toasts.iter().any(|t| t.text.starts_with("Saved a.txt")), "it says so");
}

#[test]
fn taking_a_side_that_waited_steps_on_when_it_ran() {
    let mut st = editing(refused("fth-side"), false);
    let take = Msg::Conflict(ConflictMsg::ResolveWithSide { path: "a.txt".into(), side: Side::Theirs });
    assert!(files(send(&mut st, take)).is_empty(), "it waits");
    assert_eq!(marks(&st), (true, true, false));
    let ran = files(decide(&mut st, true));
    assert!(
        matches!(&ran[..], [(FilesEffect::LocalOp { op: LocalOp::ResolveWithSide { side: Side::Theirs, .. }, .. }, Some(_))]),
        "{ran:?}"
    );
    done(&mut st);
    assert!(!view(&st).saving && view(&st).step_on_resolve);
}

#[test]
fn a_waiting_conflict_change_that_is_dropped_leaves_the_view_ready_to_save_again() {
    let saves: [(bool, fn() -> Msg); 4] = [
        (true, || Msg::Conflict(ConflictMsg::SaveConflict)),
        (false, || Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers)),
        (false, || Msg::Conflict(ConflictMsg::ResolveWithSide { path: "a.txt".into(), side: Side::Ours })),
        (false, || Msg::Conflict(ConflictMsg::MarkResolvedConfirmed(vec!["a.txt".into()]))),
    ];
    for (i, (all, save)) in saves.into_iter().enumerate() {
        // Don't trust.
        let mut st = editing(held(&format!("fth-drop-{i}")), all);
        send(&mut st, save());
        assert!(st.trust.waiting.is_some());
        assert!(files(decide(&mut st, false)).is_empty());
        assert_eq!(marks(&st), (false, false, false), "{i}: nothing is on its way");
        // The prompt closed.
        send(&mut st, save());
        assert!(st.trust.waiting.is_some());
        send(&mut st, Msg::CloseModal);
        assert_eq!(marks(&st), (false, false, false), "{i}: nor after Cancel");
        assert_eq!(view(&st).can_save(), all);
    }
}

fn discard() -> Msg {
    Msg::Stage(StageMsg::DiscardFile("a.txt".into()))
}

#[test]
fn waiting_work_is_dropped_when_another_dialog_takes_the_prompts_place() {
    let mut st = held("fth-replaced");
    send(&mut st, discard());
    assert_eq!(waits(&st).as_deref(), Some("Discard"));
    // A result handler opens its own dialog over the prompt.
    st.modal = Some(Modal::danger("Title", "Body", "Go", Msg::ClearLog));
    send(&mut st, Msg::ClearLog);
    assert!(st.trust.waiting.is_none(), "the prompt that named it is gone");
    // The prompt brought back from the chip names nothing, and Trust on it runs nothing.
    st.modal = None;
    let real = real(&st);
    send(&mut st, Msg::Trust(TrustMsg::Ask { real }));
    assert_eq!(waits(&st), None);
    assert!(files(decide(&mut st, true)).is_empty());
}

#[test]
fn a_prompt_runs_only_the_work_it_names() {
    // A prompt that is already open names what waits for it: file work asked for then doesn't.
    let mut st = held("fth-open-prompt");
    let at = real(&st);
    send(&mut st, Msg::Trust(TrustMsg::Ask { real: at }));
    assert!(matches!(st.modal, Some(Modal::Trust(_))));
    assert!(files(send(&mut st, stage_file())).is_empty());
    assert_eq!(waits(&st), None, "the open prompt's text is unchanged");
    assert!(st.trust.waiting.is_none());
    assert!(files(decide(&mut st, true)).is_empty(), "Trust stages nothing");

    // Waiting work whose prompt came to name something else is dropped.
    let mut st = held("fth-other-prompt");
    send(&mut st, discard());
    if let Some(Modal::Trust(p)) = &mut st.modal {
        p.held = Some("Pull".into());
    }
    assert!(files(decide(&mut st, true)).is_empty(), "this prompt didn't name the discard");
    assert!(st.trust.waiting.is_none());
}

#[test]
fn a_staged_rename_that_waits_is_not_logged_as_failed() {
    let mut st = held("fth-rename-log");
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", 'M')]);
    let rename = Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "b.txt".into(), stage: true });
    assert!(files(send(&mut st, rename)).is_empty());
    assert_eq!(waits(&st).as_deref(), Some("Rename"));
    assert!(!st.log.iter().any(|l| l.text.contains("Could not rename")), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert_eq!(st.log.iter().filter(|l| l.text.contains("Rename")).count(), 1, "one line says why it waits");
}
