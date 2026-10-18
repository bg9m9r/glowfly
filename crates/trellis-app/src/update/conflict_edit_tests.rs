//! Headless tests for picking single lines, editing the output by hand and Save with markers.

use crate::effect::{Effect, LocalOp};
use crate::msg::{ConflictMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::update::conflict_tests::{conflicted_repo, with_fake};
use crate::update::tests::{drive, git, scratch_repo, state};
use crate::update::update;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::conflict_pick::{ChunkPick, LineRef};
use trellis_core::conflict_rows::Trivial;
use trellis_core::repo::{Choice, ConflictContent, Segment, Side, marker_regions};

fn send(st: &mut AppState, m: ConflictMsg) -> Vec<Effect> {
    update(st, Msg::Conflict(m), Instant::now())
}

fn view(st: &AppState) -> &crate::state::conflict::ConflictView {
    st.conflict.as_ref().expect("view open")
}

/// `with_fake(n)` with chunk `k` holding two lines per side ("ours k", "ours k more", ...).
fn with_two_lines(n: usize, k: usize) -> AppState {
    let mut st = with_fake(n);
    let c = st.conflict.as_mut().unwrap();
    if let Some(f) = c.file.as_mut()
        && let ConflictContent::Text(segs) = &mut f.content
        && let Segment::Conflict(chunk) = &mut segs[1 + 2 * k]
    {
        chunk.ours.push(format!("ours {k} more\n"));
        chunk.theirs.push(format!("theirs {k} more\n"));
    }
    st
}

/// Replaces chunk `k`'s text in the open editor with `text` and reports the edit.
fn type_in_chunk(st: &mut AppState, k: usize, text: &str) {
    let e = st.conflict.as_mut().unwrap().edit.as_mut().expect("editing");
    let r = e.rendered.chunks[k].clone();
    e.buffer.replace_range(r, text);
    send(st, ConflictMsg::ConflictOutputEdited);
}

#[test]
fn line_toggles_build_the_output_in_tick_order() {
    let mut st = with_two_lines(3, 1);
    st.conflict.as_mut().unwrap().auto[1] = Some(Trivial::Whitespace);
    send(&mut st, ConflictMsg::ConflictToggleLine { chunk: 1, side: Side::Theirs, line: 1 });
    send(&mut st, ConflictMsg::ConflictToggleLine { chunk: 1, side: Side::Ours, line: 0 });
    let c = view(&st);
    let want = ChunkPick::Lines(vec![LineRef { side: Side::Theirs, line: 1 }, LineRef { side: Side::Ours, line: 0 }]);
    assert_eq!((&c.picks[1], c.auto[1], c.current), (&want, None, 1), "the automatic tag goes");
    assert_eq!(c.output(), None, "the other chunks are still open");
    for k in [0, 2] {
        send(&mut st, ConflictMsg::ConflictToggle { chunk: k, side: Side::Ours });
    }
    let out = view(&st).output().expect("every chunk picked");
    assert_eq!(out, "top\nours 0\nbetween\ntheirs 1 more\nours 1\nbetween\nours 2\nbetween\n");
    // The header's Take finishes a partly taken side, then leaves it out.
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 1, side: Side::Theirs });
    let line = |side, line| LineRef { side, line };
    let want = ChunkPick::Lines(vec![line(Side::Ours, 0), line(Side::Theirs, 0), line(Side::Theirs, 1)]);
    assert_eq!(view(&st).picks[1], want);
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 1, side: Side::Theirs });
    assert_eq!(view(&st).picks[1], ChunkPick::Lines(vec![LineRef { side: Side::Ours, line: 0 }]));
    // Unticking the last line leaves the chunk open again.
    send(&mut st, ConflictMsg::ConflictToggleLine { chunk: 1, side: Side::Ours, line: 0 });
    assert_eq!((view(&st).picks[1].clone(), view(&st).unresolved()), (ChunkPick::default(), 1));
}

#[test]
fn edit_inside_a_chunk_makes_it_edited() {
    let mut st = with_fake(2);
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    let e = view(&st).edit.as_ref().expect("editing");
    assert_eq!(e.buffer.matches("<<<<<<< main\n").count(), 2, "open chunks show as markers");
    type_in_chunk(&mut st, 1, "mine\n");
    let edited = ChunkPick::Edited { lines: vec!["mine\n".into()], before: Box::default() };
    assert_eq!((&view(&st).picks[1], view(&st).current), (&edited, 1));
    assert_eq!(view(&st).edit.as_ref().unwrap().buffer.matches("<<<<<<<").count(), 1, "rebuilt with the typed text");
    for m in [
        ConflictMsg::ConflictTakeAll(Side::Ours),
        ConflictMsg::ConflictReset,
        ConflictMsg::ConflictToggle { chunk: 1, side: Side::Ours },
        ConflictMsg::ConflictToggleLine { chunk: 1, side: Side::Theirs, line: 0 },
    ] {
        send(&mut st, m);
        assert_eq!(view(&st).picks[1], edited, "hand edits stay");
    }
    send(&mut st, ConflictMsg::ConflictTakeAll(Side::Ours));
    assert!(view(&st).can_save());
    assert_eq!(view(&st).output().unwrap(), "top\nours 0\nbetween\nmine\nbetween\n");
    send(&mut st, ConflictMsg::ConflictDiscardEdit(1));
    assert_eq!(view(&st).picks[1], ChunkPick::default(), "the pick before the edit");
    assert!(view(&st).edit.as_ref().unwrap().buffer.contains("ours 1\n=======\ntheirs 1\n"), "the editor is rebuilt");
}

