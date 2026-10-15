//! The credential prompt over the main window: sign in, a passphrase retried, and a host-key
//! confirmation. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::CredentialMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::askpass::PendingPrompt;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use trellis_core::askpass::Prompt;
use trellis_core::settings::ThemeChoice;

const HOST_KEY: &str = "The authenticity of host 'example.com (192.0.2.10)' can't be established.\n\
ED25519 key fingerprint is SHA256:Xq7fW0dkN3vQ1nYp8tLr2mH5sJc9bGa4eZo6iKu0TwE.\n\
This key is not known by any other names.\n\
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

fn prompt(st: &mut AppState, text: &str, retry: bool, rejected_before: bool) {
    st.askpass.queue.push_back(PendingPrompt { id: 1, prompt: Prompt::new(text, None), retry, rejected_before });
    st.askpass.focus_pending = true;
}

fn pushed(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn credential_prompt_sign_in() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "askpass-sign-in");
    prompt(&mut st, "Username for 'https://github.com': ", false, true);
    st.askpass.user = "ada".into();
    st.askpass.secret = "hunter2".into();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("credential_prompt_sign_in");
    assert!(h.query_by_label_contains("hunter2").is_none(), "the password is masked");
    assert!(h.query_by_label_contains("were not accepted").is_some());
    let sign_in = h.get_by_label("Sign in");
    assert!(!sign_in.is_disabled());
    sign_in.click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn credential_prompt_passphrase_retry_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "askpass-passphrase");
    prompt(&mut st, "Enter passphrase for key '~/.ssh/id_ed25519': ", true, false);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("credential_prompt_passphrase_retry_light");
    assert!(h.get_by_label("Unlock").is_disabled(), "nothing typed yet");
    assert!(h.query_by_label_contains("didn't unlock the key").is_some());
    h.press_key(egui::Key::Escape);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::CancelCredentials))));
    assert!(!pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn credential_prompt_host_key() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "askpass-host-key");
    prompt(&mut st, HOST_KEY, false, false);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("credential_prompt_host_key");
    assert!(h.query_by_label("Yes").is_some_and(|n| !n.is_disabled()));
    h.get_by_label("No").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::CancelCredentials))));
    drop(h);
    remove_fixture(&dir);
}
