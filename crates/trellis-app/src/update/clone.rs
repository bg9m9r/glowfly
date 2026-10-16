//! The Clone dialog: opening it, checking its parent folder on disk as the user types (the view
//! only reads the answer), its Your repositories tab (`clone_service`), and the end of a sparse
//! clone. A plain clone is a git job (`ops`); a sparse one is `CloneEffect::Sparse`.

use crate::effect::Effect;
use crate::msg::CloneMsg;
use crate::state::clone::CloneForm;
use crate::state::clone_service::listing_accounts;
use crate::state::{AppState, Modal};
use std::time::{Duration, Instant};

/// How long after an edit the parent folder is checked. Each edit asks again after this pause;
/// answers for text that has changed since are dropped.
pub(super) const PARENT_CHECK_DELAY: Duration = Duration::from_millis(250);

pub(super) fn handle(st: &mut AppState, msg: CloneMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        CloneMsg::Dialog => open_with(st, None, fx),
        CloneMsg::ParentEdited => check_parent(st, PARENT_CHECK_DELAY, fx),
        CloneMsg::ParentChecked { parent, found } => {
            if let Some(Modal::Clone(form)) = st.modal.as_mut()
                && form.parent.trim() == parent
            {
                form.parent_checked = Some((parent, found));
            }
        }
        CloneMsg::Start(req) => super::ops::clone_repo(st, req, fx),
        CloneMsg::Source(s) => super::clone_service::source(st, s, fx),
        CloneMsg::ServiceAccount(i) => super::clone_service::account(st, i, fx),
        CloneMsg::RetryRepos => super::clone_service::retry(st, fx),
        CloneMsg::PickRepo(i) => super::clone_service::pick(st, i),
        CloneMsg::UseSsh(ssh) => super::clone_service::use_ssh(st, ssh),
        CloneMsg::ReposLoaded { account, result } => super::clone_service::loaded(st, account, *result),
        CloneMsg::Finished(o) => {
            let headline = (o.cloned && !o.ok).then(|| {
                let name = st
                    .job
                    .as_ref()
                    .and_then(|j| j.then_open.as_ref())
                    .and_then(|(p, _)| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let last = super::ops::last_line(&o.output);
                format!("Cloned {name}, but setting its folders failed: {last}. Choose them in Preferences > Sparse Checkout.")
            });
            super::jobs::finish_made(st, o.cloned, o.ok, o.output, headline, now, fx);
        }
    }
}

/// Asks whether the open Clone dialog's parent folder exists, after `delay`. An empty field
/// needs no answer.
/// Opens the Clone dialog, with `url` filled in (a link's remote).
pub(super) fn open_with(st: &mut AppState, url: Option<String>, fx: &mut Vec<Effect>) {
    let parent = st.recent.first().and_then(|p| p.parent()).map(trellis_core::native_path::shown).unwrap_or_default();
    let mut form = CloneForm::new(parent);
    form.service.accounts = listing_accounts(&st.settings.connections);
    if let Some(url) = url {
        form.url = url;
    }
    st.modal = Some(Modal::Clone(form));
    check_parent(st, Duration::ZERO, fx);
}

pub(super) fn check_parent(st: &AppState, delay: Duration, fx: &mut Vec<Effect>) {
    if let Some(Modal::Clone(form)) = &st.modal {
        let parent = form.parent.trim();
        if !parent.is_empty() {
            fx.push(Effect::CheckCloneParent { parent: parent.to_owned(), delay });
        }
    }
}
