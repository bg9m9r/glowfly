//! Headless tests for profiles: seeding and syncing when the Profile page gets git config,
//! adding, renaming, editing and deleting profiles, and switching (pending, success, failure).
//! No effect runs: switching would write the developer's global git config.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg};
use crate::profile::{IdentityField, ProfileEffect, ProfileMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::profiles::{AddForm, ProfilesPage, RenameForm, STILL_SWITCHING};
use crate::state::{AppState, Modal, PrefPage};
use std::time::Instant;
use trellis_core::identity::Identity;
use trellis_core::settings::{Profile, ProfileId};

pub(super) const WORK: ProfileId = ProfileId(1);
pub(super) const PERSONAL: ProfileId = ProfileId(2);

pub(super) fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

pub(super) fn profile(st: &mut AppState, m: ProfileMsg) -> Vec<Effect> {
    run(st, Msg::Profile(m))
}

pub(super) fn values(name: &str, email: &str) -> GitConfigValues {
    GitConfigValues { user_name: name.into(), user_email: email.into(), signing_format: "openpgp".into(), ..Default::default() }
}

fn page(st: &AppState) -> &ProfilesPage {
    &st.prefs.as_ref().expect("Preferences open").profiles
}

fn page_mut(st: &mut AppState) -> &mut ProfilesPage {
    &mut st.prefs.as_mut().expect("Preferences open").profiles
}

pub(super) fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

fn git_writes(fx: &[Effect]) -> Vec<(String, Option<String>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::SetGitConfig { global: true, key, value, .. } => Some((key.clone(), value.clone())),
            _ => None,
        })
        .collect()
}

/// "Work" (current) and "Personal", Preferences open on the Profile page with Work's identity
/// in git config.
pub(super) fn two_profiles() -> AppState {
    let mut st = state();
    st.settings.profiles = vec![
        Profile {
            id: WORK,
            name: "Work".into(),
            author_name: "Ada".into(),
            author_email: "ada@work.example".into(),
            signing_key: None,
            ..Profile::default()
        },
        Profile {
            id: PERSONAL,
            name: "Personal".into(),
            author_name: "Ada L".into(),
            author_email: "ada@home.example".into(),
            signing_key: Some("HOMEKEY".into()),
            ..Profile::default()
        },
    ];
    st.settings.active_profile = Some(WORK);
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Profile)));
    run(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values("Ada", "ada@work.example"))));
    st
}

#[test]
fn first_visit_seeds_default_from_git() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Profile)));
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values("Ada", "ada@example.com"))));
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profiles.len(), 1);
    let p = &st.settings.profiles[0];
    assert_eq!((p.name.as_str(), p.author_name.as_str(), p.author_email.as_str()), ("Default", "Ada", "ada@example.com"));
    assert_eq!(st.settings.active_profile, Some(p.id));
    let pg = page(&st);
    assert_eq!(pg.selected, Some(p.id));
    assert_eq!((pg.author_name.as_str(), pg.author_email.as_str(), pg.signing_key.as_str()), ("Ada", "ada@example.com", ""));
}

#[test]
fn other_pages_never_seed() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Signing)));
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values("Ada", "ada@example.com"))));
    assert!(matches!(&fx[..], [Effect::ListSigningKeys { .. }]), "{fx:?}");
    assert!(st.settings.profiles.is_empty());
}

#[test]
fn show_page_syncs_an_outside_change() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
    run(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values("Ada", "ada@example.com"))));
    assert!(st.settings.profiles.is_empty(), "General doesn't seed");
    let fx = profile(&mut st, ProfileMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().unwrap().page, PrefPage::Profile);
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profiles.len(), 1);

    // git config changed outside Trellis: the current profile follows it.
    st.prefs.as_mut().unwrap().git = Some(values("Ada", "ada@new.example"));
    let fx = profile(&mut st, ProfileMsg::ShowPage);
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profiles[0].author_email, "ada@new.example");
    assert_eq!(page(&st).author_email, "ada@new.example");
    // Nothing new, nothing saved.
    assert!(profile(&mut st, ProfileMsg::ShowPage).is_empty());
}

