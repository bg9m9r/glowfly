//! Preferences: opening/closing, settings persistence, git config, folder pickers.

use crate::effect::Effect;
use crate::msg::PrefsMsg;
use crate::msg::{FilePurpose, FolderPurpose};
use crate::state::{AppState, Modal, PrefPage, PrefsState, host_text};
use std::time::Instant;
use trellis_core::forge::Provider;
use trellis_core::gitcli::args;
use trellis_core::settings::TrackerChoice;
use trellis_core::signing::SigningFormat;

pub(super) fn handle(st: &mut AppState, msg: PrefsMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        PrefsMsg::OpenPrefs(page) => {
            let mut prefs = PrefsState::new(page);
            if let Some(w) = st.workdir()
                && let TrackerChoice::Jira { project, .. } = st.settings.tracker_for(&w)
            {
                prefs.jira_project = project;
            }
            st.prefs = Some(prefs);
            if page == PrefPage::RepoGitFlow {
                super::gitflow::fill(st);
            }
            if page == PrefPage::RepoLfs {
                super::lfs::load(st, fx);
            }
            if page == PrefPage::RepoSparse {
                super::sparse::load(st, fx);
            }
            if page == PrefPage::Ssh {
                super::ssh::load(st, fx);
            }
            // Start the Integrations page on the open repository's service.
            let provider = st.forge.as_ref().and_then(|f| f.host_provider()).unwrap_or(Provider::GitHub);
            select_integration(st, provider);
            fx.push(Effect::ReadGitConfig { cwd: st.any_dir(), has_repo: st.snap.is_some() });
        }
        PrefsMsg::ClosePrefs => {
            st.prefs = None;
            // The Commit Signing page may have changed the program or the allowed signers file.
            st.signatures.clear();
            // The diff's scroll offset is in pixels: keep the same top row if the editor font
            // size (and so the row height) changed.
            if st.diff.is_some() {
                st.view.diff_scroll_to_row = Some(st.view.diff_top_row);
            }
            // The commit limit or branch visibility may have changed.
            super::repo::refresh(st, fx);
        }
        PrefsMsg::SelectIntegration(p) => select_integration(st, p),
        PrefsMsg::GitConfig { global, key, value } => fx.push(Effect::SetGitConfig { cwd: st.any_dir(), global, key, value }),
        PrefsMsg::SettingsEdited => {
            if serde_json::to_string(&st.settings).ok().as_deref() != Some(st.saved_settings.as_str()) {
                fx.push(save_settings(st));
            }
            if st.settings.watch_files != st.watch.enabled {
                st.watch.enabled = st.settings.watch_files;
                super::watch::sync(st, fx);
            }
        }
        PrefsMsg::SetTheme(t) => {
            if st.settings.theme != t {
                st.settings.theme = t;
                fx.push(save_settings(st));
            }
        }
        PrefsMsg::SetToolbarLabels(on) => {
            if st.settings.toolbar_labels != on {
                st.settings.toolbar_labels = on;
                fx.push(save_settings(st));
            }
        }
        PrefsMsg::GitConfigLoaded(values) => {
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.prefill_comment(&values.user_email);
                p.git = Some(values);
            }
            load_signing_keys(st, fx);
        }
        PrefsMsg::LoadSigningKeys => load_signing_keys(st, fx),
        PrefsMsg::SigningKeysLoaded(result) => {
            if let Some(p) = st.prefs.as_mut() {
                match result {
                    Ok(keys) => {
                        p.signing.keys = Some(keys);
                        p.signing.keys_error = None;
                    }
                    Err(e) => {
                        p.signing.keys = Some(Vec::new());
                        p.signing.keys_error = Some(e);
                    }
                }
            }
        }
        PrefsMsg::GenerateGpgKey => {
            let Some(p) = st.prefs.as_mut() else { return };
            let Some(git) = p.git.as_ref() else { return };
            let program = non_empty(&git.gpg_program, "gpg");
            p.signing.generating = true;
            fx.push(Effect::GenerateGpgKey {
                program,
                name: git.user_name.clone(),
                email: git.user_email.clone(),
                passphrase: std::mem::take(&mut p.signing.passphrase),
            });
        }
        PrefsMsg::GpgKeyGenerated(result) => {
            if let Some(p) = st.prefs.as_mut() {
                p.signing.generating = false;
            }
            match result {
                Ok(id) => {
                    if let Some(git) = st.prefs.as_mut().and_then(|p| p.git.as_mut()) {
                        git.signing_key = id.clone();
                    }
                    fx.push(Effect::SetGitConfig {
                        cwd: st.any_dir(),
                        global: true,
                        key: "user.signingkey".into(),
                        value: Some(id.clone()),
                    });
                    st.toast(true, format!("Created GPG key {id} and set it as your signing key"), now);
                    load_signing_keys(st, fx);
                }
                Err(e) => st.log(false, format!("Could not generate a GPG key: {e}")),
            }
        }
        PrefsMsg::CopyPublicKey => {
            let Some(git) = st.prefs.as_ref().and_then(|p| p.git.as_ref()) else { return };
            if git.signing_key.is_empty() {
                return;
            }
            let format = SigningFormat::from_config(&git.signing_format);
            let program = non_empty(git.program(), format.default_program());
            fx.push(Effect::ExportPublicKey { format, program, key: git.signing_key.clone() });
        }
        PrefsMsg::PublicKeyExported(result) => match result {
            Ok(text) => {
                fx.push(Effect::Ui(crate::effect::UiEffect::CopyText(text)));
                st.toast(true, "Public key copied to the clipboard", now);
            }
            Err(e) => st.log(false, format!("Could not export the public key: {e}")),
        },
        PrefsMsg::PickFile(purpose) => {
            let title = match purpose {
                FilePurpose::SigningProgram => "Signing program",
                FilePurpose::SshAllowedSigners => "Allowed signers file",
            };
            fx.push(Effect::PickFile { title: title.into(), purpose });
        }
        PrefsMsg::FilePicked { purpose, path: Some(path) } => {
            let value = path.display().to_string();
            let Some(git) = st.prefs.as_mut().and_then(|p| p.git.as_mut()) else { return };
            let key = match purpose {
                FilePurpose::SigningProgram => {
                    let format = SigningFormat::from_config(&git.signing_format);
                    match format {
                        SigningFormat::OpenPgp => git.gpg_program = value.clone(),
                        SigningFormat::Ssh => git.ssh_program = value.clone(),
                        SigningFormat::X509 => git.x509_program = value.clone(),
                    }
                    format.program_key()
                }
                FilePurpose::SshAllowedSigners => {
                    git.ssh_allowed_signers = value.clone();
                    "gpg.ssh.allowedSignersFile"
                }
            };
            fx.push(Effect::SetGitConfig { cwd: st.any_dir(), global: true, key: key.into(), value: Some(value) });
            load_signing_keys(st, fx);
        }
        PrefsMsg::FilePicked { path: None, .. } => {}
        PrefsMsg::PickFolder(purpose) => {
            let title = match purpose {
                FolderPurpose::OpenRepo => "Open a Git repository",
                FolderPurpose::InitRepo => "Initialize a repository in…",
                FolderPurpose::CloneParent => "Clone into folder",
                FolderPurpose::HooksPath => "Hooks folder",
                FolderPurpose::WorktreeParent => "Create the worktree in…",
            };
            fx.push(Effect::PickFolder { title: title.into(), purpose });
        }
        PrefsMsg::FolderPicked { purpose, path: Some(path) } => match purpose {
            FolderPurpose::OpenRepo => super::tabs::open(st, path, fx),
            FolderPurpose::WorktreeParent => super::worktrees::parent_picked(st, path),
            FolderPurpose::InitRepo => {
                let branch = st.settings.default_branch.trim().to_owned();
                let cmd = if branch.is_empty() { args(&["init"]) } else { args(&["init", "-b", &branch]) };
                let spec = super::ops::JobSpec {
                    cwd: Some(path.clone()),
                    then_open: Some((path, crate::state::trust::Created::Init)),
                    ..super::ops::JobSpec::new("Init".into(), vec![cmd])
                };
                super::ops::start_job(st, spec, fx);
            }
            FolderPurpose::CloneParent => {
                if let Some(Modal::Clone(form)) = st.modal.as_mut() {
                    form.parent = path.display().to_string();
                }
                super::clone::check_parent(st, std::time::Duration::ZERO, fx);
            }
            FolderPurpose::HooksPath => {
                let value = path.display().to_string();
                if let Some(git) = st.prefs.as_mut().and_then(|p| p.git.as_mut()) {
                    git.hooks_path = value.clone();
                }
                fx.push(Effect::SetGitConfig { cwd: st.any_dir(), global: false, key: "core.hooksPath".into(), value: Some(value) });
            }
        },
        PrefsMsg::FolderPicked { path: None, .. } => {}
        PrefsMsg::SettingsUnreadable(text) => unreadable(st, text, now),
    }
}

