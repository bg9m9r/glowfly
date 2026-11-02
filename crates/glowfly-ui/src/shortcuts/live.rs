//! What is open this frame (`Live`) and which bindings that lets through (`allowed`), for the
//! shortcut matching in `global.rs`.

use crate::keymap::BindingId;
use crate::state::layout::Pane;
use crate::state::{AppState, Modal};
use eframe::egui;
use git2::RepositoryState;

/// What is open this frame, for deciding which bindings apply.
pub(super) struct Live {
    pub(super) prefs: bool,
    pub(super) palette: bool,
    pub(super) askpass: bool,
    /// The shortcut sheet.
    pub(super) sheet: bool,
    /// A dialog other than the sheet.
    pub(super) dialog: bool,
    /// A text field has the keyboard (a focused file list isn't typing).
    pub(super) typing: bool,
    /// A working-tree diff is open, or a working-tree file is opening.
    pub(super) working_diff: bool,
    /// A commit or comparison diff is open, with no conflict or blame view over it.
    pub(super) commit_diff: bool,
    /// A diff or the conflict view is open (and no blame view over it).
    pub(super) stepping: bool,
    /// A diff is open, with no conflict or blame view over it.
    pub(super) diff: bool,
    /// A loaded blame view is open.
    pub(super) blame: bool,
    /// The commit graph is the central view.
    pub(super) graph: bool,
    /// The terminal has the keyboard.
    pub(super) terminal: bool,
    /// A rebase is in progress.
    pub(super) rebasing: bool,
    /// The pane with the keyboard focus, if any (`ui/pane_focus.rs`).
    pub(super) pane: Option<Pane>,
}

impl Live {
    pub(super) fn now(ctx: &egui::Context, st: &AppState) -> Self {
        let sheet = matches!(st.modal, Some(Modal::Shortcuts { .. }));
        Self {
            prefs: st.prefs.is_some(),
            palette: st.palette.is_some(),
            askpass: st.askpass.showing(),
            sheet,
            // The welcome counts as a dialog, except while Preferences hides it.
            dialog: (st.modal.is_some() && !sheet) || (st.onboarding.welcome.is_some() && st.prefs.is_none()),
            // A focused list, the graph or a pane's own focus target isn't typing.
            typing: ctx.wants_keyboard_input()
                && crate::ui::wip_keys::focused_list(ctx).is_none()
                && crate::ui::file_list_keys::focused(ctx).is_none()
                && !crate::ui::pane_focus::target_focused(ctx),
            working_diff: st
                .view
                .diff_opening
                .as_ref()
                .or(st.session.diff.view.as_ref().map(|(t, _)| t))
                .is_some_and(|t| t.is_working_tree()),
            commit_diff: st.session.blame.view.is_none()
                && st.session.conflict.view.is_none()
                && st.session.diff.view.as_ref().is_some_and(|(t, _)| t.reverts_hunks()),
            stepping: st.session.blame.view.is_none() && (st.session.conflict.view.is_some() || st.session.diff.view.is_some()),
            diff: st.session.blame.view.is_none() && st.session.conflict.view.is_none() && st.session.diff.view.is_some(),
            blame: st.session.blame.view.as_ref().is_some_and(|v| v.data.is_some()),
            graph: st.graph_shown(),
            terminal: st.terminal.focused,
            rebasing: st.snap.as_ref().is_some_and(|s| {
                use RepositoryState::*;
                matches!(s.state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase)
            }),
            pane: crate::ui::pane_focus::focused_pane(ctx),
        }
    }
}

/// The actions whose keys a focused terminal keeps for its shell when pressed without Shift,
/// Alt or Cmd (Ctrl+W, Ctrl+P, Ctrl+Q, F5, and macOS's Control+digit pane keys); their shifted
/// keys still act.
fn terminal_keeps(id: BindingId) -> bool {
    use BindingId as I;
    matches!(
        id,
        I::NewTab
            | I::CloseTab
            | I::NextTab
            | I::GoToTab
            | I::LastTab
            | I::Palette
            | I::Search
            | I::Refresh
            | I::Quit
            | I::FocusSidebar
            | I::FocusCentral
            | I::FocusFiles
            | I::FocusDiff
            | I::FocusCommitMessage
    )
}

