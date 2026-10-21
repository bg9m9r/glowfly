//! The keyboard bindings: every key Trellis answers to, grouped, labelled, and with the place it
//! works in (`table.rs`), and their ids and settings-file names (`ids.rs`); chords, their
//! display text (PC or macOS modifier names, `KeyLabels`) and their settings-file form (`chord.rs`); the effective keymap, the defaults
//! with the user's changes from Preferences > Keyboard (`active.rs`); and the rules for
//! changing a key (`rebind.rs`). The shortcut matching
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
mod rebind_tests;
#[cfg(test)]
mod tests;

pub use active::Keymap;
pub use chord::{Chord, KeyName, Mods};
pub use ids::{BindingId, FAVORITE_BINDINGS};
pub use rebind::{Check, Slot, apply, check, fold_ctrl, reset_taking, same_keys, swap_chord};
pub use table::*;

/// What the staging, commit-message and sheet keys, and the arrows of a focused Unstaged or
/// Staged Files list, ask `update/shortcuts.rs` to do.
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
    /// Up/Down in the focused Unstaged (`staged: false`) or Staged Files list: open the file
    /// below (`down`) or above.
    StepFile {
        staged: bool,
        down: bool,
    },
}
