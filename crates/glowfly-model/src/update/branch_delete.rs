//! Deleting a local branch from its menu: the Delete dialog opens at once and, unless the
//! checked-out branch contains the branch, counts the commits only it has in the background;
//! the count fills in the dialog that asked (a later dialog or a moved branch drops it); the
//! dialog's danger button deletes the branch as one git job, recorded for Undo, refused by the
//! runner if the branch has moved off the tip the dialog was opened for.

use super::branches::find;
use crate::effect::{BranchEffect, Effect};
use crate::state::branch_delete::{DeleteForm, Lost};
use crate::state::worktree_branches;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::gitcli::args;

/// Opens the Delete dialog for local branch `name`, unless it is checked out here or in another
/// worktree, and starts the count when it isn't known to lose nothing.
pub(super) fn open(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(b) = find(st, name) else { return };
    let Some(tip) = b.oid else { return };
    if let Some(why) = refused(st, name, b.is_head) {
        st.log(false, why);
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let form = DeleteForm::new(snap, &b, tip);
    if form.lost == Lost::Counting {
        fx.push(Effect::Branch(BranchEffect::CountUnmerged { workdir: snap.workdir.clone(), name: name.to_owned(), tip }));
    }
    st.modal = Some(Modal::DeleteBranch(form));
}

/// The count for the dialog still open for `name` at `tip`; anything else is stale.
pub(super) fn counted(st: &mut AppState, name: &str, tip: Oid, count: Result<usize, String>) {
    let Some(Modal::DeleteBranch(form)) = st.modal.as_mut() else { return };
    if form.name != name || form.tip != tip || form.lost != Lost::Counting {
        return;
    }
    form.lost = match count {
        Ok(n) => Lost::Known(n),
        Err(e) => Lost::Failed(e),
    };
}

/// Deletes local branch `name` (`-D` when `force`) as the single active job, closing its dialog.
/// `JobMsg::Git`'s path records it for Undo and the journal; the revision check refuses it if
/// the branch is no longer at `tip`.
pub(super) fn delete(st: &mut AppState, name: &str, tip: Oid, force: bool, fx: &mut Vec<Effect>) {
    if matches!(&st.modal, Some(Modal::DeleteBranch(f)) if f.name == name) {
        st.modal = None;
    }
    let Some(b) = find(st, name) else { return };
    if let Some(why) = refused(st, name, b.is_head) {
        st.log(false, why);
        return;
    }
    let cmd = args(&["branch", if force { "-D" } else { "-d" }, "--", name]);
    let expect = Some((format!("refs/heads/{name}"), tip));
    super::git_job::run(st, "Delete branch".into(), vec![cmd], None, expect, fx);
}

/// Why `name` can't be deleted: it is checked out here or in another worktree.
fn refused(st: &AppState, name: &str, is_head: bool) -> Option<String> {
    let t = st.settings.lang();
    if is_head {
        return Some(t.trf("branch.delete.checked_out", &[("branch", &name)]));
    }
    st.snap.as_ref().and_then(|s| worktree_branches::delete_blocked(st.settings.lang(), s, name))
}
