//! Preferences: opening and closing, the settings file and the settings that apply at once, and
//! the routing of its other messages: showing a page (prefs_nav.rs), the Save / Revert bar
//! (prefs_save.rs, prefs_leave.rs), signing keys (prefs_signing.rs) and pickers (prefs_pick.rs).

use crate::effect::{Effect, PrefsEffect};
use crate::msg::PrefsMsg;
use crate::state::prefs::GitConfigValues;
use crate::state::{AppState, PrefPage, PrefsState, host_text};
use glowfly_core::forge::Provider;
use glowfly_core::settings::TrackerChoice;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: PrefsMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        PrefsMsg::OpenPrefs(page) => open(st, page, fx),
        PrefsMsg::ClosePrefs => close(st, fx),
        PrefsMsg::Navigate(page) => super::prefs_nav::navigate(st, page, fx),
        PrefsMsg::SelectIntegration(p) => select_integration(st, p),
        PrefsMsg::ShowAccountForm(on) => show_account_form(st, on),
        // The settings file and the settings that apply at once.
        PrefsMsg::SettingsEdited => {
            if serde_json::to_string(&st.settings).ok().as_deref() != Some(st.settings_file.saved.as_str()) {
                fx.extend(save_settings(st));
            }
            sync_live_settings(st, fx);
        }
        PrefsMsg::SetTheme(t) => {
            if st.settings.theme != t {
                st.settings.theme = t;
                fx.extend(save_settings(st));
            }
        }
        PrefsMsg::SetToolbarLabels(on) => {
            if st.settings.toolbar_labels != on {
                st.settings.toolbar_labels = on;
                fx.extend(save_settings(st));
            }
        }
        PrefsMsg::SetDesktopNotifications(on) => super::notify::set_desktop(st, on, fx),
        PrefsMsg::DesktopNotificationFailed { text, confirm } => super::notify::desktop_failed(st, text, confirm, now),
        PrefsMsg::SettingsUnreadable(problem) => super::settings_file::at_start(st, problem, now),
        PrefsMsg::RetrySettingsRead => super::settings_file::retry(st, fx),
        PrefsMsg::SettingsReread(loaded) => super::settings_file::reread(st, loaded.0, loaded.1, now, fx),
        PrefsMsg::OpenSettingsFolder => super::settings_file::open_folder(st, fx),
        // Git config and the Save / Revert bar.
        PrefsMsg::GitConfigLoaded(values) => config_loaded(st, values, fx),
        PrefsMsg::GitConfig { global, key, value } => super::prefs_save::write_now(st, global, key, value, fx),
        PrefsMsg::SavePage => {
            super::prefs_save::save_page(st, fx);
        }
        PrefsMsg::ConfigSaved { ticket, result } => super::prefs_save::written(st, ticket, result, now),
        PrefsMsg::RevertPage => super::prefs_save::revert_page(st),
        PrefsMsg::LeaveSave => super::prefs_save::leave_save(st, now, fx),
        PrefsMsg::LeaveDiscard => super::prefs_save::leave_discard(st, now, fx),
        // Signing keys.
        PrefsMsg::LoadSigningKeys => super::prefs_signing::load_keys(st, fx),
        PrefsMsg::SigningKeysLoaded(result) => super::prefs_signing::keys_loaded(st, result),
        PrefsMsg::GenerateGpgKey => super::prefs_signing::generate_gpg_key(st, fx),
        PrefsMsg::GpgKeyGenerated(result) => super::prefs_signing::gpg_key_generated(st, result, now, fx),
        PrefsMsg::CopyPublicKey => super::prefs_signing::copy_public_key(st, fx),
        PrefsMsg::PublicKeyExported(result) => super::prefs_signing::public_key_exported(st, result, now, fx),
        PrefsMsg::ExtendKey => super::prefs_signing::extend_key(st, fx),
        PrefsMsg::KeyExtended(result) => super::prefs_signing::key_extended(st, result, now, fx),
        // File and folder pickers.
        PrefsMsg::PickFile(purpose) => super::prefs_pick::pick_file(purpose, fx),
        PrefsMsg::FilePicked { purpose, path: Some(path) } => super::prefs_pick::file_picked(st, purpose, &path, fx),
        PrefsMsg::FilePicked { path: None, .. } => {}
        PrefsMsg::PickFolder(purpose) => super::prefs_pick::pick_folder(purpose, fx),
        PrefsMsg::FolderPicked { purpose, path: Some(path) } => super::prefs_pick::folder_picked(st, purpose, path, fx),
        PrefsMsg::FolderPicked { path: None, .. } => {}
    }
}

