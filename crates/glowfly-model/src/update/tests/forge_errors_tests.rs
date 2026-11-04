//! The words of a hosting service's failed sign-in or call (#367): a connection that couldn't be
//! made (`ForgeMsg::Connected`), a Trello account's boards (`TrelloBoardsLoaded`), the Publish
//! dialog's owners (`PublishMsg::PublishOwners`) and its new repository (`Published`), each with
//! a `ServiceError`. Every variant's text in English, German and Spanish, as the page, the
//! dialog and the log show it; the HTTP status and the service's words are kept.

use super::tests::state;
use super::update;
use crate::msg::{ForgeMsg, Msg, PublishMsg};
use crate::state::prefs::PrefsState;
use crate::state::publish::PublishForm;
use crate::state::trello::BoardList;
use crate::state::{AppState, Modal, PrefPage};
use glowfly_core::forge::{Account, CreatedRepo, Provider, ServiceError as E};
use std::path::PathBuf;
use std::time::Instant;

const LANGS: [&str; 3] = ["en", "de", "es"];

/// Each variant, with its English, German and Spanish fragment.
fn variants() -> Vec<(E, [&'static str; 3])> {
    vec![
        (
            E::Auth { said: "the token expired".into() },
            [
                "authentication failed: the token expired",
                "Authentifizierung fehlgeschlagen: the token expired",
                "la autenticación falló: the token expired",
            ],
        ),
        (
            E::Forbidden { said: "rate limit exceeded".into() },
            ["access denied: rate limit exceeded", "Zugriff verweigert: rate limit exceeded", "acceso denegado: rate limit exceeded"],
        ),
        (E::NotFound { said: "acme".into() }, ["not found: acme", "nicht gefunden: acme", "no encontrado: acme"]),
        (E::Http { status: 502, message: "bad gateway".into() }, ["HTTP 502: bad gateway"; 3]),
        (E::Network { said: "timed out".into() }, ["network error: timed out", "Netzwerkfehler: timed out", "error de red: timed out"]),
        (
            E::Decode { said: "expected a list".into() },
            ["unexpected response: expected a list", "unerwartete Antwort: expected a list", "respuesta inesperada: expected a list"],
        ),
        (
            E::Keyring { said: "no entry".into() },
            ["credential store: no entry", "Credential-Store: no entry", "almacén de credenciales: no entry"],
        ),
        (E::Other { said: "Git Credential Manager said no".into() }, ["Git Credential Manager said no"; 3]),
    ]
}

/// Whether the Publish dialog shows a plain-words sentence for `e` (with the words under
/// Details) rather than the words themselves: `error::advice` knows every kind but these, and a
/// missing owner only while creating.
fn advised(e: &E, creating: bool) -> bool {
    match e {
        E::Decode { .. } | E::Keyring { .. } | E::Other { .. } => false,
        E::NotFound { .. } => creating,
        _ => true,
    }
}

fn github() -> Account {
    Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() }
}

fn in_lang(code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    st
}

fn send(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

fn workdir() -> PathBuf {
    std::env::temp_dir().join("glowfly-forge-errors")
}

/// The Publish dialog, for `workdir`, with the owners loaded and Create pressed.
fn publishing(code: &str) -> AppState {
    let mut st = in_lang(code);
    st.modal = Some(Modal::Publish(Box::new(PublishForm {
        workdir: workdir(),
        branch: "main".into(),
        accounts: vec![github()],
        account: 0,
        user: Some("ada".into()),
        owners: vec!["ada".into()],
        owner: "ada".into(),
        name: "demo".into(),
        description: String::new(),
        private: true,
        loading: true,
        creating: true,
        error: None,
        error_detail: None,
        touched: Default::default(),
    })));
    st
}

fn form(st: &AppState) -> &PublishForm {
    match &st.modal {
        Some(Modal::Publish(f)) => f,
        _ => panic!("the Publish dialog stays open"),
    }
}

fn logged(st: &AppState, ok: bool, want: &str) -> bool {
    st.log.iter().any(|l| l.ok == ok && l.text == want)
}

#[test]
fn a_failed_connection_says_why_on_the_page() {
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = in_lang(code);
            st.prefs = Some(PrefsState::new(PrefPage::Integrations));
            let connected = ForgeMsg::Connected { account: github(), method: "access token".into(), token: None, result: Err(e.clone()) };
            send(&mut st, Msg::Forge(connected));
            assert_eq!(st.prefs.as_ref().unwrap().int_error.as_deref(), Some(words[i]), "{code} {e:?}");
            assert!(st.settings.connections.is_empty(), "{code}: nothing is connected");
        }
    }
}

