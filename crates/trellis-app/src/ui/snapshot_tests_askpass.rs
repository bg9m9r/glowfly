//! The credential prompt over the main window: sign in (with Remember this sign-in), a
//! passphrase retried, an unknown SSH host (as fields) and a confirmation that isn't read as one,
//! a signing key's passphrase (first and retried), and an SSH signing key's passphrase. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::CredentialMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::askpass::PendingPrompt;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use trellis_core::askpass::{GpgAsk, Prompt, SshSignAsk};
use trellis_core::settings::ThemeChoice;

const HOST_KEY: &str = "The authenticity of host 'example.com (192.0.2.10)' can't be established.\n\
ED25519 key fingerprint is SHA256:Xq7fW0dkN3vQ1nYp8tLr2mH5sJc9bGa4eZo6iKu0TwE.\n\
This key is not known by any other names.\n\
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

fn prompt(st: &mut AppState, text: &str, retry: bool, rejected_before: bool) {
    st.askpass.queue.push_back(PendingPrompt { id: 1, prompt: Prompt::new(text, None), retry, rejected_before });
    st.askpass.focus_pending = true;
}

fn gpg_prompt(st: &mut AppState, retry: bool) {
    let ask = GpgAsk { key: "ABCDEF0123456789".into(), user: Some("Ada Lovelace <ada@example.com>".into()), retry };
    st.askpass.queue.push_back(PendingPrompt { id: 1, prompt: Prompt::gpg(ask), retry, rejected_before: false });
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
    st.askpass.remember = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("credential_prompt_sign_in");
    // Visible labels above the fields, which also name them.
    assert!(h.query_all_by_label("Username").any(|n| n.role() == egui::accesskit::Role::Label));
    assert!(h.query_all_by_label("Username").any(|n| n.role() == egui::accesskit::Role::TextInput));
    let token = "Password or personal access token";
    assert!(h.query_all_by_label(token).any(|n| n.role() == egui::accesskit::Role::Label));
    assert!(h.query_all_by_label(token).any(|n| n.role() == egui::accesskit::Role::PasswordInput));
    assert!(h.query_by_label_contains("hunter2").is_none(), "the password is masked");
    assert!(h.query_by_label_contains("were not accepted").is_some());
    assert!(h.query_by_label_contains("saves this sign-in with git's credential helper").is_some());
    h.get_by_label("Remember this sign-in").click();
    h.run();
    assert!(!h.state().0.askpass.remember, "the box is only this sign-in's");
    assert!(h.query_by_label_contains("doesn't save what you type here").is_some());
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
    for label in ["Unknown SSH host", "example.com", "192.0.2.10", "ED25519", "Details"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label_contains("SHA256:Xq7f").is_some());
    assert!(h.query_by_label("Trust and connect").is_some_and(|n| !n.is_disabled()));
    let raw = "Are you sure you want to continue connecting";
    assert!(h.query_by_label_contains(raw).is_none(), "ssh's text waits under Details");
    // A stray Enter doesn't accept an unknown host key: Cancel is the default.
    h.press_key(eframe::egui::Key::Enter);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::CancelCredentials))));
    h.state_mut().1.clear();
    h.get_by_label("Details").click();
    h.run();
    assert!(h.query_by_label_contains(raw).is_some());
    h.get_by_label("Trust and connect").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    h.state_mut().1.clear();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::CancelCredentials))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn credential_prompt_confirm_raw_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "askpass-confirm-raw");
    let text = "Allow use of key /home/ada/.ssh/id_ed25519?\nKey fingerprint SHA256:abc.";
    st.askpass.queue.push_back(PendingPrompt {
        id: 1,
        prompt: Prompt::new(text, Some("confirm".into())),
        retry: false,
        rejected_before: false,
    });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("credential_prompt_confirm_raw_light");
    assert!(h.query_by_label("Confirm SSH connection").is_some());
    assert!(h.query_by_label_contains("Allow use of key /home/ada/.ssh/id_ed25519?").is_some());
    assert!(h.query_by_label("Yes").is_some_and(|n| !n.is_disabled()));
    assert!(h.query_by_label("Details").is_none());
    h.press_key(egui::Key::Enter);
    h.run();
    assert!(!pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::CancelCredentials))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn credential_prompt_https_password_label() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "askpass-https-password");
    prompt(&mut st, "Password for 'https://ada@github.com': ", false, false);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_all_by_label("Password or personal access token").any(|n| n.role() == egui::accesskit::Role::PasswordInput));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn credential_prompt_gpg() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "askpass-gpg");
    gpg_prompt(&mut st, false);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("credential_prompt_gpg");
    assert!(h.get_by_label("Unlock").is_disabled(), "nothing typed yet");
    assert!(h.query_by_label_contains("Key ABCDEF0123456789").is_some());
    assert!(h.query_by_label_contains("Ada Lovelace <ada@example.com>").is_some());
    assert!(h.query_by_label_contains("gpg-agent remembers the passphrase").is_some());
    assert!(h.query_by_label_contains("didn't unlock").is_none());
    h.state_mut().0.askpass.secret = "hunter2".into();
    h.run();
    assert!(h.query_by_label_contains("hunter2").is_none(), "the passphrase is masked");
    h.get_by_label("Unlock").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn credential_prompt_gpg_retry_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "askpass-gpg-retry");
    gpg_prompt(&mut st, true);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("credential_prompt_gpg_retry_light");
    assert!(h.query_by_label_contains("didn't unlock the key").is_some());
    h.press_key(egui::Key::Escape);
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::CancelCredentials))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn credential_prompt_ssh_signing() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "askpass-ssh-signing");
    let prompt = Prompt::ssh_sign(SshSignAsk { key: "~/.ssh/id_ed25519".into() });
    st.askpass.queue.push_back(PendingPrompt { id: 1, prompt, retry: false, rejected_before: false });
    st.askpass.focus_pending = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("credential_prompt_ssh_signing");
    assert!(h.query_by_label("Unlock signing key").is_some());
    assert!(h.query_by_label_contains("Git is signing with your SSH key").is_some());
    assert!(h.query_by_label_contains("Key ~/.ssh/id_ed25519").is_some());
    assert!(h.query_by_label_contains("ssh-agent (ssh-add)").is_some());
    assert!(h.query_by_label_contains("didn't unlock").is_none());
    assert!(h.get_by_label("Unlock").is_disabled(), "nothing typed yet");
    h.state_mut().0.askpass.secret = "hunter2".into();
    h.run();
    assert!(h.query_by_label_contains("hunter2").is_none(), "the passphrase is masked");
    h.get_by_label("Unlock").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    drop(h);
    remove_fixture(&dir);
}