#[test]
fn adding_copies_the_current_identity() {
    let mut st = two_profiles();
    page_mut(&mut st).adding = Some(AddForm::new());
    let fx = profile(&mut st, ProfileMsg::Add { name: " Client ".into(), copy_current: true, copy_accounts: false });
    assert_eq!(saves(&fx), 1);
    let new = st.settings.profiles.last().unwrap().clone();
    assert_eq!((new.name.as_str(), new.author_name.as_str(), new.author_email.as_str()), ("Client", "Ada", "ada@work.example"));
    assert_eq!(st.settings.active_profile, Some(WORK), "not made current");
    assert!(page(&st).adding.is_none());
    assert_eq!(page(&st).selected, Some(new.id));

    let fx = profile(&mut st, ProfileMsg::Add { name: "Blank".into(), copy_current: false, copy_accounts: false });
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profiles.last().unwrap().identity(), Identity::default());
}

#[test]
fn adding_refuses_blank_and_taken_names() {
    let mut st = two_profiles();
    page_mut(&mut st).adding = Some(AddForm::new());
    for name in ["  ", "work"] {
        let fx = profile(&mut st, ProfileMsg::Add { name: name.into(), copy_current: true, copy_accounts: false });
        assert_eq!(saves(&fx), 0);
        assert!(page(&st).adding.as_ref().unwrap().error.is_some(), "{name:?}");
    }
    assert_eq!(page(&st).adding.as_ref().unwrap().error.as_deref(), Some("Another profile already has this name."));
    assert_eq!(st.settings.profiles.len(), 2);
}

#[test]
fn renaming() {
    let mut st = two_profiles();
    page_mut(&mut st).renaming = Some(RenameForm::new(PERSONAL, "Personal"));
    let fx = profile(&mut st, ProfileMsg::Rename { id: PERSONAL, name: "WORK".into() });
    assert_eq!(saves(&fx), 0);
    assert_eq!(page(&st).renaming.as_ref().unwrap().error.as_deref(), Some("Another profile already has this name."));
    let fx = profile(&mut st, ProfileMsg::Rename { id: PERSONAL, name: " Home ".into() });
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profile(PERSONAL).unwrap().name, "Home");
    assert!(page(&st).renaming.is_none());
}

#[test]
fn editing_the_current_profile_writes_git_config() {
    let mut st = two_profiles();
    let set = |field, value: &str| ProfileMsg::SetIdentity { id: WORK, field, value: value.into() };
    let fx = profile(&mut st, set(IdentityField::AuthorEmail, " ada@new.example "));
    assert_eq!(git_writes(&fx), vec![("user.email".to_owned(), Some("ada@new.example".to_owned()))]);
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profile(WORK).unwrap().author_email, "ada@new.example");
    assert_eq!(st.prefs.as_ref().unwrap().git.as_ref().unwrap().user_email, "ada@new.example");

    let fx = profile(&mut st, set(IdentityField::AuthorName, "  "));
    assert_eq!(git_writes(&fx), vec![("user.name".to_owned(), None)], "a blank name unsets it");

    let fx = profile(&mut st, set(IdentityField::SigningKey, "ABCD"));
    assert_eq!(git_writes(&fx), vec![("user.signingkey".to_owned(), Some("ABCD".to_owned()))]);
    let fx = profile(&mut st, set(IdentityField::SigningKey, ""));
    assert!(git_writes(&fx).is_empty(), "clearing the key leaves git's alone");
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.settings.profile(WORK).unwrap().signing_key, None);

    assert!(profile(&mut st, set(IdentityField::AuthorEmail, "ada@new.example")).is_empty(), "unchanged");
}

