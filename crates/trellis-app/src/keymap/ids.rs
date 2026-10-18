//! The binding ids, and the stable names the settings file knows them by.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum BindingId {
    Stage,
    Unstage,
    StageAll,
    UnstageAll,
    FocusMessage,
    Commit,
    StageAllAndCommit,
    SelectRow,
    NextChange,
    PrevChange,
    CloseView,
    NewTab,
    CloseTab,
    ReopenTab,
    NextTab,
    PrevTab,
    GoToTab,
    LastTab,
    Palette,
    PaletteMove,
    PaletteRun,
    PaletteBack,
    PaletteClose,
    Sheet,
    Search,
    SearchNext,
    SearchFilter,
    Prefs,
    Refresh,
    Undo,
    Redo,
    DialogSubmit,
    ToggleTerminal,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleLeftPanel,
    ToggleDetailsPanel,
    ToggleToolbar,
    BlameNextRun,
    BlamePrevRun,
    OpenFavorite1,
    OpenFavorite2,
    OpenFavorite3,
    OpenFavorite4,
    OpenFavorite5,
    OpenFavorite6,
    OpenFavorite7,
    OpenFavorite8,
    OpenFavorite9,
    RewordCommit,
    SquashCommit,
    DropCommit,
    RebaseContinue,
    RebaseSkip,
    RebaseStepSelect,
    RebaseStepMove,
    RebasePick,
    RebaseReword,
    RebaseSquash,
    RebaseFixup,
    RebaseDrop,
    RebaseEdit,
    RebaseAddExec,
    RebaseAddBreak,
    RebaseRemoveStep,
    RebaseEditStep,
}

use BindingId as I;

/// Every id with its stable name in the settings file. Renaming one loses users' changes to it.
const KEYS: [(BindingId, &str); 67] = [
    (I::Stage, "stage"),
    (I::Unstage, "unstage"),
    (I::StageAll, "stage-all"),
    (I::UnstageAll, "unstage-all"),
    (I::FocusMessage, "focus-message"),
    (I::Commit, "commit"),
    (I::StageAllAndCommit, "stage-all-and-commit"),
    (I::SelectRow, "select-row"),
    (I::NextChange, "next-change"),
    (I::PrevChange, "prev-change"),
    (I::CloseView, "close-view"),
    (I::NewTab, "new-tab"),
    (I::CloseTab, "close-tab"),
    (I::ReopenTab, "reopen-tab"),
    (I::NextTab, "next-tab"),
    (I::PrevTab, "prev-tab"),
    (I::GoToTab, "go-to-tab"),
    (I::LastTab, "last-tab"),
    (I::Palette, "palette"),
    (I::PaletteMove, "palette-move"),
    (I::PaletteRun, "palette-run"),
    (I::PaletteBack, "palette-back"),
    (I::PaletteClose, "palette-close"),
    (I::Sheet, "shortcut-sheet"),
    (I::Search, "search"),
    (I::SearchNext, "search-next"),
    (I::SearchFilter, "search-filter"),
    (I::Prefs, "preferences"),
    (I::Refresh, "refresh"),
    (I::Undo, "undo"),
    (I::Redo, "redo"),
    (I::DialogSubmit, "dialog-submit"),
    (I::ToggleTerminal, "toggle-terminal"),
    (I::ZoomIn, "zoom-in"),
    (I::ZoomOut, "zoom-out"),
    (I::ZoomReset, "zoom-reset"),
    (I::ToggleLeftPanel, "toggle-left-panel"),
    (I::ToggleDetailsPanel, "toggle-details-panel"),
    (I::ToggleToolbar, "toggle-toolbar"),
    (I::BlameNextRun, "blame-next-run"),
    (I::BlamePrevRun, "blame-prev-run"),
    (I::OpenFavorite1, "open-favorite-1"),
    (I::OpenFavorite2, "open-favorite-2"),
    (I::OpenFavorite3, "open-favorite-3"),
    (I::OpenFavorite4, "open-favorite-4"),
    (I::OpenFavorite5, "open-favorite-5"),
    (I::OpenFavorite6, "open-favorite-6"),
    (I::OpenFavorite7, "open-favorite-7"),
    (I::OpenFavorite8, "open-favorite-8"),
    (I::OpenFavorite9, "open-favorite-9"),
    (I::RewordCommit, "reword-commit"),
    (I::SquashCommit, "squash-commit"),
    (I::DropCommit, "drop-commit"),
    (I::RebaseContinue, "rebase-continue"),
    (I::RebaseSkip, "rebase-skip"),
    (I::RebaseStepSelect, "rebase-step-select"),
    (I::RebaseStepMove, "rebase-step-move"),
    (I::RebasePick, "rebase-pick"),
    (I::RebaseReword, "rebase-reword"),
    (I::RebaseSquash, "rebase-squash"),
    (I::RebaseFixup, "rebase-fixup"),
    (I::RebaseDrop, "rebase-drop"),
    (I::RebaseEdit, "rebase-edit"),
    (I::RebaseAddExec, "rebase-add-exec"),
    (I::RebaseAddBreak, "rebase-add-break"),
    (I::RebaseRemoveStep, "rebase-remove-step"),
    (I::RebaseEditStep, "rebase-edit-step"),
];

/// Open favorite repository 1 to 9, in favorite order.
pub const FAVORITE_BINDINGS: [BindingId; 9] = [
    I::OpenFavorite1,
    I::OpenFavorite2,
    I::OpenFavorite3,
    I::OpenFavorite4,
    I::OpenFavorite5,
    I::OpenFavorite6,
    I::OpenFavorite7,
    I::OpenFavorite8,
    I::OpenFavorite9,
];

impl BindingId {
    /// Every id, in declaration order (`id as usize` indexes it).
    pub const ALL: [BindingId; 67] = {
        let mut all = [I::Stage; 67];
        let mut n = 0;
        while n < 67 {
            all[n] = KEYS[n].0;
            n += 1;
        }
        all
    };

    /// The stable name in the settings file, e.g. "new-tab".
    pub fn key(self) -> &'static str {
        KEYS[self as usize].1
    }

    pub fn from_key(key: &str) -> Option<Self> {
        KEYS.iter().find(|(_, k)| *k == key).map(|(id, _)| *id)
    }

    /// Which favorite (0 for the first) an Open favorite repository binding opens.
    pub fn favorite_slot(self) -> Option<usize> {
        FAVORITE_BINDINGS.iter().position(|&id| id == self)
    }
}
