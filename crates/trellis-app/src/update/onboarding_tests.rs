//! Headless tests for the first-run welcome (startup, the identity step's read, checks and
//! write, step navigation, closing and finishing) and the tour (steps, resume, starting after a
//! repository opens) and their palette entries. No git config is read or written: the effects
//! are checked, and their results sent by hand.

use super::tests::{scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{Msg, PaletteMsg, PrefsMsg, RepoMsg};
use crate::onboarding::{OnboardingEffect, OnboardingMsg as O, StartChoice};
use crate::state::onboarding::{SAVED_LOG, TOUR_DONE, WelcomeStep};
use crate::state::onboarding_tour::TourStep;
use crate::state::palette::PaletteCommand as C;
use crate::state::palette::entries;
use crate::state::{AppState, Modal, PrefPage};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::i18n::Lang;
use trellis_core::identity::Identity;
use trellis_core::repo::{self, LoadOptions};
use trellis_core::settings::{Profile, ProfileId};

fn run(st: &mut AppState, m: O) -> Vec<Effect> {
    update(st, Msg::Onboarding(m), Instant::now())
}

fn id(name: &str, email: &str, key: Option<&str>) -> Identity {
    Identity { name: name.into(), email: email.into(), signing_key: key.map(str::to_owned) }
}

/// A first run with the welcome open and git's identity read as `git`.
fn welcome(git: Identity) -> AppState {
    let mut st = state();
    run(&mut st, O::Startup);
    run(&mut st, O::IdentityRead(Ok(git)));
    st
}

fn step(st: &AppState) -> Option<WelcomeStep> {
    st.onboarding.welcome.as_ref().map(|w| w.step)
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::SaveSettings(_)))
}

/// The first Ok line in the activity log, if any.
fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text == text)
}

/// A state with the scratch repository loaded, as after opening it.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.settings.welcome_done = true;
    load(&mut st, &dir, LoadKind::Open);
    (st, dir)
}

fn load(st: &mut AppState, dir: &std::path::Path, kind: LoadKind) {
    let snap = Box::new(repo::load(dir, &LoadOptions::all(100)).unwrap());
    st.loading = true;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind, result: Ok(snap) });
    update(st, m, Instant::now());
}

#[test]
fn startup_opens_the_welcome_on_a_first_run() {
    let mut st = state();
    assert!(!st.settings.welcome_done && st.onboarding.welcome.is_none(), "AppState::new alone never opens it");
    let fx = run(&mut st, O::Startup);
    assert!(matches!(fx.as_slice(), [Effect::Onboarding(OnboardingEffect::ReadIdentity { .. })]));
    let w = st.onboarding.welcome.as_ref().unwrap();
    assert_eq!(w.step, WelcomeStep::Identity);
    assert!(w.identity.reading && w.tour_after);
}

#[test]
fn startup_does_nothing_for_an_existing_user() {
    let mut st = state();
    st.settings.welcome_done = true;
    assert!(run(&mut st, O::Startup).is_empty());
    assert!(st.onboarding.welcome.is_none());
}

#[test]
fn a_set_identity_seeds_the_fields_and_writes_nothing() {
    let mut st = welcome(id(" Ada ", "ada@example.com", Some("K")));
    let f = &st.onboarding.welcome.as_ref().unwrap().identity;
    assert_eq!((f.name.as_str(), f.email.as_str(), f.reading), ("Ada", "ada@example.com", false));
    assert!(run(&mut st, O::SaveIdentity).is_empty());
    assert_eq!(step(&st), Some(WelcomeStep::Connect));
}

#[test]
fn a_missing_identity_is_written_to_the_global_config() {
    let mut st = welcome(Identity::default());
    let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
    f.name = " Ada ".into();
    f.email = "ada@example.com".into();
    let fx = run(&mut st, O::Next);
    let [Effect::Onboarding(OnboardingEffect::WriteIdentity { identity, .. })] = fx.as_slice() else { panic!("one write") };
    assert_eq!(identity, &id("Ada", "ada@example.com", None));
    let written = identity.clone();
    let w = st.onboarding.welcome.as_ref().unwrap();
    assert!(w.identity.saving && w.step == WelcomeStep::Identity);
    assert!(run(&mut st, O::SaveIdentity).is_empty(), "not twice while saving");
    let fx = run(&mut st, O::IdentityWritten { identity: written, result: Ok(()) });
    assert!(fx.is_empty(), "no profiles, nothing to save");
    assert_eq!(step(&st), Some(WelcomeStep::Connect));
    assert!(logged(&st, SAVED_LOG));
    assert!(!st.settings.welcome_done && st.settings.profiles.is_empty(), "no profile is made");
}