#[test]
fn a_new_connection_is_logged_in_each_language() {
    let words = [
        "Connected to GitHub (github.com) as ada.",
        "Mit GitHub (github.com) als ada verbunden.",
        "Conectado a GitHub (github.com) como ada.",
    ];
    for (code, want) in LANGS.into_iter().zip(words) {
        let mut st = in_lang(code);
        let connected = ForgeMsg::Connected { account: github(), method: "access token".into(), token: None, result: Ok("ada".into()) };
        send(&mut st, Msg::Forge(connected));
        assert!(logged(&st, true, want), "{code}: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    }
}

#[test]
fn trello_boards_that_failed_say_why_on_the_page() {
    let trello = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = in_lang(code);
            let mut prefs = PrefsState::new(PrefPage::RepoTracker);
            prefs.trello_boards = BoardList { account: Some(trello.clone()), loading: true, ..Default::default() };
            st.prefs = Some(prefs);
            send(&mut st, Msg::Forge(ForgeMsg::TrelloBoardsLoaded { account: trello.clone(), result: Err(e.clone()) }));
            let list = &st.prefs.as_ref().unwrap().trello_boards;
            assert_eq!(list.error.as_deref(), Some(words[i]), "{code} {e:?}");
            assert!(!list.loading, "{code}: Try again is offered");
        }
    }
}

#[test]
fn owners_that_failed_say_why_in_the_dialog() {
    let heads = [
        "Couldn't sign in to github.com: ",
        "Die Anmeldung bei github.com ist fehlgeschlagen: ",
        "No se pudo iniciar sesión en github.com: ",
    ];
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = publishing(code);
            send(&mut st, Msg::Publish(PublishMsg::PublishOwners { account: github(), result: Err(e.clone()) }));
            let f = form(&st);
            if advised(&e, false) {
                assert!(f.error.as_deref().is_some_and(|s| s != words[i] && !s.is_empty()), "{code} {e:?}: a sentence");
                assert_eq!(f.error_detail.as_deref(), Some(words[i]), "{code} {e:?}: the words under Details");
            } else {
                let want = format!("{}{}", heads[i], words[i]);
                assert_eq!((f.error.as_deref(), f.error_detail.as_deref()), (Some(want.as_str()), None), "{code} {e:?}");
            }
        }
    }
}

#[test]
fn a_repository_that_couldnt_be_created_says_why() {
    let heads = [
        "Creating the GitHub repository failed: ",
        "Das GitHub-Repository konnte nicht erstellt werden: ",
        "No se pudo crear el repositorio de GitHub: ",
    ];
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = publishing(code);
            let published = PublishMsg::Published { workdir: workdir(), branch: "main".into(), result: Err(e.clone()) };
            send(&mut st, Msg::Publish(published));
            let f = form(&st);
            assert!(!f.creating, "{code}: Create can be pressed again");
            if advised(&e, true) {
                assert!(f.error.as_deref().is_some_and(|s| s != words[i] && !s.is_empty()), "{code} {e:?}: a sentence");
                assert_eq!(f.error_detail.as_deref(), Some(words[i]), "{code} {e:?}: the words under Details");
            } else {
                assert_eq!((f.error.as_deref(), f.error_detail.as_deref()), (Some(words[i]), None), "{code} {e:?}");
            }

            // With the dialog closed, the activity log says it.
            let mut st = in_lang(code);
            let published = PublishMsg::Published { workdir: workdir(), branch: "main".into(), result: Err(e.clone()) };
            send(&mut st, Msg::Publish(published));
            assert!(logged(&st, false, &format!("{}{}", heads[i], words[i])), "{code} {e:?}: {:?}", st.log.last().map(|l| &l.text));
        }
    }
}

#[test]
fn a_created_repository_is_logged_in_each_language() {
    let words = [
        "Created ada/demo (https://github.com/ada/demo).",
        "ada/demo wurde erstellt (https://github.com/ada/demo).",
        "Se creó ada/demo (https://github.com/ada/demo).",
    ];
    for (code, want) in LANGS.into_iter().zip(words) {
        let mut st = publishing(code);
        let repo = CreatedRepo {
            full_name: "ada/demo".into(),
            html_url: "https://github.com/ada/demo".into(),
            clone_url: "https://github.com/ada/demo.git".into(),
        };
        send(&mut st, Msg::Publish(PublishMsg::Published { workdir: workdir(), branch: "main".into(), result: Ok(repo) }));
        assert!(logged(&st, true, want), "{code}: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
        assert!(st.modal.is_none(), "{code}: the dialog closes");
    }
}
