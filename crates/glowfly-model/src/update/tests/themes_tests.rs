//! Headless tests for a retired theme: the settings named one, so the first themes load shows
//! one toast naming the theme that took its place, with Choose a theme, and saves the settings.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg};
use crate::state::{AppState, PrefPage};
use crate::themes::ThemeMsg;
use glowfly_core::settings::{BuiltinTheme, ThemeCatalog, ThemeChoice};
use std::time::Instant;

fn loaded(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::Theme(ThemeMsg::Loaded(ThemeCatalog::default())), Instant::now())
}

#[test]
fn a_retired_theme_is_announced_once() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Midnight;
    st.settings.retired_theme = Some(("Garden Dark".to_owned(), BuiltinTheme::Midnight));
    let fx = loaded(&mut st);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.theme == ThemeChoice::Midnight && s.retired_theme.is_none()), "{fx:?}");
    assert_eq!(st.toasts.len(), 1);
    let toast = &st.toasts[0];
    assert_eq!(toast.text, "Garden Dark is no longer included; Glowfly now uses Midnight.");
    let action = toast.action.as_ref().expect("Choose a theme");
    assert_eq!(action.label, "Choose a theme");
    assert!(matches!(*action.msg, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ui))));
    // A Reload, or the next load, says nothing more.
    assert!(loaded(&mut st).is_empty());
    assert_eq!(st.toasts.len(), 1);
}

#[test]
fn choose_a_theme_opens_appearance() {
    let mut st = state();
    st.settings.retired_theme = Some(("Sand".to_owned(), BuiltinTheme::Porcelain));
    loaded(&mut st);
    let id = st.toasts[0].id;
    update(&mut st, Msg::ToastAction(id), Instant::now());
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Ui));
}

#[test]
fn no_retired_theme_no_toast() {
    let mut st = state();
    assert!(loaded(&mut st).is_empty());
    assert!(st.toasts.is_empty());
}
