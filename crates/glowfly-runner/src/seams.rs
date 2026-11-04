//! What the runner's work reaches outside the repositories with, which tests replace: the trash
//! a delete moves files to (and Undo takes them back out of), the sequence editor an
//! interactive rebase installs its todo with, how long a path search works before it reports a
//! chunk, and where saves of Glowfly's own files (settings, recent repositories, open tabs) go.
//! The app runs with `Seams::REAL`; `runner::testing` runs everything with stand-ins
//! (`testing::SEAMS`), so tests never touch the real Recycle Bin or Trash, never start the test
//! binary as git's editor and never write over the developer's own settings. Passed explicitly
//! rather than chosen by `cfg(test)`, which a dependency is never built with.

use glowfly_core::rebase::SequenceEditor;
use glowfly_core::{repo, undo};
use glowfly_model::effect::SaveEffect;
use glowfly_model::msg::Msg;
use std::time::Duration;

/// Which set a `Seams` is, so a check can tell them apart without comparing function
/// addresses (Rust doesn't promise each function a unique one). Only the sandbox's checks
/// read it (`testing_tests.rs`).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeamsKind {
    /// The app's (`Seams::REAL`).
    Real,
    /// The tests' (`testing::SEAMS`).
    StandIn,
}

#[derive(Clone, Copy, Debug)]
pub struct Seams {
    /// Which set this is.
    #[cfg_attr(not(test), allow(dead_code))]
    pub kind: SeamsKind,
    /// Where a delete moves a file, and how Undo takes it back out.
    pub trashers: undo::Trashers,
    /// The command git runs as `GIT_SEQUENCE_EDITOR`.
    pub sequence_editor: fn() -> SequenceEditor,
    /// How long a path search's chunk may take before it is reported.
    pub chunk: Duration,
    /// Writes a save of Glowfly's own files (`SaveEffect`); a failure comes back as a line for
    /// the activity log.
    pub save: fn(SaveEffect) -> Option<Msg>,
}

impl Seams {
    /// The app's: the system trash (or the home Trash when a drive's can't be used), this
    /// executable as the sequence editor, chunks of about 150 ms so matches fill in from the
    /// top, and saves into the user's config folder.
    pub const REAL: Seams = Seams {
        kind: SeamsKind::Real,
        trashers: undo::Trashers { system: repo::os_trash, home: repo::home_trash, restore: repo::os_untrash },
        sequence_editor: helper_editor,
        chunk: Duration::from_millis(150),
        save: super::saves::save_now,
    };
}

/// This executable in `--write-todo` mode (main.rs), which checks the prepared todo before
/// installing it; `cp` when the executable can't be found.
fn helper_editor() -> SequenceEditor {
    std::env::current_exe().map(SequenceEditor::Helper).unwrap_or(SequenceEditor::Copy)
}