#[test]
fn a_bad_identity_is_refused() {
    let en = |k| Some(Lang::En.tr(k));
    let cases = [
        ("", "a@b", (en("welcome.identity.name_blank"), None)),
        ("Ada", " ", (None, en("welcome.identity.email_blank"))),
        ("Ada", "ada", (None, en("welcome.identity.email_no_at"))),
        ("Ada", "ada @x", (None, en("welcome.identity.email_space"))),
    ];
    for (name, email, problems) in cases {
        let mut st = welcome(Identity::default());
        let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
        f.name = name.into();
        f.email = email.into();
        assert!(run(&mut st, O::SaveIdentity).is_empty(), "{email}");
        let f = &st.onboarding.welcome.as_ref().unwrap().identity;
        assert_eq!((f.error.as_deref(), f.saving), (None, false), "field problems stay out of the form's error");
        assert!(f.touched.shows("name") && f.touched.shows("email"), "Continue makes every problem show");
        assert_eq!(f.field_problems(Lang::En), problems, "{name} / {email}");
    }
}

#[test]
fn nothing_complains_before_the_user_types() {
    let mut st = welcome(Identity::default());
    let f = &st.onboarding.welcome.as_ref().unwrap().identity;
    assert!(f.field_problems(Lang::En).1.is_some(), "the blank email has a problem");
    assert!(!f.touched.shows("email") && !f.touched.shows("name"), "but it doesn't show yet");
    // An unchanged blank form just moves on; a name without an email is refused.
    st.onboarding.welcome.as_mut().unwrap().identity.name = "Ada".into();
    run(&mut st, O::SaveIdentity);
    assert_eq!(step(&st), Some(WelcomeStep::Identity));
    assert!(st.onboarding.welcome.as_ref().unwrap().identity.touched.shows("email"));
}

#[test]
fn the_greeting_uses_the_name_git_holds() {
    let mut st = welcome(Identity::default());
    let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
    (f.name, f.email) = ("Ada Lovelace".into(), "ada@example.com".into());
    run(&mut st, O::SaveIdentity);
    assert_eq!(st.onboarding.welcome.as_ref().unwrap().greeting_name(), None, "still on the identity step");
    run(&mut st, O::IdentityWritten { identity: id("Ada Lovelace", "ada@example.com", None), result: Ok(()) });
    assert_eq!(st.onboarding.welcome.as_ref().unwrap().greeting_name(), Some("Ada Lovelace"));

    // A typed name skipped without saving: git's old name, or none.
    let mut st = welcome(id("Ada", "ada@example.com", None));
    st.onboarding.welcome.as_mut().unwrap().identity.name = "Augusta".into();
    run(&mut st, O::Skip);
    assert_eq!(st.onboarding.welcome.as_ref().unwrap().greeting_name(), Some("Ada"));
    let mut st = welcome(Identity::default());
    st.onboarding.welcome.as_mut().unwrap().identity.name = "Augusta".into();
    run(&mut st, O::Skip);
    assert_eq!(st.onboarding.welcome.as_ref().unwrap().greeting_name(), None);
}

#[test]
fn a_failed_write_stays_with_the_error() {
    let mut st = welcome(Identity::default());
    let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
    (f.name, f.email) = ("Ada".into(), "ada@example.com".into());
    run(&mut st, O::SaveIdentity);
    run(&mut st, O::IdentityWritten { identity: id("Ada", "ada@example.com", None), result: Err("could not lock config file".into()) });
    let w = st.onboarding.welcome.as_ref().unwrap();
    assert_eq!((w.step, w.identity.saving), (WelcomeStep::Identity, false));
    assert_eq!(w.identity.error.as_deref(), Some("could not lock config file"));
    run(&mut st, O::SaveIdentity);
    let f = &st.onboarding.welcome.as_ref().unwrap().identity;
    assert!(f.saving && f.error.is_none(), "the next try clears it");
}

#[test]
fn a_failed_read_still_writes() {
    let mut st = state();
    run(&mut st, O::Startup);
    run(&mut st, O::IdentityRead(Err("git could not run".into())));
    let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
    assert_eq!((f.read_error.as_deref(), f.reading, f.name.as_str()), (Some("git could not run"), false, ""));
    assert!(run(&mut st, O::SaveIdentity).is_empty(), "blank fields are refused");
    let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
    (f.name, f.email) = ("Ada".into(), "ada@example.com".into());
    assert!(matches!(run(&mut st, O::SaveIdentity).as_slice(), [Effect::Onboarding(OnboardingEffect::WriteIdentity { .. })]));
}

