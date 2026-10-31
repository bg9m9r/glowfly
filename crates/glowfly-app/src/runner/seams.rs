//! What the runner's work reaches outside the repositories with, which tests replace: the trash
//! a delete moves files to (and Undo takes them back out of), the sequence editor an
//! interactive rebase installs its todo with, and how long a path search works before it
//! reports a chunk. The app runs with `Seams::REAL`; `runner::testing` runs everything with
//! stand-ins (`testing::SEAMS`), so tests never touch the real Recycle Bin or Trash and never
//! start the test binary as git's editor. Passed explicitly rather than chosen by `cfg(test)`,
//! which a dependency is never built with.

use glowfly_core::rebase::SequenceEditor;
use glowfly_core::{repo, undo};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct Seams {
    /// Where a delete moves a file, and how Undo takes it back out.
    pub trashers: undo::Trashers,
    /// The command git runs as `GIT_SEQUENCE_EDITOR`.
    pub sequence_editor: fn() -> SequenceEditor,
    /// How long a path search's chunk may take before it is reported.
    pub chunk: Duration,
}

impl Seams {
    /// The app's: the system trash (or the home Trash when a drive's can't be used), this
    /// executable as the sequence editor, and chunks of about 150 ms so matches fill in from the
    /// top.
    pub const REAL: Seams = Seams {
        trashers: undo::Trashers { system: repo::os_trash, home: repo::home_trash, restore: repo::os_untrash },
        sequence_editor: helper_editor,
        chunk: Duration::from_millis(150),
    };
}

/// This executable in `--write-todo` mode (main.rs), which checks the prepared todo before
/// installing it; `cp` when the executable can't be found.
fn helper_editor() -> SequenceEditor {
    std::env::current_exe().map(SequenceEditor::Helper).unwrap_or(SequenceEditor::Copy)
}
