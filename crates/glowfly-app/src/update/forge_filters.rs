//! Pull request and issue filters: choosing what a section shows (saved per repository), the
//! signed-in user, issue searches, removing and moving saved filters, and their results. The
//! filter dialog is in `forge_filter_edit`. Every call goes through `forge::request`, so
//! untrusted repositories and unconnected self-hosted servers are never contacted.

use super::forge::request;
use crate::effect::{Effect, FilterCall, FilterResult, ForgeCall};
use crate::msg::FilterMsg;
use crate::state::forge_filters::{IssueList, MeState, OptionsState};
use crate::state::{AppState, ForgeState, Status};
use glowfly_core::forge::filters::matching;
use glowfly_core::forge::{FilterChoice, FilterKind};
use glowfly_core::settings::RepoForgeFilters;

pub(super) fn handle(st: &mut AppState, msg: FilterMsg, fx: &mut Vec<Effect>) {
    match msg {
        FilterMsg::Choose { kind, choice } => choose(st, kind, choice, fx),
        FilterMsg::NewDialog(kind) => super::forge_filter_edit::new_dialog(st, kind, fx),
        FilterMsg::EditDialog(id) => super::forge_filter_edit::edit_dialog(st, id, fx),
        FilterMsg::Save => super::forge_filter_edit::save(st, fx),
        FilterMsg::Remove(id) => {
            let shown = filters(st).choice(FilterKind::Issues) == FilterChoice::Saved(id);
            if edit_filters(st, fx, |f| f.remove(id)) && shown {
                run_issue_query(st, false, fx);
            }
        }
        FilterMsg::Move { id, up } => {
            edit_filters(st, fx, |f| f.move_within_kind(id, up));
        }
    }
}

/// The open repository's filters (none without a repository).
pub(super) fn filters(st: &AppState) -> RepoForgeFilters {
    st.snap.as_ref().map(|s| st.settings.forge_filters_for(&s.real_workdir)).unwrap_or_default()
}

/// Changes the open repository's filters with `change`, saving them when it reports a change.
pub(super) fn edit_filters(st: &mut AppState, fx: &mut Vec<Effect>, change: impl FnOnce(&mut RepoForgeFilters) -> bool) -> bool {
    let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return false };
    let mut f = st.settings.forge_filters_for(&real);
    if !change(&mut f) {
        return false;
    }
    st.settings.set_forge_filters(&real, f);
    fx.push(super::prefs::save_settings(st));
    true
}

fn choose(st: &mut AppState, kind: FilterKind, choice: FilterChoice, fx: &mut Vec<Effect>) {
    edit_filters(st, fx, |f| {
        let before = f.choice(kind);
        f.set_choice(kind, choice);
        f.choice(kind) != before
    });
    shown(st, kind, fx);
}

/// What a newly shown filter of `kind` needs: the signed-in user, or a search.
pub(super) fn shown(st: &mut AppState, kind: FilterKind, fx: &mut Vec<Effect>) {
    match kind {
        FilterKind::Prs => ensure_me(st, fx),
        FilterKind::Issues => run_issue_query(st, false, fx),
    }
}

/// Asks the code host who the signed-in user is, when the shown pull request filter needs it and
/// it isn't known or being asked.
fn ensure_me(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (_, c, _) = filters(st).conditions(FilterKind::Prs);
    let Some(fs) = st.forge.as_mut() else { return };
    let Some(host) = fs.host.clone().filter(|_| matching::needs_me(FilterKind::Prs, &c)) else { return };
    if !matches!(fs.filters.me, MeState::Unknown | MeState::Failed(_)) {
        return;
    }
    let before = std::mem::replace(&mut fs.filters.me, MeState::Asking);
    let epoch = fs.filters.host_epoch;
    let sent = fx.len();
    request(st, host, false, ForgeCall::Filter(FilterCall::Me { epoch }), fx);
    if fx.len() == sent
        && let Some(fs) = st.forge.as_mut()
    {
        fs.filters.me = before;
    }
}

