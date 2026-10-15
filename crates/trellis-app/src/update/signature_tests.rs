//! Signature checks for the details panel's badge: asked for once per signed commit, never for
//! an unsigned one or while the repository's git is held, cached per commit and tab, and
//! forgotten on a manual refresh, when Preferences closes or on Check again.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg, RepoMsg, TabsMsg, TrustMsg};
use crate::state::AppState;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo;
use trellis_core::signature_check::{SigStatus, SignatureCheck, parse_check};
use trellis_core::signing::SigningFormat;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// The repository at `dir`, opened.
fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn head(st: &AppState) -> Oid {
    st.snap.as_ref().unwrap().commits[0].oid
}

/// Selects `oid` and delivers its detail, marked as signed in `signature`. Returns the effects
/// of the delivery.
fn show(st: &mut AppState, dir: &Path, oid: Oid, signature: Option<SigningFormat>) -> Vec<Effect> {
    run(st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    let mut d = repo::commit_detail(dir, oid).unwrap();
    d.signature = signature;
    run(st, Msg::Repo(RepoMsg::DetailLoaded { oid, result: Ok(d) }))
}

/// The VerifySignature effects in `fx`, as (oid, format, generation).
fn checks(fx: &[Effect]) -> Vec<(Oid, SigningFormat, u64)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::VerifySignature { oid, format, generation, .. } => Some((*oid, *format, *generation)),
            _ => None,
        })
        .collect()
}

fn good() -> Result<SignatureCheck, String> {
    Ok(parse_check(SigningFormat::OpenPgp, "G\0Ada\0ABCD\0ABCD\0ultimate\0gpg: Good\n", "").unwrap())
}

fn want(st: &mut AppState, oid: Oid) -> Vec<(Oid, SigningFormat, u64)> {
    checks(&run(st, Msg::Repo(RepoMsg::WantSignature(oid))))
}

