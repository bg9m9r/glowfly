//! Showing a Preferences page: the one path that opening Preferences on a page, the navigation
//! column and each area's `ShowPage` message take, running what that page needs as it opens
//! (finding installed gits, filling the profiles, reading ~/.ssh, the repository's tracker
//! boards, LFS and sparse status, the Git Flow form, or git config's diff and merge tools).
//! The handlers are called directly, not through `update`, so the unsaved-changes guard sees
//! the row once.

use crate::effect::Effect;
use crate::state::{AppState, PrefPage};

/// A navigation row: shows `page` in open Preferences.
pub(super) fn navigate(st: &mut AppState, page: PrefPage, fx: &mut Vec<Effect>) {
    let snap = st.snap.as_ref();
    let Some(p) = st.prefs.as_mut() else { return };
    // The open page's own row would fill it again over its unsaved edits.
    if p.page == page && crate::state::prefs_save::unsaved(p, &st.settings, snap) {
        return;
    }
    // A key being recorded or a warning belongs to the page it was on.
    if p.page != page {
        p.keys.recording = None;
        p.keys.pending = None;
        p.keys.confirm_reset_all = false;
    }
    enter_page(st, page, fx);
}

/// Shows `page` in open Preferences and starts what it needs, so a page opens the same way
/// whichever way led to it. Nothing while Preferences is closed.
pub(super) fn enter_page(st: &mut AppState, page: PrefPage, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    p.page = page;
    match page {
        PrefPage::Git => super::git_env::discover(st, fx),
        // Before git config is read, `PrefsMsg::GitConfigLoaded` fills the profiles.
        PrefPage::Profile => {
            if p.git.is_some() {
                super::profiles::on_config(st, fx);
            }
        }
        PrefPage::Ssh => {
            // Before git config is read, `PrefsMsg::GitConfigLoaded` prefills the comment.
            if let Some(email) = p.git.as_ref().map(|g| g.user_email.clone()) {
                p.ssh.prefill_comment(&email);
            }
            super::ssh::load(st, fx);
        }
        PrefPage::RepoTracker => super::trello::want_boards(st, fx),
        PrefPage::RepoLfs => super::lfs::load(st, fx),
        PrefPage::RepoGitFlow => super::gitflow::fill(st),
        PrefPage::RepoSparse => super::sparse::load(st, fx),
        PrefPage::Tools => super::tool_git::read_for_page(st, fx),
        PrefPage::General
        | PrefPage::Integrations
        | PrefPage::Ui
        | PrefPage::Language
        | PrefPage::DiffViewer
        | PrefPage::Keyboard
        | PrefPage::Terminal
        | PrefPage::Signing
        | PrefPage::RepoIdentity
        | PrefPage::RepoHooks => {}
    }
}
