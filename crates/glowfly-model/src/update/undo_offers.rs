//! What an undo needs from the app beyond its plan: refusing while a worktree it would remove is
//! open in a tab, and the headline and Fetch button after a removed remote comes back.

use crate::effect::UndoResult;
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::AppState;
use crate::state::toasts::ToastAction;
use crate::state::worktree_manage::showing_tabs;
use glowfly_core::undo::Action;
use std::path::PathBuf;

/// Why undoing `action` can't start: the worktree it would remove is shown in a tab (git would
/// remove the folder under it). Only for an undo; a redo makes the worktree.
pub(super) fn blocked(st: &AppState, action: &Action, redo: bool) -> Option<String> {
    let Action::AddWorktree { path, .. } = action else { return None };
    if redo {
        return None;
    }
    let snap = st.snap.as_ref()?;
    let wt = snap.worktrees.iter().find(|w| glowfly_core::paths::same_spelling(&w.path, path))?;
    if showing_tabs(&st.tabs, wt).is_empty() {
        return None;
    }
    Some(st.settings.lang().trf("undo.worktree_open", &[("folder", &wt.folder_name())]))
}

/// The headline after an undo put a removed remote back: its remote-tracking branches come back
/// only with a fetch.
pub(super) fn headline(st: &AppState, undo: &Option<UndoResult>) -> Option<String> {
    let name = remote_back(undo)?;
    Some(st.settings.lang().trf("undo.remote_back", &[("remote", &name)]))
}

/// The remote an undo just put back, if it did.
pub(super) fn remote_back(undo: &Option<UndoResult>) -> Option<String> {
    match undo {
        Some(UndoResult::Done { redo: false, before: Action::RemoveRemote { name, .. }, .. }) => Some(name.clone()),
        _ => None,
    }
}

/// Gives the newest toast (the undo's) a button that fetches remote `name` in the repository
/// `workdir`.
pub(super) fn offer_fetch(st: &mut AppState, name: String, workdir: Option<PathBuf>) {
    let label = st.settings.lang().trf("undo.remote_back.fetch", &[("remote", &name)]);
    st.set_last_toast_action(ToastAction { label, msg: Box::new(Msg::Remote(RemoteMsg::Fetch(name))), workdir });
}
