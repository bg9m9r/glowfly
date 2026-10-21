//! Deleting the branch Git Flow names as production or development: the shared danger
//! confirmation made firmer (its own title, a warning paragraph after the question and a
//! "… anyway" button) for the branch menus, the remote branch menu and the pull request's delete
//! offer, and the warning the Remove worktree dialog shows. Branch names match exactly
//! (case-sensitive), as git's do.

use crate::msg::Msg;
use trellis_core::gitflow::FlowBase;
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;

/// Which copy of the branch the confirmed step deletes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Deleting<'a> {
    Local,
    /// The branch on this remote.
    Remote(&'a str),
    /// The local branch and its copy on this remote.
    LocalAndRemote(&'a str),
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
    let title = match role {
        FlowBase::Production => t.tr("gitflow.delete_base.production.title"),
        FlowBase::Development => t.tr("gitflow.delete_base.development.title"),
    };
    let mut warn = warning(t, role, branch);
    if let Deleting::Remote(remote) | Deleting::LocalAndRemote(remote) = what {
        warn.push(' ');
        warn.push_str(&t.trf("gitflow.delete_base.remote.warn", &[("remote", &remote)]));
    }
    let button = match what {
        Deleting::LocalAndRemote(_) => t.tr("gitflow.delete_base.delete_both.button"),
        Deleting::Local | Deleting::Remote(_) => t.tr("gitflow.delete_base.delete.button"),
    };
    Msg::ConfirmDanger { title: title.into(), body: format!("{body}\n\n{warn}"), button: button.into(), ack, then }
}
