//! Headless tests for Preferences > Accessibility: the High contrast check's round trip, how
//! long toasts stay with each Notifications stay choice (and failures with a fix or Details
//! always), what holds a toast, and the activity line every toast leaves.

use super::tests::state;
use super::update;
use crate::msg::{Msg, NotifyMsg, PrefsMsg};
use crate::state::toasts::ToastAction;
use crate::state::{AppState, Modal};
use crate::themes::ThemeMsg;
use glowfly_core::settings::{Profile, ProfileId, ProfilePrefs, ThemeChoice, ToastTime};
use std::time::{Duration, Instant};

fn prefs(st: &mut AppState, msg: PrefsMsg, now: Instant) {
    update(st, Msg::Prefs(msg), now);
}

fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

#[test]
fn high_contrast_off_restores_the_exact_previous_theme() {
    let (mut st, now) = (state(), Instant::now());
    st.settings.theme = ThemeChoice::Custom("ocean".into());
    prefs(&mut st, PrefsMsg::SetHighContrast { on: true, dark: false }, now);
    assert_eq!(st.settings.theme, ThemeChoice::HighContrastLight, "as dark as the palette in effect");
    // Saving or editing another setting keeps what turning it off brings back.
    prefs(&mut st, PrefsMsg::SetToolbarLabels(false), now);
    prefs(&mut st, PrefsMsg::SettingsEdited, now);
    prefs(&mut st, PrefsMsg::SetHighContrast { on: false, dark: false }, now);
    assert_eq!(st.settings.theme, ThemeChoice::Custom("ocean".into()));
    assert_eq!(st.settings.theme_before_contrast, None);
}

#[test]
fn picking_another_theme_forgets_the_saved_one() {
    let (mut st, now) = (state(), Instant::now());
    st.settings.theme = ThemeChoice::Mist;
    prefs(&mut st, PrefsMsg::SetHighContrast { on: true, dark: true }, now);
    assert_eq!(st.settings.theme, ThemeChoice::HighContrastDark);
    prefs(&mut st, PrefsMsg::SetTheme(ThemeChoice::Porcelain), now);
    assert_eq!(st.settings.theme_before_contrast, None, "Mist is no longer the theme to bring back");
    prefs(&mut st, PrefsMsg::SetTheme(ThemeChoice::HighContrastLight), now);
    prefs(&mut st, PrefsMsg::SetHighContrast { on: false, dark: false }, now);
    assert_eq!(st.settings.theme, ThemeChoice::Porcelain, "nothing saved: the default light theme");
}

/// A finished switch to profile `id`, as the runner reports it.
fn switched_to(st: &mut AppState, id: u32, now: Instant) {
    let result = Ok(crate::state::prefs::GitConfigValues::default());
    update(st, Msg::Profile(crate::profile::ProfileMsg::Applied { id: ProfileId(id), result }), now);
}

#[test]
fn a_profile_switch_keeps_each_profiles_theme_to_bring_back() {
    let (mut st, now) = (state(), Instant::now());
    for (id, name) in [(1, "A"), (2, "B")] {
        st.settings.profiles.push(Profile { id: ProfileId(id), name: name.into(), ..Profile::default() });
    }
    st.settings.active_profile = Some(ProfileId(1));
    st.settings.profiles[1].prefs = Some(ProfilePrefs { theme: ThemeChoice::Porcelain, ..ProfilePrefs::default() });
    st.settings.theme = ThemeChoice::Mist;
    prefs(&mut st, PrefsMsg::SetHighContrast { on: true, dark: true }, now);
    switched_to(&mut st, 2, now);
    assert_eq!((st.settings.theme.clone(), st.settings.theme_before_contrast.clone()), (ThemeChoice::Porcelain, None), "B's own");
    // B turns High contrast on and off: B's theme comes back, not A's.
    prefs(&mut st, PrefsMsg::SetHighContrast { on: true, dark: false }, now);
    prefs(&mut st, PrefsMsg::SetHighContrast { on: false, dark: false }, now);
    assert_eq!(st.settings.theme, ThemeChoice::Porcelain);
    prefs(&mut st, PrefsMsg::SettingsEdited, now);
    switched_to(&mut st, 1, now);
    assert_eq!(st.settings.theme, ThemeChoice::HighContrastDark, "A's theme, still high contrast");
    prefs(&mut st, PrefsMsg::SetHighContrast { on: false, dark: true }, now);
    assert_eq!(st.settings.theme, ThemeChoice::Mist, "what A's High contrast replaced");
}

#[test]
fn show_all_shortcuts_opens_the_sheet_over_preferences() {
    let (mut st, now) = (state(), Instant::now());
    prefs(&mut st, PrefsMsg::OpenPrefs(crate::state::PrefPage::Accessibility), now);
    prefs(&mut st, PrefsMsg::ShowShortcutSheet, now);
    assert!(matches!(st.modal, Some(Modal::Shortcuts { .. })));
    assert!(st.prefs.is_some(), "Preferences stays underneath");
}