#[test]
fn editing_another_profile_only_saves() {
    let mut st = two_profiles();
    let fx = profile(&mut st, ProfileMsg::SetIdentity { id: PERSONAL, field: IdentityField::AuthorName, value: "Ada Lovelace".into() });
    assert!(git_writes(&fx).is_empty());
    assert_eq!(saves(&fx), 1);
    let fx = profile(&mut st, ProfileMsg::SetIdentity { id: PERSONAL, field: IdentityField::SigningKey, value: " ".into() });
    assert!(git_writes(&fx).is_empty());
    assert_eq!(st.settings.profile(PERSONAL).unwrap().signing_key, None);
    assert_eq!(st.prefs.as_ref().unwrap().git.as_ref().unwrap().user_name, "Ada", "the mirror is git's");
}

#[test]
fn deleting_asks_first_and_never_the_current_profile() {
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::AskDelete(WORK));
    assert!(st.modal.is_none());

    profile(&mut st, ProfileMsg::Select(PERSONAL));
    profile(&mut st, ProfileMsg::AskDelete(PERSONAL));
    match &st.modal {
        Some(Modal::Confirm { title, body, button, .. }) => {
            assert_eq!(title, "Delete profile");
            assert!(body.contains("\"Personal\"") && body.contains("can't be undone"), "{body}");
            assert_eq!(button, "Delete");
        }
        _ => panic!("expected a confirmation"),
    }
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(saves(&fx), 1);
    assert!(st.settings.profile(PERSONAL).is_none());
    assert_eq!(page(&st).selected, Some(WORK), "back to the current profile");
}

#[test]
fn switching_runs_one_at_a_time() {
    let mut st = two_profiles();
    let fx = profile(&mut st, ProfileMsg::Switch(PERSONAL));
    assert_eq!(st.profile_switch, Some(PERSONAL));
    match &fx[..] {
        [Effect::Profile(ProfileEffect::Apply { id, identity, .. })] => {
            assert_eq!(*id, PERSONAL);
            assert_eq!(identity, &st.settings.profile(PERSONAL).unwrap().identity());
        }
        other => panic!("expected the apply effect, got {other:?}"),
    }
    let fx = profile(&mut st, ProfileMsg::Switch(WORK));
    assert!(fx.is_empty());
    assert!(st.log.iter().any(|l| l.text == STILL_SWITCHING));
    // The switch's target can't be deleted meanwhile.
    profile(&mut st, ProfileMsg::AskDelete(PERSONAL));
    assert!(st.modal.is_none());
}

#[test]
fn a_switch_that_worked_makes_the_profile_current() {
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    let after = GitConfigValues { signing_key: "HOMEKEY".into(), ..values("Ada L", "ada@home.example") };
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Ok(after.clone()) });
    assert_eq!(st.profile_switch, None);
    assert_eq!(st.settings.active_profile, Some(PERSONAL));
    assert_eq!(saves(&fx), 1);
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Switched to profile Personal"));
    assert_eq!(st.prefs.as_ref().unwrap().git.as_ref(), Some(&after));
    assert_eq!(page(&st).selected, Some(PERSONAL));
    assert_eq!(page(&st).signing_key, "HOMEKEY");
}

#[test]
fn a_failed_switch_keeps_the_current_profile() {
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Err("could not lock config file".into()) });
    assert_eq!(st.profile_switch, None);
    assert_eq!(st.settings.active_profile, Some(WORK));
    let text = "Could not switch to profile Personal: could not lock config file";
    assert!(st.log.iter().any(|l| !l.ok && l.text == text));
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == text));
    assert!(matches!(&fx[..], [Effect::ReadGitConfig { .. }]), "{fx:?}");
}

fn rewrite(fx: &[Effect]) -> Option<(ProfileId, Identity)> {
    fx.iter().find_map(|e| match e {
        Effect::Profile(ProfileEffect::Rewrite { id, identity, .. }) => Some((*id, identity.clone())),
        _ => None,
    })
}

