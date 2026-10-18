//! Rebases in the undo history: a finished one is recorded as one action, a stopped one keeps
//! the history, one that can't be recorded clears it, and graph edits name what they did.

use super::rebase_tests::{repo, running_job};
use super::update;
use crate::effect::{Effect, RebaseEffect, UndoResult};
use crate::msg::{Msg, RebaseMsg};
use crate::state::AppState;
use git2::Oid;
use std::time::Instant;
use trellis_core::rebase::{Outcome, QuickEdit, RebaseProgress, Stop, StopKind};
use trellis_core::undo::{Action, MoveOp};

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn finished(st: &mut AppState, result: Result<Outcome, String>, undo: Option<UndoResult>) {
    up(st, Msg::Rebase(RebaseMsg::RebaseFinished { result, undo }));
}

fn undo_label(st: &AppState) -> String {
    st.undo.undo_entry(st.settings.lang(), true).0
}

#[test]
fn a_finished_rebase_is_one_undoable_action() {
    let (mut st, dir, c) = repo("rebase-undo-record");
    st.job = Some(running_job(&dir));
    let moved = Action::Moved {
        op: MoveOp::Rewrite,
        detail: Some("interactive rebase of main".into()),
        branch: Some("main".into()),
        from: c[2],
        to: c[1],
    };
    let done = Ok(Outcome::Finished { output: String::new(), stash_kept: false });
    finished(&mut st, done, Some(UndoResult::Recorded(Some(moved.clone()))));
    assert_eq!(st.undo.undo.last(), Some(&moved));
    assert_eq!(undo_label(&st), "Undo interactive rebase of main");

    // A stop keeps what is there.
    st.job = Some(running_job(&dir));
    let progress = RebaseProgress {
        step: 1,
        total: 3,
        stop: Stop { kind: StopKind::Edit, oid: Some(c[0]) },
        summary: String::new(),
        autostash: false,
        interactive: true,
        command: None,
    };
    finished(&mut st, Ok(Outcome::Stopped { ok: true, output: String::new(), progress }), Some(UndoResult::Recorded(None)));
    assert_eq!(st.undo.undo.last(), Some(&moved), "the earlier action stays");

    // A finished rebase that couldn't be recorded clears the history.
    st.job = Some(running_job(&dir));
    finished(&mut st, Ok(Outcome::Finished { output: String::new(), stash_kept: false }), None);
    assert!(st.undo.undo.is_empty());
    assert!(st.undo.blocked.is_some(), "the label says why nothing can be undone");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn graph_edits_name_themselves_for_undo() {
    let (mut st, dir, c) = repo("rebase-undo-detail");
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::Edit(QuickEdit::SquashIntoParent(c[2]))));
    let [Effect::Rebase(RebaseEffect::PrepareEdit { workdir, edit })] = &fx[..] else { panic!("{fx:?}") };
    let result = trellis_core::rebase::prepare_edit(workdir, edit).map_err(|e| e.to_string());
    up(&mut st, Msg::Rebase(RebaseMsg::EditPrepared { workdir: workdir.clone(), result }));
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword));
    let [Effect::Rebase(RebaseEffect::Start { detail, .. })] = &fx[..] else { panic!("{fx:?}") };
    assert_eq!(detail, &format!("squash of {} into its parent", &c[2].to_string()[..7]));

    st.job = None;
    let edit = QuickEdit::Reword(c[1]);
    let result = trellis_core::rebase::prepare_edit(&dir, &edit).map_err(|e| e.to_string());
    up(&mut st, Msg::Rebase(RebaseMsg::EditPrepared { workdir: dir.clone(), result }));
    let Some(crate::state::Modal::RewordCommit(form)) = st.modal.as_mut() else { panic!("the Reword dialog") };
    form.draft = "c2, reworded".into();
    let fx = up(&mut st, Msg::Rebase(RebaseMsg::SubmitReword));
    let [Effect::Rebase(RebaseEffect::Start { detail, .. })] = &fx[..] else { panic!("{fx:?}") };
    assert_eq!(detail, &format!("reword of {}", &c[1].to_string()[..7]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_details() {
    use crate::state::rebase_quick::undo_detail;
    let o = |n: u8| Oid::from_bytes(&[n; 20]).unwrap();
    assert_eq!(undo_detail(&QuickEdit::Reword(o(0x1a))), "reword of 1a1a1a1");
    assert_eq!(undo_detail(&QuickEdit::SquashIntoParent(o(0x1a))), "squash of 1a1a1a1 into its parent");
    assert_eq!(undo_detail(&QuickEdit::Squash(vec![o(1), o(2), o(3)])), "squash of 3 commits");
    assert_eq!(undo_detail(&QuickEdit::Drop(vec![o(0x2b)])), "drop of 2b2b2b2");
    assert_eq!(undo_detail(&QuickEdit::Drop(vec![o(1), o(2)])), "drop of 2 commits");
}
