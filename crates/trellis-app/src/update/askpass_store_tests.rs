//! Headless tests for "Remember this sign-in" (update/askpass_store.rs): what is recorded while
//! the job runs, what is saved or dropped when it ends, and how the results are reported.

use super::tests::state;
use super::update;
use crate::effect::{CredentialEffect, Effect};
use crate::msg::{CredentialMsg, JobMsg, Msg};
use crate::state::{AppState, RunningJob};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::askpass::Prompt;
use trellis_core::git_credential::Remembered;

const USERNAME: &str = "Username for 'https://github.com': ";
const PASSWORD: &str = "Password for 'https://ada@github.com': ";
const AUTH_FAILED: &str = "$ git push\nfatal: Authentication failed for 'https://github.com/o/r.git/'";

fn send(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn ask(st: &mut AppState, id: u64, text: &str) -> Vec<Effect> {
    send(st, Msg::Credentials(CredentialMsg::CredentialPrompt { id, prompt: Prompt::new(text, None) }))
}

/// Types into the dialog and answers the shown prompt.
fn type_in(st: &mut AppState, user: &str, secret: &str) -> Vec<Effect> {
    st.askpass.user = user.into();
    st.askpass.secret = secret.into();
    send(st, Msg::Credentials(CredentialMsg::AnswerCredentials))
}

fn start_job(st: &mut AppState, workdir: Option<PathBuf>) {
    st.job = Some(RunningJob {
        label: "Push".into(),
        workdir,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
    });
}

fn finish(st: &mut AppState, ok: bool, output: &str) -> Vec<Effect> {
    send(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }))
}

/// (remember, cwd, protocol, host, user, password) of each credential effect.
fn stored(fx: &[Effect]) -> Vec<(bool, PathBuf, String, String, String, String)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Credential(CredentialEffect::Remember { cwd, credential: c }) => Some((true, cwd.clone(), c)),
            Effect::Credential(CredentialEffect::Forget { cwd, credential: c }) => Some((false, cwd.clone(), c)),
            _ => None,
        })
        .map(|(r, cwd, c)| (r, cwd, c.protocol.clone(), c.host.clone(), c.username.clone(), c.password.expose().to_owned()))
        .collect()
}

/// A Username prompt for `host` answered with ada / `password`.
fn sign_in(st: &mut AppState, id: u64, host: &str, password: &str) {
    ask(st, id, &format!("Username for 'https://{host}': "));
    type_in(st, "ada", password);
}

#[test]
fn a_sign_in_that_worked_is_saved_once() {
    let workdir = trellis_core::paths::temp_dir().join("trellis-remember-repo");
    for cwd in [Some(workdir.clone()), None] {
        let mut st = state();
        start_job(&mut st, cwd.clone());
        ask(&mut st, 1, USERNAME);
        assert!(st.askpass.remember, "on by default");
        type_in(&mut st, "ada", "hunter2");
        // git's follow-up password prompt is answered with the kept password, not recorded again.
        ask(&mut st, 2, PASSWORD);
        assert_eq!(st.askpass.typed.len(), 1);
        let fx = finish(&mut st, true, "$ git push\nEverything up-to-date");
        let expected = cwd.unwrap_or_else(AppState::outside_dir);
        assert_eq!(stored(&fx), [(true, expected, "https".into(), "github.com".into(), "ada".into(), "hunter2".into())]);
        assert!(st.askpass.typed.is_empty());
    }
}

#[test]
fn nothing_is_saved_with_remember_off() {
    let mut st = state();
    start_job(&mut st, None);
    ask(&mut st, 1, USERNAME);
    st.askpass.remember = false;
    type_in(&mut st, "ada", "hunter2");
    assert!(st.askpass.typed.is_empty());
    assert!(stored(&finish(&mut st, true, "")).is_empty());
    start_job(&mut st, None);
    sign_in(&mut st, 2, "github.com", "hunter2");
    assert_eq!(st.askpass.typed.len(), 1, "the next prompt starts from the setting again");
    finish(&mut st, true, "");
    st.settings.remember_https_credentials = false;
    start_job(&mut st, None);
    sign_in(&mut st, 3, "github.com", "hunter2");
    assert!(st.askpass.typed.is_empty());
    assert!(stored(&finish(&mut st, false, AUTH_FAILED)).is_empty(), "nor dropped");
}

#[test]
fn a_password_prompt_for_an_https_url_is_recorded() {
    let mut st = state();
    start_job(&mut st, None);
    ask(&mut st, 1, PASSWORD);
    type_in(&mut st, "", "token123");
    let fx = finish(&mut st, true, "");
    assert_eq!(stored(&fx), [(true, AppState::outside_dir(), "https".into(), "github.com".into(), "ada".into(), "token123".into())]);
}