/// Whether `id` applies with what is open; `shell` says whether the press would reach a focused
/// terminal's shell. Nothing reaches the main window's actions under Preferences, a credential
/// prompt, the palette or a dialog, except where noted; text fields keep plain keys and their
/// own undo.
pub(super) fn allowed(id: BindingId, shell: bool, l: &Live) -> bool {
    use BindingId as I;
    if l.terminal && shell && terminal_keeps(id) {
        return false;
    }
    let covered = l.prefs || l.askpass || l.palette || l.dialog;
    match id {
        I::Palette => !l.prefs && !l.dialog && !l.sheet && !l.askpass,
        // Closes the sheet too.
        I::Sheet => !covered,
        I::FocusMessage => !covered && !l.sheet,
        I::StageAll | I::UnstageAll => !covered && !l.sheet && !l.typing,
        I::Stage | I::Unstage => !covered && !l.sheet && !l.typing && l.working_diff,
        I::RevertHunk => !covered && !l.sheet && !l.typing && l.commit_diff,
        I::ToggleTerminal | I::ToggleLeftPanel | I::ToggleDetailsPanel | I::ToggleToolbar => !covered && !l.sheet,
        // Zoom and full screen work over everything (recording a key takes every press before this).
        I::ZoomIn | I::ZoomOut | I::ZoomReset | I::ToggleFullscreen => true,
        I::NewTab | I::CloseTab | I::ReopenTab | I::NextTab | I::PrevTab | I::LastTab | I::MoveTabLeft | I::MoveTabRight | I::GoToTab => {
            !l.prefs && !l.palette
        }
        I::Undo | I::Redo => !covered && !l.sheet && !l.typing,
        I::NextChange | I::PrevChange => !covered && !l.sheet && !l.typing && l.stepping,
        I::OpenFold | I::CloseFold => !covered && !l.sheet && !l.typing && l.diff,
        I::BlameNextRun | I::BlamePrevRun => !covered && !l.sheet && !l.typing && l.blame,
        I::SelectAllCommits | I::SelectFocusedCommit => !covered && !l.sheet && !l.typing && l.graph && !l.terminal,
        // The pane keys work from a text field too; a focused terminal keeps its plain F6.
        I::FocusSidebar | I::FocusCentral | I::FocusFiles | I::FocusDiff | I::FocusCommitMessage => !covered && !l.sheet,
        I::NextPane | I::PrevPane => !covered && !l.sheet && !(l.terminal && shell),
        I::SelectHead | I::PrevRefCommit | I::NextRefCommit | I::CopyCommitIds => {
            !covered && !l.sheet && !l.typing && l.graph && !l.terminal
        }
        // Not while typing: AltGr is Ctrl+Alt on Windows, and AltGr+digit types characters.
        I::OpenFavorite1
        | I::OpenFavorite2
        | I::OpenFavorite3
        | I::OpenFavorite4
        | I::OpenFavorite5
        | I::OpenFavorite6
        | I::OpenFavorite7
        | I::OpenFavorite8
        | I::OpenFavorite9 => !covered && !l.sheet && !l.typing,
        I::RewordCommit | I::SquashCommit | I::DropCommit => !covered && !l.sheet && !l.typing && l.graph,
        I::RebaseContinue | I::RebaseSkip => !covered && !l.sheet && !l.typing && l.rebasing,
        // Palette commands; a focused terminal keeps every key of theirs that reaches its shell.
        I::Fetch
        | I::Pull
        | I::Push
        | I::CreateBranch
        | I::CreateTag
        | I::CreateAnnotatedTag
        | I::StashChanges
        | I::StashWithMessage
        | I::PopStash
        | I::FileHistory
        | I::BlameFile
        | I::OpenJournal
        | I::OpenTerminal
        | I::ShowRepoFolder
        | I::CherryPick
        | I::RevertCommit => !covered && !l.sheet && !l.typing && !(l.terminal && shell),
        I::Refresh => true,
        I::Search | I::SearchFilter => !l.palette && !l.prefs,
        I::Prefs => !l.prefs && !l.palette,
        I::Quit => !l.askpass,
        // Saves the open Preferences page, from its fields too.
        I::PrefsSave => l.prefs && !l.askpass && !l.dialog,
        I::Commit
        | I::StageAllAndCommit
        | I::SelectRow
        | I::ExtendSelection
        | I::CloseView
        | I::PaletteMove
        | I::PaletteRun
        | I::PaletteBack
        | I::PaletteClose
        | I::SearchNext
        | I::DialogSubmit
        | I::SendPrText
        | I::RebaseStepSelect
        | I::RebaseStepMove
        | I::RebasePick
        | I::RebaseReword
        | I::RebaseSquash
        | I::RebaseFixup
        | I::RebaseDrop
        | I::RebaseEdit
        | I::RebaseAddExec
        | I::RebaseAddBreak
        | I::RebaseRemoveStep
        | I::RebaseEditStep
        | I::PrefsRevert
        | I::WipStepFile
        | I::WipExtendFiles
        | I::WipEdgeFile
        | I::WipPageFile
        | I::WipToggleFiles
        | I::WipOpenFile
        | I::WipDiscardFile
        | I::WipOtherList
        | I::FileStepFile
        | I::FilePageFile
        | I::FileEdgeFile
        | I::GraphPageRow
        | I::GraphEdgeRow
        | I::OpenCommitDetails => false,
    }
}
