//! Headless tests for Preferences > SSH: reading ~/.ssh, the key choice and the agent,
//! prefilling the form, generating a key (refusals, the Replace confirmation, success and
//! failure) and copying a public key. The Generate SSH key dialog is in `ssh_keygen_tests`,
//! Use existing in `ssh_pick_tests`.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::PrefsEffect;
use crate::effect::{Effect, UiEffect};
use crate::msg::{Msg, PrefsMsg, RepoMsg};
use crate::ssh::{GenerateFailure, SshEffect, SshMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::ssh::SshPage;
use crate::state::{AppState, Modal, PrefPage};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::RepoTrust;
use glowfly_core::ssh_keys::{AgentStatus, KeyChoice, SshKeyPair, SshListing, default_location};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

pub(super) fn ssh(st: &mut AppState, m: SshMsg) -> Vec<Effect> {
    run(st, Msg::Ssh(m))
}

pub(super) fn home() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "C:/h" } else { "/h" })
}

pub(super) fn pair(name: &str, comment: &str) -> SshKeyPair {
    let private = home().join(".ssh").join(name);
    SshKeyPair {
        public: home().join(".ssh").join(format!("{name}.pub")),
        private,
        key_type: "ssh-ed25519".into(),
        kind: "ED25519".into(),
        bits: Some(256),
        fingerprint: Some(format!("SHA256:{name}")),
        comment: comment.into(),
        public_key: format!("ssh-ed25519 AAAA{name} {comment}"),
    }
}

pub(super) fn listing(keys: Vec<SshKeyPair>, keygen: bool) -> SshListing {
    SshListing {
        home: home(),
        dir: home().join(".ssh"),
        keys,
        keygen: keygen.then(|| PathBuf::from("C:/bin/ssh-keygen.exe")),
        config_file: home().join(".ssh").join("config"),
        ..Default::default()
    }
}

pub(super) fn page(st: &AppState) -> &SshPage {
    &st.prefs.as_ref().expect("Preferences open").ssh
}

pub(super) fn page_mut(st: &mut AppState) -> &mut SshPage {
    &mut st.prefs.as_mut().expect("Preferences open").ssh
}

pub(super) fn lists(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::List { generation, .. }) => Some(*generation),
            _ => None,
        })
        .collect()
}

pub(super) fn email(st: &mut AppState, email: &str) {
    let values = GitConfigValues { user_email: email.into(), ..Default::default() };
    run(st, Msg::Prefs(PrefsMsg::GitConfigLoaded(values)));
}

/// Preferences open on the SSH page with `keys` listed.
pub(super) fn loaded(keys: Vec<SshKeyPair>, keygen: bool) -> AppState {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    let generation = page(&st).generation;
    ssh(&mut st, SshMsg::Loaded { generation, result: Ok(listing(keys, keygen)) });
    st
}

#[test]
fn show_page_reads_and_fills() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
    let fx = ssh(&mut st, SshMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().unwrap().page, PrefPage::Ssh);
    assert_eq!(lists(&fx), [1]);
    assert!(page(&st).listing.is_none(), "loading");

    email(&mut st, "ada@example.com");
    assert_eq!(page(&st).comment, "ada@example.com");

    ssh(&mut st, SshMsg::Loaded { generation: 1, result: Ok(listing(vec![], true)) });
    assert!(page(&st).listing.is_some());
    assert_eq!(page(&st).location, default_location(&home()).display().to_string());
    assert!(page(&st).location.starts_with(&*home().to_string_lossy()));

    // The nav row again, with the config already read: the comment stays as it is.
    page_mut(&mut st).comment = "work".into();
    ssh(&mut st, SshMsg::ShowPage);
    assert_eq!(page(&st).comment, "work");
}

#[test]
fn opening_on_the_page_reads_it() {
    let mut st = state();
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    assert_eq!(lists(&fx), [1]);
    assert!(fx.iter().any(|e| matches!(e, Effect::Prefs(PrefsEffect::ReadGitConfig { .. }))));
    assert!(Effect::Ssh(SshEffect::List { generation: 1, extra: Vec::new() }).scope().is_none(), "unscoped");
}

#[test]
fn stale_failed_and_closed_results() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    assert_eq!(lists(&ssh(&mut st, SshMsg::Reload)), [2]);
    ssh(&mut st, SshMsg::Loaded { generation: 1, result: Ok(listing(vec![], true)) });
    assert!(page(&st).listing.is_none(), "an older read is ignored");
    ssh(&mut st, SshMsg::Loaded { generation: 2, result: Err("Couldn't find your home folder.".into()) });
    assert_eq!(page(&st).error.as_deref(), Some("Couldn't find your home folder."));

    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    let fx = ssh(&mut st, SshMsg::Loaded { generation: 2, result: Ok(listing(vec![], true)) });
    assert!(st.prefs.is_none() && fx.is_empty());
    assert!(lists(&ssh(&mut st, SshMsg::Reload)).is_empty());
}

