//! Headless tests for the key git uses (Preferences > SSH): choosing a key and Let ssh decide
//! at each level, the confirmations for a command Trellis didn't write, the trust rule for a
//! repository's own setting, and the Use local SSH agent setting.

use super::ssh_tests::{listing, loaded, page, page_mut, pair, run, ssh};
use super::tests::{drive, finish, scratch_repo, state};
use super::trust_tests::untrusted_repo;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg, RepoMsg};
use crate::ssh::{SshEffect, SshMsg};
use crate::state::ssh_use::{Choices, RepoTarget, not_trusted};
use crate::state::trust::{Standing, record};
use crate::state::{AppState, Modal, PrefPage};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::RepoTrust;
use glowfly_core::ssh_keys::{KeyChoice, SshKeyPair};
use std::path::PathBuf;

fn key() -> SshKeyPair {
    pair("id_ed25519", "ada")
}

fn other(value: &str) -> KeyChoice {
    KeyChoice::Other { value: value.into(), key: None }
}

/// Preferences on the SSH page with `key()` listed and core.sshCommand read as given.
fn chosen(global: KeyChoice, repo: Option<(KeyChoice, bool)>) -> AppState {
    let mut st = loaded(vec![key()], true);
    let p = page_mut(&mut st);
    if let Some((_, trusted)) = &repo {
        p.repo = Some(RepoTarget { name: "app".into(), workdir: "C:/r/app".into(), real: "C:/r/app".into(), trusted: *trusted });
    }
    p.choice = Some(Choices { global: Ok(global), repo: repo.map(|(c, _)| Ok(c)) });
    st
}

/// The SetKeyChoice effects: (repository, key).
fn sets(fx: &[Effect]) -> Vec<(Option<PathBuf>, Option<PathBuf>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::SetKeyChoice { repo, key }) => Some((repo.clone(), key.clone())),
            _ => None,
        })
        .collect()
}

fn use_key(repo: bool) -> SshMsg {
    SshMsg::UseKey { key: key().private, repo, confirmed: false }
}

#[test]
fn use_for_all_repositories() {
    let mut st = chosen(KeyChoice::Unset, None);
    let fx = ssh(&mut st, use_key(false));
    assert_eq!(sets(&fx), [(None, Some(key().private))]);
    assert!(page(&st).choice_busy);
    assert!(ssh(&mut st, use_key(false)).is_empty(), "one write at a time");

    let fx = ssh(&mut st, SshMsg::KeyChoiceSet { repo: false, result: Ok(()) });
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("git now uses id_ed25519 for SSH remotes (all repositories)"));
    assert!(!page(&st).choice_busy && page(&st).choice.is_none());
    let reads: Vec<u64> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::ReadChoice { generation, .. }) => Some(*generation),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [page(&st).choice_generation]);
    assert!(page(&st).choice_generation >= 2, "a new read");
}

#[test]
fn a_failed_write_is_shown_and_logged() {
    let mut st = chosen(KeyChoice::Unset, None);
    ssh(&mut st, use_key(false));
    let fx = ssh(&mut st, SshMsg::KeyChoiceSet { repo: false, result: Err("git config core.sshCommand failed: locked".into()) });
    assert!(fx.is_empty());
    assert_eq!(page(&st).choice_error.as_deref(), Some("git config core.sshCommand failed: locked"));
    let entry = st.log.last().expect("logged");
    assert!(!entry.ok && entry.text.starts_with("Could not change core.sshCommand:"), "{}", entry.text);
}

#[test]
fn replacing_a_foreign_command_asks_first() {
    let mut st = chosen(other("ssh -v"), None);
    assert!(ssh(&mut st, use_key(false)).is_empty());
    let Some(Modal::Confirm { title, body, button, then }) = &st.modal else { panic!("a confirmation") };
    assert_eq!((title.as_str(), button.as_str()), ("Replace SSH command", "Replace"));
    assert!(body.contains("all repositories") && body.contains("ssh -v"), "{body}");
    assert!(matches!(**then, Msg::Ssh(SshMsg::UseKey { repo: false, confirmed: true, .. })));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(sets(&fx), [(None, Some(key().private))]);
}

#[test]
fn stop_using() {
    let mut st = chosen(KeyChoice::Key(key().private), None);
    assert_eq!(sets(&ssh(&mut st, SshMsg::StopUsing { repo: false, confirmed: false })), [(None, None)]);
    ssh(&mut st, SshMsg::KeyChoiceSet { repo: false, result: Ok(()) });
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("git no longer uses a chosen key (all repositories)"));

    let mut st = chosen(other("plink -batch"), None);
    assert!(ssh(&mut st, SshMsg::StopUsing { repo: false, confirmed: false }).is_empty());
    let Some(Modal::Confirm { title, button, .. }) = &st.modal else { panic!("a confirmation") };
    assert_eq!((title.as_str(), button.as_str()), ("Remove SSH command", "Remove"));
    assert_eq!(sets(&run(&mut st, Msg::ConfirmAccepted)), [(None, None)]);

    let mut st = chosen(KeyChoice::Unset, None);
    assert!(ssh(&mut st, SshMsg::StopUsing { repo: false, confirmed: false }).is_empty(), "nothing to stop");
}

#[test]
fn let_ssh_decide_runs_with_the_agent_on() {
    // The Which key dropdown keeps Let ssh decide enabled while Use local SSH agent is on: an
    // unset core.sshCommand is what that setting wants.
    let mut st = chosen(KeyChoice::Key(key().private), None);
    st.settings.use_ssh_agent = true;
    assert_eq!(sets(&ssh(&mut st, SshMsg::StopUsing { repo: false, confirmed: false })), [(None, None)]);
    assert!(page(&st).choice_busy && !page(&st).choice_repo, "the all-repositories row shows the spinner");
}

