//! Preferences > SSH: reading ~/.ssh, copying a public key, and generating a key (asking before
//! replacing one). The key git uses and the agent are in `ssh_use`, adding a key to a service in
//! `ssh_service`. Its effects are unscoped: the page belongs to no repository.

use crate::effect::{Effect, UiEffect};
use crate::msg::Msg;
use crate::ssh::{GenerateFailure, SshEffect, SshMsg};
use crate::state::ssh::{COPIED, REPLACE_BUTTON, REPLACE_TITLE, replace_body};
use crate::state::ssh_use::RepoTarget;
use crate::state::trust::Standing;
use crate::state::{AppState, Modal, PrefPage};
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::NewKey;

pub(super) fn handle(st: &mut AppState, m: SshMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        SshMsg::ShowPage => {
            let Some(p) = st.prefs.as_mut() else { return };
            p.page = PrefPage::Ssh;
            if let Some(email) = p.git.as_ref().map(|g| g.user_email.clone()) {
                p.ssh.prefill_comment(&email);
            }
            load(st, fx);
        }
        SshMsg::Reload => load(st, fx),
        SshMsg::Loaded { generation, result } => {
            let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh).filter(|s| s.generation == generation) else { return };
            match result {
                Ok(listing) => {
                    page.listing = Some(listing);
                    page.fill_location();
                    page.error = None;
                }
                Err(e) => page.error = Some(e),
            }
        }
        SshMsg::CopyPublicKey(path) => {
            let listing = st.prefs.as_ref().and_then(|p| p.ssh.listing.as_ref());
            let Some(key) = listing.and_then(|l| l.keys.iter().find(|k| same_spelling(&k.private, &path))) else { return };
            fx.push(Effect::Ui(UiEffect::CopyText(key.public_key.clone())));
            st.toast(true, COPIED, now);
        }
        SshMsg::Generate { overwrite } => generate(st, overwrite, fx),
        SshMsg::UseKey { key, repo, confirmed } => super::ssh_use::use_key(st, key, repo, confirmed, fx),
        SshMsg::StopUsing { repo, confirmed } => super::ssh_use::stop_using(st, repo, confirmed, fx),
        SshMsg::SetUseAgent(on) => super::ssh_use::set_use_agent(st, on, now, fx),
        SshMsg::ReloadAgent => super::ssh_use::read_agent(st, fx),
        SshMsg::AddToAgent(key) => super::ssh_use::add_to_agent(st, &key, fx),
        SshMsg::ChoiceLoaded { generation, global, repo } => super::ssh_use::choice_loaded(st, generation, global, repo),
        SshMsg::KeyChoiceSet { repo, result } => super::ssh_use::key_choice_set(st, repo, result, now, fx),
        SshMsg::AgentLoaded { generation, status } => super::ssh_use::agent_loaded(st, generation, status),
        SshMsg::AgentAdded { key, result } => super::ssh_use::agent_added(st, &key, result, now, fx),
        SshMsg::ChooseService { key, account } => super::ssh_service::choose(st, &key, account, now, fx),
        SshMsg::SendToService => super::ssh_service::send(st, fx),
        SshMsg::CancelService => {
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.upload = None;
            }
        }
        SshMsg::OpenKeySettings { key, account } => super::ssh_service::open_key_settings(st, &key, &account, now, fx),
        SshMsg::AddedToService { key, account, result } => super::ssh_service::added(st, &key, &account, result, now, fx),
        SshMsg::Generated(result) => {
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.generating = false;
            }
            match result {
                Ok(pair) => {
                    let page = st.prefs.as_ref().map(|p| p.page);
                    if let Some(p) = st.prefs.as_mut() {
                        p.ssh.passphrase.clear();
                        p.ssh.confirm.clear();
                    }
                    st.toast(true, format!("Created SSH key {}", pair.file_name()), now);
                    if page == Some(PrefPage::Ssh) {
                        load(st, fx);
                    }
                }
                Err(GenerateFailure::Exists(path)) => {
                    // The question only makes sense while the form it would re-run is open.
                    if st.prefs.is_some() {
                        st.modal = Some(Modal::danger(
                            REPLACE_TITLE,
                            replace_body(&path),
                            REPLACE_BUTTON,
                            Msg::Ssh(SshMsg::Generate { overwrite: true }),
                        ));
                    }
                }
                Err(GenerateFailure::Failed(e)) => {
                    if let Some(p) = st.prefs.as_mut() {
                        p.ssh.form_error = Some(e.clone());
                    }
                    st.log(false, format!("Could not generate an SSH key: {e}"));
                }
            }
        }
    }
}

/// Reads the page again for the open Preferences (~/.ssh, the key git uses and the agent),
/// showing the loading state meanwhile.
pub(super) fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let repo = st.snap.as_ref().map(|s| RepoTarget {
        name: s.name.clone(),
        workdir: s.workdir.clone(),
        real: s.real_workdir.clone(),
        trusted: st.trust_standing() == Some(Standing::Trusted),
    });
    let Some(p) = st.prefs.as_mut() else { return };
    p.ssh.generation += 1;
    p.ssh.listing = None;
    p.ssh.error = None;
    p.ssh.repo = repo;
    fx.push(Effect::Ssh(SshEffect::List { generation: p.ssh.generation }));
    super::ssh_use::read_choice(st, fx);
    super::ssh_use::read_agent(st, fx);
}

/// Starts ssh-keygen with the form, unless something blocks it (the button is disabled then
/// too). The passphrase stays in the form so a Replace confirmation can reuse it.
fn generate(st: &mut AppState, overwrite: bool, fx: &mut Vec<Effect>) {
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    if let Some(reason) = page.blocked() {
        page.form_error = Some(reason);
        return;
    }
    let (Some(keygen), Some(path)) = (page.listing.as_ref().and_then(|l| l.keygen.clone()), page.resolved()) else { return };
    page.generating = true;
    page.form_error = None;
    let key = NewKey { path, comment: page.comment.trim().into(), passphrase: page.passphrase.clone() };
    fx.push(Effect::Ssh(SshEffect::Generate { keygen, key, overwrite }));
}
