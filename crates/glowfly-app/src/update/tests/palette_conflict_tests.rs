//! Headless tests for the palette's conflict entries: when they are listed, what running each
//! does, and how they are remembered.

use super::super::tests::conflict_files_tests::both_resolved;
use super::super::tests::conflict_tests::two_conflicts;
use super::super::tests::{drive, scratch_repo, state};
use super::super::update;
use crate::effect::{ConflictRead, Effect};
use crate::msg::{ConflictMsg, Msg, PaletteMsg, RepoMsg};
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::{AppState, Modal};
use std::time::Instant;

fn run_cmd(st: &mut AppState, cmd: C) -> Vec<Effect> {
    update(st, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)), Instant::now())
}

fn listed(st: &AppState) -> Vec<(String, Option<String>)> {
    entries(st).into_iter().filter(|e| e.label.starts_with("Mark ")).map(|e| (e.label, e.detail)).collect()
}

fn owned(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn conflict_entries_follow_the_lists() {
    let dir = scratch_repo("pc-clean");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(listed(&st).is_empty(), "nothing in a clean repository");
    let _ = std::fs::remove_dir_all(&dir);

    let dir = two_conflicts("pc-conflicted");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(listed(&st), [("Mark all resolved".to_owned(), Some("2 files".to_owned()))]);
    let fx = run_cmd(&mut st, C::MarkAllResolved);
    let want = owned(&["a.txt", "b.txt"]);
    assert!(
        matches!(&fx[..], [Effect::ReadConflictFiles { purpose: ConflictRead::BeforeMarkResolved, paths, .. }] if *paths == want),
        "the markers check comes first: {fx:?}"
    );
    assert_eq!(C::MarkAllResolved.recall_key(), Some("mark-all-resolved"));
    assert_eq!(st.palette_recent.position("mark-all-resolved"), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolved_files_can_be_marked_conflicted_from_the_palette() {
    let (mut st, dir) = both_resolved("pc-resolved");
    let all = listed(&st);
    assert_eq!(
        all,
        [
            ("Mark all conflicted".to_owned(), Some("2 files".to_owned())),
            ("Mark a.txt conflicted".to_owned(), None),
            ("Mark b.txt conflicted".to_owned(), None),
        ]
    );
    run_cmd(&mut st, C::MarkAllConflicted);
    let Some(Modal::Confirm { body, then, .. }) = st.modal.take() else { panic!("expected a confirm") };
    assert!(body.starts_with("Mark 2 files conflicted again?"), "{body}");
    assert!(matches!(*then, Msg::Conflict(ConflictMsg::MarkConflicted(ref p)) if *p == owned(&["a.txt", "b.txt"])));
    assert_eq!(st.palette_recent.position("mark-all-conflicted"), Some(0));
    run_cmd(&mut st, C::MarkConflicted("a.txt".into()));
    let Some(Modal::Confirm { body, then, .. }) = st.modal.take() else { panic!("expected a confirm") };
    assert!(body.starts_with("Mark a.txt conflicted again?"), "{body}");
    assert!(matches!(*then, Msg::Conflict(ConflictMsg::MarkConflicted(ref p)) if *p == owned(&["a.txt"])));
    assert_eq!(C::MarkConflicted("a.txt".into()).recall_key(), None, "one file isn't remembered");
    let _ = std::fs::remove_dir_all(&dir);
}
