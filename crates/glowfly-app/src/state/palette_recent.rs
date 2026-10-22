//! The palette's per-session memory of recently run commands, and the stable key each plain
//! command is remembered by. Only commands whose meaning doesn't depend on a particular ref,
//! file, stash or path have a key; a toggle is remembered by what it controls, not by which way
//! it flips, so the entry that now reads the other way still ranks first.

use super::layout::Panel;
use super::palette_commands::PaletteCommand;
use super::palette_commands::palette_commands_rebase::RebaseCommand;
use super::palette_commands::palette_stack::StackCommand;
use super::palette_kinds::StashAction;
use crate::logs::{LogKind, LogsMsg};
use crate::msg::LayoutMsg;
use crate::msg::UpdateMsg;
use crate::onboarding::OnboardingMsg;
use crate::remote::RemoteMsg;
use crate::submodule::SubmoduleMsg;
use crate::tools::ToolsMsg;
use glowfly_core::gitflow::FlowKind;
use glowfly_core::rebase::Resume;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::stash_ops::StashScope;

/// Most commands the palette remembers.
pub const MAX_RECENT_COMMANDS: usize = 10;

/// Recently run commands' keys, most recent first, without duplicates.
#[derive(Default, Debug)]
pub struct RecentCommands {
    keys: Vec<&'static str>,
}

impl RecentCommands {
    /// Puts `key` first, dropping an earlier copy and the oldest key past the cap.
    pub fn record(&mut self, key: &'static str) {
        self.keys.retain(|k| *k != key);
        self.keys.insert(0, key);
        self.keys.truncate(MAX_RECENT_COMMANDS);
    }

    /// Where `key` is in the list (0 is the most recent), if it is there.
    pub fn position(&self, key: &str) -> Option<usize> {
        self.keys.iter().position(|k| *k == key)
    }

