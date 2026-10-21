//! Headless tests of the panel's selection, exec and break steps and combined messages.

use super::rebase_tests::with_panel;
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::{Editing, RebasePanel};
use crate::state::rebase_steps::NewStep;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::rebase::{self, RebaseAction, RebaseBase, TodoStep};
use std::time::Instant;

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn rb(st: &mut AppState, msg: RebaseMsg) -> Vec<Effect> {
    up(st, Msg::Rebase(msg))
}

fn panel(st: &AppState) -> &RebasePanel {
    st.rebase.as_ref().expect("panel open")
}

#[test]
fn selecting_a_commit_row_shows_the_commit() {
    let (mut st, dir, c) = with_panel("steps-select", RebaseBase::FromCommit(Oid::zero()));
    let fx = rb(&mut st, RebaseMsg::RebaseSelect(1));
    assert_eq!(panel(&st).selected, Some(1));
    assert_eq!(st.selection, Selection::Commit(c[1]));
    assert!(
        matches!(&fx[..], [Effect::LoadDetail { oid, .. }, Effect::LineCounts(crate::effect::LineCountsEffect::Commit { .. })] if *oid == c[1]),
        "{fx:?}"
    );
    assert!(st.rebase.is_some(), "the panel stays open");
    rb(&mut st, RebaseMsg::RebaseAddStep { after: Some(1), step: NewStep::Break });
    let fx = rb(&mut st, RebaseMsg::RebaseSelect(2));
    assert!(fx.is_empty(), "a step has no commit to show");
    assert_eq!(panel(&st).selected, Some(2));
    assert!(rb(&mut st, RebaseMsg::RebaseSelect(9)).is_empty());
    assert_eq!(panel(&st).selected, Some(2), "out of range");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_exec_step_runs_with_the_list() {
    let (mut st, dir, _) = with_panel("steps-exec", RebaseBase::FromCommit(Oid::zero()));
    rb(&mut st, RebaseMsg::RebaseAddStep { after: Some(0), step: NewStep::Exec });
    assert_eq!(panel(&st).editing, Some(Editing::Command(1)));
    // An empty command blocks Start, and says why in the log.
    assert!(rb(&mut st, RebaseMsg::StartRebase { confirmed: false, autostash: false }).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(rebase::EMPTY_EXEC));
    st.rebase.as_mut().unwrap().draft = "git log -1 --format=%s > seen.txt".into();
    rb(&mut st, RebaseMsg::RebaseSaveMessage);
    let fx = rb(&mut st, RebaseMsg::StartRebase { confirmed: false, autostash: false });
    let [Effect::Rebase(RebaseEffect::Start { request, detail, .. })] = &fx[..] else { panic!("expected the rebase, got {fx:?}") };
    assert_eq!(request.items[1], TodoStep::Exec("git log -1 --format=%s > seen.txt".into()));
    assert_eq!(detail, "interactive rebase of main");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_group_heads_combined_message() {
    let (mut st, dir, _) = with_panel("steps-combined", RebaseBase::FromCommit(Oid::zero()));
    rb(&mut st, RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Squash });
    rb(&mut st, RebaseMsg::RebaseEditMessage(0));
    let p = panel(&st);
    assert_eq!(p.editing, Some(Editing::Combined(0)));
    assert_eq!(p.draft, rebase::default_combined(&p.items, 0));
    assert!(p.draft.starts_with("c1") && p.draft.contains("c2"), "{}", p.draft);
    st.rebase.as_mut().unwrap().draft = "Both at once".into();
    rb(&mut st, RebaseMsg::RebaseSaveMessage);
    assert_eq!(panel(&st).items[0].commit().unwrap().combined_message.as_deref(), Some("Both at once"));
    rb(&mut st, RebaseMsg::RebaseEditMessage(0));
    assert_eq!(panel(&st).draft, "Both at once", "the typed message comes back");
    rb(&mut st, RebaseMsg::RebaseResetCombined);
    assert_eq!(panel(&st).items[0].commit().unwrap().combined_message, None);
    assert_eq!(panel(&st).editing, None);
    let _ = std::fs::remove_dir_all(&dir);
}
