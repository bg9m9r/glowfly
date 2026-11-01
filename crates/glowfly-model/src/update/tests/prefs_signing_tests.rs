//! Headless tests for Preferences > Commit signing: Extend by 2 years on an expired OpenPGP key
//! (the run, its result, the reload) and the help link the page sends.

use super::tests::state;
use super::update;
use crate::effect::{Effect, OsEffect, PrefsEffect};
use crate::msg::{Msg, PrefsMsg};
use crate::state::prefs::{GitConfigValues, PrefsState};
use crate::state::{AppState, PrefPage};
use glowfly_core::gpg_prompt::extend::ExtendFailed;
use glowfly_core::signing::SigningKey;
use std::time::Instant;

const KEY: &str = "ABCDEF0123456789";

/// Preferences open on Commit signing, with the expired OpenPGP key `KEY` chosen.
fn signing_page() -> AppState {
    let mut st = state();
    let mut prefs = PrefsState::new(PrefPage::Signing);
    prefs.git = Some(GitConfigValues { signing_format: "openpgp".into(), signing_key: KEY.into(), ..Default::default() });
    prefs.signing.keys = Some(vec![SigningKey { id: KEY.into(), label: "Ada".into(), expires: Some(1), expired: true }]);
    st.prefs = Some(prefs);
    st
}

fn run(st: &mut AppState, m: PrefsMsg) -> Vec<Effect> {
    update(st, Msg::Prefs(m), Instant::now())
}

fn signing(st: &AppState) -> &crate::state::prefs::SigningState {
    &st.prefs.as_ref().expect("Preferences are open").signing
}

#[test]
fn extend_runs_gpg_on_the_chosen_key_once() {
    let mut st = signing_page();
    st.prefs.as_mut().and_then(|p| p.git.as_mut()).unwrap().gpg_program = "C:/gnupg/gpg.exe".into();
    let fx = run(&mut st, PrefsMsg::ExtendKey);
    assert!(
        matches!(&fx[..], [Effect::Prefs(PrefsEffect::ExtendGpgKey { program, key })] if program == "C:/gnupg/gpg.exe" && key == KEY),
        "{:?}",
        fx.iter().map(Effect::name).collect::<Vec<_>>()
    );
    assert!(signing(&st).extending);
    assert!(run(&mut st, PrefsMsg::ExtendKey).is_empty(), "a second click waits for the first");
}

#[test]
fn extend_uses_gpg_when_no_program_is_set_and_needs_a_key() {
    let mut st = signing_page();
    let fx = run(&mut st, PrefsMsg::ExtendKey);
    assert!(matches!(&fx[..], [Effect::Prefs(PrefsEffect::ExtendGpgKey { program, .. })] if program == "gpg"));
    let mut none = signing_page();
    none.prefs.as_mut().and_then(|p| p.git.as_mut()).unwrap().signing_key.clear();
    assert!(run(&mut none, PrefsMsg::ExtendKey).is_empty());
    let mut ssh = signing_page();
    ssh.prefs.as_mut().and_then(|p| p.git.as_mut()).unwrap().signing_format = "ssh".into();
    assert!(run(&mut ssh, PrefsMsg::ExtendKey).is_empty(), "only OpenPGP keys are extended");
}

#[test]
fn an_extended_key_is_announced_and_the_keys_reload() {
    let mut st = signing_page();
    run(&mut st, PrefsMsg::ExtendKey);
    let fx = run(&mut st, PrefsMsg::KeyExtended(Ok(())));
    assert!(matches!(&fx[..], [Effect::Prefs(PrefsEffect::ListSigningKeys { .. })]));
    assert!(!signing(&st).extending);
    assert!(signing(&st).keys.is_none(), "the list reloads");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == format!("Extended {KEY} by 2 years")));
}

#[test]
fn a_failure_stays_on_the_page_with_gpgs_message() {
    let mut st = signing_page();
    run(&mut st, PrefsMsg::ExtendKey);
    let fx = run(&mut st, PrefsMsg::KeyExtended(Err(ExtendFailed::Gpg("gpg: agent refused".into()))));
    assert!(fx.iter().all(|e| !matches!(e, Effect::Prefs(PrefsEffect::ListSigningKeys { .. }))), "nothing changed to reload");
    assert_eq!(signing(&st).extend_error.as_deref(), Some("gpg: agent refused"));
    assert!(st.log.iter().any(|l| !l.ok && l.text == format!("Could not extend {KEY}: gpg: agent refused")));
    // Trying again clears it until the new result.
    run(&mut st, PrefsMsg::ExtendKey);
    assert_eq!(signing(&st).extend_error, None);
    run(&mut st, PrefsMsg::KeyExtended(Err(ExtendFailed::TimedOut)));
    assert_eq!(signing(&st).extend_error.as_deref(), Some("The passphrase prompt timed out."));
}

#[test]
fn a_cancelled_prompt_leaves_no_error() {
    let mut st = signing_page();
    run(&mut st, PrefsMsg::ExtendKey);
    let fx = run(&mut st, PrefsMsg::KeyExtended(Err(ExtendFailed::Cancelled)));
    assert!(fx.is_empty());
    assert!(!signing(&st).extending);
    assert_eq!(signing(&st).extend_error, None);
    assert!(st.log.iter().all(|l| l.ok));
}

#[test]
fn the_help_link_opens_the_browser_while_preferences_are_open() {
    let mut st = signing_page();
    let url = "https://docs.github.com/en/authentication/managing-commit-signature-verification/updating-an-expired-gpg-key";
    let fx = update(&mut st, Msg::OpenUrl(url.into()), Instant::now());
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::OpenUrl(u))] if u == url));
    assert!(st.prefs.is_some(), "the page stays open");
}
