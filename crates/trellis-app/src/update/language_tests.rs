//! Headless tests for the Language setting: text built in update follows it, and choosing a
//! language on Preferences > General is saved like any other edit made in place.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{CommitMsg, Msg, PrefsMsg};
use crate::state::{PrefPage, PrefsState};
use std::time::Instant;
use trellis_core::i18n::Lang;

#[test]
fn the_commit_refusal_toast_is_in_the_chosen_language() {
    let mut st = state();
    st.settings.language = "de".into();
    st.snap = None;
    update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now());
    let text = st.toasts.last().map(|t| t.text.clone()).expect("a refusal toast");
    assert_eq!(text, Lang::De.tr("commit.blocked.no_repo"));
    assert_ne!(text, Lang::En.tr("commit.blocked.no_repo"));
}

#[test]
fn choosing_a_language_saves_it() {
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    st.settings.language = "es".into();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    let saved = fx.iter().find_map(|e| match e {
        Effect::SaveSettings(s) => Some(s.clone()),
        _ => None,
    });
    let saved = saved.expect("the settings are saved");
    assert_eq!(saved.language, "es");
    assert!(serde_json::to_string(&*saved).unwrap().contains(r#""language":"es""#));
    assert_eq!(st.settings.lang(), Lang::Es, "takes effect at once");
}