#[test]
fn prefill_happens_once() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    email(&mut st, "ada@example.com");
    page_mut(&mut st).comment.clear();
    email(&mut st, "ada@example.com");
    assert_eq!(page(&st).comment, "", "a cleared comment stays cleared");

    page_mut(&mut st).location = "~/.ssh/work".into();
    let generation = page(&st).generation;
    ssh(&mut st, SshMsg::Loaded { generation, result: Ok(listing(vec![], true)) });
    assert_eq!(page(&st).location, "~/.ssh/work", "a typed location is kept");
}

pub(super) fn fill(st: &mut AppState, location: &str, comment: &str, passphrase: &str, confirm: &str) {
    let p = page_mut(st);
    p.location = location.into();
    p.comment = comment.into();
    p.passphrase = passphrase.into();
    p.confirm = confirm.into();
}

#[test]
fn generate_sends_the_form() {
    let mut st = loaded(vec![], true);
    fill(&mut st, "~/.ssh/work", " ada ", "s3cret", "s3cret");
    let fx = ssh(&mut st, SshMsg::Generate { overwrite: false });
    let [Effect::Ssh(SshEffect::Generate { key, overwrite: false, .. })] = &fx[..] else { panic!("one Generate effect, got {fx:?}") };
    assert_eq!(key.path, home().join(".ssh").join("work"));
    assert_eq!((key.comment.as_str(), key.passphrase.as_str()), ("ada", "s3cret"));
    assert!(page(&st).generating);
    assert!(!format!("{fx:?}").contains("s3cret"), "the passphrase stays out of Debug output");

    assert!(ssh(&mut st, SshMsg::Generate { overwrite: false }).is_empty(), "one generation at a time");
}

#[test]
fn generate_is_refused_with_a_reason() {
    let refused = |st: &mut AppState, why: &str| {
        let fx = ssh(st, SshMsg::Generate { overwrite: false });
        assert!(fx.is_empty(), "{why}: no effect");
        assert!(page(st).form_error.is_some(), "{why}: a reason");
        assert!(!page(st).generating);
    };
    let mut st = loaded(vec![], true);
    fill(&mut st, "~/.ssh/work", "", "a", "b");
    refused(&mut st, "mismatch");
    assert_eq!(page(&st).form_error.as_deref(), Some("The passphrases don't match."));
    fill(&mut st, "keys/id", "", "", "");
    refused(&mut st, "relative");
    fill(&mut st, "~/.ssh/work", "a\nb", "", "");
    refused(&mut st, "newline");

    let mut st = loaded(vec![], false);
    fill(&mut st, "~/.ssh/work", "", "", "");
    refused(&mut st, "no ssh-keygen");
    assert_eq!(page(&st).form_error.as_deref(), Some("ssh-keygen wasn't found."));

    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    fill(&mut st, "C:/x/id", "", "", "");
    refused(&mut st, "not loaded");
}

#[test]
fn replace_flow() {
    let mut st = loaded(vec![pair("id_ed25519", "ada")], true);
    ssh(&mut st, SshMsg::OpenGenerate);
    fill(&mut st, "~/.ssh/id_ed25519", "ada", "s3cret", "s3cret");
    assert!(page(&st).location_taken());
    ssh(&mut st, SshMsg::Generate { overwrite: false });
    assert!(matches!(st.modal, Some(Modal::SshKeygen)), "open while ssh-keygen runs");
    let path = home().join(".ssh").join("id_ed25519");
    ssh(&mut st, SshMsg::Generated(Err(GenerateFailure::Exists(path.clone()))));
    assert!(!page(&st).generating);
    let Some(Modal::ConfirmDanger { title, body, button, .. }) = &st.modal else { panic!("the confirmation replaces the dialog") };
    assert_eq!((title.as_str(), button.as_str()), ("Replace SSH key", "Replace key"));
    assert!(body.starts_with(&path.display().to_string()));

    let fx = run(&mut st, Msg::ConfirmAccepted);
    let [Effect::Ssh(SshEffect::Generate { key, overwrite: true, .. })] = &fx[..] else { panic!("Generate with overwrite, got {fx:?}") };
    assert_eq!(key.passphrase, "s3cret", "the same passphrase");
}

#[test]
fn generated_reloads_and_clears() {
    let mut st = loaded(vec![], true);
    ssh(&mut st, SshMsg::OpenGenerate);
    fill(&mut st, "~/.ssh/id_ed25519", "", "s3cret", "s3cret");
    ssh(&mut st, SshMsg::Generate { overwrite: false });
    let fx = ssh(&mut st, SshMsg::Generated(Ok(pair("id_ed25519", "ada"))));
    assert!(st.modal.is_none(), "the dialog closes");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Created SSH key id_ed25519"));
    assert_eq!((page(&st).passphrase.as_str(), page(&st).confirm.as_str()), ("", ""));
    assert!(!page(&st).generating);
    assert_eq!(lists(&fx).len(), 1, "the list reloads");

    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    let fx = ssh(&mut st, SshMsg::Generated(Ok(pair("id_work", ""))));
    assert!(lists(&fx).is_empty());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Created SSH key id_work"));
}