#[test]
fn a_write_syncs_the_current_profile() {
    let mut st = welcome(id("Ada", "ada@old.example", Some("K")));
    let p = Profile {
        id: ProfileId(1),
        name: "Work".into(),
        author_name: "Ada".into(),
        author_email: "ada@old.example".into(),
        signing_key: Some("K".into()),
        ..Profile::default()
    };
    st.settings.profiles.push(p);
    st.settings.active_profile = Some(ProfileId(1));
    st.onboarding.welcome.as_mut().unwrap().identity.email = "ada@new.example".into();
    run(&mut st, O::SaveIdentity);
    let fx = run(&mut st, O::IdentityWritten { identity: id("Ada", "ada@new.example", None), result: Ok(()) });
    assert!(saves(&fx));
    let p = &st.settings.profiles[0];
    assert_eq!((p.author_email.as_str(), p.signing_key.as_deref()), ("ada@new.example", Some("K")), "the key stays");
}

#[test]
fn no_write_during_a_profile_switch() {
    let mut st = welcome(Identity::default());
    let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
    (f.name, f.email) = ("Ada".into(), "ada@example.com".into());
    st.profile_switch = Some(ProfileId(2));
    assert!(run(&mut st, O::SaveIdentity).is_empty());
    let f = &st.onboarding.welcome.as_ref().unwrap().identity;
    assert_eq!((f.error.as_deref(), f.saving), (Some(Lang::En.tr("welcome.identity.not_during_switch")), false));
    st.profile_switch = None;
    assert!(matches!(run(&mut st, O::SaveIdentity).as_slice(), [Effect::Onboarding(OnboardingEffect::WriteIdentity { .. })]));
    assert!(st.onboarding.welcome.as_ref().unwrap().identity.error.is_none(), "the next try clears it");
}

#[test]
fn step_navigation() {
    let mut st = welcome(Identity::default());
    assert!(run(&mut st, O::Skip).is_empty());
    assert_eq!(step(&st), Some(WelcomeStep::Connect));
    run(&mut st, O::Next);
    assert_eq!(step(&st), Some(WelcomeStep::Git));
    assert!(run(&mut st, O::Skip).is_empty());
    assert_eq!(step(&st), Some(WelcomeStep::Git), "no skip on the git step");
    run(&mut st, O::Next);
    assert_eq!(step(&st), Some(WelcomeStep::Start));
    run(&mut st, O::Next);
    run(&mut st, O::Skip);
    assert_eq!(step(&st), Some(WelcomeStep::Start), "Next and Skip do nothing on the last step");
    for expected in [WelcomeStep::Git, WelcomeStep::Connect, WelcomeStep::Identity, WelcomeStep::Identity] {
        run(&mut st, O::Back);
        assert_eq!(step(&st), Some(expected));
    }
}

#[test]
fn the_welcome_waits_behind_preferences() {
    let mut st = welcome(Identity::default());
    run(&mut st, O::Skip);
    update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)), Instant::now());
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Integrations));
    update(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs), Instant::now());
    assert!(st.prefs.is_none());
    assert_eq!(step(&st), Some(WelcomeStep::Connect));
}

#[test]
fn closing_counts_as_seen() {
    let mut st = welcome(Identity::default());
    let fx = run(&mut st, O::CloseWelcome);
    assert!(saves(&fx) && st.settings.welcome_done && st.onboarding.welcome.is_none());
    let fx = run(&mut st, O::ShowWelcome);
    assert!(matches!(fx.as_slice(), [Effect::Onboarding(OnboardingEffect::ReadIdentity { .. })]));
    assert_eq!(step(&st), Some(WelcomeStep::Identity));
    assert!(!saves(&run(&mut st, O::CloseWelcome)), "already seen: nothing to save");
}

#[test]
fn finish_runs_the_choice() {
    let mut st = welcome(Identity::default());
    let fx = run(&mut st, O::Finish(Some(StartChoice::Clone)));
    assert!(saves(&fx) && st.settings.welcome_done && st.onboarding.welcome.is_none());
    assert!(matches!(st.modal, Some(Modal::Clone(_))));

    let mut st = welcome(Identity::default());
    let fx = run(&mut st, O::Finish(Some(StartChoice::Open)));
    assert!(fx.iter().any(|e| matches!(e, Effect::PickFolder { .. })));

    let mut st = welcome(Identity::default());
    run(&mut st, O::Finish(Some(StartChoice::Init)));
    assert!(matches!(st.modal, Some(Modal::Init(_))));

    let mut st = welcome(Identity::default());
    let fx = run(&mut st, O::Finish(None));
    assert!(saves(&fx) && fx.len() == 1 && st.modal.is_none() && st.onboarding.welcome.is_none());
}

