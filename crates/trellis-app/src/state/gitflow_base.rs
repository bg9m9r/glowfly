//! Deleting the branch Git Flow names as production or development: the destructive
//! confirmation that replaces the plain Confirm (branch menus, the remote branch menu, the pull
//! request's delete offer), and the warning the Remove worktree dialog shows.

use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use trellis_core::gitflow::FlowBase;
use trellis_core::repo::Snapshot;

/// The confirmation (Modal::GitFlowDeleteBase).
pub struct BaseDelete {
    /// "Delete development branch" (or production).
    pub title: String,
    /// The plain confirmation's question ("Delete local branch develop?").
    pub question: String,
    /// The red paragraph: what the branch is for and what stops working.
    pub warning: String,
    /// "Delete anyway".
    pub button: String,
    /// The delete to run.
    pub then: Box<Msg>,
}

/// What `branch` is for in Git Flow and what can't run without it.
pub fn warning(role: FlowBase, branch: &str) -> String {
    let what = match role {
        FlowBase::Production => "Releases and hotfixes are merged into it and tagged there, and hotfixes start from it.",
        FlowBase::Development => "Features and releases start from it and every Finish merges into it.",
    };
    format!(
        "{branch} is the Git Flow {} branch. {what} Until it exists again or Preferences > Git Flow names another \
         branch, those steps can't run.",
        role.noun()
    )
}

/// Asks before running `then`, which deletes the local branch `branch`: the destructive
/// confirmation when Git Flow names it as production or development, else the plain one.
pub fn confirm_delete(snap: &Snapshot, branch: &str, title: &str, question: String, button: &str, then: Msg) -> Msg {
    ask(snap, branch, None, title, question, button, then)
}

/// The same for deleting `name` on `remote` (the warning adds that the remote holds the shared
/// copy).
pub fn confirm_remote_delete(snap: &Snapshot, remote: &str, name: &str, title: &str, question: String, button: &str, then: Msg) -> Msg {
    ask(snap, name, Some(remote), title, question, button, then)
}

fn ask(snap: &Snapshot, branch: &str, remote: Option<&str>, title: &str, question: String, button: &str, then: Msg) -> Msg {
    let Some(role) = snap.gitflow.base_role(branch) else { return Msg::confirm(title, question, button, then) };
    let mut warning = warning(role, branch);
    if let Some(r) = remote {
        warning.push_str(&format!(" {r} holds the copy everyone else uses."));
    }
    Msg::GitFlow(GitFlowMsg::DeleteBase(Box::new(BaseDelete {
        title: format!("Delete {} branch", role.noun()),
        question,
        warning,
        button: format!("{button} anyway"),
        then: Box::new(then),
    })))
}

/// The warning for removing a worktree together with its branch, when that branch is the
/// production or development branch.
pub fn worktree_warning(snap: &Snapshot, branch: Option<&str>) -> Option<String> {
    let branch = branch?;
    snap.gitflow.base_role(branch).map(|role| warning(role, branch))
}
