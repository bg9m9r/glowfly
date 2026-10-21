use super::protocol::SshSignAsk;
use super::ssh_sign::{ask_for, private_key_path, wrong_passphrase};

fn ask(key: &str) -> Option<SshSignAsk> {
    Some(SshSignAsk { key: key.into() })
}

#[test]
fn ssh_keygens_prompts_become_signing_asks() {
    let marker = Some("/k/id_ed25519.pub");
    assert_eq!(ask_for("Enter passphrase: ", marker), ask("/k/id_ed25519"), "the configured file's private half");
    assert_eq!(ask_for("Enter passphrase for \"/k/other\": ", marker), ask("/k/other"), "the prompt names the key");
    assert_eq!(ask_for("Enter passphrase for \"\": ", marker), ask("/k/id_ed25519"));
    assert_eq!(ask_for("Enter passphrase:", Some("/k/id_ed25519-cert.pub")), ask("/k/id_ed25519"));
    assert_eq!(ask_for("Enter passphrase: ", Some("/k/id_ed25519")), ask("/k/id_ed25519"));
    assert_eq!(private_key_path("/k/a.pub"), "/k/a");
    assert_eq!(private_key_path("/k/a-cert.pub"), "/k/a");
    assert_eq!(private_key_path("/k/a"), "/k/a");
}

#[test]
fn other_passphrase_prompts_are_left_alone() {
    for prompt in ["Enter passphrase: ", "Enter passphrase for \"/k/id\": "] {
        assert_eq!(ask_for(prompt, None), None, "no marker");
        assert_eq!(ask_for(prompt, Some("")), None, "an empty marker");
    }
    let marker = Some("/k/id_ed25519.pub");
    assert_eq!(ask_for("Enter passphrase for key '/k/id_rsa': ", marker), None, "ssh's connection prompt");
    assert_eq!(ask_for("Enter passphrase for /k/id: ", marker), None, "ssh-add's prompt");
    assert_eq!(ask_for("Password for 'https://x': ", marker), None);
}

#[test]
fn a_wrong_passphrase_is_recognised() {
    let out = "error: Load key \"/k/id\": incorrect passphrase supplied to decrypt private key\nfatal: failed to write commit object";
    assert!(wrong_passphrase(out));
    assert!(!wrong_passphrase("error: gpg failed to sign the data"));
}
