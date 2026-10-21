//! Messages and effects for theme files (update/themes.rs, runner/themes.rs,
//! ui/prefs/theme_row.rs): the catalog of files in the themes folder, reloading it, opening the
//! folder and creating a file from the theme in use.

use glowfly_core::settings::{ThemeCatalog, ThemeProblem};
use std::path::PathBuf;

/// The theme files Glowfly knows about (global, not per tab).
#[derive(Clone, Debug, Default)]
pub struct ThemeState {
    pub catalog: ThemeCatalog,
    /// Bumped on every Loaded; the shell re-resolves the palette when it changes.
    pub generation: u64,
    /// Problems last reported for the selected theme, so Reload doesn't log them again.
    pub reported: Option<(String, Vec<ThemeProblem>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThemeMsg {
    /// Read the themes folder again.
    Reload,
    /// Show the themes folder (created if missing) in the file manager.
    OpenFolder,
    /// The view sends the active palette's colors, and whether it is dark (the base for a
    /// "Match system" theme).
    CreateFromCurrent {
        colors: Vec<(&'static str, [u8; 3])>,
        dark: bool,
    },
    // ---- from the runner or the shell
    Loaded(ThemeCatalog),
    /// A new theme file: its id and path, or why it couldn't be written.
    Created(Result<(String, PathBuf), String>),
}

impl ThemeMsg {
    /// Themes are global: nothing is scoped to a tab.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Reload | Self::OpenFolder | Self::CreateFromCurrent { .. } | Self::Loaded(_) | Self::Created(_) => None,
        }
    }
}

/// Writes every problem of a freshly read catalog to the debug log.
pub fn trace_problems(cat: &ThemeCatalog) {
    for f in &cat.files {
        for p in &f.problems {
            tracing::warn!("theme {}: {p}", f.file_name());
        }
    }
    for note in cat.error.iter().chain(&cat.skipped) {
        tracing::warn!("themes: {note}");
    }
}

/// Run by runner/themes.rs.
#[derive(Debug)]
pub enum ThemeEffect {
    Load,
    OpenFolder,
    /// Writes a new theme file holding `json`.
    Create {
        json: String,
    },
}

impl ThemeEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            ThemeEffect::Load => "load-themes",
            ThemeEffect::OpenFolder => "open-themes-folder",
            ThemeEffect::Create { .. } => "create-theme",
        }
    }
}
