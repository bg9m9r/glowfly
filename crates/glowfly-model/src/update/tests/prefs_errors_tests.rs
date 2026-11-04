//! The words of Preferences' failed results (#367): a signing key that couldn't be listed,
//! generated or exported (`KeyError`), a page's git config write that git refused
//! (`ConfigWriteFailed`) and the palette's file list that couldn't be read (`ListFilesFailed`),
//! each in English, German and Spanish as the page, the toast and the log show them.

use super::tests::state;
use super::update;
use crate::msg::{Msg, PaletteMsg, PrefsMsg};
use crate::state::palette::{Items, Mode, Palette, Pick, PickKind};
use crate::state::prefs::{GitConfigValues, PrefsState};
use crate::state::{AppState, PrefPage};
use glowfly_core::identity::ConfigWriteFailed;
use glowfly_core::repo::ListFilesFailed;
use glowfly_core::signing::KeyError as E;
use std::time::Instant;

const LANGS: [&str; 3] = ["en", "de", "es"];

/// Each variant, with its English, German and Spanish fragment.
fn variants() -> Vec<(E, [&'static str; 3])> {
    let said = "No such file or directory (os error 2)";
    vec![
        (
            E::NoIdentity,
            [
                "set your name and email in Profile first",
                "trage zuerst unter Profile deinen Namen und deine E-Mail-Adresse ein",
                "primero indica tu nombre y tu correo electrónico en Perfiles",
            ],
        ),
        (
            E::NotRun { program: "gpg".into(), said: "program not found".into() },
            [
                "could not run gpg: program not found",
                "gpg konnte nicht gestartet werden: program not found",
                "no se pudo ejecutar gpg: program not found",
            ],
        ),
        (
            E::Failed { program: "gpg".into(), said: "gpg: keybox locked".into() },
            ["gpg failed: gpg: keybox locked", "gpg ist fehlgeschlagen: gpg: keybox locked", "gpg falló: gpg: keybox locked"],
        ),
        (
            E::GenerateFailed { said: "gpg: agent refused".into() },
            [
                "key generation failed: gpg: agent refused",
                "die Schlüsselerzeugung ist fehlgeschlagen: gpg: agent refused",
                "la generación de la clave falló: gpg: agent refused",
            ],
        ),
        (E::NewKeyNotFound, ["the new key was not found", "der neue Schlüssel wurde nicht gefunden", "no se encontró la clave nueva"]),
        (
            E::NoPublicKey { key: "ABCD".into() },
            [
                "not found: no public key for ABCD",
                "nicht gefunden: es gibt keinen öffentlichen Schlüssel für ABCD",
                "no encontrado: no hay ninguna clave pública para ABCD",
            ],
        ),
        (E::Io { said: said.into() }, [said; 3]),
    ]
}

/// Preferences open on Commit signing, in `code`'s language, with an identity and a key chosen.
fn signing_page(code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    let mut prefs = PrefsState::new(PrefPage::Signing);
    prefs.git = Some(GitConfigValues {
        signing_format: "openpgp".into(),
        signing_key: "ABCD".into(),
        user_name: "Ada".into(),
        user_email: "ada@example.com".into(),
        ..Default::default()
    });
    st.prefs = Some(prefs);
    st
}

fn send(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("logged")
}

#[test]
fn a_key_list_that_failed_says_why_on_the_page() {
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = signing_page(code);
            send(&mut st, Msg::Prefs(PrefsMsg::SigningKeysLoaded(Err(e.clone()))));
            let signing = &st.prefs.as_ref().unwrap().signing;
            assert_eq!(signing.keys_error.as_deref(), Some(words[i]), "{code} {e:?}");
            assert_eq!(signing.keys.as_deref(), Some(&[][..]), "{code}: no keys to choose");
        }
    }
}

#[test]
fn a_failed_key_generation_says_why_in_each_language() {
    let heads = ["Could not generate a GPG key: ", "Es konnte kein GPG-Schlüssel erzeugt werden: ", "No se pudo generar una clave GPG: "];
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = signing_page(code);
            send(&mut st, Msg::Prefs(PrefsMsg::GpgKeyGenerated(Err(e.clone()))));
            assert_eq!(last_log(&st), (false, format!("{}{}", heads[i], words[i])), "{code} {e:?}");
            assert!(!st.prefs.as_ref().unwrap().signing.generating, "{code}: the button is free again");
        }
    }
}

