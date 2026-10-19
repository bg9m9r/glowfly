//! Messages of the Preferences window and the command palette (update/prefs.rs,
//! update/palette.rs).

use crate::state::PrefPage;
use crate::state::prefs::GitConfigValues;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;

/// What a folder picker was opened for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FolderPurpose {
    OpenRepo,
    InitRepo,
    CloneParent,
    HooksPath,
    WorktreeParent,
    /// The Repository not found dialog's Locate on disk….
    LocateLinkedRepo,
    /// The Find repositories dialog's Add folder….
    ProjectFolder,
}

/// What a file picker was opened for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FilePurpose {
    /// The signing program for the current format (gpg, ssh-keygen, gpgsm).
    SigningProgram,
    SshAllowedSigners,
}

/// The Preferences window: pages, git config, settings, signing keys, and the file and folder
/// pickers (update/prefs.rs).
// `GitConfigLoaded` is large, as it was when it was a `Msg` variant (each `Msg` is that size
// anyway); boxing it would only add an allocation per result.
#[allow(clippy::large_enum_variant)]
pub enum PrefsMsg {
    PickFolder(FolderPurpose),
    OpenPrefs(PrefPage),
    ClosePrefs,
    SelectIntegration(trellis_core::forge::Provider),
    GitConfig {
        global: bool,
        key: String,
        value: Option<String>,
    },
    /// Settings were edited in place by a view; persist them.
    SettingsEdited,
    LoadSigningKeys,
    GenerateGpgKey,
    CopyPublicKey,
    PickFile(FilePurpose),
    /// Switch the colour theme (saved in settings).
    SetTheme(ThemeChoice),
    /// Show or hide the text under toolbar buttons (saved in settings).
    SetToolbarLabels(bool),
    /// The Desktop notifications checkbox (the view only asks; update saves).
    SetDesktopNotifications(bool),
    /// The OS refused a desktop notification; `confirm` when it was the one sent on turning them on.
    DesktopNotificationFailed {
        text: String,
        confirm: bool,
    },
    GitConfigLoaded(GitConfigValues),
    FilePicked {
        purpose: FilePurpose,
        path: Option<PathBuf>,
    },
    SigningKeysLoaded(Result<Vec<trellis_core::signing::SigningKey>, String>),
    GpgKeyGenerated(Result<String, String>),
    PublicKeyExported(Result<String, String>),
    FolderPicked {
        purpose: FolderPurpose,
        path: Option<PathBuf>,
    },
    /// The settings file couldn't be read at startup (it was backed up); shown once.
    SettingsUnreadable(String),
    /// A navigation row: show this page (asking first when the open one has unsaved edits).
    Navigate(PrefPage),
    /// The Save / Revert bar's Save: write the open page's edits.
    SavePage,
    /// The Save / Revert bar's Revert: put the open page's fields back to what was loaded.
    RevertPage,
    /// "Save your changes?" answered Save: save, then complete the leave.
    LeaveSave,
    /// "Save your changes?" answered Discard: revert, then complete the leave.
    LeaveDiscard,
    /// Integrations: show (Add another account) or hide (Cancel) the sign-in form.
    ShowAccountForm(bool),
}

impl PrefsMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::PickFolder(..)
            | Self::OpenPrefs(..)
            | Self::ClosePrefs
            | Self::SelectIntegration(..)
            | Self::GitConfig { .. }
            | Self::SettingsEdited
            | Self::LoadSigningKeys
            | Self::GenerateGpgKey
            | Self::CopyPublicKey
            | Self::PickFile(..)
            | Self::SetTheme(..)
            | Self::SetToolbarLabels(..)
            | Self::SetDesktopNotifications(..)
            | Self::DesktopNotificationFailed { .. }
            | Self::GitConfigLoaded(..)
            | Self::FilePicked { .. }
            | Self::SigningKeysLoaded(..)
            | Self::GpgKeyGenerated(..)
            | Self::PublicKeyExported(..)
            | Self::FolderPicked { .. }
            | Self::SettingsUnreadable(..)
            | Self::Navigate(..)
            | Self::SavePage
            | Self::RevertPage
            | Self::LeaveSave
            | Self::LeaveDiscard
            | Self::ShowAccountForm(..) => None,
        }
    }
}

/// The command palette (update/palette.rs).
pub enum PaletteMsg {
    TogglePalette,
    ClosePalette,
    /// Move the palette selection by this many rows, wrapping.
    PaletteMove(isize),
    /// Move the palette selection by a page or to an end of the list, clamped.
    PaletteJump(crate::state::palette_nav::Jump),
    /// The pointer moved onto row `.0` of the palette list: select it, without scrolling.
    PaletteHover(usize),
    /// Close the palette and run this command (or, for a two-step command, open its sub-list).
    RunPaletteCommand(crate::state::palette::PaletteCommand),
    /// Leave a palette sub-list for the command list.
    PaletteBack,
    TrackedPathsLoaded {
        generation: u64,
        result: Result<Vec<String>, String>,
    },
}

impl PaletteMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::TogglePalette
            | Self::ClosePalette
            | Self::PaletteMove(..)
            | Self::PaletteJump(..)
            | Self::PaletteHover(..)
            | Self::RunPaletteCommand(..)
            | Self::PaletteBack
            | Self::TrackedPathsLoaded { .. } => None,
        }
    }
}