#[test]
fn config_read_during_a_switch_leaves_the_current_profile_alone() {
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    // Read while the switch had written Personal's name but not yet its email.
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values("Ada L", "ada@work.example"))));
    assert_eq!(saves(&fx), 0);
    let fx = profile(&mut st, ProfileMsg::ShowPage);
    assert_eq!(saves(&fx), 0);
    let work = st.settings.profile(WORK).unwrap();
    assert_eq!((work.author_name.as_str(), work.author_email.as_str()), ("Ada", "ada@work.example"));

    // The switch failed (git's earlier values were put back): the read afterwards syncs again.
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Err("could not lock config file".into()) });
    assert!(matches!(&fx[..], [Effect::ReadGitConfig { .. }]), "{fx:?}");
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values("Ada", "ada@work.example"))));
    assert_eq!(saves(&fx), 0, "nothing changed");
    assert_eq!(st.settings.profile(WORK).unwrap().author_name, "Ada");
}

#[test]
fn editing_the_current_profile_during_a_switch_waits_for_it() {
    // The Switch click and the field losing focus land in the same frame.
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    let edit = ProfileMsg::SetIdentity { id: WORK, field: IdentityField::AuthorEmail, value: "ada@new.example".into() };
    let fx = profile(&mut st, edit.clone());
    assert!(git_writes(&fx).is_empty(), "no write races the switch: {fx:?}");
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.prefs.as_ref().unwrap().git.as_ref().unwrap().user_email, "ada@work.example", "git's mirror stays");

    // The switch worked: Work isn't git's identity any more, so nothing more is written.
    let after = GitConfigValues { signing_key: "HOMEKEY".into(), ..values("Ada L", "ada@home.example") };
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Ok(after) });
    assert_eq!(rewrite(&fx), None, "{fx:?}");
    assert_eq!(st.profile_switch, None);
    assert_eq!(st.settings.profile(WORK).unwrap().author_email, "ada@new.example");
    assert_eq!(st.settings.profile(PERSONAL).unwrap().author_email, "ada@home.example");

    // The same edit, then a failed switch: Work stays current and its edited identity is written.
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    profile(&mut st, edit);
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Err("could not lock config file".into()) });
    assert_eq!(st.settings.active_profile, Some(WORK));
    let (id, identity) = rewrite(&fx).expect("a rewrite");
    assert_eq!((id, identity.email.as_str()), (WORK, "ada@new.example"));
    assert!(!fx.iter().any(|e| matches!(e, Effect::ReadGitConfig { .. })), "the rewrite reads back: {fx:?}");
    assert_eq!(st.profile_switch, Some(WORK), "busy until written");
    assert!(profile(&mut st, ProfileMsg::Switch(PERSONAL)).is_empty());

    let fx = profile(&mut st, ProfileMsg::Rewritten { id: WORK, result: Ok(values("Ada", "ada@new.example")) });
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.profile_switch, None);
    assert_eq!(st.prefs.as_ref().unwrap().git.as_ref().unwrap().user_email, "ada@new.example");
    assert_eq!(st.settings.profile(WORK).unwrap().author_email, "ada@new.example");
}

#[test]
fn editing_the_target_during_a_switch_writes_it_once_current() {
    let mut st = two_profiles();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    let fx = profile(&mut st, ProfileMsg::SetIdentity { id: PERSONAL, field: IdentityField::AuthorName, value: "Ada Lovelace".into() });
    assert!(git_writes(&fx).is_empty());
    // The switch wrote Personal's identity from before the edit.
    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Ok(values("Ada L", "ada@home.example")) });
    assert_eq!(st.settings.active_profile, Some(PERSONAL));
    let (id, identity) = rewrite(&fx).expect("a rewrite");
    assert_eq!((id, identity.name.as_str()), (PERSONAL, "Ada Lovelace"));
    // Its read-back doesn't undo the edit.
    run(&mut st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values("Ada L", "ada@home.example"))));
    assert_eq!(st.settings.profile(PERSONAL).unwrap().author_name, "Ada Lovelace");

    let fx = profile(&mut st, ProfileMsg::Rewritten { id: PERSONAL, result: Err("could not lock config file".into()) });
    assert_eq!(st.profile_switch, None);
    let text = "Could not write profile Personal's identity to your Git config: could not lock config file";
    assert!(st.log.iter().any(|l| !l.ok && l.text == text));
    assert!(matches!(&fx[..], [Effect::ReadGitConfig { .. }]), "{fx:?}");
}
