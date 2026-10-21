//! Headless tests that Preferences > SSH gives its reasons in the chosen language: why Which
//! key, Add to agent and Add key can't run, and the Add to service notes. (The Generate SSH key
//! dialog's are in `ssh_keygen_tests`.)

use super::ssh_tests::{loaded, page, page_mut, pair, ssh};
use crate::ssh::SshMsg;
use crate::state::AppState;
use crate::state::ssh_service::{UploadForm, UploadState, already_note, fallback_note};
use crate::state::ssh_use::{add_blocked, not_trusted, use_blocked};
use glowfly_core::forge::{Account, Provider};
use glowfly_core::i18n::Lang;

const DE: Lang = Lang::De;
const EN: Lang = Lang::En;

/// The page with one key listed, in German.
fn german() -> AppState {
    let mut st = loaded(vec![pair("id_ed25519", "ada")], true);
    st.settings.language = "de".into();
    st
}

#[test]
fn which_key_refusals_are_translated() {
    let mut st = german();
    st.settings.use_ssh_agent = true;
    let key = pair("id_ed25519", "ada").private;
    assert!(ssh(&mut st, SshMsg::UseKey { key, repo: false, confirmed: false }).is_empty());
    let agent_on = DE.tr("prefs.ssh.which.agent_on.tip");
    assert_ne!(agent_on, EN.tr("prefs.ssh.which.agent_on.tip"), "German has its own text");
    assert_eq!(page(&st).choice_error.as_deref(), Some(agent_on));

    // The dropdown's disabled tip comes from the same rule.
    let p = page(&st);
    assert_eq!(use_blocked(p, true, false, DE), Some(agent_on));
    assert_eq!(use_blocked(p, false, true, DE), Some(DE.tr("prefs.ssh.which.no_repo.tip")));
    assert_ne!(not_trusted(DE), not_trusted(EN));
    let busy = crate::state::ssh::SshPage { choice_busy: true, ..Default::default() };
    assert_eq!(use_blocked(&busy, false, false, DE), Some(DE.tr("prefs.ssh.which.busy.tip")));
    let reading = crate::state::ssh::SshPage::default();
    assert_eq!(use_blocked(&reading, false, false, DE), Some(DE.tr("prefs.ssh.which.reading.tip")));
}

#[test]
fn add_to_agent_refusals_are_translated() {
    let mut st = german();
    page_mut(&mut st).agent = None;
    ssh(&mut st, SshMsg::AddToAgent(pair("id_ed25519", "ada").private));
    let checking = DE.tr("prefs.ssh.card.add_agent.checking.tip");
    assert_ne!(checking, EN.tr("prefs.ssh.card.add_agent.checking.tip"));
    assert_eq!(page(&st).agent_error.as_deref(), Some(checking));

    let mut running = glowfly_core::ssh_keys::AgentStatus {
        ssh_add: Some("ssh-add".into()),
        reach: Some(glowfly_core::ssh_keys::AgentReach::Keys(vec![])),
        ..Default::default()
    };
    page_mut(&mut st).agent = Some(running.clone());
    let job = add_blocked(page(&st), Some("Pull"), DE).expect("a running job blocks");
    assert!(job.contains("Pull") && job != add_blocked(page(&st), Some("Pull"), EN).unwrap(), "{job}");
    running.ssh_add = None;
    page_mut(&mut st).agent = Some(running);
    assert_eq!(add_blocked(page(&st), None, DE).as_deref(), Some(DE.tr("prefs.ssh.card.add_agent.no_ssh_add.tip")));
}

#[test]
fn add_to_service_texts_are_translated() {
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    let mut form = UploadForm::new(pair("id_ed25519", "ada").private, account, "Laptop".into());
    form.title = "a\nb".into();
    let control = form.send_blocked(DE).expect("a control character");
    assert_eq!(form.title_problem(DE), Some(control));
    assert_ne!(Some(control), form.title_problem(EN));
    form.title.clear();
    form.state = UploadState::Sending;
    assert_eq!(form.send_blocked(DE), Some(DE.tr("prefs.ssh.service.sending.tip")));

    let note = already_note(DE, "GitHub", "Laptop");
    assert!(note.contains("GitHub") && note.contains("Laptop") && note != already_note(EN, "GitHub", "Laptop"), "{note}");
    let fallback = fallback_note(DE, "GitHub");
    assert!(fallback.contains("GitHub") && fallback != fallback_note(EN, "GitHub"), "{fallback}");
}