#[test]
fn an_armed_tour_starts_when_a_repository_opens() {
    let dir = scratch_repo("onboarding-armed");
    let mut st = welcome(Identity::default());
    run(&mut st, O::Finish(Some(StartChoice::Open)));
    assert!(st.onboarding.tour_after_open);
    load(&mut st, &dir, LoadKind::Refresh);
    assert_eq!(st.onboarding.tour, None, "a refresh is not an open");
    load(&mut st, &dir, LoadKind::Open);
    assert_eq!(st.onboarding.tour, Some(TourStep::Toolbar));
    assert!(!st.onboarding.tour_after_open, "once");

    let mut st = welcome(Identity::default());
    st.onboarding.welcome.as_mut().unwrap().tour_after = false;
    run(&mut st, O::Finish(Some(StartChoice::Open)));
    load(&mut st, &dir, LoadKind::Open);
    assert_eq!(st.onboarding.tour, None, "the checkbox was off");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tour_steps_and_resume() {
    let mut st = state();
    run(&mut st, O::StartTour);
    assert_eq!(st.onboarding.tour, None, "no repository");
    let (mut st, dir) = opened("onboarding-tour");
    run(&mut st, O::StartTour);
    assert_eq!(st.onboarding.tour, Some(TourStep::Toolbar));
    run(&mut st, O::TourBack);
    assert_eq!(st.onboarding.tour, Some(TourStep::Toolbar));
    for _ in 0..4 {
        run(&mut st, O::TourNext);
    }
    assert_eq!(st.onboarding.tour, Some(TourStep::Palette));
    run(&mut st, O::TourBack);
    run(&mut st, O::TourBack);
    assert_eq!(st.onboarding.tour, Some(TourStep::Graph));
    run(&mut st, O::CloseTour);
    assert_eq!((st.onboarding.tour, st.onboarding.tour_resume), (None, Some(TourStep::Graph)));
    run(&mut st, O::StartTour);
    assert_eq!((st.onboarding.tour, st.onboarding.tour_resume), (Some(TourStep::Graph), None));
    run(&mut st, O::TourNext);
    run(&mut st, O::TourNext);
    run(&mut st, O::TourNext);
    assert_eq!((st.onboarding.tour, st.onboarding.tour_resume), (None, None));
    assert!(st.toasts.iter().any(|t| t.text == TOUR_DONE));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn show_welcome_closes_the_tour() {
    let (mut st, dir) = opened("onboarding-reopen");
    st.onboarding.tour = Some(TourStep::Details);
    run(&mut st, O::ShowWelcome);
    assert_eq!((st.onboarding.tour, st.onboarding.tour_resume), (None, Some(TourStep::Details)));
    assert_eq!(step(&st), Some(WelcomeStep::Identity));
    run(&mut st, O::StartTour);
    assert!(st.onboarding.welcome.is_none(), "the tour closes the welcome");
    assert_eq!(st.onboarding.tour, Some(TourStep::Details));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn palette_entries() {
    let labels = |st: &AppState| entries(st).into_iter().map(|e| (e.label, e.detail)).collect::<Vec<_>>();
    let mut st = state();
    st.settings.welcome_done = true;
    let l = labels(&st);
    assert!(l.contains(&("Show the welcome".to_owned(), None)));
    assert!(!l.iter().any(|(label, _)| label.contains("tour")), "no tour without a repository");
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Onboarding(O::ShowWelcome))), Instant::now());
    assert_eq!(step(&st), Some(WelcomeStep::Identity));
    assert_eq!(st.palette_recent.keys(), &["show-welcome"]);

    let (mut st, dir) = opened("onboarding-palette");
    assert!(labels(&st).contains(&("Take the tour".to_owned(), None)));
    st.onboarding.tour = Some(TourStep::Graph);
    run(&mut st, O::CloseTour);
    assert!(labels(&st).contains(&("Resume the tour".to_owned(), Some("step 3 of 5".to_owned()))));
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Onboarding(O::StartTour))), Instant::now());
    assert_eq!(st.onboarding.tour, Some(TourStep::Graph));
    assert_eq!(st.palette_recent.keys(), &["tour"]);
    let _ = std::fs::remove_dir_all(&dir);
}