#[test]
fn refusals() {
    let mut st = chosen(KeyChoice::Unset, Some((KeyChoice::Unset, false)));
    assert!(ssh(&mut st, use_key(true)).is_empty());
    assert_eq!(page(&st).choice_error.as_deref(), Some(not_trusted(Lang::En)));

    let mut st = chosen(KeyChoice::Unset, None);
    assert!(ssh(&mut st, use_key(true)).is_empty(), "no repository");
    st.settings.use_ssh_agent = true;
    assert!(ssh(&mut st, use_key(false)).is_empty());
    assert_eq!(page(&st).choice_error.as_deref(), Some(Lang::En.tr("prefs.ssh.which.agent_on.tip")));
    assert!(ssh(&mut st, SshMsg::UseKey { key: "C:/elsewhere/id".into(), repo: false, confirmed: false }).is_empty(), "not listed");

    // A trusted repository that is no longer the open one.
    let mut st = chosen(KeyChoice::Unset, Some((KeyChoice::Unset, true)));
    assert!(ssh(&mut st, use_key(true)).is_empty());
    assert!(page(&st).choice_error.as_deref().is_some_and(|e| e.contains("Another repository")));

    let mut st = loaded(vec![key()], true);
    assert!(ssh(&mut st, use_key(false)).is_empty(), "the choice isn't read yet");
}

#[test]
fn turning_the_agent_on_unsets_only_trellis_choices() {
    let mut st = chosen(KeyChoice::Key(key().private), Some((other("ssh -v"), true)));
    let fx = ssh(&mut st, SshMsg::SetUseAgent(true));
    assert!(st.settings.use_ssh_agent);
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    assert_eq!(sets(&fx), [(None, None)], "the foreign command stays");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Trellis doesn't choose a key in core.sshCommand; ssh uses your agent."));

    let mut st = chosen(other("ssh -v"), Some((KeyChoice::Key(key().private), false)));
    assert!(sets(&ssh(&mut st, SshMsg::SetUseAgent(true))).is_empty(), "an untrusted repository's config is left alone");

    let mut st = chosen(KeyChoice::Key(key().private), None);
    st.settings.use_ssh_agent = true;
    let fx = ssh(&mut st, SshMsg::SetUseAgent(false));
    assert!(!st.settings.use_ssh_agent);
    assert!(sets(&fx).is_empty() && fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "only the setting is saved");
}

/// Opens `dir` and Preferences on the SSH page with `key()` listed, reading core.sshCommand for
/// real (the developer's global value is read, never written).
fn open_on_page(st: &mut AppState, dir: &std::path::Path) {
    drive(st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    if matches!(st.modal, Some(Modal::Trust(_))) {
        drive(st, Msg::CloseModal);
    }
    let fx = run(st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    finish(st, fx.into_iter().filter(|e| matches!(e, Effect::Ssh(SshEffect::ReadChoice { .. }))).collect());
    let generation = page(st).generation;
    ssh(st, SshMsg::Loaded { generation, result: Ok(listing(vec![key()], true)) });
}

#[test]
fn only_a_trusted_repository_gets_its_own_key() {
    let clean = scratch_repo("ssh-use-clean");
    let mut st = state();
    open_on_page(&mut st, &clean);
    assert_eq!(st.trust_standing(), Some(Standing::Clean));
    assert!(ssh(&mut st, use_key(true)).is_empty());
    assert_eq!(page(&st).choice_error.as_deref(), Some(not_trusted(Lang::En)));
    let _ = std::fs::remove_dir_all(&clean);

    let undecided = untrusted_repo("ssh-use-undecided");
    let mut st = state();
    open_on_page(&mut st, &undecided);
    assert_eq!(st.trust_standing(), Some(Standing::Undecided));
    assert!(ssh(&mut st, use_key(true)).is_empty());
    assert_eq!(page(&st).choice_error.as_deref(), Some(not_trusted(Lang::En)));
    let _ = std::fs::remove_dir_all(&undecided);
}

#[test]
fn a_trusted_repository_keeps_its_key_without_asking_again() {
    let dir = scratch_repo("ssh-use-trusted");
    let mut st = state();
    record(&mut st.settings, &std::fs::canonicalize(&dir).unwrap(), RepoTrust::Trusted);
    record(&mut st.settings, &dir, RepoTrust::Trusted);
    open_on_page(&mut st, &dir);
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    assert!(page(&st).repo.as_ref().is_some_and(|r| r.trusted));
    assert_eq!(page(&st).choice.as_ref().and_then(|c| c.repo.clone()), Some(Ok(KeyChoice::Unset)));

    // The runner writes the repository's own core.sshCommand and reads it back.
    drive(&mut st, Msg::Ssh(use_key(true)));
    assert!(page(&st).choice_error.is_none(), "{:?}", page(&st).choice_error);
    let Some(Ok(KeyChoice::Key(back))) = page(&st).choice.as_ref().and_then(|c| c.repo.clone()) else { panic!("{:?}", page(&st).choice) };
    assert!(glowfly_core::paths::same_spelling(&back, &key().private), "{back:?}");
    let name = st.snap.as_ref().unwrap().name.clone();
    let toast = format!("git now uses id_ed25519 for SSH remotes ({name} only)");
    assert!(st.toasts.iter().any(|t| t.text == toast), "{toast}");

    // A full load lists the setting, but the saved decision stands.
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let scan = &st.snap.as_ref().unwrap().trust_scan;
    assert!(scan.config.iter().any(|c| c.key == "core.sshCommand"), "{scan:?}");
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    assert!(!matches!(st.modal, Some(Modal::Trust(_))) && !st.held());
    let _ = std::fs::remove_dir_all(&dir);
}
