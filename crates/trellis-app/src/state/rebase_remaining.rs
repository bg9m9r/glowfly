//! The rebase panel on the remaining steps of an interactive rebase in progress
//! (`git rebase --edit-todo`): its texts, when it can open, and the request it sends.

use super::rebase::RebasePanel;
use git2::RepositoryState;
use trellis_core::rebase::{self, EditTodoRequest};
use trellis_core::repo::Snapshot;

pub const REMAINING_TITLE: &str = "Edit rebase steps";
/// The primary button.
pub const UPDATE: &str = "Update steps";
pub const REMAINING_HINT: &str = "Steps run from top to bottom; the steps already done aren't listed.";
pub const LOCKED_NOTE: &str = "These steps finish the commit being reworded and can't be changed.";
pub const ENDED: &str = "The rebase ended, so its steps can't be edited any more.";
pub const UPDATED: &str = "Updated the remaining steps of the rebase";
/// The job label.
pub const EDIT_JOB: &str = "Edit rebase steps";
pub const NO_STEPS: &str = "No steps are left; add an exec or break step.";

/// "3 steps left in the rebase of feature onto 1a2b3c4 Base · 2 done".
pub fn subtitle(panel: &RebasePanel) -> String {
    let Some(r) = &panel.remaining else { return "Loading steps…".into() };
    let n = panel.items.len();
    let steps = if n == 1 { "1 step".to_owned() } else { format!("{n} steps") };
    let of = match &r.branch {
        Some(b) => b.clone(),
        None => "a detached HEAD".into(),
    };
    format!("{steps} left in the rebase of {of} onto {} · {} done", r.onto_label, r.done)
}

/// Why the remaining steps can't be edited now, if they can't: only an interactive rebase's.
pub fn open_remaining_blocker(snap: &Snapshot) -> Option<String> {
    (snap.state != RepositoryState::RebaseInteractive).then(|| rebase::NOT_INTERACTIVE.into())
}

/// What Update steps sends, once the list is loaded.
pub fn edit_request(panel: &RebasePanel) -> Option<EditTodoRequest> {
    let r = panel.remaining.as_ref()?;
    Some(EditTodoRequest {
        steps: panel.items.clone(),
        locked_text: r.locked_text.clone(),
        todo: r.todo.clone(),
        first_may_combine: r.first_may_combine,
    })
}
