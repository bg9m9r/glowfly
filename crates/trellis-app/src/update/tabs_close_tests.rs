//! Headless tests for closing tabs: a close that would lose a commit message draft or conflict
//! choices asks first (and the answer closes the right tab even if tabs moved meanwhile), others
//! close at once; and reopening a chosen closed tab.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{CloseRequest, Msg, TabsMsg};
use crate::state::conflict::ConflictView;
use crate::state::tabs::{ClosedTab, Parked};
use crate::state::{AppState, CommitDraft, Modal, Tab};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::conflict_pick::ChunkPick;
use trellis_core::conflict_rows::Trivial;
use trellis_core::repo::{Choice, CommitTemplate};

pub(super) fn fake(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join("trellis-close-tabs").join(name)
}

/// Unloaded tabs for made-up folders, tab `active` active.
pub(super) fn fake_tabs(names: &[&str], active: usize) -> AppState {
    let mut st = state();
    st.tabs = names.iter().map(|n| Tab::unloaded(fake(n))).collect();
    st.active_tab = active;
    st
}

pub(super) fn send(st: &mut AppState, msg: TabsMsg) -> Vec<Effect> {
    update(st, Msg::Tabs(msg), Instant::now())
}

pub(super) fn names(st: &AppState) -> Vec<String> {
    st.tabs.iter().map(|t| t.name.clone()).collect()
}

pub(super) fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveTabs { .. })).count()
}

fn draft(summary: &str) -> CommitDraft {
    CommitDraft { summary: summary.into(), ..Default::default() }
}

/// Parks a draft with tab `i`, as switching away from it would.
fn park_draft(st: &mut AppState, i: usize, summary: &str) {
    st.tabs[i].parked = Some(Box::new(Parked { draft: draft(summary), ..Default::default() }));
}

/// The open confirmation's title and body.
fn confirm(st: &AppState) -> Option<(&str, &str)> {
    match &st.modal {
        Some(Modal::Confirm { title, body, .. }) => Some((title, body)),
        _ => None,
    }
}

fn accept(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::ConfirmAccepted, Instant::now())
}

#[test]
fn closing_a_tab_with_a_draft_asks_first() {
    let mut st = fake_tabs(&["a", "b"], 1);
    st.draft.summary = "wip".into();
    let fx = send(&mut st, TabsMsg::CloseTab(1));
    assert_eq!(st.tabs.len(), 2, "nothing closes yet");
    let (title, body) = confirm(&st).expect("a confirmation");
    assert_eq!(title, "Close tab?");
    assert!(body.starts_with("b holds work that isn't saved anywhere:"), "{body}");
    assert!(body.contains("- its commit message draft"), "{body}");
    assert_eq!(saves(&fx), 0);
    assert!(matches!(&st.modal, Some(Modal::Confirm { button, .. }) if button == "Close tab"));

    let fx = accept(&mut st);
    assert_eq!(names(&st), ["a"]);
    assert!(st.modal.is_none());
    assert_eq!(st.closed_tabs.last().map(|c| c.path.clone()), Some(fake("b")));
    assert!(saves(&fx) > 0);
}

#[test]
fn ctrl_w_and_bulk_closes_ask_too() {
    let mut st = fake_tabs(&["a", "b", "c"], 0);
    st.draft.summary = "wip".into();
    send(&mut st, TabsMsg::CloseTab(usize::MAX));
    assert!(confirm(&st).is_some_and(|(t, b)| t == "Close tab?" && b.starts_with("a holds")), "Ctrl+W asks");
    st.modal = None;
    st.draft = CommitDraft::default();

    park_draft(&mut st, 1, "fix: b");
    park_draft(&mut st, 2, "fix: c");
    for msg in [TabsMsg::CloseOtherTabs(0), TabsMsg::CloseTabsToRight(0)] {
        st.modal = None;
        let fx = send(&mut st, msg);
        let (title, body) = confirm(&st).expect("a confirmation");
        assert_eq!(title, "Close 2 tabs?");
        assert!(body.starts_with("Some of these tabs hold work that isn't saved anywhere:"), "{body}");
        assert!(body.contains("- b: its commit message draft") && body.contains("- c: its commit message draft"), "{body}");
        assert!(matches!(&st.modal, Some(Modal::Confirm { button, .. }) if button == "Close 2 tabs"));
        assert_eq!((st.tabs.len(), saves(&fx)), (3, 0));
    }
    accept(&mut st);
    assert_eq!(names(&st), ["a"], "accepting closes them without asking again");
    assert!(st.modal.is_none());
}

