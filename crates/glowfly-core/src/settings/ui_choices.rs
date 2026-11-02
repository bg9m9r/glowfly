//! The look and layout choices among the settings: notification corner, descriptions, branch
//! visibility, the history table's row density and the diff viewer's layout and mode.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Corner {
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum DescriptionMode {
    Always,
    OnHover,
    Never,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum BranchVisibility {
    All,
    /// "Relevant branches": the checked-out branch, its upstream, the default branch, open PRs.
    Smart,
}

/// How tall the history table's rows are (Preferences > Appearance > Row density).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Density {
    /// Roomy rows with larger author nodes.
    #[default]
    Comfortable,
    /// Shorter rows, to see more history at once.
    Compact,
}

/// How the diff viewer lays out a file's changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DiffLayout {
    /// Changes in one column, old and new numbers side by side.
    #[default]
    Inline,
    /// Old file on the left, new file on the right.
    Split,
}

/// How much of a file the diff viewer shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DiffMode {
    /// Only the changes, with a few lines of context.
    #[default]
    Hunks,
    /// Every line of the file, changes marked in place.
    WholeFile,
}