#[test]
fn edit_outside_the_chunks_is_undone() {
    let mut st = with_fake(2);
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    let picks = view(&st).picks.clone();
    st.conflict.as_mut().unwrap().edit.as_mut().unwrap().buffer.insert(1, 'X');
    send(&mut st, ConflictMsg::ConflictOutputEdited);
    let c = view(&st);
    let e = c.edit.as_ref().unwrap();
    assert_eq!(e.buffer, e.rendered.text, "the change is undone");
    assert!(c.edit_note.is_some() && c.picks == picks);
    type_in_chunk(&mut st, 0, "fine\n");
    assert!(view(&st).edit_note.is_none(), "an accepted edit clears the note");
    st.conflict.as_mut().unwrap().edit_note = Some("x".into());
    send(&mut st, ConflictMsg::ConflictEditOutput(false));
    assert!(view(&st).edit.is_none() && view(&st).edit_note.is_none(), "so does leaving the editor");
}

#[test]
fn picks_rebuild_the_editor() {
    let mut st = with_two_lines(2, 0);
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    type_in_chunk(&mut st, 1, "kept\n");
    send(&mut st, ConflictMsg::ConflictToggleLine { chunk: 0, side: Side::Theirs, line: 1 });
    let buffer = &view(&st).edit.as_ref().unwrap().buffer;
    assert_eq!(buffer, "top\ntheirs 0 more\nbetween\nkept\nbetween\n");
}

#[test]
fn an_edited_chunk_with_markers_is_still_open() {
    let mut st = with_fake(2);
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours });
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    let e = st.conflict.as_mut().unwrap().edit.as_mut().unwrap();
    let at = e.buffer.find("ours 1").unwrap();
    e.buffer.insert_str(at, "my ");
    send(&mut st, ConflictMsg::ConflictOutputEdited);
    let c = view(&st);
    assert!(c.picks[1].is_edited() && c.picks[1].is_open());
    assert_eq!((c.unresolved(), c.can_save(), c.can_save_partial()), (1, false, true));
    assert!(c.partial_output().unwrap().contains("<<<<<<< main\nmy ours 1\n=======\n"), "written as typed");
}

/// A repository mid-merge where f.txt conflicts in two chunks (lines 2 and 10), opened in the
/// conflict view.
fn two_chunks(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    let base: String = (1..=12).map(|i| format!("line {i}\n")).collect();
    let side = |s: &str| base.replace("line 2\n", &format!("line 2 {s}\n")).replace("line 10\n", &format!("line 10 {s}\n"));
    std::fs::write(dir.join("f.txt"), &base).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("f.txt"), side("theirs")).unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("f.txt"), side("ours")).unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = trellis_core::gitcli::run(&dir, &trellis_core::gitcli::args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("f.txt".into())));
    assert_eq!(view(&st).chunk_count(), 2);
    (st, dir)
}

fn conflicted(st: &AppState, path: &str) -> bool {
    st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!' && f.path == path))
}

#[test]
fn save_with_markers_writes_and_stays_conflicted() {
    let (mut st, dir) = two_chunks("conflict-keep-markers");
    assert!(send(&mut st, ConflictMsg::SaveConflictKeepMarkers).is_empty(), "nothing picked yet");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours });
    let fx = send(&mut st, ConflictMsg::SaveConflictKeepMarkers);
    match &fx[..] {
        [Effect::LocalOp { op: LocalOp::SavePartialResolution { path, contents, .. }, .. }] => {
            assert_eq!(path, "f.txt");
            assert_eq!(marker_regions(contents.as_bytes()), 1);
            assert!(contents.contains("line 2 ours\n") && contents.contains("<<<<<<< main\nline 10 ours\n"));
        }
        other => panic!("expected one partial save, got {other:?}"),
    }
    crate::update::tests::finish(&mut st, fx);
    let text = std::fs::read_to_string(dir.join("f.txt")).unwrap();
    assert_eq!(marker_regions(text.as_bytes()), 1);
    assert!(!text.contains("line 2 theirs"));
    assert!(conflicted(&st, "f.txt"), "still conflicted");
    let c = view(&st);
    assert!(!c.saving && c.picks[0] == ChunkPick::Sides(Choice::Ours), "the view stays with its picks");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Saved f.txt with 1 conflict left (still conflicted)"));
    // Picking the rest saves and stages as before.
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 1, side: Side::Theirs });
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    assert!(!conflicted(&st, "f.txt"));
    assert!(std::fs::read_to_string(dir.join("f.txt")).unwrap().contains("line 2 ours\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_with_an_edited_chunk_stages_the_typed_text() {
    let dir = conflicted_repo("conflict-save-typed");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    type_in_chunk(&mut st, 0, "typed\n");
    match &send(&mut st, ConflictMsg::SaveConflict)[..] {
        [Effect::LocalOp { op: LocalOp::ResolveConflict { contents, .. }, .. }] => assert_eq!(contents, "one\ntyped\n"),
        other => panic!("expected one resolve, got {other:?}"),
    }
    assert!(send(&mut st, ConflictMsg::SaveConflictKeepMarkers).is_empty(), "nothing is left for markers");
    let _ = std::fs::remove_dir_all(&dir);
}
