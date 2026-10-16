//! The filter dialog (`Modal::ForgeFilter`): opening it for a new filter (starting from the one on
//! show) or a saved one, loading its pickers' options, and saving.

use super::forge::request;
use super::forge_filters::{edit_filters, filters, shown};
use crate::effect::{Effect, FilterCall, ForgeCall};
use crate::state::forge_filter_form::FilterForm;
use crate::state::forge_filters::{OptionsState, target_for};
use crate::state::{AppState, Modal};
use trellis_core::forge::{FilterChoice, FilterKind, SavedFilter};

pub(super) fn new_dialog(st: &mut AppState, kind: FilterKind, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_ref() else { return };
    let provider = target_for(fs, kind).map(|t| t.provider);
    let (_, start, _) = filters(st).conditions(kind);
    st.modal = Some(Modal::ForgeFilter(Box::new(FilterForm::new(kind, provider, &start))));
    load_options(st, kind, fx);
}

pub(super) fn edit_dialog(st: &mut AppState, id: u64, fx: &mut Vec<Effect>) {
    let Some(saved): Option<SavedFilter> = filters(st).get(id).cloned() else { return };
    let Some(fs) = st.forge.as_ref() else { return };
    let provider = target_for(fs, saved.kind).map(|t| t.provider);
    st.modal = Some(Modal::ForgeFilter(Box::new(FilterForm::edit(&saved, provider))));
    load_options(st, saved.kind, fx);
}

/// Asks for labels, milestones and sprints unless they are loaded or on their way.
fn load_options(st: &mut AppState, kind: FilterKind, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_mut() else { return };
    if !matches!(fs.filters.options(kind), OptionsState::NotAsked | OptionsState::Failed(_)) {
        return;
    }
    let Some(target) = target_for(fs, kind) else { return };
    *fs.filters.options_mut(kind) = OptionsState::Loading;
    let sent = fx.len();
    request(st, target, false, ForgeCall::Filter(FilterCall::Options(kind)), fx);
    if fx.len() == sent
        && let Some(fs) = st.forge.as_mut()
    {
        // Held (an untrusted repository) or not connected: offer what is known.
        *fs.filters.options_mut(kind) = OptionsState::Failed("The service isn't contacted for this repository.".into());
    }
}

/// Stores the dialog's filter, shows it and closes the dialog; while it has problems nothing
/// happens and the dialog stays open.
pub(super) fn save(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::ForgeFilter(form)) = st.modal.as_ref() else { return };
    if !form.problems(&filters(st)).is_empty() {
        return;
    }
    let (kind, editing, name, conditions) = (form.kind, form.editing, form.name.trim().to_owned(), form.conditions());
    let saved = edit_filters(st, fx, |f| {
        let id = match editing {
            Some(id) if f.replace(id, &name, conditions.clone()) => id,
            _ => f.add(kind, &name, conditions),
        };
        f.set_choice(kind, FilterChoice::Saved(id));
        true
    });
    if saved {
        st.modal = None;
        shown(st, kind, fx);
    }
}
