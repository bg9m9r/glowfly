//! Headless tests for Add to service (Preferences > SSH): opening the form, the fallback for
//! services without an API, sending, and each kind of answer.

use super::ssh_tests::{loaded, page, page_mut, pair, ssh};
use crate::effect::{Effect, UiEffect};
use crate::ssh::{ServiceKeyError, SshEffect, SshMsg};
use crate::state::AppState;
use crate::state::ssh_service::UploadState;
use trellis_core::forge::{Account, AddKeyOutcome, Connection, Provider};
use trellis_core::ssh_keys::SshKeyPair;

fn key() -> SshKeyPair {
    pair("id_ed25519", "ada@laptop")
}

fn account(provider: Provider, host: &str) -> Account {
    Account { provider, host: host.into(), org: String::new() }
}

fn github() -> Account {
    account(Provider::GitHub, "github.com")
}

/// The page with two keys listed and `accounts` connected.
fn connected(accounts: &[Account]) -> AppState {
    let mut st = loaded(vec![key(), pair("id_work", "")], true);
    st.settings.connections =
        accounts.iter().map(|a| Connection { account: a.clone(), user: "ada".into(), method: "token".into() }).collect();
    st
}

fn copied_and_opened(fx: &[Effect]) -> (Option<String>, Option<String>) {
    let copied = fx.iter().find_map(|e| match e {
        Effect::Ui(UiEffect::CopyText(t)) => Some(t.clone()),
        _ => None,
    });
    let opened = fx.iter().find_map(|e| match e {
        Effect::OpenUrl(u) => Some(u.clone()),
        _ => None,
    });
    (copied, opened)
}

fn choose(st: &mut AppState, key: &SshKeyPair, account: Account) -> Vec<Effect> {
    ssh(st, SshMsg::ChooseService { key: key.private.clone(), account })
}

#[test]
fn choosing_a_service_opens_the_form() {
    let mut st = connected(&[github()]);
    assert!(choose(&mut st, &key(), github()).is_empty());
    let form = page(&st).upload.clone().expect("the form");
    assert_eq!((form.default_title.as_str(), form.title.as_str(), &form.state), ("ada@laptop", "", &UploadState::Editing));
    choose(&mut st, &pair("id_work", ""), github());
    assert_eq!(page(&st).upload.as_ref().unwrap().default_title, "id_work", "no comment: the file name");

    let mut st = connected(&[]);
    assert!(choose(&mut st, &key(), github()).is_empty());
    assert!(page(&st).upload.is_none(), "an account no longer connected is ignored");
}

#[test]
fn a_service_without_an_api_gets_the_key_pasted() {
    let dc = account(Provider::BitbucketDataCenter, "bb.acme.io");
    let mut st = connected(std::slice::from_ref(&dc));
    let fx = choose(&mut st, &key(), dc);
    assert_eq!(copied_and_opened(&fx), (Some(key().public_key), Some("https://bb.acme.io/plugins/servlet/ssh/account/keys".into())));
    assert!(page(&st).upload.is_none());
    assert_eq!(
        st.toasts.last().map(|t| t.text.as_str()),
        Some("Public key copied. Paste it on the Bitbucket Data Center page that opened.")
    );

    let azure = account(Provider::AzureDevOps, "dev.azure.com");
    let mut st = connected(std::slice::from_ref(&azure));
    let fx = choose(&mut st, &key(), azure);
    assert_eq!(copied_and_opened(&fx), (Some(key().public_key), None), "no known page without an organization");
}