#[test]
fn signed_detail_asks_once() {
    let dir = scratch_repo("sig-asks-once");
    let mut st = opened(&dir);
    let oid = head(&st);
    let fx = show(&mut st, &dir, oid, Some(SigningFormat::OpenPgp));
    let found = checks(&fx);
    assert_eq!(found.len(), 1, "one check");
    assert_eq!((found[0].0, found[0].1), (oid, SigningFormat::OpenPgp));
    assert!(st.signatures.pending(oid));
    assert!(want(&mut st, oid).is_empty(), "already on its way");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unsigned_detail_runs_nothing() {
    let dir = scratch_repo("sig-unsigned");
    let mut st = opened(&dir);
    let oid = head(&st);
    assert!(checks(&show(&mut st, &dir, oid, None)).is_empty());
    assert!(want(&mut st, oid).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn held_repo_is_not_checked() {
    let dir = scratch_repo("sig-held");
    git(&dir, &["config", "gpg.program", "evil"]);
    let mut st = opened(&dir);
    assert!(st.held(), "undecided");
    let oid = head(&st);
    assert!(checks(&show(&mut st, &dir, oid, Some(SigningFormat::OpenPgp))).is_empty());
    assert!(want(&mut st, oid).is_empty());
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    assert_eq!(want(&mut st, oid).len(), 1, "trusting lets the check run");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `scratch_repo` with a second commit; returns (dir, older, newer).
fn two_commits(name: &str) -> (PathBuf, AppState, Oid, Oid) {
    let dir = scratch_repo(name);
    git(&dir, &["commit", "-q", "-am", "second"]);
    let st = opened(&dir);
    let snap = st.snap.as_ref().unwrap();
    let (newer, older) = (snap.commits[0].oid, snap.commits[1].oid);
    (dir, st, older, newer)
}

#[test]
fn result_cached_per_commit() {
    let (dir, mut st, older, newer) = two_commits("sig-cached");
    let fx = show(&mut st, &dir, newer, Some(SigningFormat::OpenPgp));
    let [(_, _, generation)] = checks(&fx)[..] else { panic!("one check") };
    run(&mut st, Msg::Repo(RepoMsg::SignatureChecked { oid: newer, generation, result: good() }));
    assert!(matches!(st.signatures.get(newer), Some(Ok(c)) if c.status == SigStatus::Good));
    assert!(!st.signatures.pending(newer));
    assert!(checks(&show(&mut st, &dir, older, None)).is_empty());
    assert!(checks(&show(&mut st, &dir, newer, Some(SigningFormat::OpenPgp))).is_empty(), "the cached result is reused");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_generation_ignored() {
    let dir = scratch_repo("sig-stale");
    let mut st = opened(&dir);
    let oid = head(&st);
    let [(_, _, old)] = checks(&show(&mut st, &dir, oid, Some(SigningFormat::OpenPgp)))[..] else { panic!("one check") };
    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    run(&mut st, Msg::Repo(RepoMsg::SignatureChecked { oid, generation: old, result: good() }));
    assert!(st.signatures.get(oid).is_none(), "a reply from before the clear is ignored");
    let [(_, _, newer)] = want(&mut st, oid)[..] else { panic!("asked again") };
    assert!(newer > old);
    run(&mut st, Msg::Repo(RepoMsg::SignatureChecked { oid, generation: newer, result: good() }));
    assert!(st.signatures.get(oid).is_some());
    run(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.signatures.get(oid).is_none() && st.signatures.wanted(oid), "a manual refresh forgets the checks");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_again() {
    let dir = scratch_repo("sig-again");
    let mut st = opened(&dir);
    let oid = head(&st);
    let [(_, _, generation)] = checks(&show(&mut st, &dir, oid, Some(SigningFormat::Ssh)))[..] else { panic!("one check") };
    run(&mut st, Msg::Repo(RepoMsg::SignatureChecked { oid, generation, result: Err("boom".into()) }));
    assert!(st.signatures.get(oid).is_some());
    assert!(st.log.iter().all(|l| !l.text.contains("boom")), "failures show in the badge, not the log");
    let fx = run(&mut st, Msg::Repo(RepoMsg::CheckSignatureAgain(oid)));
    assert!(st.signatures.get(oid).is_none());
    assert_eq!(checks(&fx).iter().map(|c| (c.0, c.1)).collect::<Vec<_>>(), [(oid, SigningFormat::Ssh)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parked_tab_asks_again() {
    let (dir, mut st, older, newer) = two_commits("sig-parked-a");
    let [(_, _, generation)] = checks(&show(&mut st, &dir, older, Some(SigningFormat::OpenPgp)))[..] else { panic!("one check") };
    run(&mut st, Msg::Repo(RepoMsg::SignatureChecked { oid: older, generation, result: good() }));
    assert_eq!(checks(&show(&mut st, &dir, newer, Some(SigningFormat::OpenPgp))).len(), 1);
    assert!(st.signatures.pending(newer));

    let other = scratch_repo("sig-parked-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    assert!(st.signatures.get(older).is_none(), "the other tab has its own checks");
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(st.signatures.wanted(newer), "the check in flight was dropped with the tab");
    assert!(st.signatures.get(older).is_some(), "results survive the round trip");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

/// Signs a new commit in `dir` with a fresh SSH key, listed for test@example.com in a local
/// allowed signers file. None (after saying so) when ssh-keygen or signing isn't available.
fn ssh_signed_head(dir: &Path) -> Option<Oid> {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let key = dir.with_file_name(format!("{name}-key"));
    let _ = std::fs::remove_file(&key);
    let _ = std::fs::remove_file(dir.with_file_name(format!("{name}-key.pub")));
    let made = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test@example.com", "-f"])
        .arg(&key)
        .stdin(std::process::Stdio::null())
        .output();
    if !made.as_ref().is_ok_and(|o| o.status.success()) {
        eprintln!("ssh-keygen unavailable or signing failed, skipping: {made:?}");
        return None;
    }
    let public = dir.with_file_name(format!("{name}-key.pub"));
    let config = dir.with_file_name(format!("{name}-gitconfig"));
    std::fs::write(&config, "").unwrap();
    let env = [("GIT_CONFIG_GLOBAL", config.to_str().unwrap()), ("GIT_CONFIG_NOSYSTEM", "1")];
    let signing_key = format!("user.signingkey={}", public.display());
    let a = trellis_core::gitcli::args(&["-c", "gpg.format=ssh", "-c", &signing_key, "commit", "-q", "-S", "-am", "signed"]);
    let (ok, out) = trellis_core::gitcli::run_with_input(dir, &a, "", &env);
    if !ok {
        eprintln!("ssh-keygen unavailable or signing failed, skipping: {out}");
        return None;
    }
    let allowed = dir.with_file_name(format!("{name}-allowed"));
    std::fs::write(&allowed, format!("test@example.com {}", std::fs::read_to_string(&public).unwrap())).unwrap();
    git(dir, &["config", "gpg.ssh.allowedSignersFile", &allowed.display().to_string()]);
    Some(git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap())
}

#[test]
fn e2e_ssh_signature_verifies() {
    let dir = scratch_repo("sig-e2e");
    let Some(oid) = ssh_signed_head(&dir) else { return };
    let mut st = opened(&dir);
    assert_eq!(head(&st), oid);
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    assert_eq!(st.detail.as_ref().and_then(|d| d.signature), Some(SigningFormat::Ssh));
    match st.signatures.get(oid) {
        Some(Ok(c)) => assert_eq!((c.status, c.signer.as_str()), (SigStatus::Good, "test@example.com"), "{c:?}"),
        other => panic!("not checked: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    for f in ["key", "key.pub", "gitconfig", "allowed"] {
        let _ = std::fs::remove_file(dir.with_file_name(format!("{name}-{f}")));
    }
}