/// Opens Preferences on `page`: that page's own reads (prefs_nav.rs) and what every page needs.
fn open(st: &mut AppState, page: PrefPage, fx: &mut Vec<Effect>) {
    let mut prefs = PrefsState::new(page);
    if let Some(w) = st.snap.as_ref().map(|s| s.real_workdir.clone())
        && let TrackerChoice::Jira { project, .. } = st.settings.tracker_for(&w)
    {
        prefs.jira_project = project;
    }
    st.prefs = Some(prefs);
    super::prefs_nav::enter_page(st, page, fx);
    // Which program opens glowfly:// links (General), read again at every opening.
    super::deep_link::check_scheme(st, fx);
    // Whether the downloaded update is still there (General's Updates row).
    super::updates_download::check_saved(st, fx);
    // Start the Integrations page on the open repository's service.
    let provider = st.session.forge.as_ref().and_then(|f| f.host_provider()).unwrap_or(Provider::GitHub);
    select_integration(st, provider);
    fx.push(Effect::Prefs(PrefsEffect::ReadGitConfig { cwd: st.any_dir(), has_repo: st.snap.is_some() }));
}

fn close(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.prefs = None;
    // The Commit Signing page may have changed the program or the allowed signers file.
    st.session.reads.signatures.clear();
    // The diff's scroll offset is in pixels: keep the same top row if the editor font
    // size (and so the row height) changed.
    if st.session.diff.view.is_some() {
        st.view.diff_scroll_to_row = Some(st.session.tab_view.diff_top_row);
    }
    // The commit limit or branch visibility may have changed.
    super::repo::refresh(st, fx);
    // So may the profiles' folder rules or the repository's identity.
    super::profile_rules::want_identity(st, fx);
}

/// Git config was read: the SSH comment, the Save / Revert bar's baseline, the profiles and the
/// signing keys fill from it.
fn config_loaded(st: &mut AppState, values: GitConfigValues, fx: &mut Vec<Effect>) {
    if let Some(p) = st.prefs.as_mut() {
        p.ssh.prefill_comment(&values.user_email);
        p.git_saved = Some(values.clone());
        p.git = Some(values);
        if p.page == PrefPage::Profile {
            super::profiles::on_config(st, fx);
        }
    }
    super::prefs_signing::load_keys(st, fx);
}

fn show_account_form(st: &mut AppState, on: bool) {
    if let Some(p) = st.prefs.as_mut() {
        p.int_adding = on;
        p.int_token.clear();
        p.int_user.clear();
        p.int_error = None;
    }
}

/// Records the current settings as saved and returns the effect that writes them; open
/// Preferences shows its "Saved" tick. None while the settings file couldn't be read: the
/// settings in use then started from defaults, and writing them would replace the user's own
/// (update/settings_file.rs).
pub(super) fn save_settings(st: &mut AppState) -> Option<Effect> {
    if !st.settings_file.writable() {
        return None;
    }
    st.settings_file.saved = serde_json::to_string(&st.settings).unwrap_or_default();
    if let Some(p) = st.prefs.as_mut() {
        p.saved.mark();
    }
    Some(Effect::SaveSettings(Box::new(st.settings.clone())))
}

/// Makes the file watcher, the conflict check and the tab groups follow the settings. Sent every frame while
/// Preferences is open (and after a profile switch): both are no-ops when nothing changed.
pub fn sync_live_settings(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.settings.watch_files != st.watch.enabled {
        st.watch.enabled = st.settings.watch_files;
        super::watch::sync(st, fx);
    }
    if !st.settings.conflict_check {
        if !st.session.reads.conflict_checks.is_empty() {
            st.session.reads.conflict_checks.clear();
        }
    } else {
        super::conflict_check::want(st, fx);
    }
    super::history_index::sync(st, fx);
    super::tab_groups::regroup(st, fx);
    super::branch_colour::sync(st);
}

/// Switches the Integrations page to `provider`, prefilling the host for the open repository.
pub(super) fn select_integration(st: &mut AppState, provider: Provider) {
    let host = st.session.forge.as_ref().and_then(|f| f.host.clone()).filter(|t| t.provider == provider);
    let saved = st.settings.connections.iter().find(|c| c.account.provider == provider).map(|c| c.account.clone());
    let Some(p) = st.prefs.as_mut() else { return };
    p.int_provider = provider;
    p.int_adding = false;
    p.int_error = None;
    p.int_token.clear();
    p.int_user.clear();
    p.int_host = match (host, saved) {
        (Some(t), _) => host_text(&t.account()),
        (None, Some(a)) => host_text(&a),
        _ => String::new(),
    };
}