#[test]
fn generate_failure_is_shown_and_logged() {
    let mut st = loaded(vec![], true);
    ssh(&mut st, SshMsg::Generated(Err(GenerateFailure::Failed("ssh-keygen failed: boom".into()))));
    assert_eq!(page(&st).form_error.as_deref(), Some("ssh-keygen failed: boom"));
    let entry = st.log.last().expect("a log entry");
    assert!(!entry.ok && entry.text.starts_with("Could not generate an SSH key"), "{}", entry.text);
}

#[test]
fn copy_public_key() {
    let second = pair("id_work", "work");
    let mut st = loaded(vec![pair("id_ed25519", "ada"), second.clone()], true);
    let fx = ssh(&mut st, SshMsg::CopyPublicKey(second.private.clone()));
    let [Effect::Ui(UiEffect::CopyText(text))] = &fx[..] else { panic!("a copy, got {fx:?}") };
    assert_eq!(text, &second.public_key);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Public key copied to the clipboard"));
    assert!(ssh(&mut st, SshMsg::CopyPublicKey(PathBuf::from("C:/nowhere/id"))).is_empty());
}

#[test]
fn page_rules() {
    let mut st = loaded(vec![pair("id_ed25519", "ada")], true);
    let listed = home().join(".ssh").join("id_ed25519").display().to_string();
    fill(&mut st, &listed, "", "", "");
    assert!(page(&st).location_taken());
    // Case is the volume's business (a folder may be case-sensitive even on Windows): the
    // listing is matched by spelling, and generating refuses an existing file on disk.
    fill(&mut st, &listed.to_uppercase(), "", "", "");
    assert!(!page(&st).location_taken(), "spelling never folds case");
    fill(&mut st, "~/.ssh/other", "", "", "");
    assert!(!page(&st).location_taken());
    assert!(page(&st).can_generate());

    let en = Lang::En;
    let mut p = SshPage { generating: true, passphrase: "a".into(), ..Default::default() };
    assert_eq!(p.blocked(en), Some("A key is being generated."));
    p.generating = false;
    assert!(p.blocked(en).is_some_and(|b| b.contains(".ssh folder")), "listing first");
    assert_eq!(p.location_problem(), None, "no problem shown before the home folder is known");
    p.listing = Some(listing(vec![], true));
    assert_eq!(p.blocked(en), Some("Enter where to save the private key."));
    p.location = "~/.ssh/k".into();
    p.comment = "a\tb".into();
    assert!(p.blocked(en).is_some_and(|b| b.contains("control characters")));
    p.comment.clear();
    assert_eq!(p.blocked(en), Some("The passphrases don't match."));
    p.confirm = "a".into();
    assert_eq!(p.blocked(en), None);
    assert!(crate::state::ssh::replace_body(Path::new("C:/k/id")).contains("will stop accepting it"));
}

fn read_choices(fx: &[Effect]) -> Vec<(u64, Option<PathBuf>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::ReadChoice { generation, repo }) => Some((*generation, repo.clone())),
            _ => None,
        })
        .collect()
}

fn agents(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::Agent { generation }) => Some(*generation),
            _ => None,
        })
        .collect()
}

#[test]
fn show_page_reads_the_choice_and_the_agent() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
    let fx = ssh(&mut st, SshMsg::ShowPage);
    assert_eq!((lists(&fx), read_choices(&fx), agents(&fx)), (vec![1], vec![(1, None)], vec![1]));
    assert!(page(&st).repo.is_none(), "no repository is open");

    let unset = || Ok(KeyChoice::Unset);
    ssh(&mut st, SshMsg::ChoiceLoaded { generation: 0, global: unset(), repo: None });
    assert!(page(&st).choice.is_none(), "an older read is ignored");
    ssh(&mut st, SshMsg::ChoiceLoaded { generation: 1, global: unset(), repo: None });
    assert_eq!(page(&st).choice.as_ref().map(|c| c.global.clone()), Some(unset()));

    assert_eq!(agents(&ssh(&mut st, SshMsg::ReloadAgent)), [2]);
    ssh(&mut st, SshMsg::AgentLoaded { generation: 1, status: AgentStatus::default() });
    assert!(page(&st).agent.is_none(), "an older check is ignored");
    ssh(&mut st, SshMsg::AgentLoaded { generation: 2, status: AgentStatus::default() });
    assert!(page(&st).agent.is_some());

    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    assert!(ssh(&mut st, SshMsg::ChoiceLoaded { generation: 1, global: unset(), repo: None }).is_empty());
    assert!(ssh(&mut st, SshMsg::ReloadAgent).is_empty(), "nothing to check for");
}

#[test]
fn an_open_repository_is_read_too() {
    let dir = scratch_repo("ssh-choice-repo");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let workdir = st.snap.as_ref().expect("opened").workdir.clone();
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    assert_eq!(read_choices(&fx), [(1, Some(workdir.clone()))]);
    let repo = page(&st).repo.clone().expect("the repository");
    assert_eq!(repo.workdir, workdir);
    assert!(!repo.trusted, "clean, but not trusted by the user");

    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);
    ssh(&mut st, SshMsg::Reload);
    assert!(page(&st).repo.as_ref().is_some_and(|r| r.trusted), "follows the saved decision");
    let _ = std::fs::remove_dir_all(&dir);
}