/// Searches the tracker for the shown issue filter; the default ("Assigned to me") is
/// `my_issues` and needs no search. `keep` keeps the last answer's rows while the search loads.
fn run_issue_query(st: &mut AppState, keep: bool, fx: &mut Vec<Effect>) {
    let (choice, conditions, _) = filters(st).conditions(FilterKind::Issues);
    let Some(fs) = st.forge.as_mut() else { return };
    if choice == FilterChoice::default_for(FilterKind::Issues) {
        fs.filters.issues = None;
        return;
    }
    let Some(tracker) = fs.tracker.clone() else { return };
    if let Some(why) = matching::unsupported(tracker.provider, FilterKind::Issues, &conditions) {
        fs.filters.issues = Some(IssueList { choice, conditions, items: Vec::new(), status: Status::Error(why) });
        return;
    }
    let items = match fs.filters.issues.take() {
        Some(l) if keep && l.choice == choice && l.conditions == conditions => l.items,
        _ => Vec::new(),
    };
    fs.filters.issues = Some(IssueList { choice, conditions: conditions.clone(), items, status: Status::Idle });
    // The code host's user is the tracker's only when they are the same service.
    let me = fs.filters.known_me().filter(|_| fs.host.as_ref() == Some(&tracker)).cloned().map(Box::new);
    let epoch = fs.filters.tracker_epoch;
    request(st, tracker, false, ForgeCall::Filter(FilterCall::Issues { choice, conditions, me, epoch }), fx);
}

/// After a refresh's requests: the signed-in user if still needed, and the shown issue search
/// again (keeping its rows until the answer).
pub(super) fn refresh(st: &mut AppState, fx: &mut Vec<Effect>) {
    ensure_me(st, fx);
    run_issue_query(st, true, fx);
}

/// A filter call's answer from the service whose account key is `key`; `from_host` and
/// `from_tracker` say which of the repository's services it is. An answer from a service the
/// state no longer uses for that kind (the tracker changed), or asked under an earlier account,
/// is dropped.
pub(super) fn replied(fs: &mut ForgeState, r: FilterResult, key: &str, from_host: bool, from_tracker: bool) {
    match r {
        FilterResult::Me { epoch, result } => {
            if !from_host || epoch != fs.filters.host_epoch {
                return;
            }
            match result {
                Ok(me) => fs.filters.me = MeState::Known(me),
                Err(e) => {
                    if e.auth {
                        fs.auth.remove(key);
                    }
                    fs.filters.me = MeState::Failed(e.message);
                }
            }
        }
        FilterResult::Issues { choice, conditions, epoch, me, result } => {
            if !from_tracker || epoch != fs.filters.tracker_epoch {
                return;
            }
            // The user a search looked up is the code host's when the host is the tracker.
            if let Some(me) = me.filter(|_| from_host && fs.filters.known_me().is_none()) {
                fs.filters.me = MeState::Known(me);
            }
            // A reply for a filter no longer shown (or changed since) is dropped.
            let Some(list) = fs.filters.issues.as_mut().filter(|l| l.choice == choice && l.conditions == conditions) else { return };
            match result {
                Ok(items) => {
                    for i in &items {
                        fs.cache.insert(i.key.clone(), i.clone());
                    }
                    list.items = items;
                    list.status = Status::Ok;
                }
                Err(e) => {
                    if e.auth {
                        fs.auth.remove(key);
                    }
                    list.items.clear();
                    list.status = Status::Error(e.message);
                }
            }
        }
        FilterResult::Options(kind, result) => {
            let from_target = match kind {
                FilterKind::Prs => from_host,
                FilterKind::Issues => from_tracker,
            };
            if !from_target {
                return;
            }
            let state = match result {
                Ok(o) => OptionsState::Loaded(o),
                Err(e) => {
                    if e.auth {
                        fs.auth.remove(key);
                    }
                    OptionsState::Failed(e.message)
                }
            };
            *fs.filters.options_mut(kind) = state;
        }
    }
}
