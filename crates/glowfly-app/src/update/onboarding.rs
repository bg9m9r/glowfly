//! The first-run welcome (identity, hosting service, git check, start) and the tour of the main
//! window. The welcome reads and writes only user.name and user.email in the global git config;
//! closing or finishing it sets `settings.welcome_done`, the one thing saved.

use crate::effect::{Effect, LoadKind};
use crate::onboarding::{OnboardingEffect, OnboardingMsg, StartChoice};
use crate::state::AppState;
use crate::state::onboarding::{SAVED_LOG, TOUR_DONE, Welcome, WelcomeStep, identity_problem};
use crate::state::onboarding_tour::TourStep;
use glowfly_core::identity::Identity;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: OnboardingMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        OnboardingMsg::Startup => {
            if !st.settings.welcome_done {
                open(st, fx);
            }
        }
        OnboardingMsg::ShowWelcome => {
            close_tour(st);
            open(st, fx);
        }
        OnboardingMsg::IdentityRead(result) => {
            let Some(f) = st.onboarding.welcome.as_mut().map(|w| &mut w.identity).filter(|f| f.reading) else { return };
            match result {
                Ok(id) => f.seed(id),
                Err(e) => {
                    f.read_error = Some(e);
                    f.reading = false;
                }
            }
        }
        OnboardingMsg::Next => match st.onboarding.welcome.as_ref().map(|w| w.step) {
            Some(WelcomeStep::Identity) => save_identity(st, fx),
            Some(step) => go(st, step.next()),
            None => {}
        },
        OnboardingMsg::Back => {
            let prev = st.onboarding.welcome.as_ref().and_then(|w| w.step.prev());
            go(st, prev);
        }
        OnboardingMsg::Skip => match st.onboarding.welcome.as_ref().map(|w| w.step) {
            Some(step @ (WelcomeStep::Identity | WelcomeStep::Connect)) => go(st, step.next()),
            Some(WelcomeStep::Git | WelcomeStep::Start) | None => {}
        },
        OnboardingMsg::SaveIdentity => save_identity(st, fx),
        OnboardingMsg::IdentityWritten { identity, result } => written(st, identity, result, fx),
        OnboardingMsg::Finish(choice) => {
            let Some(w) = &st.onboarding.welcome else { return };
            let armed = w.tour_after && st.snap.is_none();
            finish(st, fx);
            if let Some(choice) = choice {
                st.onboarding.tour_after_open = armed;
                start(st, choice, now, fx);
            }
        }
        OnboardingMsg::CloseWelcome => finish(st, fx),
        OnboardingMsg::StartTour => {
            if st.snap.is_none() {
                return;
            }
            if st.onboarding.welcome.is_some() {
                finish(st, fx);
            }
            let o = &mut st.onboarding;
            o.tour = Some(o.tour_resume.take().unwrap_or(TourStep::Toolbar));
            o.tour_after_open = false;
        }
        OnboardingMsg::TourNext => match st.onboarding.tour.map(|s| s.next()) {
            Some(Some(next)) => st.onboarding.tour = Some(next),
            Some(None) => {
                st.onboarding.tour = None;
                st.onboarding.tour_resume = None;
                st.toast(true, TOUR_DONE, now);
            }
            None => {}
        },
        OnboardingMsg::TourBack => {
            if let Some(prev) = st.onboarding.tour.and_then(TourStep::prev) {
                st.onboarding.tour = Some(prev);
            }
        }
        OnboardingMsg::CloseTour => close_tour(st),
    }
}

/// A repository finished loading: start the tour the welcome armed, once.
pub(super) fn loaded(st: &mut AppState, kind: LoadKind) {
    if kind == LoadKind::Open && st.onboarding.tour_after_open {
        st.onboarding.tour_after_open = false;
        st.onboarding.tour = Some(TourStep::Toolbar);
        st.onboarding.tour_resume = None;
    }
}

/// Opens the welcome at the first step and reads git's identity for it.
fn open(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.onboarding.welcome = Some(Welcome::new());
    fx.push(Effect::Onboarding(OnboardingEffect::ReadIdentity { cwd: AppState::outside_dir() }));
}

/// Moves the welcome to `step`, when there is one.
fn go(st: &mut AppState, step: Option<WelcomeStep>) {
    if let (Some(w), Some(step)) = (st.onboarding.welcome.as_mut(), step) {
        w.step = step;
    }
}

/// Closes the welcome; the first time, remembers that it was seen.
fn finish(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.onboarding.welcome = None;
    if !st.settings.welcome_done {
        st.settings.welcome_done = true;
        fx.push(super::prefs::save_settings(st));
    }
}

/// Runs the last step's choice as the welcome screen's button would.
fn start(st: &mut AppState, choice: StartChoice, now: Instant, fx: &mut Vec<Effect>) {
    fx.extend(super::update(st, choice.msg(), now));
}

/// Ends the tour, remembering its step for "Resume the tour" unless it was the last.
fn close_tour(st: &mut AppState) {
    if let Some(step) = st.onboarding.tour.take() {
        st.onboarding.tour_resume = step.next().map(|_| step);
    }
}

/// The identity step's Continue: advances when nothing changed, else checks the fields and
/// writes them to the global config.
fn save_identity(st: &mut AppState, fx: &mut Vec<Effect>) {
    let switching = st.profile_switch.is_some();
    let t = st.settings.lang();
    let Some(w) = st.onboarding.welcome.as_mut().filter(|w| w.step == WelcomeStep::Identity) else { return };
    let f = &mut w.identity;
    if f.saving || f.reading {
        return;
    }
    // After a failed read there is nothing to compare with, so the fields are always written.
    if !f.needs_write() && f.read_error.is_none() {
        f.error = None;
        w.step = WelcomeStep::Connect;
        return;
    }
    // A field's problem shows under it (the view works it out from the text); Continue only
    // makes every field's problem show.
    if identity_problem(t, &f.name, &f.email).is_some() {
        f.touched.attempt();
        f.error = None;
        return;
    }
    if switching {
        f.error = Some(t.tr("welcome.identity.not_during_switch").into());
        return;
    }
    f.saving = true;
    f.error = None;
    let identity = Identity { name: f.name.trim().to_owned(), email: f.email.trim().to_owned(), signing_key: None };
    fx.push(Effect::Onboarding(OnboardingEffect::WriteIdentity { cwd: AppState::outside_dir(), identity }));
}

/// The identity write finished: log it, keep the current profile in step and move on, or show
/// git's error on the step.
fn written(st: &mut AppState, identity: Identity, result: Result<(), String>, fx: &mut Vec<Effect>) {
    let waiting = st.onboarding.welcome.as_mut().filter(|w| w.identity.saving);
    match result {
        Ok(()) => {
            if let Some(w) = waiting {
                let f = &mut w.identity;
                f.had = Identity { signing_key: f.had.signing_key.take(), ..identity.clone() };
                f.saving = false;
                f.read_error = None;
                w.step = WelcomeStep::Connect;
            }
            st.log(true, SAVED_LOG);
            if !st.settings.profiles.is_empty() {
                // The profile's own key: the welcome never changes signing.
                let s = &st.settings;
                let key = s.active_profile().or(s.profiles.first()).and_then(|p| p.signing_key.clone());
                if st.settings.seed_or_sync_profiles(&Identity { signing_key: key, ..identity }) {
                    fx.push(super::prefs::save_settings(st));
                }
            }
        }
        Err(e) => match waiting {
            Some(w) => {
                w.identity.saving = false;
                w.identity.error = Some(e);
            }
            None => st.log(false, format!("Could not save your name and email: {e}")),
        },
    }
}