/// A plain toast and one with a button, shown at `t0` with `time` chosen.
fn two_toasts(time: ToastTime) -> (AppState, Instant) {
    let (mut st, t0) = (state(), Instant::now());
    st.settings.a11y.toast_time = time;
    st.toast(true, "plain", t0);
    let action = ToastAction { label: "Pull".into(), msg: Box::new(Msg::ClearLog), workdir: None };
    st.toast_action(true, "with a button", action, t0);
    (st, t0)
}

fn texts(st: &AppState) -> Vec<&str> {
    st.toasts.iter().map(|t| t.text.as_str()).collect()
}

#[test]
fn toast_lifetimes_follow_notifications_stay() {
    let (mut st, t0) = two_toasts(ToastTime::Standard);
    st.expire_toasts(t0 + secs(5));
    assert_eq!(texts(&st), ["with a button"]);
    st.expire_toasts(t0 + secs(10));
    assert!(st.toasts.is_empty());

    let (mut st, t0) = two_toasts(ToastTime::Longer);
    st.expire_toasts(t0 + secs(14));
    assert_eq!(st.toasts.len(), 2);
    st.expire_toasts(t0 + secs(15));
    assert_eq!(texts(&st), ["with a button"]);
    st.expire_toasts(t0 + secs(29));
    assert_eq!(st.toasts.len(), 1);
    st.expire_toasts(t0 + secs(30));
    assert!(st.toasts.is_empty());

    let (mut st, t0) = two_toasts(ToastTime::UntilClosed);
    st.expire_toasts(t0 + secs(600));
    assert_eq!(st.toasts.len(), 2, "ten minutes on, both stay");
    assert_eq!(st.toasts[0].opacity(t0 + secs(600), ToastTime::UntilClosed), 1.0, "and never fade");
    let id = st.toasts[0].id;
    update(&mut st, Msg::Notify(NotifyMsg::CloseToast(id)), t0 + secs(600));
    assert_eq!(texts(&st), ["with a button"], "closing one is the way it goes");
}

#[test]
fn a_failure_with_a_fix_or_details_never_expires() {
    let (mut st, t0) = (state(), Instant::now());
    st.toast(false, "Push failed", t0);
    st.set_last_toast_action(ToastAction { label: "Details".into(), msg: Box::new(Msg::ClearLog), workdir: None });
    st.toast(false, "Plain failure", t0);
    st.expire_toasts(t0 + secs(3600));
    assert_eq!(texts(&st), ["Push failed"], "whatever Notifications stay says");
}

#[test]
fn holding_a_toast_keeps_it_and_letting_go_leaves_some_time() {
    let (mut st, t0) = two_toasts(ToastTime::Standard);
    let id = st.toasts[0].id;
    // The pointer, or the keyboard focus (the view sends the same message for both).
    update(&mut st, Msg::Notify(NotifyMsg::HoldToast(Some(id))), t0 + secs(4));
    st.expire_toasts(t0 + secs(600));
    assert_eq!(st.toasts.len(), 1, "the held toast stays, the other went");
    update(&mut st, Msg::Notify(NotifyMsg::HoldToast(None)), t0 + secs(600));
    st.expire_toasts(t0 + secs(601));
    assert_eq!(st.toasts.len(), 1);
    st.expire_toasts(t0 + secs(602));
    assert!(st.toasts.is_empty());
}

#[test]
fn every_toast_leaves_one_activity_line() {
    let (mut st, now) = (state(), Instant::now());
    // A success toast, which nothing else logs.
    update(&mut st, Msg::Theme(ThemeMsg::Created(Ok(("ocean".into(), "ocean.json".into())))), now);
    assert_eq!(st.toasts.len(), 1);
    let lines: Vec<(bool, &str)> = st.log.iter().map(|e| (e.ok, e.text.as_str())).collect();
    assert_eq!(lines, [(true, st.toasts[0].text.as_str())]);
    // A failure: its log line is announced by a toast, and logged once.
    update(&mut st, Msg::Theme(ThemeMsg::Created(Err("disk full".into()))), now);
    assert_eq!(st.toasts.len(), 2);
    assert_eq!(st.log.len(), 2, "{:?}", st.log.iter().map(|e| &e.text).collect::<Vec<_>>());
    // A toast the limit drops keeps its line.
    for i in 0..4 {
        update(&mut st, Msg::Theme(ThemeMsg::Created(Ok((format!("t{i}"), "t.json".into())))), now);
    }
    assert_eq!(st.toasts.len(), 4);
    assert!(st.log.iter().any(|e| e.text.contains("ocean.json")), "the first toast's line stays");
}
