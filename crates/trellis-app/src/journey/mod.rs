//! Whole-app journeys: the real app (state, update, views and the effect runner running git)
//! started headlessly on fixture repositories, driven only by clicking, right-clicking and
//! typing on widgets found by their accessible names, with the screen and the repository on
//! disk checked after every step.
//!
//! - `clone_commit`: clone from a local bare remote, stage a hunk and a file, commit, push, pull.
//! - `branches`: create a branch, merge it, delete it; check out with local changes.
//! - `rebase_conflict`: rebase onto a diverged branch, resolve the conflict in the conflict
//!   view, continue.
//! - `stash_undo`: stash and pop; undo and redo a commit and a merge.
//! - `journal_restore`: hard reset a branch, restore it from the operation journal, undo that.
//! - `range_diff`: compare an amended branch with its version before, from its menu.
//! - `probe`: what the real-input smoke test (`scripts/smoke-linux.sh`) reads from the probe.
//!
//! The app is built by `TrellisApp::for_test`: its saves go to the fixture's config folder,
//! and effects that would reach the network, the OS shell or the global git config are
//! dropped (`runner/sandbox.rs`). The file watcher is off, so after writing files a journey
//! clicks Refresh as a user would. These live in the crate because the package has no library
//! target that `tests/` could reach.

mod find;
mod fixture;
mod harness;

mod branches;
mod clone_commit;
mod harness_tests;
mod journal_restore;
mod op_preview;
mod probe;
mod range_diff;
mod rebase_conflict;
mod stash_undo;
