//! What the conflict keys do (`When::Conflict`): take ours, theirs or both (in either order)
//! for the current conflict, the one Next and Previous conflict move to, or clear its picks;
//! Mark resolved once it is enabled; and Continue the merge, rebase or cherry-pick, which runs
//! the conflict strip's Commit merge or Continue while it can, and the rebase's own Continue
//! otherwise. `ui/shortcuts/global.rs` sends what these return.

use crate::keymap::BindingId;
use crate::msg::{ConflictMsg, Msg, RebaseMsg};
use crate::state::AppState;
use crate::state::conflict_strip::conflict_strip;
use crate::state::tool_compare::merging;
use git2::RepositoryState;
use glowfly_core::rebase::Resume;
use glowfly_core::repo::Choice;

/// What the conflict key `id` sends now; None when it has nothing to do (no conflict view, or
/// Mark resolved still disabled). Other ids send nothing here.
pub fn conflict_key(st: &AppState, id: BindingId) -> Option<Msg> {
    use BindingId as I;
    let pick = |choice| Some(Msg::Conflict(ConflictMsg::ConflictPickCurrent(choice)));
    let view = st.session.conflict.view.as_ref();
    match id {
        I::RebaseContinue => continue_msg(st),
        _ if view.is_none_or(|v| v.chunk_count() == 0) => None,
        I::ConflictTakeOurs => pick(Choice::Ours),
        I::ConflictTakeTheirs => pick(Choice::Theirs),
        I::ConflictTakeBoth => pick(Choice::OursThenTheirs),
        I::ConflictTakeBothTheirsFirst => pick(Choice::TheirsThenOurs),
        I::ConflictClear => pick(Choice::Unresolved),
        // As the header's Mark resolved: only while it is enabled.
        I::ConflictMarkResolved => view.filter(|v| v.can_save() && !merging(st, &v.path)).map(|_| Msg::Conflict(ConflictMsg::SaveConflict)),
        _ => None,
    }
}

/// Continue the merge, rebase or cherry-pick: the conflict strip's Commit merge or Continue
/// while it can run; otherwise, during a rebase, the rebase's Continue (which says what stops
/// it); otherwise nothing.
fn continue_msg(st: &AppState) -> Option<Msg> {
    if let Some(f) = conflict_strip(st).and_then(|s| s.finish).filter(|f| f.why.is_none()) {
        return Some(f.msg());
    }
    rebasing(st).then_some(Msg::Rebase(RebaseMsg::Resume(Resume::Continue)))
}

/// Whether a rebase is in progress.
pub fn rebasing(st: &AppState) -> bool {
    use RepositoryState::*;
    st.snap.as_ref().is_some_and(|s| matches!(s.state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase))
}

#[cfg(test)]
#[path = "conflict_keys_tests.rs"]
mod tests;
