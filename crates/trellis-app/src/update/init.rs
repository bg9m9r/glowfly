//! The Init dialog: opening it, checking its folder on disk as the user types (the view only
//! reads the answer), suggesting git's user.name as a license's copyright holder, and the init
//! job (`InitEffect::Run`), which opens the new repository even when only a later step failed.

use super::clone::PARENT_CHECK_DELAY;
use super::jobs::Started;
use crate::effect::{Effect, InitEffect};
use crate::msg::InitMsg;
use crate::state::init::InitForm;
use crate::state::trust::Created;
use crate::state::{AppState, Modal};
use std::time::{Duration, Instant};

/// Opens the dialog in the most recent repository's parent folder, with the default branch from
/// Preferences.
pub(super) fn open(st: &mut AppState, fx: &mut Vec<Effect>) {
    let shown = st.recent.first().and_then(|p| p.parent()).map(trellis_core::native_path::shown).unwrap_or_default();
    let trimmed = shown.trim_end_matches(['/', '\\']);
    // A root ("C:\", "/") keeps its separator.
    let folder = if trimmed.is_empty() || trimmed.ends_with(':') { shown.clone() } else { trimmed.to_owned() };
    st.modal = Some(Modal::Init(InitForm::new(folder, st.settings.default_branch.trim().to_owned())));
    check_folder(st, Duration::ZERO, fx);
    fx.push(Effect::Init(InitEffect::ReadAuthor { cwd: st.any_dir() }));
}

fn form(st: &mut AppState) -> Option<&mut InitForm> {
    match st.modal.as_mut() {
        Some(Modal::Init(f)) => Some(f),
        _ => None,
    }
}

pub(super) fn handle(st: &mut AppState, msg: InitMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        InitMsg::FolderEdited => check_folder(st, PARENT_CHECK_DELAY, fx),
        InitMsg::FolderChecked { folder, found } => {
            if let Some(f) = form(st)
                && f.folder.trim() == folder
            {
                f.folder_checked = Some((folder, found));
            }
        }
        InitMsg::AuthorRead(Some(name)) => {
            if let Some(f) = form(st)
                && (f.holder.trim().is_empty() || f.holder == f.holder_suggested)
            {
                f.holder = name.clone();
                f.holder_suggested = name;
            }
        }
        InitMsg::AuthorRead(None) => {}
        InitMsg::Start(req) => {
            // The dialog blocks this; checked again so no other sender can init a relative path.
            if !req.folder.is_absolute() {
                st.log(false, format!("Could not create the repository: {} isn't the full path of a folder.", req.folder.display()));
                return;
            }
            let (folder, plan) = (req.folder, req.plan);
            let then_open = Some((folder.clone(), Created::Init));
            let started =
                super::jobs::start_effect_job(st, "Init", folder, then_open, |cwd| Effect::Init(InitEffect::Run { folder: cwd, plan }), fx);
            if matches!(started, Started::Now | Started::Held) && matches!(st.modal, Some(Modal::Init(_))) {
                st.modal = None;
            }
        }
        InitMsg::Finished(o) => {
            let headline = (o.initialized && !o.ok).then(|| {
                let step = o.failed.as_deref().unwrap_or("a step");
                format!("Created the repository, but {step} failed: {}", super::ops::last_line(&o.output))
            });
            super::jobs::finish_made(st, o.initialized, o.ok, o.output, headline, now, fx);
        }
    }
}

/// Asks what the open Init dialog's folder text names, after `delay`. An empty field needs no
/// answer.
pub(super) fn check_folder(st: &AppState, delay: Duration, fx: &mut Vec<Effect>) {
    if let Some(Modal::Init(form)) = &st.modal {
        let folder = form.folder.trim();
        if !folder.is_empty() {
            fx.push(Effect::Init(InitEffect::CheckFolder { folder: folder.to_owned(), delay }));
        }
    }
}
