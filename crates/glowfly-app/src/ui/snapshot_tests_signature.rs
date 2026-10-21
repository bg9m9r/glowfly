//! The details panel's signature badge: Verified with its tooltip, a check that needs an allowed
//! signers file, a bad signature, an unchecked one in a held repository, and no badge at all
//! for an unsigned commit. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use crate::msg::{Msg, RepoMsg};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::signature_check::{SignatureCheck, parse_check};
use glowfly_core::signing::SigningFormat;
use std::path::PathBuf;

const KEY: &str = "SHA256:nu3mXdiXjT/P69lIp2s2NqrOSBxSCmevSqfmgZp4JuA";

/// The fixture with HEAD selected, its detail marked as signed with `signature`.
pub(super) fn with_signed_head(theme: ThemeChoice, name: &str, signature: Option<SigningFormat>) -> (AppState, PathBuf, Oid) {
    let (mut st, dir) = with_repo(theme, name);
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.selection = Selection::Commit(oid);
    let mut d = repo::commit_detail(&dir, oid).unwrap();
    d.signature = signature;
    st.detail = Some(d);
    (st, dir, oid)
}

/// `st` with a finished check of `oid` cached.
pub(super) fn checked(st: &mut AppState, oid: Oid, result: Result<SignatureCheck, String>) {
    let generation = st.signatures.start(oid);
    st.signatures.finish(oid, generation, result);
}

pub(super) fn ssh(stdout: &str, stderr: &str) -> Result<SignatureCheck, String> {
    Ok(parse_check(SigningFormat::Ssh, stdout, stderr).unwrap())
}

fn hover_shows(name: &str, st: AppState, dark: bool, label: &str, tip: &str) {
    let mut h = harness(st, dark);
    h.run();
    h.get_by_label(label).hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains(tip).is_some(), "the tooltip says {tip:?}");
    h.check_snapshot(name);
}

#[test]
fn signature_verified() {
    let (mut st, dir, oid) = with_signed_head(ThemeChoice::Dark, "sig-verified", Some(SigningFormat::Ssh));
    let out = format!("G\0ada@example.com\0{KEY}\0{KEY}\0fully\0Good \"git\" signature for ada@example.com\n");
    checked(&mut st, oid, ssh(&out, ""));
    hover_shows("signature_verified", st, true, "Verified", "allowed signers file lists");
    remove_fixture(&dir);
}

#[test]
fn signature_no_allowed_signers() {
    let (mut st, dir, oid) = with_signed_head(ThemeChoice::Light, "sig-no-allowed", Some(SigningFormat::Ssh));
    let stderr = "error: gpg.ssh.allowedSignersFile needs to be configured and exist for ssh signature verification\n";
    checked(&mut st, oid, ssh("N\0\0\0\0undefined\0\n", stderr));
    hover_shows("signature_no_allowed_signers", st, false, "Signed", "Preferences > Commit signing");
    remove_fixture(&dir);
}

#[test]
fn signature_bad() {
    let (mut st, dir, oid) = with_signed_head(ThemeChoice::Dark, "sig-bad", Some(SigningFormat::Ssh));
    checked(&mut st, oid, ssh("B\0\0\0\0never\0Could not verify signature.\n", ""));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Bad signature").is_some());
    h.check_snapshot("signature_bad");
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::WantSignature(_)))), "nothing to ask for");
    remove_fixture(&dir);
}

#[test]
fn signature_badge_held() {
    let (mut st, dir, _) = with_signed_head(ThemeChoice::Dark, "sig-held", Some(SigningFormat::OpenPgp));
    git(&dir, &["config", "gpg.program", "evil"], "2026-01-05T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    assert!(st.held());
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Signed").hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("you haven't trusted this repository").is_some());
    h.check_snapshot("signature_not_checked");
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::WantSignature(_)))), "held: nothing runs");
    remove_fixture(&dir);
}

#[test]
fn signed_commit_asks_for_its_check() {
    let (st, dir, oid) = with_signed_head(ThemeChoice::Dark, "sig-asks", Some(SigningFormat::Ssh));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Signed").is_some(), "grey while checking");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::WantSignature(o)) if *o == oid)));
    remove_fixture(&dir);
}

#[test]
fn unsigned_commit_has_no_badge() {
    let (st, dir, _) = with_signed_head(ThemeChoice::Dark, "sig-unsigned", None);
    let mut h = harness(st, true);
    h.run();
    for label in ["Verified", "Signed", "Bad signature"] {
        assert!(h.query_by_label(label).is_none(), "{label}");
    }
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::WantSignature(_)))));
    remove_fixture(&dir);
}