fn sent(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::AddToService { title, public_key, .. }) => {
                assert_eq!(public_key, &key().public_key);
                Some(title.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn sending_uses_the_typed_or_default_title() {
    let mut st = connected(&[github()]);
    choose(&mut st, &key(), github());
    page_mut(&mut st).upload.as_mut().unwrap().title = "a\nb".into();
    assert!(ssh(&mut st, SshMsg::SendToService).is_empty(), "control characters are refused");
    page_mut(&mut st).upload.as_mut().unwrap().title = "  ".into();
    assert_eq!(sent(&ssh(&mut st, SshMsg::SendToService)), ["ada@laptop"]);
    assert_eq!(page(&st).upload.as_ref().unwrap().state, UploadState::Sending);
    assert!(ssh(&mut st, SshMsg::SendToService).is_empty(), "one send at a time");

    let mut st = connected(&[github()]);
    choose(&mut st, &key(), github());
    page_mut(&mut st).upload.as_mut().unwrap().title = " Work laptop ".into();
    assert_eq!(sent(&ssh(&mut st, SshMsg::SendToService)), ["Work laptop"]);
}

fn answer(st: &mut AppState, result: Result<AddKeyOutcome, ServiceKeyError>) -> Vec<Effect> {
    ssh(st, SshMsg::AddedToService { key: key().private, account: github(), result })
}

fn sending() -> AppState {
    let mut st = connected(&[github()]);
    choose(&mut st, &key(), github());
    ssh(&mut st, SshMsg::SendToService);
    st
}

#[test]
fn answers() {
    let mut st = sending();
    answer(&mut st, Ok(AddKeyOutcome::Added { title: "ada@laptop".into() }));
    assert!(page(&st).upload.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Added id_ed25519 to GitHub as \u{201c}ada@laptop\u{201d}"));

    let mut st = sending();
    answer(&mut st, Ok(AddKeyOutcome::AlreadyThere { title: "Laptop".into() }));
    assert_eq!(page(&st).upload.as_ref().unwrap().state, UploadState::AlreadyThere("Laptop".into()));

    let mut st = sending();
    let fx = answer(&mut st, Err(ServiceKeyError { message: "access denied: no".into(), fallback: true }));
    assert_eq!(copied_and_opened(&fx), (Some(key().public_key), Some("https://github.com/settings/keys".into())));
    let state = page(&st).upload.as_ref().unwrap().state.clone();
    assert!(
        matches!(&state, UploadState::Failed { fallback: true, message } if message.contains("permission to write SSH keys")),
        "{state:?}"
    );

    let mut st = sending();
    let fx = answer(&mut st, Err(ServiceKeyError { message: "authentication failed: 401".into(), fallback: false }));
    assert_eq!(copied_and_opened(&fx), (None, None));
    let state = page(&st).upload.as_ref().unwrap().state.clone();
    assert_eq!(state, UploadState::Failed { message: "authentication failed: 401".into(), fallback: false });
    assert!(st.log.last().is_some_and(|e| !e.ok && e.text.contains("authentication failed")));

    // The Copy key and open settings button.
    let fx = ssh(&mut st, SshMsg::OpenKeySettings { key: key().private, account: github() });
    assert_eq!(copied_and_opened(&fx), (Some(key().public_key), Some("https://github.com/settings/keys".into())));
}

#[test]
fn answers_for_another_form_are_ignored() {
    let mut st = sending();
    ssh(
        &mut st,
        SshMsg::AddedToService {
            key: pair("id_work", "").private,
            account: github(),
            result: Ok(AddKeyOutcome::Added { title: "t".into() }),
        },
    );
    let gl = account(Provider::GitLab, "gitlab.com");
    ssh(&mut st, SshMsg::AddedToService { key: key().private, account: gl, result: Ok(AddKeyOutcome::Added { title: "t".into() }) });
    assert_eq!(page(&st).upload.as_ref().unwrap().state, UploadState::Sending, "still waiting for its own answer");
    ssh(&mut st, SshMsg::CancelService);
    assert!(page(&st).upload.is_none());
    assert!(answer(&mut st, Ok(AddKeyOutcome::Added { title: "t".into() })).is_empty());
    assert!(st.toasts.iter().all(|t| !t.text.starts_with("Added")), "a closed form's answer is dropped");
}
