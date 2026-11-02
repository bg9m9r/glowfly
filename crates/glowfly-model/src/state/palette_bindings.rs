//! Which binding each palette command shares a key with, and the palette command a Repository
//! commands binding runs.

use super::AppState;
use super::palette::PaletteCommand;
use super::toolbar_tips;
use crate::keymap::BindingId;

#[cfg(test)]
#[path = "palette_bindings_tests.rs"]
mod tests;

impl PaletteCommand {
    /// The binding that does the same thing; the palette shows its current key.
    pub fn binding(&self) -> Option<BindingId> {
        use crate::msg::LayoutMsg;
        use BindingId as I;
        use PaletteCommand as C;
        Some(match self {
            C::SearchCommits => I::Search,
            C::CloseTab => I::CloseTab,
            C::OpenPrefs => I::Prefs,
            C::Undo => I::Undo,
            C::Redo => I::Redo,
            C::NewTab => I::NewTab,
            C::ReopenClosedTab => I::ReopenTab,
            C::MoveTab(d) if *d < 0 => I::MoveTabLeft,
            C::MoveTab(_) => I::MoveTabRight,
            C::Refresh => I::Refresh,
            C::StageAll => I::StageAll,
            C::UnstageAll => I::UnstageAll,
            C::KeyboardShortcuts => I::Sheet,
            C::ToggleTerminal => I::ToggleTerminal,
            C::Layout(LayoutMsg::ZoomIn) => I::ZoomIn,
            C::Layout(LayoutMsg::ZoomOut) => I::ZoomOut,
            C::Layout(LayoutMsg::ZoomReset) => I::ZoomReset,
            C::Layout(LayoutMsg::ToggleFullscreen) => I::ToggleFullscreen,
            C::Layout(LayoutMsg::SetPanel { panel, .. } | LayoutMsg::TogglePanel(panel)) => return panel.binding(),
            C::RepoList(c) => return c.binding(),
            C::Rebase(c) => return c.binding(),
            C::Fetch => I::Fetch,
            C::Pull => I::Pull,
            C::Push => I::Push,
            C::CreateBranch => I::CreateBranch,
            C::CreateTag { annotated: false } => I::CreateTag,
            C::CreateTag { annotated: true } => I::CreateAnnotatedTag,
            C::Stash => I::StashChanges,
            C::StashMessage => I::StashWithMessage,
            C::PopStash => I::PopStash,
            C::FileHistory => I::FileHistory,
            C::Blame => I::BlameFile,
            C::OpenJournal => I::OpenJournal,
            C::OpenTerminal => I::OpenTerminal,
            C::ShowInFileManager => I::ShowRepoFolder,
            C::PickSelected { revert: false } => I::CherryPick,
            C::PickSelected { revert: true } => I::RevertCommit,
            _ => return None,
        })
    }
}

/// The palette command a Repository commands binding runs; None for every other binding.
pub fn command_for(id: BindingId) -> Option<PaletteCommand> {
    use BindingId as I;
    use PaletteCommand as C;
    Some(match id {
        I::Fetch => C::Fetch,
        I::Pull => C::Pull,
        I::Push => C::Push,
        I::CreateBranch => C::CreateBranch,
        I::CreateTag => C::CreateTag { annotated: false },
        I::CreateAnnotatedTag => C::CreateTag { annotated: true },
        I::StashChanges => C::Stash,
        I::StashWithMessage => C::StashMessage,
        I::PopStash => C::PopStash,
        I::FileHistory => C::FileHistory,
        I::BlameFile => C::Blame,
        I::OpenJournal => C::OpenJournal,
        I::OpenTerminal => C::OpenTerminal,
        I::ShowRepoFolder => C::ShowInFileManager,
        I::CherryPick => C::PickSelected { revert: false },
        I::RevertCommit => C::PickSelected { revert: true },
        _ => return None,
    })
}

/// Why the toolbar button that does what `cmd` does is disabled now, the sentence its tooltip
/// shows; None when the toolbar has no such button or it is enabled.
pub fn blocked(st: &AppState, cmd: &PaletteCommand) -> Option<String> {
    use PaletteCommand as C;
    match cmd {
        C::Fetch => toolbar_tips::fetch_blocked(st),
        C::Pull => toolbar_tips::pull_blocked(st),
        C::Push => toolbar_tips::push_blocked(st),
        C::CreateBranch | C::CreateTag { .. } => toolbar_tips::branch_blocked(st),
        C::Stash | C::StashMessage => toolbar_tips::stash_blocked(st),
        C::PopStash => toolbar_tips::pop_blocked(st),
        _ => None,
    }
}
