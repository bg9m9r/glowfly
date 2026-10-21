//! The filter dialog: opening it, loading its options, saving, and moving and removing saved
//! filters.

use super::forge_filters_tests::{filter_calls, open, reply, saved, send};
use crate::effect::{FilterCall, FilterResult};
use crate::msg::FilterMsg;
use crate::state::forge_filter_form::FilterForm;
use crate::state::forge_filters::OptionsState;
use crate::state::{AppState, Modal};
use glowfly_core::forge::filters::Who;
use glowfly_core::forge::{Builtin, FilterChoice, FilterKind, FilterOptions};
use glowfly_core::settings::RepoForgeFilters;

fn form(st: &mut AppState) -> &mut FilterForm {
    match st.modal.as_mut() {
        Some(Modal::ForgeFilter(f)) => f,
        _ => panic!("the filter dialog is open"),
    }
}

fn filters(st: &AppState) -> RepoForgeFilters {
    st.settings.forge_filters_for(&st.snap.as_ref().unwrap().real_workdir)
}

/// Saves a new filter of `kind` called `name` through the dialog; returns its id.
fn create(st: &mut AppState, kind: FilterKind, name: &str) -> u64 {
    send(st, FilterMsg::NewDialog(kind));
    form(st).name = name.into();
    assert!(saved(&send(st, FilterMsg::Save)).is_some());
    match filters(st).choice(kind) {
        FilterChoice::Saved(id) => id,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_new_filter_starts_from_the_one_on_show_and_loads_options_once() {
    let (mut st, dir) = open("ffe-new");
    send(&mut st, FilterMsg::Choose { kind: FilterKind::Prs, choice: FilterChoice::Builtin(Builtin::MyPrs) });
    let fx = send(&mut st, FilterMsg::NewDialog(FilterKind::Prs));
    assert_eq!(form(&mut st).c.author, Who::Me, "pre-filled from Mine");
    assert_eq!(form(&mut st).title(), "New pull request filter");
    let calls = filter_calls(&fx);
    assert!(matches!(calls[..], [(_, FilterCall::Options(FilterKind::Prs))]), "{calls:?}");
    assert!(filter_calls(&send(&mut st, FilterMsg::NewDialog(FilterKind::Prs))).is_empty(), "loading");
    let host = st.forge.as_ref().unwrap().host.clone().unwrap();
    let options = FilterOptions { labels: vec!["bug".into()], ..Default::default() };
    reply(&mut st, host, FilterResult::Options(FilterKind::Prs, Ok(options.clone())));
    assert_eq!(st.forge.as_ref().unwrap().filters.pr_options, OptionsState::Loaded(options));
    assert!(filter_calls(&send(&mut st, FilterMsg::NewDialog(FilterKind::Prs))).is_empty(), "loaded");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn save_refuses_a_missing_or_taken_name() {
    let (mut st, dir) = open("ffe-names");
    send(&mut st, FilterMsg::NewDialog(FilterKind::Prs));
    assert!(saved(&send(&mut st, FilterMsg::Save)).is_none());
    assert!(matches!(st.modal, Some(Modal::ForgeFilter(_))), "stays open");
    form(&mut st).name = "Mine".into();
    let fx = send(&mut st, FilterMsg::Save);
    let id = match saved(&fx).unwrap().forge_filters_for(&st.snap.as_ref().unwrap().real_workdir).choice(FilterKind::Prs) {
        FilterChoice::Saved(id) => id,
        other => panic!("{other:?}"),
    };
    assert!(st.modal.is_none(), "closed");
    assert_eq!(filters(&st).get(id).unwrap().name, "Mine");

    send(&mut st, FilterMsg::NewDialog(FilterKind::Prs));
    form(&mut st).name = " mine".into();
    assert!(saved(&send(&mut st, FilterMsg::Save)).is_none());
    assert_eq!(filters(&st).saved_of(FilterKind::Prs).count(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn editing_replaces_in_place_and_moving_stays_within_a_kind() {
    let (mut st, dir) = open("ffe-edit");
    let a = create(&mut st, FilterKind::Prs, "A");
    let issue = create(&mut st, FilterKind::Issues, "Issues");
    let b = create(&mut st, FilterKind::Prs, "B");

    send(&mut st, FilterMsg::EditDialog(a));
    assert_eq!(form(&mut st).title(), "Edit pull request filter");
    form(&mut st).name = "A2".into();
    form(&mut st).labels = "ui".into();
    send(&mut st, FilterMsg::Save);
    let f = filters(&st);
    let names: Vec<(u64, String)> = f.saved_of(FilterKind::Prs).map(|s| (s.id, s.name.clone())).collect();
    assert_eq!(names, [(a, "A2".to_owned()), (b, "B".to_owned())]);
    assert_eq!(f.get(a).unwrap().conditions.labels, ["ui"]);
    assert_eq!(f.choice(FilterKind::Prs), FilterChoice::Saved(a));

    assert!(saved(&send(&mut st, FilterMsg::Move { id: a, up: true })).is_none(), "already first");
    assert!(saved(&send(&mut st, FilterMsg::Move { id: b, up: true })).is_some());
    let order: Vec<u64> = filters(&st).saved_of(FilterKind::Prs).map(|s| s.id).collect();
    assert_eq!(order, [b, a]);
    assert!(saved(&send(&mut st, FilterMsg::Move { id: issue, up: true })).is_none(), "the only issue filter");

    let before = filters(&st);
    send(&mut st, FilterMsg::EditDialog(999));
    assert!(st.modal.is_none() && filters(&st) == before, "a missing filter does nothing");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn removing_the_filter_on_show_goes_back_to_the_default() {
    let (mut st, dir) = open("ffe-remove");
    let id = create(&mut st, FilterKind::Issues, "Bugs");
    assert!(st.forge.as_ref().unwrap().filters.issues.is_some(), "searching");
    assert!(saved(&send(&mut st, FilterMsg::Remove(id))).is_some());
    assert_eq!(filters(&st).choice(FilterKind::Issues), FilterChoice::default_for(FilterKind::Issues));
    assert!(st.forge.as_ref().unwrap().filters.issues.is_none(), "assigned issues again");
    assert!(filters(&st).is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
