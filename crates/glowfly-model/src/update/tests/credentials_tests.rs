//! The words of a failed save or drop of an HTTPS sign-in (`CredentialMsg::Remembered` and
//! `Forgotten` with a `CredentialError`, update/askpass_store.rs): every variant's text in
//! English, German and Spanish, as the toast and the log show it.

use super::tests::state;
use super::update;
use crate::msg::{CredentialMsg, Msg};
use crate::state::AppState;
use glowfly_core::git_credential::CredentialError as E;
use std::time::Instant;

/// Each variant, with its English, German and Spanish fragment.
fn variants() -> Vec<(E, [&'static str; 3])> {
    let said = || Some("fatal: the keychain is locked".to_owned());
    let git = "fatal: the keychain is locked";
    vec![
        (
            E::NoStore { install: Some("git-credential-libsecret".into()) },
            [
                "no credential store is set up (install git-credential-libsecret or set credential.helper)",
                "es ist kein Credential-Store eingerichtet (installiere git-credential-libsecret oder setze credential.helper)",
                "no hay ningún almacén de credenciales configurado (instala git-credential-libsecret o configura credential.helper)",
            ],
        ),
        (
            E::NoStore { install: None },
            [
                "no credential store is set up (set credential.helper)",
                "es ist kein Credential-Store eingerichtet (setze credential.helper)",
                "no hay ningún almacén de credenciales configurado (configura credential.helper)",
            ],
        ),
        (
            E::SetUpFailed { said: None },
            ["could not set credential.helper", "credential.helper konnte nicht gesetzt werden", "no se pudo configurar credential.helper"],
        ),
        (
            E::ApproveFailed { said: None },
            ["git credential approve failed", "git credential approve ist fehlgeschlagen", "git credential approve falló"],
        ),
        (
            E::RejectFailed { said: None },
            ["git credential reject failed", "git credential reject ist fehlgeschlagen", "git credential reject falló"],
        ),
        (E::SetUpFailed { said: said() }, [git; 3]),
        (E::ApproveFailed { said: said() }, [git; 3]),
        (E::RejectFailed { said: said() }, [git; 3]),
    ]
}

fn in_lang(code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    st
}

fn send(st: &mut AppState, msg: CredentialMsg) {
    update(st, Msg::Credentials(msg), Instant::now());
}

#[test]
fn a_failed_save_says_why_in_each_language() {
    let heads = [
        "Couldn't save the sign-in for github.com: ",
        "Die Anmeldung für github.com konnte nicht gespeichert werden: ",
        "No se pudo guardar el inicio de sesión de github.com: ",
    ];
    for (e, words) in variants() {
        for (i, code) in ["en", "de", "es"].into_iter().enumerate() {
            let mut st = in_lang(code);
            send(&mut st, CredentialMsg::Remembered { host: "github.com".into(), result: Err(e.clone()) });
            let want = format!("{}{}", heads[i], words[i]);
            let toast = st.toasts.last().expect("a failed save is toasted");
            assert!(!toast.ok && toast.text == want, "{code} {e:?}: {}", toast.text);
            assert!(st.log.last().is_some_and(|l| !l.ok && l.text == want), "{code} {e:?}: logged too");
        }
    }
}

#[test]
fn a_failed_drop_says_why_in_each_language() {
    let heads = [
        "Couldn't drop the rejected sign-in for github.com: ",
        "Die abgelehnte Anmeldung für github.com konnte nicht entfernt werden: ",
        "No se pudo quitar el inicio de sesión rechazado de github.com: ",
    ];
    for (e, words) in variants() {
        for (i, code) in ["en", "de", "es"].into_iter().enumerate() {
            let mut st = in_lang(code);
            send(&mut st, CredentialMsg::Forgotten { host: "github.com".into(), result: Err(e.clone()) });
            let want = format!("{}{}", heads[i], words[i]);
            assert!(st.log.last().is_some_and(|l| !l.ok && l.text == want), "{code} {e:?}: {:?}", st.log.last().map(|l| &l.text));
        }
    }
}

#[test]
fn a_dropped_sign_in_is_logged_quietly_in_each_language() {
    let words = [
        "Dropped the rejected sign-in for github.com from git's credential helper.",
        "Die abgelehnte Anmeldung für github.com wurde aus dem Credential-Helper von Git entfernt.",
        "Se quitó el inicio de sesión rechazado de github.com del asistente de credenciales de Git.",
    ];
    for (code, want) in ["en", "de", "es"].into_iter().zip(words) {
        let mut st = in_lang(code);
        send(&mut st, CredentialMsg::Forgotten { host: "github.com".into(), result: Ok(()) });
        assert!(st.toasts.is_empty(), "{code}: dropping a sign-in isn't toasted");
        assert!(st.log.last().is_some_and(|l| l.ok && l.text == want), "{code}: {:?}", st.log.last().map(|l| &l.text));
        assert!(!st.view.show_log);
    }
}