#[test]
fn ssh_passwords_are_never_recorded() {
    let mut st = state();
    start_job(&mut st, None);
    ask(&mut st, 1, "ada@host's password: ");
    type_in(&mut st, "", "secret");
    ask(&mut st, 2, "Enter passphrase for key '/home/ada/.ssh/id_ed25519': ");
    type_in(&mut st, "", "open sesame");
    assert!(st.askpass.typed.is_empty());
    assert!(stored(&finish(&mut st, true, "")).is_empty());
}

#[test]
fn a_rejected_sign_in_is_dropped_for_its_host_only() {
    let mut st = state();
    start_job(&mut st, None);
    sign_in(&mut st, 1, "github.com", "wrong");
    sign_in(&mut st, 2, "gitlab.com", "fine");
    let fx = finish(&mut st, false, AUTH_FAILED);
    assert_eq!(stored(&fx), [(false, AppState::outside_dir(), "https".into(), "github.com".into(), "ada".into(), "wrong".into())]);
    assert!(st.askpass.typed.is_empty());
}

#[test]
fn a_rejection_that_names_no_url_drops_a_lone_sign_in() {
    let mut st = state();
    start_job(&mut st, None);
    sign_in(&mut st, 1, "github.com", "wrong");
    let fx = finish(&mut st, false, "remote: HTTP Basic: Access denied");
    assert_eq!(stored(&fx).len(), 1);
    assert!(!stored(&fx)[0].0, "a Forget");

    start_job(&mut st, None);
    sign_in(&mut st, 2, "github.com", "a");
    sign_in(&mut st, 3, "gitlab.com", "b");
    assert!(stored(&finish(&mut st, false, "remote: HTTP Basic: Access denied")).is_empty(), "two: no telling which");
}

#[test]
fn other_failures_drop_nothing() {
    for output in [" ! [rejected]        main -> main (non-fast-forward)", "fatal: unable to access: Could not resolve host"] {
        let mut st = state();
        start_job(&mut st, None);
        sign_in(&mut st, 1, "github.com", "hunter2");
        assert!(stored(&finish(&mut st, false, output)).is_empty(), "{output}");
        assert!(st.askpass.typed.is_empty());
    }
    // A cancelled prompt ended the job: its output says nothing about the typed sign-in.
    let mut st = state();
    start_job(&mut st, None);
    sign_in(&mut st, 1, "github.com", "hunter2");
    ask(&mut st, 2, "Username for 'https://gitlab.com': ");
    send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    assert!(stored(&finish(&mut st, false, AUTH_FAILED)).is_empty());
    assert!(st.askpass.typed.is_empty());
}

#[test]
fn the_box_starts_from_the_setting() {
    for on in [true, false] {
        let mut st = state();
        st.settings.remember_https_credentials = on;
        st.askpass.remember = !on;
        ask(&mut st, 1, USERNAME);
        assert_eq!(st.askpass.remember, on);
    }
}

#[test]
fn reports_what_became_of_the_sign_in() {
    let mut st = state();
    let remembered = |st: &mut AppState, result| {
        send(st, Msg::Credentials(CredentialMsg::Remembered { host: "github.com".into(), result }));
    };
    remembered(&mut st, Ok(Remembered::Helper));
    assert!(st.toasts.is_empty(), "routine: no toast");
    assert!(st.log.last().is_some_and(|l| l.ok && l.text.contains("github.com")));
    assert!(!st.view.show_log);

    remembered(&mut st, Ok(Remembered::SetUp("libsecret".into())));
    let text = "Git now saves sign-ins with libsecret (credential.helper in your global git config). Saved the sign-in for github.com.";
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == text));
    assert!(st.log.last().is_some_and(|l| l.text == text));

    let e = "no credential store is set up (install git-credential-libsecret or set credential.helper)";
    remembered(&mut st, Err(e.into()));
    let failed = format!("Couldn't save the sign-in for github.com: {e}");
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text == failed));

    let toasts = st.toasts.len();
    send(&mut st, Msg::Credentials(CredentialMsg::Forgotten { host: "github.com".into(), result: Ok(()) }));
    assert!(st.log.last().is_some_and(|l| l.ok));
    send(&mut st, Msg::Credentials(CredentialMsg::Forgotten { host: "github.com".into(), result: Err("helper failed".into()) }));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("helper failed")));
    assert_eq!(st.toasts.len(), toasts, "dropping a sign-in is never toasted");
}
