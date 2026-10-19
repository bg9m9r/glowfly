//! The unsaved-changes guard of Preferences: a message that would leave a page with edits the
//! Save / Revert bar hasn't saved is held, and "Save your changes?" asks what to do with them
//! (Save, Discard or Cancel). The answer's handler (update/prefs_save.rs) saves or reverts the
//! page, then replays the held message once through `leave_approved`.

use crate::git_env::GitEnvMsg;
use crate::gitflow::GitFlowMsg;
use crate::lfs::LfsMsg;
use crate::msg::{ForgeMsg, Msg, PrefsMsg, SparseMsg, TabsMsg};
use crate::profile::ProfileMsg;
use crate::ssh::SshMsg;
use crate::state::prefs_save::{repo_page, unsaved};
use crate::state::{AppState, Modal, PrefPage};
use crate::tools::ToolsMsg;

/// Returns `msg` to run now, or None when it waits on the question (or was dropped because
/// another dialog is open).
pub(super) fn hold(st: &mut AppState, msg: Msg) -> Option<Msg> {
    if st.prefs.is_none() || !leaves(st, &msg) {
        return Some(msg);
    }
    let snap = st.snap.as_ref();
    let Some(p) = st.prefs.as_mut() else { return Some(msg) };
    if std::mem::take(&mut p.leave_approved) || !unsaved(p, &st.settings, snap) {
        return Some(msg);
    }
    if st.modal.is_some() {
        tracing::debug!("a leave from an edited Preferences page was dropped: another dialog is open");
        return None;
    }
    let t = st.settings.lang();
    let page = p.page.title(t);
    p.leaving = Some(Box::new(msg));
    st.modal = Some(Modal::ConfirmChoice {
        title: t.tr("prefs.unsaved.title").into(),
        body: t.trf("prefs.unsaved.body", &[("page", &page)]),
        button: t.tr("prefs.unsaved.save.button").into(),
        then: Box::new(Msg::Prefs(PrefsMsg::LeaveSave)),
        alt: t.tr("prefs.unsaved.discard.button").into(),
        alt_then: Box::new(Msg::Prefs(PrefsMsg::LeaveDiscard)),
    });
    None
}

/// After each message: a held leave whose question closed (Cancel, Esc, a backdrop click) is
/// forgotten, and an approval that wasn't used doesn't outlive its message.
pub(super) fn settle(st: &mut AppState) {
    let asking = matches!(st.modal, Some(Modal::ConfirmChoice { .. }));
    if let Some(p) = st.prefs.as_mut() {
        if !asking {
            p.leaving = None;
        }
        p.leave_approved = false;
    }
}

/// Whether `msg` would take Preferences off its page (or close it).
fn leaves(st: &AppState, msg: &Msg) -> bool {
    let Some(p) = st.prefs.as_ref() else { return false };
    let page = p.page;
    let other = |target: PrefPage| target != page;
    match msg {
        Msg::Prefs(PrefsMsg::ClosePrefs | PrefsMsg::OpenPrefs(_)) => true,
        Msg::Prefs(PrefsMsg::Navigate(target)) => other(*target),
        Msg::GitEnv(GitEnvMsg::ShowPage) => other(PrefPage::Git),
        Msg::Profile(ProfileMsg::ShowPage) => other(PrefPage::Profile),
        Msg::Profile(ProfileMsg::Select(id)) => page == PrefPage::Profile && p.profiles.selected != Some(*id),
        // Adding selects the new profile; a switch writes stored identities and then refills
        // the page. (Renaming and deleting another profile keep the edits: profiles::refresh.)
        Msg::Profile(ProfileMsg::Add { .. } | ProfileMsg::Switch(_)) => page == PrefPage::Profile,
        Msg::Ssh(SshMsg::ShowPage) => other(PrefPage::Ssh),
        Msg::Lfs(LfsMsg::ShowPage) => other(PrefPage::RepoLfs),
        Msg::GitFlow(GitFlowMsg::ShowPage) => other(PrefPage::RepoGitFlow),
        Msg::Sparse(SparseMsg::ShowPage) => other(PrefPage::RepoSparse),
        Msg::Forge(ForgeMsg::ShowTrackerPage) => other(PrefPage::RepoTracker),
        Msg::Tools(ToolsMsg::ShowPage) => other(PrefPage::Tools),
        Msg::Tabs(m) if repo_page(page) => switches_tab(st, m),
        _ => false,
    }
}

/// Whether a tab message would show another repository (or none) in place of the active one.
fn switches_tab(st: &AppState, m: &TabsMsg) -> bool {
    let active = st.active_tab;
    match m {
        TabsMsg::SelectTab(i) => *i != active,
        TabsMsg::CycleTab(_) | TabsMsg::NewTab | TabsMsg::ReopenClosedTab | TabsMsg::ReopenClosed(_) => true,
        TabsMsg::CloseTab(i) => *i == active || *i == usize::MAX,
        TabsMsg::CloseOtherTabs(i) => *i != active,
        TabsMsg::CloseTabsToRight(i) => *i < active,
        _ => false,
    }
}
