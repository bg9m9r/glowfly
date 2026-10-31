//! Deleting the branch Git Flow names as production or development: the shared danger
//! confirmation made firmer (its own title, a warning paragraph after the question and a
//! "… anyway" button) for Delete together with the remote branch and the pull request's delete
//! offer; the title and warning the branch Delete dialog (`branch_delete.rs`), the Delete remote
//! branch dialog (`branch_sync.rs`) and the Remove worktree dialog show. Branch names match exactly
//! (case-sensitive), as git's do.

use crate::msg::Msg;
use glowfly_core::gitflow::FlowBase;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// Which copy of the branch the confirmed step deletes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Deleting<'a> {
    /// The branch on this remote.
    Remote(&'a str),
    /// The local branch and its copy on this remote.
    LocalAndRemote(&'a str),
}

/// The firmer dialog's title for deleting the branch with `role`.
pub fn title(t: Lang, role: FlowBase) -> &'static str {
    match role {
        FlowBase::Production => t.tr("gitflow.delete_base.production.title"),
        FlowBase::Development => t.tr("gitflow.delete_base.development.title"),
    }
}

/// What `branch` is for in Git Flow and what can't run without it.
pub fn warning(t: Lang, role: FlowBase, branch: &str) -> String {
    match role {
        FlowBase::Production => t.trf("gitflow.delete_base.production.warn", &[("branch", &branch)]),
        FlowBase::Development => t.trf("gitflow.delete_base.development.warn", &[("branch", &branch)]),
    }
}

/// `confirm` (a `Msg::ConfirmDanger` that deletes `branch`) made firmer when Git Flow names
/// `branch` as production or development; anything else comes back unchanged.
pub fn guard(t: Lang, snap: &Snapshot, branch: &str, what: Deleting, confirm: Msg) -> Msg {
    let Some(role) = snap.gitflow.base_role(branch) else { return confirm };
    let Msg::ConfirmDanger { body, ack, then, .. } = confirm else { return confirm };
    let title = title(t, role);
    let mut warn = warning(t, role, branch);
    let (Deleting::Remote(remote) | Deleting::LocalAndRemote(remote)) = what;
    warn.push(' ');
    warn.push_str(&t.trf("gitflow.delete_base.remote.warn", &[("remote", &remote)]));
    let button = match what {
        Deleting::LocalAndRemote(_) => t.tr("gitflow.delete_base.delete_both.button"),
        Deleting::Remote(_) => t.tr("gitflow.delete_base.delete.button"),
    };
    Msg::ConfirmDanger { title: title.into(), body: format!("{body}\n\n{warn}"), button: button.into(), ack, then }
}
