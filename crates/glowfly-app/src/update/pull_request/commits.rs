//! The pull request panel's Commits tab: a clicked row shows its commit in the commit panel
//! (shown if hidden) while the pull request panel stays open, so the commit's files open in
//! the tab (`keep_for`). Like `repo::select_commit`, without closing the panel.

use crate::effect::Effect;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::DiffTarget;

pub(super) fn select(st: &mut AppState, oid: Oid, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    if !st.pull_request.as_ref().is_some_and(|v| v.has_commit(oid)) {
        return;
    }
    if matches!(&st.diff, Some((DiffTarget::Commit(o, _), _)) if *o != oid) {
        super::super::repo::close_diff(st);
    }
    if st.selection != Selection::Commit(oid) {
        st.selection = Selection::Commit(oid);
        fx.push(Effect::LoadDetail { workdir, oid });
        super::super::line_counts::want_commit(st, oid, fx);
    }
    super::super::layout::reveal_details(st, fx);
}