#[test]
fn a_generated_key_is_toasted_in_each_language() {
    let words = [
        "Created GPG key NEWKEY and set it as your signing key",
        "GPG-Schlüssel NEWKEY wurde erstellt und als dein Signaturschlüssel festgelegt",
        "Se creó la clave GPG NEWKEY y se estableció como tu clave de firma",
    ];
    for (code, want) in LANGS.into_iter().zip(words) {
        let mut st = signing_page(code);
        send(&mut st, Msg::Prefs(PrefsMsg::GpgKeyGenerated(Ok("NEWKEY".into()))));
        assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == want), "{code}: {:?}", st.toasts.last().map(|t| &t.text));
    }
}

#[test]
fn a_failed_export_says_why_and_a_copied_key_is_toasted() {
    let heads = [
        "Could not export the public key: ",
        "Der öffentliche Schlüssel konnte nicht exportiert werden: ",
        "No se pudo exportar la clave pública: ",
    ];
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = signing_page(code);
            send(&mut st, Msg::Prefs(PrefsMsg::PublicKeyExported(Err(e.clone()))));
            assert_eq!(last_log(&st), (false, format!("{}{}", heads[i], words[i])), "{code} {e:?}");
        }
    }
    let copied = [
        "Public key copied to the clipboard",
        "Der öffentliche Schlüssel wurde in die Zwischenablage kopiert",
        "Se copió la clave pública al portapapeles",
    ];
    for (code, want) in LANGS.into_iter().zip(copied) {
        let mut st = signing_page(code);
        send(&mut st, Msg::Prefs(PrefsMsg::PublicKeyExported(Ok("ssh-ed25519 AAAA".into()))));
        assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == want), "{code}: {:?}", st.toasts.last().map(|t| &t.text));
    }
}

#[test]
fn a_refused_config_write_says_why_in_each_language() {
    let words = [
        "Couldn't save your preferences: git config user.name failed: error: could not lock config file",
        "Deine Änderungen in den Einstellungen konnten nicht gespeichert werden: git config user.name ist fehlgeschlagen: error: could not lock config file",
        "No se pudieron guardar tus preferencias: git config user.name falló: error: could not lock config file",
    ];
    let refused = || ConfigWriteFailed { key: "user.name".into(), said: "error: could not lock config file".into() };
    for (code, want) in LANGS.into_iter().zip(words) {
        let mut st = signing_page(code);
        // A ticket no save is waiting for: the window has moved on, so it's toasted and logged.
        send(&mut st, Msg::Prefs(PrefsMsg::ConfigSaved { ticket: u64::MAX, result: Err(refused()) }));
        assert_eq!(last_log(&st), (false, want.to_owned()), "{code}");
        assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text == want), "{code}: {:?}", st.toasts.last().map(|t| &t.text));
    }
}

#[test]
fn an_unreadable_file_list_says_why_in_each_language() {
    let words = [
        "Could not list the repository's files: could not find repository",
        "Die Dateien des Repositorys konnten nicht aufgelistet werden: could not find repository",
        "No se pudieron listar los archivos del repositorio: could not find repository",
    ];
    for (code, want) in LANGS.into_iter().zip(words) {
        let mut st = state();
        st.settings.language = code.into();
        st.palette = Some(Palette { mode: Mode::Pick(Pick::new(PickKind::FileHistory)), ..Default::default() });
        let (generation, result) = (st.palette_gen, Err(ListFilesFailed { said: "could not find repository".into() }));
        send(&mut st, Msg::Palette(PaletteMsg::TrackedPathsLoaded { generation, result }));
        let Some(Palette { mode: Mode::Pick(pick), .. }) = st.palette.as_ref() else { panic!("{code}: the file step stays open") };
        assert_eq!(pick.items, Items::Failed(want.into()), "{code}");
        assert_eq!(last_log(&st), (false, want.to_owned()), "{code}");
    }
}