#[test]
fn blank_and_template_drafts_close_at_once() {
    let mut st = fake_tabs(&["a", "b", "c", "d"], 3);
    st.draft.summary = "   ".into();
    let fx = send(&mut st, TabsMsg::CloseTab(3));
    assert!(st.modal.is_none() && saves(&fx) > 0, "a whitespace-only summary is nothing to lose");
    assert_eq!(names(&st), ["a", "b", "c"]);

    let template = CommitTemplate { summary: "feat:".into(), description: "# why".into(), comment_char: '#' };
    let mut untouched = CommitDraft::default();
    untouched.apply_template(&template);
    st.tabs[1].parked = Some(Box::new(Parked { draft: untouched, ..Default::default() }));
    send(&mut st, TabsMsg::CloseTab(1));
    assert!(st.modal.is_none(), "an untouched template is nothing to lose");
    assert_eq!(names(&st), ["a", "c"]);

    // A restored tab never loaded has nothing parked.
    assert!(st.tabs[0].parked.is_none());
    send(&mut st, TabsMsg::CloseTab(0));
    assert!(st.modal.is_none());
    assert_eq!(names(&st), ["c"]);
}

fn conflict(picks: Vec<ChunkPick>, auto: Vec<Option<Trivial>>) -> ConflictView {
    let mut v = ConflictView::loading("src/lib.rs".into());
    v.picks = picks;
    v.auto = auto;
    v
}

#[test]
fn user_conflict_choices_ask_but_automatic_ones_do_not() {
    let mut st = fake_tabs(&["a", "b"], 1);
    st.conflict = Some(conflict(vec![ChunkPick::Sides(Choice::Ours), ChunkPick::default()], vec![None, None]));
    send(&mut st, TabsMsg::CloseTab(1));
    let (_, body) = confirm(&st).expect("a confirmation");
    assert!(body.contains("- 1 conflict choice in src/lib.rs"), "{body}");
    assert!(!body.contains("draft"), "{body}");
    st.modal = None;

    st.conflict = Some(conflict(vec![ChunkPick::Sides(Choice::Ours), ChunkPick::default()], vec![Some(Trivial::Unchanged), None]));
    send(&mut st, TabsMsg::CloseTab(1));
    assert!(st.modal.is_none(), "an automatic pick is made again when the file opens again");
    assert_eq!(names(&st), ["a"]);

    // Two choices in a parked tab, next to a draft.
    let mut st = fake_tabs(&["a", "b"], 0);
    let picks = vec![ChunkPick::Sides(Choice::Theirs), ChunkPick::Sides(Choice::Ours)];
    let parked = Parked { draft: draft("wip"), conflict: Some(conflict(picks, vec![None, None])), ..Default::default() };
    st.tabs[1].parked = Some(Box::new(parked));
    send(&mut st, TabsMsg::CloseTab(1));
    let (_, body) = confirm(&st).expect("a confirmation");
    assert!(body.contains("- its commit message draft\n- 2 conflict choices in src/lib.rs"), "{body}");
}