    /// The keys, most recent first (for tests).
    #[cfg(test)]
    pub fn keys(&self) -> &[&'static str] {
        &self.keys
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl PaletteCommand {
    /// The key this command is remembered by among recent commands; None for a command about a
    /// particular ref, file, stash or path, and for a step's pick or typed text.
    pub fn recall_key(&self) -> Option<&'static str> {
        use PaletteCommand as C;
        Some(match self {
            C::Fetch => "fetch",
            C::Pull => "pull",
            C::FetchFullHistory => "fetch-full-history",
            C::FetchMoreCommits => "fetch-more-commits",
            C::FetchMoreCommitsBy(_) => return None,
            C::Push => "push",
            C::Stash => "stash",
            C::PopStash => "pop-stash",
            C::ChooseStash(StashAction::Apply) => "stash-apply",
            C::ChooseStash(StashAction::Pop) => "stash-pop",
            C::ChooseStash(StashAction::Drop) => "stash-drop",
            C::StashOp { .. } => return None,
            C::ChooseProfile => "profile-switch",
            C::SwitchProfile(_) => return None,
            C::ChooseStashToRename => "stash-rename",
            C::RenameStash { .. } => return None,
            C::StashMessage => "stash-message",
            C::StashWithMessage(_) => return None,
            C::StashDialog(StashScope::All) => "stash-dialog",
            C::StashDialog(StashScope::Staged) => "stash-staged",
            C::StashDialog(StashScope::Files) => "stash-files",
            C::StageAll => "stage-all",
            C::UnstageAll => "unstage-all",
            C::MarkAllResolved => "mark-all-resolved",
            C::MarkAllConflicted => "mark-all-conflicted",
            C::MarkConflicted(_) => return None,
            C::Undo => "undo",
            C::Redo => "redo",
            C::CreateBranch => "create-branch",
            C::CreateTag { annotated: false } => "create-tag",
            C::CreateTag { annotated: true } => "create-annotated-tag",
            C::RenameBranch(_) => "rename-branch",
            C::CompareWithWorkdir(_) => "compare-workdir",
            C::CreatePr(_) => "create-pr",
            C::PushAndCreatePr(_) => "push-create-pr",
            C::SearchCommits => "search-commits",
            C::InitializeLfs => "lfs-init",
            C::ConfigureLfs => "lfs-configure",
            C::ConfigureGitFlow => "gitflow-configure",
            C::ConfigureSparse => "sparse-configure",
            C::ConfigureTerminal => "configure-terminal",
            C::StartGitFlow(FlowKind::Feature) => "gitflow-start-feature",
            C::StartGitFlow(FlowKind::Release) => "gitflow-start-release",
            C::StartGitFlow(FlowKind::Hotfix) => "gitflow-start-hotfix",
            C::FinishGitFlow(_) | C::ContinueGitFlowFinish => return None,
            C::OpenGitFlowPanel => "gitflow-panel",
            C::OpenJournal => "journal",
            C::ChooseGitFlowIssue(FlowKind::Feature) => "gitflow-issue-feature",
            C::ChooseGitFlowIssue(FlowKind::Release) => "gitflow-issue-release",
            C::ChooseGitFlowIssue(FlowKind::Hotfix) => "gitflow-issue-hotfix",
            C::StartGitFlowFromIssue { .. } => return None,
            C::CreateIssue => "create-issue",
            C::ChooseIssueToView => "view-issue",
            C::ViewIssue(_) => return None,
            C::Refresh => "refresh",
            C::OpenTerminal => "open-terminal",
            C::ToggleTerminal => "toggle-terminal",
            C::ShowInFileManager => "show-folder",
            C::DiffLayout(_) => "diff-layout",
            C::IgnoreWhitespace(_) => "ignore-whitespace",
            C::OpenRepo => "open-repo",
            C::NewTab => "new-tab",
            C::ReopenClosedTab => "reopen-tab",
            C::CloseTab => "close-tab",
            C::RenameTab => "rename-tab",
            C::MoveTab(d) if *d < 0 => "move-tab-left",
            C::MoveTab(_) => "move-tab-right",
            C::ReopenClosed(_) => return None,
            C::OpenPrefs => "prefs",
            C::Dashboard => "dashboard",
            C::KeyboardShortcuts => "shortcuts",
            C::ToolbarLabels(_) => "toolbar-labels",
            C::Theme(ThemeChoice::Dark) => "theme-dark",
            C::Theme(ThemeChoice::Light) => "theme-light",
            C::Theme(ThemeChoice::System) => "theme-system",
            C::Theme(ThemeChoice::GardenDark) => "theme-garden-dark",
            C::Theme(ThemeChoice::GardenLight) => "theme-garden-light",
            C::Theme(ThemeChoice::HighContrastDark) => "theme-high-contrast-dark",
            C::Theme(ThemeChoice::HighContrastLight) => "theme-high-contrast-light",
            C::Theme(ThemeChoice::Sand) => "theme-sand",
            C::Theme(ThemeChoice::Custom(_)) => return None,
            C::Checkout(_) | C::CheckoutRemote { .. } | C::CheckoutTag(_) | C::OpenRecent(_) => return None,
            // Names are the user's own data.
            C::Custom(_) => return None,
            C::CopyLink(crate::deep_link::LinkTarget::Repository { .. }) => "copy-repo-link",
            C::CopyLink(_) => return None,
            C::FileHistory => "file-history",
            C::Blame => "blame",
            C::ReviewTrust => "review-trust",
            C::ApplyPatch => "apply-patch",
            C::Patch(crate::patch::PatchMsg::ChangesDialog { path: None, .. }) => "create-patch-uncommitted",
            C::Patch(_) => return None,
            C::Rebase(RebaseCommand::Edit(_) | RebaseCommand::Onto { .. }) => return None,
            C::Rebase(RebaseCommand::Resume(Resume::Continue)) => "rebase-continue",
            C::Rebase(RebaseCommand::Resume(Resume::Skip)) => "rebase-skip",
            C::Rebase(RebaseCommand::EditRemaining) => "rebase-edit-steps",
            C::Rebase(RebaseCommand::RestoreOrder) => "rebase-restore-order",
            C::Stack(StackCommand::Push(_)) => "push-stack",
            C::Stack(StackCommand::CreatePrs(_)) => "create-stack-prs",
            C::OpenFileHistory(_) | C::OpenFolderHistory(_) | C::OpenBlame(_) => return None,
            C::Worktree(m) if **m == crate::worktree::WorktreeMsg::Prune => "prune-worktrees",
            C::Worktree(_) => return None,
            C::Remote(m) => match m {
                RemoteMsg::AddDialog => "add-remote",
                _ => return None,
            },
            C::Submodule(m) => match m {
                SubmoduleMsg::AddDialog => "add-submodule",
                _ => return None,
            },
            C::Tools(m) => match m {
                ToolsMsg::OpenRepo => "open-repo-in-editor",
                ToolsMsg::ShowPage => "tools-configure",
                ToolsMsg::OpenDiff(_) => "open-in-diff-tool",
                ToolsMsg::ResolveInMergeTool(_) => "resolve-in-merge-tool",
                _ => return None,
            },
            C::Layout(m) => match m {
                LayoutMsg::ZoomIn => "zoom-in",
                LayoutMsg::ZoomOut => "zoom-out",
                LayoutMsg::ZoomReset => "zoom-reset",
                LayoutMsg::ToggleFullscreen => "full-screen",
                LayoutMsg::SetPanel { panel: Panel::Left, .. } => "left-panel",
                LayoutMsg::SetPanel { panel: Panel::Details, .. } => "details-panel",
                LayoutMsg::SetPanel { panel: Panel::Toolbar, .. } => "toolbar",
                LayoutMsg::SetPanel { panel: Panel::MenuBar, .. } => "menu-bar",
                LayoutMsg::TogglePanel(_) | LayoutMsg::PanelSize { .. } | LayoutMsg::ExpandDetails | LayoutMsg::ToggleOpStep(_) => {
                    return None;
                }
            },
            C::Logs(m) => match m {
                LogsMsg::Show(LogKind::Activity) => "view-activity-log",
                LogsMsg::Show(LogKind::Errors) => "view-error-log",
                LogsMsg::Show(LogKind::Performance) => "view-performance-log",
                LogsMsg::OpenFolder => "open-logs-folder",
                LogsMsg::SetExtended(_) => "extended-logging",
                _ => return None,
            },
            C::Onboarding(OnboardingMsg::ShowWelcome) => "show-welcome",
            C::Onboarding(OnboardingMsg::StartTour) => "tour",
            C::Onboarding(_) => return None,
            C::Updates(UpdateMsg::ShowNotes) => "release-notes",
            C::Updates(UpdateMsg::CheckNow) => "check-updates",
            C::Updates(_) => return None,
            C::RepoList(c) => return c.recall_key(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_puts_newest_first() {
        let mut r = RecentCommands::default();
        r.record("a");
        r.record("b");
        assert_eq!(r.keys(), ["b", "a"]);
    }

    #[test]
    fn record_again_moves_to_front_without_duplicate() {
        let mut r = RecentCommands::default();
        for k in ["a", "b", "a"] {
            r.record(k);
        }
        assert_eq!(r.keys(), ["a", "b"]);
    }

    #[test]
    fn keeps_at_most_ten() {
        let mut r = RecentCommands::default();
        let keys = ["k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10", "k11"];
        for k in keys {
            r.record(k);
        }
        assert_eq!(r.keys().len(), MAX_RECENT_COMMANDS);
        assert_eq!(r.keys()[0], "k11");
        assert_eq!(r.position("k0"), None);
        assert_eq!(r.position("k1"), None);
        assert_eq!(r.position("k2"), Some(9));
    }

    #[test]
    fn position_of_unknown_is_none() {
        let mut r = RecentCommands::default();
        assert_eq!(r.position("fetch"), None);
        r.record("fetch");
        assert_eq!(r.position("fetch"), Some(0));
        assert_eq!(r.position("pull"), None);
    }
}
