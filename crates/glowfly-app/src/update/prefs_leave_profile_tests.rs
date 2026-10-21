//! Headless tests for the Profile page's unsaved identity edits and its profile actions:
//! renaming or deleting another profile keeps the edits, while adding a profile or switching
//! asks "Save your changes?" first, and a saved switch applies the typed identity without
//! racing the switch's own writes. No effect runs: switching would write the developer's
//! global git config.

use super::prefs_save_tests::{edited, run, writes};
use super::profiles_tests::{PERSONAL, WORK, two_profiles, values};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::profile::{ProfileEffect, ProfileMsg};
use crate::state::{AppState, Modal};

fn asking(st: &AppState) -> bool {
    matches!(st.modal, Some(Modal::ConfirmChoice { .. }))
}

/// Two profiles with Work (the current one) selected and its email typed over.
fn work_edited() -> AppState {
    let mut st = two_profiles();
    st.prefs.as_mut().unwrap().profiles.author_email = "ada@new.example".into();
    assert!(edited(&st));
    st
}

fn typed_email(st: &AppState) -> &str {
    &st.prefs.as_ref().unwrap().profiles.author_email
}

/// The identity of the switch's apply effect among `fx`.
fn applied_identity(fx: &[Effect]) -> Option<(glowfly_core::settings::ProfileId, &glowfly_core::identity::Identity)> {
    fx.iter().find_map(|e| match e {
        Effect::Profile(ProfileEffect::Apply { id, identity, .. }) => Some((*id, identity)),
        _ => None,
    })
}

#[test]
fn renaming_keeps_the_edits() {
    let mut st = work_edited();
    run(&mut st, Msg::Profile(ProfileMsg::Rename { id: WORK, name: "Job".into() }));
    assert!(!asking(&st));
    assert_eq!(st.settings.profile(WORK).unwrap().name, "Job");
    assert_eq!(typed_email(&st), "ada@new.example");
    assert!(edited(&st), "still unsaved");
}

#[test]
fn deleting_another_profile_keeps_the_edits() {
    let mut st = work_edited();
    run(&mut st, Msg::Profile(ProfileMsg::Delete(PERSONAL)));
    assert!(st.settings.profile(PERSONAL).is_none());
    assert_eq!(st.prefs.as_ref().unwrap().profiles.selected, Some(WORK));
    assert_eq!(typed_email(&st), "ada@new.example");
    assert!(edited(&st));
}

#[test]
fn unedited_fields_follow_a_rename() {
    let mut st = two_profiles();
    run(&mut st, Msg::Profile(ProfileMsg::Rename { id: WORK, name: "Job".into() }));
    assert_eq!(typed_email(&st), "ada@work.example");
    assert!(!edited(&st));
}

#[test]
fn adding_a_profile_asks_first() {
    let mut st = work_edited();
    let add = ProfileMsg::Add { name: "Client".into(), copy_current: true, copy_accounts: false };
    let fx = run(&mut st, Msg::Profile(add));
    assert!(asking(&st));
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.settings.profiles.len(), 2, "held");

    run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(st.settings.profile(WORK).unwrap().author_email, "ada@new.example", "saved");
    let client = st.settings.profiles.iter().find(|p| p.name == "Client").expect("added");
    assert_eq!(client.author_email, "ada@new.example", "copied the saved identity");
    assert_eq!(st.prefs.as_ref().unwrap().profiles.selected, Some(client.id));
}

#[test]
fn switching_to_the_edited_profile_applies_what_was_typed() {
    let mut st = two_profiles();
    run(&mut st, Msg::Profile(ProfileMsg::Select(PERSONAL)));
    st.prefs.as_mut().unwrap().profiles.author_email = "ada@typed.example".into();
    let fx = run(&mut st, Msg::Profile(ProfileMsg::Switch(PERSONAL)));
    assert!(asking(&st));
    assert!(fx.is_empty() && st.profile_switch.is_none(), "held");

    let fx = run(&mut st, Msg::ConfirmAccepted);
    let (id, identity) = applied_identity(&fx).expect("the switch started");
    assert_eq!(id, PERSONAL);
    assert_eq!(identity.email, "ada@typed.example");
    assert!(writes(&fx).is_empty(), "Personal isn't current, so only the switch writes git config");
}

#[test]
fn discarding_before_a_switch_applies_the_stored_identity() {
    let mut st = two_profiles();
    run(&mut st, Msg::Profile(ProfileMsg::Select(PERSONAL)));
    st.prefs.as_mut().unwrap().profiles.author_email = "ada@typed.example".into();
    run(&mut st, Msg::Profile(ProfileMsg::Switch(PERSONAL)));
    let fx = run(&mut st, Msg::ConfirmAlt);
    assert_eq!(applied_identity(&fx).unwrap().1.email, "ada@home.example");
    assert_eq!(st.settings.profile(PERSONAL).unwrap().author_email, "ada@home.example");
    assert_eq!(typed_email(&st), "ada@home.example");
}

#[test]
fn saving_the_current_profile_before_switching_away_waits_for_the_switch() {
    let mut st = work_edited();
    run(&mut st, Msg::Profile(ProfileMsg::Switch(PERSONAL)));
    assert!(asking(&st));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(applied_identity(&fx).unwrap().0, PERSONAL);
    assert!(writes(&fx).is_empty(), "no write races the switch: {fx:?}");
    assert_eq!(st.settings.profile(WORK).unwrap().author_email, "ada@new.example", "saved");

    // The switch worked: Personal is current and Work's edit isn't written over it.
    let fx = run(&mut st, Msg::Profile(ProfileMsg::Applied { id: PERSONAL, result: Ok(values("Ada L", "ada@home.example")) }));
    assert_eq!(st.settings.active_profile, Some(PERSONAL));
    assert!(st.profile_switch.is_none());
    assert!(!fx.iter().any(|e| matches!(e, Effect::Profile(ProfileEffect::Rewrite { .. }))), "{fx:?}");
}

#[test]
fn a_failed_switch_writes_the_saved_edit_afterwards() {
    let mut st = work_edited();
    run(&mut st, Msg::Profile(ProfileMsg::Switch(PERSONAL)));
    run(&mut st, Msg::ConfirmAccepted);
    let fx = run(&mut st, Msg::Profile(ProfileMsg::Applied { id: PERSONAL, result: Err("locked".into()) }));
    assert_eq!(st.settings.active_profile, Some(WORK));
    let rewrite = fx.iter().find_map(|e| match e {
        Effect::Profile(ProfileEffect::Rewrite { id, identity, .. }) => Some((*id, identity)),
        _ => None,
    });
    let (id, identity) = rewrite.expect("Work's saved identity is written next");
    assert_eq!(id, WORK);
    assert_eq!(identity.email, "ada@new.example");
}
