//! The dialog before undoing a checkout that created a branch: the delete box starts ticked, and
//! Undo sends the choice with the action.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::pushed;
use crate::msg::{JobMsg, Msg};
use crate::state::undo::UndoCreatedForm;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;
use trellis_core::undo::{Action, Created, HeadPos};

/// The fixture with the dialog open for undoing the checkout that created feature/search.
fn dialog(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    let created = Some(Created { name: "feature/search".into(), oid, upstream: None });
    let action = Action::Checkout { from: HeadPos::Branch("main".into()), to: HeadPos::Branch("feature/search".into()), created };
    st.modal = Some(Modal::UndoCreated(UndoCreatedForm::new(action, "main".into())));
    (st, dir)
}

fn undo_sent(delete: bool) -> impl Fn(&Msg) -> bool {
    move |m| {
        matches!(m, Msg::Job(JobMsg::UndoWith { redo: false, action, opts })
            if opts.delete_created == delete && !opts.stash_first
                && matches!(&**action, Action::Checkout { created: Some(c), .. } if c.name == "feature/search"))
    }
}

#[test]
fn undo_created_branch() {
    let (st, dir) = dialog(ThemeChoice::Dark, "undo-created-branch");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Undo checkout of new branch feature/search").is_some());
    assert!(h.query_by_label("Switches back to main.").is_some());
    h.check_snapshot("undo_created_branch");
    // The dialog's button (the toolbar's is "Undo the last action").
    h.get_by_label("Undo").click();
    h.run();
    assert!(pushed(&h, &undo_sent(true)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn undo_created_branch_unticked() {
    let (st, dir) = dialog(ThemeChoice::Light, "undo-created-branch-light");
    let mut h = harness(st, false);
    h.run();
    h.get_by_label("Also delete branch feature/search").click();
    h.run();
    assert!(matches!(&h.state().0.modal, Some(Modal::UndoCreated(f)) if !f.delete));
    h.check_snapshot("undo_created_branch_light");
    // The dialog's button (the toolbar's is "Undo the last action").
    h.get_by_label("Undo").click();
    h.run();
    assert!(pushed(&h, &undo_sent(false)));
    drop(h);
    remove_fixture(&dir);
}
