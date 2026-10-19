//! The Preferences navigation column: showing the page a row names, and running what that page
//! needs as it opens (finding installed gits, filling the profiles, reading ~/.ssh, the
//! repository's tracker boards, LFS and sparse status, or the Git Flow form). The handlers are
//! called directly, not through `update`, so the unsaved-changes guard sees the row once.

use crate::effect::Effect;
use crate::git_env::GitEnvMsg;
use crate::gitflow::GitFlowMsg;
use crate::lfs::LfsMsg;
use crate::msg::{ForgeMsg, SparseMsg};
use crate::profile::ProfileMsg;
use crate::ssh::SshMsg;
use crate::state::{AppState, PrefPage};
use std::time::Instant;

pub(super) fn navigate(st: &mut AppState, page: PrefPage, now: Instant, fx: &mut Vec<Effect>) {
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
    match page {
        PrefPage::Git => super::git_env::handle(st, GitEnvMsg::ShowPage, now, fx),
        PrefPage::Profile => super::profiles::handle(st, ProfileMsg::ShowPage, now, fx),
        PrefPage::Ssh => super::ssh::handle(st, SshMsg::ShowPage, now, fx),
        PrefPage::RepoTracker => super::forge::handle(st, ForgeMsg::ShowTrackerPage, now, fx),
        PrefPage::RepoLfs => super::lfs::handle(st, LfsMsg::ShowPage, now, fx),
        PrefPage::RepoGitFlow => super::gitflow::handle(st, GitFlowMsg::ShowPage, now, fx),
        PrefPage::RepoSparse => super::sparse::handle(st, SparseMsg::ShowPage, now, fx),
        other => p.page = other,
    }
}
