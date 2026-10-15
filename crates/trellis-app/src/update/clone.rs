//! The Clone dialog: opening it, and checking its parent folder on disk as the user types (the
//! view only reads the answer). Running the clone itself is a git job (`ops`).

use crate::effect::Effect;
use crate::msg::CloneMsg;
use crate::state::clone::CloneForm;
use crate::state::{AppState, Modal};
use std::time::Duration;

/// How long after an edit the parent folder is checked. Each edit asks again after this pause;
/// answers for text that has changed since are dropped.
pub(super) const PARENT_CHECK_DELAY: Duration = Duration::from_millis(250);

pub(super) fn handle(st: &mut AppState, msg: CloneMsg, fx: &mut Vec<Effect>) {
    match msg {
        CloneMsg::Dialog => {
            let parent = st.recent.first().and_then(|p| p.parent()).map(|p| p.display().to_string()).unwrap_or_default();
            st.modal = Some(Modal::Clone(CloneForm::new(parent)));
            check_parent(st, Duration::ZERO, fx);
        }
        CloneMsg::ParentEdited => check_parent(st, PARENT_CHECK_DELAY, fx),
        CloneMsg::ParentChecked { parent, is_dir } => {
            if let Some(Modal::Clone(form)) = st.modal.as_mut()
                && form.parent.trim() == parent
            {
                form.parent_checked = Some((parent, is_dir));
            }
        }
        CloneMsg::Start(req) => super::ops::clone_repo(st, req, fx),
    }
}

/// Asks whether the open Clone dialog's parent folder exists, after `delay`. An empty field
/// needs no answer.
pub(super) fn check_parent(st: &AppState, delay: Duration, fx: &mut Vec<Effect>) {
    if let Some(Modal::Clone(form)) = &st.modal {
        let parent = form.parent.trim();
        if !parent.is_empty() {
            fx.push(Effect::CheckCloneParent { parent: parent.to_owned(), delay });
        }
    }
}