#[test]
fn a_clean_edit_alone_asks_but_restored_picks_do_not() {
    let mut st = fake_tabs(&["a", "b"], 1);
    let mut c = conflict(vec![ChunkPick::default()], vec![None]);
    c.cleans.insert(0, vec!["edited\n".into()]);
    st.conflict = Some(c);
    send(&mut st, TabsMsg::CloseTab(1));
    let (_, body) = confirm(&st).expect("a confirmation");
    assert!(body.contains("- 1 conflict choice in src/lib.rs"), "{body}");
    st.modal = None;
    // What the file holds already (brought back when it opened) is nothing to lose.
    let mut c = conflict(vec![ChunkPick::Sides(Choice::Ours)], vec![None]);
    c.cleans.insert(0, vec!["edited\n".into()]);
    c.baseline = (c.picks.clone(), c.cleans.clone());
    st.conflict = Some(c);
    send(&mut st, TabsMsg::CloseTab(1));
    assert!(st.modal.is_none());
}

#[test]
fn cancel_keeps_everything() {
    let mut st = fake_tabs(&["a", "b"], 1);
    st.draft.summary = "wip".into();
    send(&mut st, TabsMsg::CloseTab(1));
    assert!(st.modal.is_some());
    let fx = update(&mut st, Msg::CloseModal, Instant::now());
    assert!(st.modal.is_none() && saves(&fx) == 0);
    assert_eq!((names(&st), st.active_tab), (vec!["a".to_owned(), "b".into()], 1));
    assert_eq!(st.draft.summary, "wip");
    assert!(st.closed_tabs.is_empty());
}

#[test]
fn confirmation_follows_its_tab() {
    // b moves to the front before the answer.
    let mut st = fake_tabs(&["a", "b", "c"], 0);
    park_draft(&mut st, 1, "wip");
    send(&mut st, TabsMsg::CloseTab(1));
    send(&mut st, TabsMsg::MoveTab { from: 0, to: 1 });
    assert_eq!(names(&st), ["b", "a", "c"]);
    accept(&mut st);
    assert_eq!(names(&st), ["a", "c"], "b closes at its new index");

    // A clean tab left of b closes before the answer.
    let mut st = fake_tabs(&["a", "b", "c"], 2);
    park_draft(&mut st, 1, "wip");
    send(&mut st, TabsMsg::CloseTab(1));
    let modal = st.modal.take();
    send(&mut st, TabsMsg::CloseTab(0));
    st.modal = modal;
    accept(&mut st);
    assert_eq!(names(&st), ["c"]);

    // b is gone by the answer: nothing else closes.
    let mut st = fake_tabs(&["a", "b", "c"], 0);
    park_draft(&mut st, 1, "wip");
    send(&mut st, TabsMsg::CloseTab(1));
    let b = st.tabs[1].id;
    let modal = st.modal.take();
    send(&mut st, TabsMsg::CloseConfirmed(CloseRequest::One(b)));
    assert_eq!(names(&st), ["a", "c"]);
    st.modal = modal;
    let fx = accept(&mut st);
    assert_eq!(names(&st), ["a", "c"]);
    assert!(fx.is_empty() && st.modal.is_none());
}

#[test]
fn reopen_a_chosen_closed_tab() {
    let mut st = fake_tabs(&["x", "a", "b", "c"], 0);
    for _ in 0..3 {
        send(&mut st, TabsMsg::CloseTab(1));
    }
    let closed = |st: &AppState| st.closed_tabs.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
    assert_eq!(closed(&st), [fake("a"), fake("b"), fake("c")]);
    send(&mut st, TabsMsg::ReopenClosed(fake("b")));
    assert_eq!(closed(&st), [fake("a"), fake("c")], "the others keep their order");
    assert!(st.tabs[st.active_tab].is(&fake("b")));
    assert_eq!(st.tabs.len(), 2);

    // An entry for a tab that is open again just selects it.
    st.closed_tabs.push(ClosedTab::from(fake("x")));
    send(&mut st, TabsMsg::ReopenClosed(fake("x")));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 0));
    assert_eq!(closed(&st), [fake("a"), fake("c")]);
}
