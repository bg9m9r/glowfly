//! The keyboard bindings: every key Glowfly answers to, grouped, labelled, and with the place it
//! works in (`table.rs`), and their ids and settings-file names (`ids.rs`); chords, their
//! display text (PC or macOS modifier names, `KeyLabels`) and their settings-file form (`chord.rs`); the effective keymap, the defaults
//! with the user's changes from Preferences > Keyboard (`active.rs`); and the rules for
//! changing a key (`rebind.rs`). The Repository commands rows bind palette commands, with no
//! default keys; `state/palette_bindings.rs` maps them to their commands. The shortcut matching
//! (`ui/shortcuts/global.rs`), the sheet, the palette's hints and button tooltips all read the
//! effective keymap in `AppState::keymap`. No egui types here.

mod active;
mod chord;
mod ids;
mod rebind;
mod table;

#[cfg(test)]
mod chord_tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod override_tests;
#[cfg(test)]
mod pane_tests;
#[cfg(test)]
mod quit_tests;
#[cfg(test)]
mod rebind_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_tests;

pub use active::Keymap;
pub use chord::{Chord, KeyName, Mods};
#[cfg(test)]
pub use ids::COMMAND_BINDINGS;
pub use ids::{BindingId, FAVORITE_BINDINGS};
pub use rebind::{Check, Slot, apply, check, fold_ctrl, reset_taking, same_keys, swap_chord};
pub use table::*;

/// What the staging, commit-message and sheet keys, and the keys of a focused file list, ask
/// `update/shortcuts.rs` to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyAction {
    Stage,
    Unstage,
    StageAll,
    UnstageAll,
    FocusMessage,
    ToggleSheet,
    /// Revert the selected lines, or the current hunk, of a commit or comparison diff.
    RevertHunk,
    /// A pane key or F6: give that pane the keyboard focus (showing its panel if hidden).
    FocusPane(crate::state::layout::Pane),
    /// Enter in the graph: show the details panel and give its file list the focus.
    OpenDetails,
    /// A key of the focused Unstaged (`staged: false`) or Staged Files list.
    WipList {
        staged: bool,
        key: crate::state::list_keys::ListKey,
    },
    /// A move in a focused list that only opens files (commit, comparison, pull request, folder
    /// history, Conflicted and Resolved Files).
    FileList {
        list: crate::state::list_keys::FileList,
        mv: crate::state::list_keys::ListMove,
    },
}