fn non_empty(value: &str, default: &str) -> String {
    if value.trim().is_empty() { default.to_owned() } else { value.trim().to_owned() }
}

/// Lists the keys usable for the configured signing format.
fn load_signing_keys(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    let Some(git) = p.git.as_ref() else { return };
    let format = SigningFormat::from_config(&git.signing_format);
    p.signing.keys = None;
    fx.push(Effect::ListSigningKeys { format, program: non_empty(git.program(), format.default_program()) });
}

/// Tells the user, in a toast and the activity log, that their settings file couldn't be read.
fn unreadable(st: &mut AppState, text: String, now: Instant) {
    st.log(false, text.clone());
    st.toast(false, text, now);
}

/// Records the current settings as saved and returns the effect that writes them.
pub(super) fn save_settings(st: &mut AppState) -> Effect {
    st.saved_settings = serde_json::to_string(&st.settings).unwrap_or_default();
    Effect::SaveSettings(Box::new(st.settings.clone()))
}

/// Switches the Integrations page to `provider`, prefilling the host for the open repository.
fn select_integration(st: &mut AppState, provider: Provider) {
    let host = st.forge.as_ref().and_then(|f| f.host.clone()).filter(|t| t.provider == provider);
    let saved = st.settings.connections.iter().find(|c| c.account.provider == provider).map(|c| c.account.clone());
    let Some(p) = st.prefs.as_mut() else { return };
    p.int_provider = provider;
    p.int_error = None;
    p.int_token.clear();
    p.int_user.clear();
    p.int_host = match (host, saved) {
        (Some(t), _) => host_text(&t.account()),
        (None, Some(a)) => host_text(&a),
        _ => String::new(),
    };
}
