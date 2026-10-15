//! Preferences > SSH snapshots: two key pairs and a filled form, no keys without ssh-keygen,
//! the form's warnings, and the folder still being read. Built from state only: no real home
//! folder or ssh-keygen.

use super::snapshot_tests::{harness, render, settings};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::settings::ThemeChoice;
use trellis_core::ssh_keys::{SshKeyPair, SshListing};

const HOME: &str = "C:/Users/ada";

fn key(name: &str, kind: &str, bits: Option<u32>, fingerprint: Option<&str>, comment: &str) -> SshKeyPair {
    let dir = PathBuf::from(HOME).join(".ssh");
    SshKeyPair {
        private: dir.join(name),
        public: dir.join(format!("{name}.pub")),
        key_type: if kind == "RSA" { "ssh-rsa".into() } else { "ssh-ed25519".into() },
        kind: kind.into(),
        bits,
        fingerprint: fingerprint.map(str::to_owned),
        comment: comment.into(),
        public_key: format!("ssh-ed25519 AAAA {comment}"),
    }
}

fn two_keys() -> Vec<SshKeyPair> {
    vec![
        key("id_ed25519", "ED25519", Some(256), Some("SHA256:3n9kV2qXcP0yLw8bTfR6uHjA1sZmDe4oGiNtYxKvQ7c"), "ada@example.com"),
        key("id_rsa_work", "RSA", Some(3072), Some("SHA256:Zp5LrW0eVb7nQk2HsXy9MfTc4GaUoJd1iCtBq8NwE3s"), ""),
    ]
}

/// Preferences open on the SSH page with `listing` (None: still reading).
fn ssh_page(theme: ThemeChoice, listing: Option<SshListing>) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Ssh);
    prefs.ssh.generation = 1;
    prefs.ssh.listing = listing;
    prefs.ssh.fill_location();
    st.prefs = Some(prefs);
    st
}

fn listing(keys: Vec<SshKeyPair>, keygen: bool) -> SshListing {
    SshListing {
        home: HOME.into(),
        dir: PathBuf::from(HOME).join(".ssh"),
        keys,
        keygen: keygen.then(|| PathBuf::from("C:/Windows/System32/OpenSSH/ssh-keygen.exe")),
    }
}

#[test]
fn prefs_ssh() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let p = &mut st.prefs.as_mut().unwrap().ssh;
    p.location = "C:/Users/ada/.ssh/id_ed25519_new".into();
    p.prefill_comment("ada@example.com");
    render("prefs_ssh", st, true);
}

#[test]
fn prefs_ssh_empty() {
    let st = ssh_page(ThemeChoice::Light, Some(listing(vec![], false)));
    assert!(!st.prefs.as_ref().unwrap().ssh.can_generate());
    render("prefs_ssh_empty", st, false);
}

#[test]
fn prefs_ssh_form_errors() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let p = &mut st.prefs.as_mut().unwrap().ssh;
    p.location = "C:/Users/ada/.ssh/id_ed25519".into();
    p.comment = "ada@example.com".into();
    p.passphrase = "one".into();
    p.confirm = "two".into();
    p.form_error = Some("ssh-keygen failed: Saving key failed: Permission denied".into());
    render("prefs_ssh_form_errors", st, true);
}

#[test]
fn prefs_ssh_loading() {
    let st = ssh_page(ThemeChoice::Dark, None);
    // The spinner keeps repainting; a fixed number of frames (egui's clock advances by a fixed
    // step) keeps the image reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("prefs_ssh_loading");
}
