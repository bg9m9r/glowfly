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
}

/// The command palette (update/palette.rs).
pub enum PaletteMsg {
    TogglePalette,
    ClosePalette,
    /// Move the palette selection by this many rows, wrapping.
    PaletteMove(isize),
    /// Close the palette and run this command (or, for a two-step command, open its sub-list).
    RunPaletteCommand(crate::state::palette::PaletteCommand),
    /// Leave a palette sub-list for the command list.
    PaletteBack,
    TrackedPathsLoaded {
        generation: u64,
        result: Result<Vec<String>, String>,
    },
}
