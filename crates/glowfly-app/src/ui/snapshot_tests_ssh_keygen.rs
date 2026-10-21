//! The Generate SSH key dialog over Preferences > SSH: filled in, with its field and form
//! errors, and while ssh-keygen runs.

use super::snapshot_tests::harness;
use super::snapshot_tests_ssh::{HOME, listing, ssh_page, two_keys};
use crate::state::touched::Touched;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

/// The SSH page with the dialog open over it.
fn dialog(theme: ThemeChoice) -> AppState {
    let mut st = ssh_page(theme, Some(listing(two_keys(), true)));
    let p = &mut st.prefs.as_mut().unwrap().ssh;
    p.location = format!("{HOME}/.ssh/id_ed25519_laptop");
    p.prefill_comment("ada@example.com");
    st.modal = Some(Modal::SshKeygen);
    st
}

fn ssh_mut(st: &mut AppState) -> &mut crate::state::ssh::SshPage {
    &mut st.prefs.as_mut().unwrap().ssh
}

#[test]
fn home_is_absolute_on_this_os() {
    // The dialog's location is built from HOME; a relative one would be refused.
    assert!(std::path::Path::new(HOME).is_absolute(), "{HOME}");
}

#[test]
fn prefs_ssh_keygen() {
    let st = dialog(ThemeChoice::Dark);
    let mut h = harness(st, true);
    h.run();
    assert!(!h.get_by_label("Generate key").is_disabled(), "a filled form can run");
    assert!(h.query_by_label("Generate SSH key").is_some());
    h.check_snapshot("prefs_ssh_keygen");
}

#[test]
fn prefs_ssh_keygen_errors() {
    let mut st = dialog(ThemeChoice::Light);
    let p = ssh_mut(&mut st);
    p.location = format!("{HOME}/.ssh/id_ed25519");
    p.passphrase = "one".into();
    p.confirm = "two".into();
    p.touched = Touched::all();
    p.form_error = Some("ssh-keygen failed: Saving key failed: Permission denied".into());
    let mut h = harness(st, false);
    h.run();
    assert!(h.get_by_label("Generate key").is_disabled());
    assert!(h.query_by_label_contains("The passphrases don't match.").is_some());
    assert!(h.query_by_label_contains("A key already exists here").is_some());
    assert!(h.query_by_label_contains("Permission denied").is_some());
    h.check_snapshot("prefs_ssh_keygen_errors");
}

#[test]
fn prefs_ssh_keygen_generating() {
    let mut st = dialog(ThemeChoice::Dark);
    ssh_mut(&mut st).generating = true;
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_label("Generate key").is_disabled());
    assert!(h.query_by_label("Generating…").is_some());
    h.check_snapshot("prefs_ssh_keygen_generating");
}

#[test]
fn prefs_ssh_keygen_in_german() {
    // The field and footer reasons come from the language tables, not from English text.
    let mut st = dialog(ThemeChoice::Dark);
    st.settings.language = "de".into();
    let p = ssh_mut(&mut st);
    p.location = "keys/id".into();
    p.comment = "a	b".into();
    p.touched = Touched::all();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("Gib einen vollständigen Pfad ein").is_some(), "the location's reason");
    assert!(h.query_by_label_contains("Steuerzeichen").is_some(), "the comment's reason");
    assert!(h.query_by_label_contains("full path").is_none() && h.query_by_label_contains("control characters").is_none());
}
