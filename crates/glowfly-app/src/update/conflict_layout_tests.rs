//! Headless tests for the conflict view's layout state: picks brought back from a file saved
//! with markers, folds opened and closed, and wrapping long lines.

use super::conflict_tests::{fake_file, with_fake};
use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{ConflictMsg, DiffMsg, Msg};
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use glowfly_core::conflict_pick::{ChunkPick, CleanEdits};
use glowfly_core::conflict_restore::SavedPicks;
use glowfly_core::repo::Choice;
use std::time::Instant;

fn send(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

fn loaded(saved: Option<SavedPicks>) -> Msg {
    Msg::Conflict(ConflictMsg::ConflictLoaded { path: "f.txt".into(), generation: 1, result: Ok(fake_file(2)), saved })
}

#[test]
fn saved_picks_come_back_when_the_file_loads() {
    let mut st = state();
    st.conflict = Some(ConflictView::loading("f.txt".into()));
    st.conflict_gen = 1;
    let cleans = CleanEdits::from([(0, vec!["TOP\n".to_owned()])]);
    send(&mut st, loaded(Some(SavedPicks { picks: vec![ChunkPick::Sides(Choice::Theirs), ChunkPick::default()], cleans: cleans.clone() })));
    let c = st.conflict.as_ref().unwrap();
    assert_eq!(c.picks, [ChunkPick::Sides(Choice::Theirs), ChunkPick::default()]);
    assert_eq!((&c.cleans, c.restored, c.current, c.user_choices()), (&cleans, 1, 1, 0));
    assert_eq!(st.view.conflict_scroll_to, Some(1));
    // A failed read brings nothing back: the view closes as before.
    let mut st = state();
    st.conflict = Some(ConflictView::loading("f.txt".into()));
    st.conflict_gen = 1;
    let failed = ConflictMsg::ConflictLoaded { path: "f.txt".into(), generation: 1, result: Err("gone".into()), saved: None };
    send(&mut st, Msg::Conflict(failed));
    assert!(st.conflict.is_none());
}

#[test]
fn folds_open_and_close_and_reset_on_load() {
    let mut st = with_fake(2);
    let fold = |seg, open| Msg::Conflict(ConflictMsg::ConflictFold { seg, open });
    send(&mut st, fold(2, true));
    send(&mut st, fold(4, true));
    assert_eq!(st.conflict.as_ref().unwrap().opened.iter().copied().collect::<Vec<_>>(), [2, 4]);
    send(&mut st, fold(2, false));
    assert_eq!(st.conflict.as_ref().unwrap().opened.iter().copied().collect::<Vec<_>>(), [4]);
    let c = st.conflict.as_mut().unwrap();
    c.set_file(fake_file(2));
    assert!(c.opened.is_empty(), "a new load folds everything again");
}

#[test]
fn wrapping_scrolls_the_current_conflict_into_view() {
    let mut st = with_fake(3);
    st.conflict.as_mut().unwrap().current = 2;
    st.view.conflict_scroll_to = None;
    let fx = send(&mut st, Msg::Diff(DiffMsg::SetWordWrap(true)));
    assert!(st.settings.diff_word_wrap);
    assert_eq!(st.view.conflict_scroll_to, Some(2));
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "the setting is saved");
}

#[test]
fn layout_intents_are_dropped_for_inactive_tabs() {
    for m in [ConflictMsg::ConflictFold { seg: 0, open: true }, ConflictMsg::ConflictDiscardCleanEdits] {
        assert!(Msg::Conflict(m).on_inactive().is_none());
    }
    let m = ConflictMsg::ConflictLoaded { path: "f.txt".into(), generation: 1, result: Ok(fake_file(1)), saved: None };
    assert!(Msg::Conflict(m).on_inactive().is_none());
}
